use std::collections::{BTreeSet, HashMap};

use petgraph::algo::{has_path_connecting, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ecs::World;
use crate::strands::{
    AudioEmitter3D, CameraRig, EventWeftStrand, FlipbookStrand, GameplayEvent, InputStrand,
    NameStrand, Sprite25D, Transform, Velocity,
};

/// A system ("Weft thread") that weaves through matching strands in the [`World`] each frame.
pub trait WeftSystem: Send + Sync {
    /// Unique identifier of this system in the Loom.
    fn name(&self) -> &'static str;

    /// Strand names read by this system.
    fn reads(&self) -> &[&'static str] {
        &[]
    }

    /// Strand names mutated/written by this system.
    fn writes(&self) -> &[&'static str] {
        &[]
    }

    /// Execute one frame pass over the [`World`].
    fn weave(&mut self, world: &mut World, dt: f32);
}

/// Metadata describing a registered system node inside the [`Loom`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeftNode {
    pub name: String,
    pub reads: BTreeSet<String>,
    pub writes: BTreeSet<String>,
}

/// Kind of data hazard detected when two systems touch the same strand without an ordering path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HazardKind {
    ReadWrite,
    WriteWrite,
}

/// A potential concurrency or ordering ambiguity between two systems on a shared strand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrandHazard {
    pub system_a: String,
    pub system_b: String,
    pub strand: String,
    pub kind: HazardKind,
}

/// Result of running `Loom::inspect_tension()` on the system DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensionReport {
    pub system_count: usize,
    pub edge_count: usize,
    pub has_cycles: bool,
    pub execution_order: Vec<String>,
    pub hazards: Vec<StrandHazard>,
}

impl TensionReport {
    /// Returns `true` when the Loom has no cycles and no unsequenced read/write or write/write hazards.
    pub fn is_taut(&self) -> bool {
        !self.has_cycles && self.hazards.is_empty()
    }
}

#[derive(Debug, Error)]
pub enum LoomError {
    #[error("System '{0}' is already registered in the Loom")]
    DuplicateSystem(String),
    #[error("Unknown system '{0}' referenced in dependency edge")]
    UnknownSystem(String),
    #[error("Loom DAG contains a dependency cycle")]
    CycleDetected,
}

/// The Loom: schedules and executes [`WeftSystem`] passes over a [`World`] using a `petgraph` DAG.
#[derive(Default)]
pub struct Loom {
    graph: DiGraph<WeftNode, ()>,
    nodes_by_name: HashMap<String, NodeIndex>,
    systems: HashMap<NodeIndex, Box<dyn WeftSystem>>,
}

impl Loom {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a Loom pre-configured with standard gameplay motion, flipbook animation, camera, and 3D audio systems.
    pub fn with_default_gameplay() -> Self {
        let mut loom = Self::new();
        loom.add_system(VelocityKinematicsSystem)
            .expect("Failed to register VelocityKinematicsSystem");
        loom.add_system(FlipbookAnimationWeftSystem)
            .expect("Failed to register FlipbookAnimationWeftSystem");
        loom.add_system(CameraOrbitWeftSystem)
            .expect("Failed to register CameraOrbitWeftSystem");
        loom.add_system(PositionalAudioWeftSystem)
            .expect("Failed to register PositionalAudioWeftSystem");
        loom.add_ordering("velocity_kinematics", "positional_audio_3d")
            .expect("Failed to order velocity_kinematics -> positional_audio_3d");
        loom.add_ordering("camera_orbit_input", "positional_audio_3d")
            .expect("Failed to order camera_orbit_input -> positional_audio_3d");
        loom
    }

    /// Adds a [`WeftSystem`] node to the Loom DAG.
    pub fn add_system<S: WeftSystem + 'static>(&mut self, system: S) -> Result<NodeIndex, LoomError> {
        let name = system.name().to_string();
        if self.nodes_by_name.contains_key(&name) {
            return Err(LoomError::DuplicateSystem(name));
        }

        let node_meta = WeftNode {
            name: name.clone(),
            reads: system.reads().iter().map(|s| (*s).to_string()).collect(),
            writes: system.writes().iter().map(|s| (*s).to_string()).collect(),
        };

