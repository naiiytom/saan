use std::collections::{BTreeSet, HashMap};

use petgraph::algo::{has_path_connecting, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ecs::World;
use crate::strands::{Transform, Velocity};

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

    /// Creates a Loom pre-configured with standard gameplay motion systems.
    pub fn with_default_gameplay() -> Self {
        let mut loom = Self::new();
        loom.add_system(VelocityKinematicsSystem)
            .expect("Failed to register VelocityKinematicsSystem");
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
}
