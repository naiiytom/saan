//! # `saan_spatial` — 2.5D & 3D Spatial Physics, Navigation, Collision & Commanding
//!
//! Provides spatial systems woven into the [`saan_core::Loom`] DAG:
//! - [`SpatialCollisionWeftSystem`]: Resolves 3D AABB & CylinderY collisions for 2.5D sprites moving in 3D space,
//!   including automatic stair/ledge step-up (`max_step_height`) and ground-plane snapping.
//! - [`CameraOcclusionWeftSystem`]: Casts a 3D ray from the active [`CameraRig`] to the hero entity and smoothly
//!   fades any occluding 2.5D/3D obstacles marked with `fade_when_occluding_camera`.
//! - [`GridMap3D`]: 3D multi-height tilemap grid with discrete A* pathfinding and continuous flow-field navigation.
//! - [`AgentCommandingWeftSystem`]: Drives hero action, RTS unit squads, and turn-based tactical pawns.

pub mod commanding;
pub mod navigation;

pub use commanding::{
    select_entities_in_rect, AgentCommandingWeftSystem, AgentController, ControllerMode,
    SelectionStrand, UnitOrder,
};
pub use navigation::{FlowField25D, GridMap3D, TileData, TileType};

use glam::Vec3;
use saan_core::{
    CameraRig, Collider3D, EntityId, EventWeftStrand, GameplayEvent, Loom, LoomError, Mesh3D,
    NameStrand, Sprite25D, Transform, WeftSystem, World,
};

/// Registers the standard 2.5D/3D spatial collision, navigation, commanding and camera occlusion Weft systems onto `loom`,
/// wired in deterministic topological order (`agent_commanding_weft` -> `velocity_kinematics` -> `spatial_collision_3d` -> `camera_occlusion_fade`).
pub fn register_spatial_systems(loom: &mut Loom) -> Result<(), LoomError> {
    loom.add_system(AgentCommandingWeftSystem)?;
    loom.add_system(SpatialCollisionWeftSystem)?;
    loom.add_system(CameraOcclusionWeftSystem)?;

    loom.add_ordering("agent_commanding_weft", "velocity_kinematics")?;
    loom.add_ordering("velocity_kinematics", "spatial_collision_3d")?;
    loom.add_ordering("spatial_collision_3d", "camera_occlusion_fade")?;
    loom.add_ordering("spatial_collision_3d", "positional_audio_3d")?;
    if loom.has_system("particle_simulation_weft") {
        loom.add_ordering("spatial_collision_3d", "particle_simulation_weft")?;
    }
    if loom.has_system("fog_of_war_update") {
        loom.add_ordering("spatial_collision_3d", "fog_of_war_update")?;
    }
    loom.add_ordering("camera_orbit_input", "camera_occlusion_fade")?;
    loom.add_ordering("flipbook_sprite_animation", "camera_occlusion_fade")?;
    Ok(())
}