        let idx = self.graph.add_node(node_meta);
        self.nodes_by_name.insert(name, idx);
        self.systems.insert(idx, Box::new(system));
        Ok(idx)
    }

    /// Adds a directed ordering constraint: `before_system` must run before `after_system`.
    pub fn add_ordering(&mut self, before_system: &str, after_system: &str) -> Result<(), LoomError> {
        let &from = self
            .nodes_by_name
            .get(before_system)
            .ok_or_else(|| LoomError::UnknownSystem(before_system.to_string()))?;
        let &to = self
            .nodes_by_name
            .get(after_system)
            .ok_or_else(|| LoomError::UnknownSystem(after_system.to_string()))?;

        if !self.graph.contains_edge(from, to) {
            self.graph.add_edge(from, to, ());
        }
        Ok(())
    }

    /// Inspects the "tension" of the Loom DAG: checks for cycles, computes topological execution order,
    /// and reports any read/write or write/write strand hazards lacking an ordering path.
    pub fn inspect_tension(&self) -> TensionReport {
        let sorted = toposort(&self.graph, None);
        let (has_cycles, execution_order) = match sorted {
            Ok(indices) => (
                false,
                indices
                    .into_iter()
                    .map(|idx| self.graph[idx].name.clone())
                    .collect(),
            ),
            Err(_) => (true, Vec::new()),
        };

        let mut hazards = Vec::new();
        let indices: Vec<NodeIndex> = self.graph.node_indices().collect();

        for i in 0..indices.len() {
            for j in (i + 1)..indices.len() {
                let a = indices[i];
                let b = indices[j];

                // If there is a directed path in either direction, their execution order is deterministic.
                let ordered = has_path_connecting(&self.graph, a, b, None)
                    || has_path_connecting(&self.graph, b, a, None);
                if ordered {
                    continue;
                }

                let node_a = &self.graph[a];
                let node_b = &self.graph[b];

                // Check Write-Write conflicts
                for strand in node_a.writes.intersection(&node_b.writes) {
                    hazards.push(StrandHazard {
                        system_a: node_a.name.clone(),
                        system_b: node_b.name.clone(),
                        strand: strand.clone(),
                        kind: HazardKind::WriteWrite,
                    });
                }

                // Check Read-Write conflicts (A writes, B reads or B writes, A reads)
                for strand in node_a
                    .writes
                    .intersection(&node_b.reads)
                    .chain(node_b.writes.intersection(&node_a.reads))
                {
                    if !hazards.iter().any(|h| {
                        h.system_a == node_a.name
                            && h.system_b == node_b.name
                            && &h.strand == strand
                    }) {
                        hazards.push(StrandHazard {
                            system_a: node_a.name.clone(),
                            system_b: node_b.name.clone(),
                            strand: strand.clone(),
                            kind: HazardKind::ReadWrite,
                        });
                    }
                }
            }
        }

        TensionReport {
            system_count: self.graph.node_count(),
            edge_count: self.graph.edge_count(),
            has_cycles,
            execution_order,
            hazards,
        }
    }

    /// Executes one full frame across all systems in topological order.
    pub fn step(&mut self, world: &mut World, dt: f32) -> Result<Vec<String>, LoomError> {
        let sorted = toposort(&self.graph, None).map_err(|_| LoomError::CycleDetected)?;
        let mut executed = Vec::with_capacity(sorted.len());

        for idx in sorted {
            let sys = self.systems.get_mut(&idx).expect("Missing system for node");
            executed.push(sys.name().to_string());
            sys.weave(world, dt);
        }

        Ok(executed)
    }
}

/// Built-in Weft system that integrates [`Velocity`] into [`Transform`] each frame.
pub struct VelocityKinematicsSystem;

impl WeftSystem for VelocityKinematicsSystem {
    fn name(&self) -> &'static str {
        "velocity_kinematics"
    }

    fn reads(&self) -> &[&'static str] {
        &["Velocity"]
    }

    fn writes(&self) -> &[&'static str] {
        &["Transform"]
    }

    fn weave(&mut self, world: &mut World, dt: f32) {
        let velocities: Vec<_> = world
            .query::<Velocity>()
            .map(|(entity, &vel)| (entity, vel))
            .collect();

        for (entity, vel) in velocities {
            if let Some(transform) = world.get_strand_mut::<Transform>(entity) {
                transform.translation += vel.linear * dt;
                if vel.angular_y.abs() > f32::EPSILON {
                    transform.rotation *= glam::Quat::from_rotation_y(vel.angular_y * dt);
                }
            }
        }
    }
}

/// Built-in Weft system that weaves [`InputStrand`] controls into the active [`CameraRig`].
pub struct CameraOrbitWeftSystem;