/// Ray-AABB intersection test using the slab method.
/// Returns `Some(t_near)` if the segment from `origin` along `dir` (with length `max_dist`) intersects `[min, max]`.
pub fn ray_intersects_aabb(origin: Vec3, dir: Vec3, max_dist: f32, min: Vec3, max: Vec3) -> Option<f32> {
    let mut tmin = 0.0f32;
    let mut tmax = max_dist;

    for axis in 0..3 {
        let o = origin[axis];
        let d = dir[axis];
        let bmin = min[axis];
        let bmax = max[axis];

        if d.abs() < 1e-6 {
            if o < bmin || o > bmax {
                return None;
            }
        } else {
            let inv_d = 1.0 / d;
            let mut t1 = (bmin - o) * inv_d;
            let mut t2 = (bmax - o) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }

    Some(tmin)
}

/// Computes the minimum translation vector (MTV) in XZ or step-up in Y to separate a dynamic ` (dyn_min, dyn_max)`
/// AABB from a static `(stat_min, stat_max)` AABB.
pub fn resolve_aabb_vs_static(
    dyn_pos: Vec3,
    dyn_collider: &Collider3D,
    stat_min: Vec3,
    stat_max: Vec3,
) -> Vec3 {
    let (dmin, dmax) = dyn_collider.world_aabb(dyn_pos);

    let overlap_x = (dmax.x.min(stat_max.x) - dmin.x.max(stat_min.x)).max(0.0);
    let overlap_y = (dmax.y.min(stat_max.y) - dmin.y.max(stat_min.y)).max(0.0);
    let overlap_z = (dmax.z.min(stat_max.z) - dmin.z.max(stat_min.z)).max(0.0);

    if overlap_x <= 0.0 || overlap_y <= 0.0 || overlap_z <= 0.0 {
        return dyn_pos;
    }

    // Check if the obstacle top surface is within `max_step_height` above the dynamic entity's feet (`dmin.y`)
    let step_up_needed = stat_max.y - dmin.y;
    if step_up_needed > 0.0 && step_up_needed <= dyn_collider.max_step_height {
        let mut stepped = dyn_pos;
        stepped.y += step_up_needed;
        return stepped;
    }

    // Otherwise push horizontally along the shallowest XZ penetration axis
    let center_d = (dmin + dmax) * 0.5;
    let center_s = (stat_min + stat_max) * 0.5;
    let mut resolved = dyn_pos;

    if overlap_x < overlap_z {
        if center_d.x < center_s.x {
            resolved.x -= overlap_x;
        } else {
            resolved.x += overlap_x;
        }
    } else if center_d.z < center_s.z {
        resolved.z -= overlap_z;
    } else {
        resolved.z += overlap_z;
    }

    resolved
}

/// Weft system that resolves 2.5D/3D collisions, stair/ledge step-ups, and ground-plane clamping.
pub struct SpatialCollisionWeftSystem;

impl WeftSystem for SpatialCollisionWeftSystem {
    fn name(&self) -> &'static str {
        "spatial_collision_3d"
    }

    fn reads(&self) -> &[&'static str] {
        &["Collider3D", "NameStrand"]
    }

    fn writes(&self) -> &[&'static str] {
        &["EventWeftStrand", "Transform"]
    }

    fn weave(&mut self, world: &mut World, _dt: f32) {
        let mut static_boxes: Vec<(String, Vec3, Vec3)> = Vec::new();
        let mut dynamic_entities: Vec<(EntityId, String, Collider3D, Vec3)> = Vec::new();

        for (entity, col) in world.query::<Collider3D>() {
            let Some(transform) = world.get_strand::<Transform>(entity) else {
                continue;
            };
            let label = world
                .get_strand::<NameStrand>(entity)
                .map(|n| n.0.clone())
                .unwrap_or_else(|| entity.to_string());
            if col.is_static {
                let (bmin, bmax) = col.world_aabb(transform.translation);
                static_boxes.push((label, bmin, bmax));
            } else {
                dynamic_entities.push((entity, label, *col, transform.translation));
            }
        }

        let mut events = Vec::new();

        for (dyn_entity, dyn_label, dyn_col, mut pos) in dynamic_entities {
            // Clamp to ground floor y >= 0.0
            if pos.y < 0.0 {
                pos.y = 0.0;
            }

            for (stat_label, stat_min, stat_max) in &static_boxes {
                let prev = pos;
                pos = resolve_aabb_vs_static(pos, &dyn_col, *stat_min, *stat_max);
                if (pos - prev).length_squared() > 1e-6 {
                    if (pos.y - prev.y).abs() > 1e-4 {
                        events.push(GameplayEvent::SteppedUp {
                            entity: dyn_label.clone(),
                            step_height: pos.y - prev.y,
                        });
                    } else {
                        events.push(GameplayEvent::CollisionContact {
                            dynamic_entity: dyn_label.clone(),
                            obstacle_entity: stat_label.clone(),
                            contact_point: pos,
                        });
                    }
                }
            }

            if let Some(transform) = world.get_strand_mut::<Transform>(dyn_entity) {
                transform.translation = pos;
            }
        }

        if !events.is_empty()
            && let Some((_, bus)) = world.query_mut::<EventWeftStrand>().next()
        {
            for ev in events {
                bus.publish(ev);
            }
        }
    }
}