impl WeftSystem for CameraOrbitWeftSystem {
    fn name(&self) -> &'static str {
        "camera_orbit_input"
    }

    fn reads(&self) -> &[&'static str] {
        &["InputStrand"]
    }

    fn writes(&self) -> &[&'static str] {
        &["CameraRig"]
    }

    fn weave(&mut self, world: &mut World, dt: f32) {
        let Some((_, &input)) = world.query::<InputStrand>().next() else {
            return;
        };

        for (_, cam) in world.query_mut::<CameraRig>() {
            let offset = cam.eye - cam.target;
            let yaw = input.orbit_yaw * dt;
            let rotated_offset = if yaw.abs() > 1e-6 {
                glam::Quat::from_rotation_y(yaw) * offset
            } else {
                offset
            };

            let forward_flat = glam::Vec3::new(-rotated_offset.x, 0.0, -rotated_offset.z).normalize_or_zero();
            let right_flat = forward_flat.cross(glam::Vec3::Y).normalize_or_zero();
            let pan = (right_flat * input.move_axis.x + forward_flat * input.move_axis.z) * 5.0 * dt;

            let zoom_scale = (1.0 - input.zoom_delta * dt).clamp(0.8, 1.25);
            cam.target += pan;
            cam.eye = cam.target + rotated_offset * zoom_scale + glam::Vec3::Y * (input.move_axis.y * 3.0 * dt);
        }
    }
}

/// Built-in Weft system that advances [`FlipbookStrand`] frame indices and updates [`Sprite25D::uv_rect`].
pub struct FlipbookAnimationWeftSystem;

impl WeftSystem for FlipbookAnimationWeftSystem {
    fn name(&self) -> &'static str {
        "flipbook_sprite_animation"
    }

    fn reads(&self) -> &[&'static str] {
        &["FlipbookStrand"]
    }

    fn writes(&self) -> &[&'static str] {
        &["FlipbookStrand", "Sprite25D"]
    }

    fn weave(&mut self, world: &mut World, dt: f32) {
        if dt <= 0.0 {
            return;
        }

        let mut updates = Vec::new();
        for (entity, flipbook) in world.query_mut::<FlipbookStrand>() {
            if flipbook.fps <= 0.0 {
                continue;
            }
            let frame_duration = 1.0 / flipbook.fps;
            flipbook.timer += dt;
            while flipbook.timer >= frame_duration {
                flipbook.timer -= frame_duration;
                if flipbook.current_frame < flipbook.end_frame {
                    flipbook.current_frame += 1;
                } else if flipbook.looping {
                    flipbook.current_frame = flipbook.start_frame;
                }
            }
            updates.push((entity, flipbook.current_uv_rect()));
        }

        for (entity, uv_rect) in updates {
            if let Some(sprite) = world.get_strand_mut::<Sprite25D>(entity) {
                sprite.uv_rect = uv_rect;
            }
        }
    }
}

/// Built-in Weft system that computes 3D spatial stereo panning & distance attenuation
/// for all [`AudioEmitter3D`] strands relative to the active [`CameraRig`] listener,
/// and publishes [`GameplayEvent::AudioCue`] events to [`EventWeftStrand`].
pub struct PositionalAudioWeftSystem;