/// Weft system that casts a ray from [`CameraRig::eye`] to the hero entity and fades occluding foreground props.
pub struct CameraOcclusionWeftSystem;

impl WeftSystem for CameraOcclusionWeftSystem {
    fn name(&self) -> &'static str {
        "camera_occlusion_fade"
    }

    fn reads(&self) -> &[&'static str] {
        &["CameraRig", "Collider3D", "NameStrand", "Transform"]
    }

    fn writes(&self) -> &[&'static str] {
        &["Mesh3D", "Sprite25D"]
    }

    fn weave(&mut self, world: &mut World, _dt: f32) {
        let Some((_, &camera)) = world.query::<CameraRig>().next() else {
            return;
        };

        // Find the primary dynamic/hero target position (or fallback to camera target)
        let hero_pos = world
            .query::<NameStrand>()
            .find(|(_, name)| name.0.contains("hero"))
            .and_then(|(e, _)| world.get_strand::<Transform>(e))
            .map(|t| t.translation + Vec3::new(0.0, 0.9, 0.0))
            .unwrap_or(camera.target);

        let to_hero = hero_pos - camera.eye;
        let max_dist = to_hero.length();
        if max_dist < 1e-4 {
            return;
        }
        let ray_dir = to_hero / max_dist;

        let mut fade_states: Vec<(EntityId, bool)> = Vec::new();
        for (entity, col) in world.query::<Collider3D>() {
            if !col.fade_when_occluding_camera {
                continue;
            }
            let Some(transform) = world.get_strand::<Transform>(entity) else {
                continue;
            };
            let (bmin, bmax) = col.world_aabb(transform.translation);
            let occluded = ray_intersects_aabb(camera.eye, ray_dir, max_dist * 0.96, bmin, bmax).is_some();
            fade_states.push((entity, occluded));
        }

        for (entity, is_occluding) in fade_states {
            let target_alpha = if is_occluding { 0.35 } else { 1.0 };
            if let Some(sprite) = world.get_strand_mut::<Sprite25D>(entity) {
                sprite.tint.w = target_alpha;
            }
            if let Some(mesh) = world.get_strand_mut::<Mesh3D>(entity) {
                mesh.material.base_color.w = target_alpha;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saan_core::{ColliderShape, Velocity};

    #[test]
    fn spatial_systems_weave_cleanly_with_zero_tension_hazards() {
        let mut loom = Loom::with_default_gameplay();
        register_spatial_systems(&mut loom).unwrap();

        let report = loom.inspect_tension();
        assert!(report.is_taut(), "Expected taut Loom DAG, got hazards: {:?}", report.hazards);
        assert_eq!(report.system_count, 9);
    }

    #[test]
    fn dynamic_cylinder_steps_up_low_ledge_and_slides_against_tall_wall() {
        let dyn_col = Collider3D {
            shape: ColliderShape::CylinderY {
                radius: 0.5,
                height: 1.8,
            },
            is_static: false,
            max_step_height: 0.45,
            fade_when_occluding_camera: false,
        };

        // Low step (height 0.35m <= max_step_height 0.45m) -> steps up onto y = 0.35
        let stepped = resolve_aabb_vs_static(
            Vec3::new(0.0, 0.0, 0.0),
            &dyn_col,
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(1.0, 0.35, 1.0),
        );
        assert!((stepped.y - 0.35).abs() < 1e-4);

        // Tall altar wall (height 1.5m > max_step_height 0.45m) -> pushed out horizontally along Z
        let pushed = resolve_aabb_vs_static(
            Vec3::new(0.0, 0.0, 0.8),
            &dyn_col,
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(1.0, 1.5, 0.5),
        );
        assert!((pushed.y - 0.0).abs() < 1e-4);
        assert!(pushed.z >= 1.0 - 1e-4);
    }

    #[test]
    fn camera_raycast_fades_occluding_foreground_tree() {
        let mut world = World::new();
        let cam_e = world.spawn();
        world.weave_strand(
            cam_e,
            CameraRig {
                eye: Vec3::new(0.0, 2.0, 10.0),
                target: Vec3::ZERO,
                ..Default::default()
            },
        );

        let hero_e = world.spawn();
        world
            .weave_strand(hero_e, NameStrand("wanderer_hero_25d".into()))
            .weave_strand(hero_e, Transform::from_xyz(0.0, 0.0, 0.0))
            .weave_strand(hero_e, Velocity::default());

        // Tree directly between camera (z=10) and hero (z=0) at z=5
        let tree_e = world.spawn();
        world
            .weave_strand(tree_e, Transform::from_xyz(0.0, 0.0, 5.0))
            .weave_strand(
                tree_e,
                Collider3D {
                    shape: ColliderShape::CylinderY {
                        radius: 1.0,
                        height: 4.0,
                    },
                    is_static: true,
                    max_step_height: 0.0,
                    fade_when_occluding_camera: true,
                },
            )
            .weave_strand(tree_e, Sprite25D::default());

        let mut sys = CameraOcclusionWeftSystem;
        sys.weave(&mut world, 0.016);

        let tree_sprite = world.get_strand::<Sprite25D>(tree_e).unwrap();
        assert!((tree_sprite.tint.w - 0.35).abs() < 1e-4);
    }

    #[test]
    fn grid_a_star_navigates_elevated_terrain_and_respects_walls() {
        let mut grid = GridMap3D::new(5, 5, 1.0, Vec3::ZERO);
        // Put a wall at (2, 0), (2, 1), (2, 2)
        grid.set_tile(2, 0, TileData { walkable: false, ..Default::default() });
        grid.set_tile(2, 1, TileData { walkable: false, ..Default::default() });
        grid.set_tile(2, 2, TileData { walkable: false, ..Default::default() });

        // Elevated step at (1, 3)
        grid.set_tile(1, 3, TileData { elevation: 0.5, ..Default::default() });

        let path = grid.find_path_a_star((0, 1), (4, 1), 1.0).unwrap();
        assert_eq!(*path.first().unwrap(), (0, 1));
        assert_eq!(*path.last().unwrap(), (4, 1));
        // Path should loop around the wall (y >= 3)
        assert!(path.iter().any(|p| p.1 >= 3));
    }

    #[test]
    fn flow_field_steers_rts_squad_towards_target() {
        let grid = GridMap3D::new(8, 8, 1.0, Vec3::ZERO);
        let flow = FlowField25D::build(&grid, (0, 0));

        let steer = flow.sample_steering(Vec3::new(4.5, 0.0, 0.5));
        // At (4, 0) steering towards (0, 0) should have negative X direction
        assert!(steer.x < 0.0);
    }

    #[test]
    fn rts_squad_executes_move_order() {
        let mut world = World::new();
        let unit_e = world.spawn();
        world
            .weave_strand(unit_e, Transform::from_xyz(0.0, 0.0, 0.0))
            .weave_strand(unit_e, Velocity::default())
            .weave_strand(
                unit_e,
                AgentController {
                    mode: ControllerMode::RtsSquad {
                        squad_id: 1,
                        move_speed: 4.0,
                        current_order: Some(UnitOrder::MoveTo(Vec3::new(10.0, 0.0, 0.0))),
                        order_queue: Vec::new(),
                    },
                },
            );

        let mut sys = AgentCommandingWeftSystem;
        sys.weave(&mut world, 0.016);

        let v = world.get_strand::<Velocity>(unit_e).unwrap();
        assert!(v.linear.x > 3.9);
    }
}