impl WeftSystem for PositionalAudioWeftSystem {
    fn name(&self) -> &'static str {
        "positional_audio_3d"
    }

    fn reads(&self) -> &[&'static str] {
        &["CameraRig", "NameStrand", "Transform"]
    }

    fn writes(&self) -> &[&'static str] {
        &["AudioEmitter3D", "EventWeftStrand"]
    }

    fn weave(&mut self, world: &mut World, _dt: f32) {
        let camera = world
            .query::<CameraRig>()
            .next()
            .map(|(_, &c)| c)
            .unwrap_or_default();

        let forward = (camera.target - camera.eye).normalize_or_zero();
        let right = forward.cross(camera.up).normalize_or_zero();

        let mut computed = Vec::new();
        for (entity, emitter) in world.query::<AudioEmitter3D>() {
            let pos = world
                .get_strand::<Transform>(entity)
                .map(|t| t.translation)
                .unwrap_or(glam::Vec3::ZERO);
            let label = world
                .get_strand::<NameStrand>(entity)
                .map(|n| n.0.clone())
                .unwrap_or_else(|| entity.to_string());

            let offset = pos - camera.eye;
            let dist = offset.length();
            let max_d = emitter.max_distance.max(0.01);

            let stereo_gain = if dist >= max_d {
                [0.0, 0.0]
            } else {
                // Smooth inverse-distance attenuation + stereo panner dot(to_source, listener_right)
                let atten = (1.0 - (dist / max_d)).powi(2) * emitter.base_volume;
                let dir = if dist > 1e-4 {
                    offset / dist
                } else {
                    glam::Vec3::ZERO
                };
                let pan = dir.dot(right).clamp(-1.0, 1.0); // -1.0 = full left, +1.0 = full right
                let left = atten * ((1.0 - pan) * 0.5).sqrt();
                let right_g = atten * ((1.0 + pan) * 0.5).sqrt();
                [left, right_g]
            };

            computed.push((entity, label, emitter.sound_id.clone(), stereo_gain));
        }

        for (entity, label, sound_id, gain) in computed {
            if let Some(emitter) = world.get_strand_mut::<AudioEmitter3D>(entity) {
                emitter.current_stereo_gain = gain;
            }
            if (gain[0] > 0.01 || gain[1] > 0.01)
                && let Some((_, bus)) = world.query_mut::<EventWeftStrand>().next()
            {
                bus.publish(GameplayEvent::AudioCue {
                    emitter: label,
                    sound_id,
                    stereo_gain: gain,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyReadTransform;
    impl WeftSystem for DummyReadTransform {
        fn name(&self) -> &'static str {
            "read_transform"
        }
        fn reads(&self) -> &[&'static str] {
            &["Transform"]
        }
        fn weave(&mut self, _world: &mut World, _dt: f32) {}
    }

    #[test]
    fn detects_unordered_strand_hazard_and_resolves_when_ordered() {
        let mut loom = Loom::new();
        loom.add_system(VelocityKinematicsSystem).unwrap();
        loom.add_system(DummyReadTransform).unwrap();

        let report = loom.inspect_tension();
        assert!(!report.is_taut());
        assert_eq!(report.hazards.len(), 1);
        assert_eq!(report.hazards[0].strand, "Transform");
        assert_eq!(report.hazards[0].kind, HazardKind::ReadWrite);

        loom.add_ordering("velocity_kinematics", "read_transform")
            .unwrap();
        let report_after = loom.inspect_tension();
        assert!(report_after.is_taut());
        assert_eq!(
            report_after.execution_order,
            vec!["velocity_kinematics", "read_transform"]
        );
    }

    #[test]
    fn detects_cycle_in_loom() {
        let mut loom = Loom::new();
        loom.add_system(VelocityKinematicsSystem).unwrap();
        loom.add_system(DummyReadTransform).unwrap();
        loom.add_ordering("velocity_kinematics", "read_transform")
            .unwrap();
        loom.add_ordering("read_transform", "velocity_kinematics")
            .unwrap();

        let report = loom.inspect_tension();
        assert!(report.has_cycles);
        assert!(!report.is_taut());
        assert!(matches!(
            loom.step(&mut World::new(), 0.016),
            Err(LoomError::CycleDetected)
        ));
    }

    #[test]
    fn positional_audio_computes_stereo_pan_and_publishes_event() {
        let mut world = World::new();
        let cam_e = world.spawn();
        world.weave_strand(
            cam_e,
            CameraRig {
                eye: glam::Vec3::new(0.0, 0.0, 10.0),
                target: glam::Vec3::ZERO,
                up: glam::Vec3::Y,
                ..Default::default()
            },
        );

        let bus_e = world.spawn();
        world.weave_strand(bus_e, EventWeftStrand::default());

        // Emitter to the right (+X) of the camera looking down -Z
        let emitter_e = world.spawn();
        world
            .weave_strand(emitter_e, NameStrand("right_bell".into()))
            .weave_strand(emitter_e, Transform::from_xyz(3.0, 0.0, 5.0))
            .weave_strand(
                emitter_e,
                AudioEmitter3D {
                    sound_id: "audio/bell.ogg".into(),
                    base_volume: 1.0,
                    max_distance: 20.0,
                    looping: true,
                    current_stereo_gain: [0.0, 0.0],
                },
            );

        let mut sys = PositionalAudioWeftSystem;
        sys.weave(&mut world, 0.016);

        let audio = world.get_strand::<AudioEmitter3D>(emitter_e).unwrap();
        assert!(
            audio.current_stereo_gain[1] > audio.current_stereo_gain[0],
            "Expected right ear gain ({}) > left ear gain ({})",
            audio.current_stereo_gain[1],
            audio.current_stereo_gain[0]
        );

        let bus = world.get_strand::<EventWeftStrand>(bus_e).unwrap();
        assert_eq!(bus.current_events.len(), 1);
    }
}
