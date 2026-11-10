//! # `saan_spatial::combat` — Real-Time Action RPG Combat, Hitbox/Hurtbox & State Machine Systems
//!
//! Provides deterministic Weft systems for real-time 2.5D combat:
//! - [`ActionStateWeftSystem`]: Manages player/enemy action state transitions (`Idle`, `Move`, `Dash`, `Attack`, `Hurt`, `Dead`),
//!   handles input buffering, combo attacks, dodge rolls, and sprite flipbook synchronization.
//! - [`CombatWeftSystem`]: Evaluates geometric overlaps between active [`HitboxStrand`] and vulnerable [`HurtboxStrand`],
//!   applies damage to [`HealthStrand`], injects knockback impulse into [`Velocity`], grants i-frames, and publishes [`GameplayEvent::CombatHit`].

use glam::{Vec2, Vec3};
use saan_core::{
    ActionState, ActionStateStrand, CombatShape, EntityId, EventWeftStrand, FlipbookStrand,
    GameplayEvent, HealthStrand, HitboxStrand, HurtboxStrand, InputStrand, NameStrand, Transform,
    Velocity, WeftSystem, World,
};

/// Evaluates whether two combat shapes overlap in 3D world space.
pub fn combat_shapes_overlap(shape_a: &CombatShape, pos_a: Vec3, shape_b: &CombatShape, pos_b: Vec3) -> bool {
    match (shape_a, shape_b) {
        (CombatShape::CylinderY { radius: r_a, height: h_a }, CombatShape::CylinderY { radius: r_b, height: h_b }) => {
            let dx = pos_a.x - pos_b.x;
            let dz = pos_a.z - pos_b.z;
            let dist_sq_xz = dx * dx + dz * dz;
            let rad_sum = r_a + r_b;
            if dist_sq_xz > rad_sum * rad_sum {
                return false;
            }
            pos_a.y < (pos_b.y + h_b) && (pos_a.y + h_a) > pos_b.y
        }
        (CombatShape::Sphere { radius: r_a }, CombatShape::Sphere { radius: r_b }) => {
            pos_a.distance_squared(pos_b) <= (r_a + r_b) * (r_a + r_b)
        }
        (CombatShape::Aabb { half_extents: h_a }, CombatShape::Aabb { half_extents: h_b }) => {
            (pos_a.x - pos_b.x).abs() <= (h_a.x + h_b.x)
                && (pos_a.y - pos_b.y).abs() <= (h_a.y + h_b.y)
                && (pos_a.z - pos_b.z).abs() <= (h_a.z + h_b.z)
        }
        // Approximate cross-shape overlaps via bounding spheres
        (a, b) => {
            let r_a = match a {
                CombatShape::CylinderY { radius, height } => radius.max(*height * 0.5),
                CombatShape::Sphere { radius } => *radius,
                CombatShape::Aabb { half_extents } => half_extents.length(),
            };
            let r_b = match b {
                CombatShape::CylinderY { radius, height } => radius.max(*height * 0.5),
                CombatShape::Sphere { radius } => *radius,
                CombatShape::Aabb { half_extents } => half_extents.length(),
            };
            pos_a.distance_squared(pos_b) <= (r_a + r_b) * (r_a + r_b)
        }
    }
}

/// Spawns a transient melee attack hitbox in front of the attacking entity.
pub fn spawn_melee_hitbox(
    world: &mut World,
    owner: EntityId,
    team: u8,
    owner_pos: Vec3,
    facing: Vec2,
    combo_index: u32,
) -> EntityId {
    let hitbox_entity = world.spawn();
    let facing_norm = facing.normalize_or_zero();
    let forward_3d = Vec3::new(facing_norm.x, 0.0, facing_norm.y);
    let hit_pos = owner_pos + forward_3d * 1.1 + Vec3::new(0.0, 0.4, 0.0);

    let damage = 20.0 + (combo_index as f32) * 10.0;
    let knockback_mag = 6.0 + (combo_index as f32) * 2.5;
    let knockback = forward_3d * knockback_mag + Vec3::new(0.0, 1.5, 0.0);
    let radius = 0.9 + (combo_index as f32) * 0.15;

    let hitbox = HitboxStrand::new(
        owner,
        team,
        damage,
        knockback,
        CombatShape::CylinderY { radius, height: 1.2 },
        0.14, // Brief 140ms hitbox window
    );

    world
        .weave_strand(hitbox_entity, NameStrand(format!("melee_hitbox_combo_{combo_index}")))
        .weave_strand(hitbox_entity, Transform::from_xyz(hit_pos.x, hit_pos.y, hit_pos.z))
        .weave_strand(hitbox_entity, hitbox);

    hitbox_entity
}

/// Action state machine Weft system for responsive ARPG character controls.
pub struct ActionStateWeftSystem;

impl WeftSystem for ActionStateWeftSystem {
    fn name(&self) -> &'static str {
        "action_state_weft"
    }

    fn reads(&self) -> &[&'static str] {
        &["InputStrand", "ActionStateStrand", "Transform"]
    }

    fn writes(&self) -> &[&'static str] {
        &["ActionStateStrand", "Velocity", "FlipbookStrand"]
    }

    fn weave(&mut self, world: &mut World, dt: f32) {
        let input = world.query::<InputStrand>().next().map(|(_, inp)| *inp);

        // Collect new attack spawns to execute after iteration
        let mut attacks_to_spawn: Vec<(EntityId, u8, Vec3, Vec2, u32)> = Vec::new();

        // 1. Process player/entity action state updates
        let entities: Vec<EntityId> = world.query::<ActionStateStrand>().map(|(e, _)| e).collect();

        for entity in entities {
            let trans_pos = world.get_strand::<Transform>(entity).map(|t| t.translation).unwrap_or(Vec3::ZERO);
            let team = world.get_strand::<HurtboxStrand>(entity).map(|h| h.team).unwrap_or(0);

            let mut new_vel: Option<Vec3> = None;
            let mut flipbook_start_end: Option<(u32, u32)> = None;

            if let Some(action) = world.get_strand_mut::<ActionStateStrand>(entity) {
                // Buffer inputs if interactive input is active and entity is player (team == 0)
                if team == 0 {
                    if let Some(inp) = input {
                        if inp.dash {
                            let move_dir = Vec2::new(inp.move_axis.x, inp.move_axis.z);
                            let dir = if move_dir.length_squared() > 1e-4 {
                                move_dir.normalize()
                            } else {
                                action.facing_direction
                            };
                            action.buffered_dash = Some(dir);
                        }
                        if inp.attack {
                            action.buffered_attack = true;
                        }
                    }
                }

                match action.state {
                    ActionState::Idle => {
                        flipbook_start_end = Some((0, 0));
                        if let Some(dir) = action.buffered_dash.take() {
                            action.state = ActionState::Dash {
                                timer: action.dash_duration,
                                max_time: action.dash_duration,
                                direction: dir,
                                speed: action.dash_speed,
                            };
                            action.facing_direction = dir;
                        } else if action.buffered_attack {
                            action.buffered_attack = false;
                            action.state = ActionState::Attack {
                                combo_index: 1,
                                timer: 0.28,
                                max_time: 0.28,
                                has_spawned_hitbox: false,
                            };
                        } else if team == 0 {
                            if let Some(inp) = input {
                                let move_dir = Vec2::new(inp.move_axis.x, inp.move_axis.z);
                                if move_dir.length_squared() > 1e-4 {
                                    let norm = move_dir.normalize();
                                    action.facing_direction = norm;
                                    action.state = ActionState::Move { speed: action.move_speed };
                                }
                            }
                        }
                    }
                    ActionState::Move { speed } => {
                        flipbook_start_end = Some((0, 3)); // 4-frame walk cycle
                        if let Some(dir) = action.buffered_dash.take() {
                            action.state = ActionState::Dash {
                                timer: action.dash_duration,
                                max_time: action.dash_duration,
                                direction: dir,
                                speed: action.dash_speed,
                            };
                            action.facing_direction = dir;
                        } else if action.buffered_attack {
                            action.buffered_attack = false;
                            action.state = ActionState::Attack {
                                combo_index: 1,
                                timer: 0.28,
                                max_time: 0.28,
                                has_spawned_hitbox: false,
                            };
                        } else if team == 0 {
                            if let Some(inp) = input {
                                let move_dir = Vec2::new(inp.move_axis.x, inp.move_axis.z);
                                if move_dir.length_squared() > 1e-4 {
                                    let norm = move_dir.normalize();
                                    action.facing_direction = norm;
                                    new_vel = Some(Vec3::new(norm.x * speed, 0.0, norm.y * speed));
                                } else {
                                    action.state = ActionState::Idle;
                                    new_vel = Some(Vec3::ZERO);
                                }
                            }
                        }
                    }
                    ActionState::Dash { ref mut timer, direction, speed, .. } => {
                        flipbook_start_end = Some((1, 1));
                        new_vel = Some(Vec3::new(direction.x * speed, 0.0, direction.y * speed));
                        *timer -= dt;
                        if *timer <= 0.0 {
                            action.state = ActionState::Idle;
                            new_vel = Some(Vec3::ZERO);
                        }
                    }
                    ActionState::Attack { combo_index, ref mut timer, max_time, ref mut has_spawned_hitbox } => {
                        flipbook_start_end = Some((2, 2));
                        // Lunge slightly on early attack frames
                        let progress = 1.0 - (*timer / max_time);
                        if progress < 0.4 {
                            let lunge = action.facing_direction * 4.0;
                            new_vel = Some(Vec3::new(lunge.x, 0.0, lunge.y));
                        } else {
                            new_vel = Some(Vec3::ZERO);
                        }

                        // Spawn damaging hitbox at midpoint of swing
                        if progress >= 0.25 && !*has_spawned_hitbox {
                            *has_spawned_hitbox = true;
                            attacks_to_spawn.push((entity, team, trans_pos, action.facing_direction, combo_index));
                        }

                        *timer -= dt;
                        if *timer <= 0.0 {
                            if action.buffered_attack && combo_index < 3 {
                                action.buffered_attack = false;
                                action.state = ActionState::Attack {
                                    combo_index: combo_index + 1,
                                    timer: 0.24,
                                    max_time: 0.24,
                                    has_spawned_hitbox: false,
                                };
                            } else {
                                action.state = ActionState::Idle;
                                new_vel = Some(Vec3::ZERO);
                            }
                        }
                    }
                    ActionState::Hurt { ref mut timer, .. } => {
                        flipbook_start_end = Some((0, 0));
                        *timer -= dt;
                        if *timer <= 0.0 {
                            action.state = ActionState::Idle;
                        }
                    }
                    ActionState::Dead => {
                        new_vel = Some(Vec3::ZERO);
                    }
                }
            }

            if let Some(vel_vec) = new_vel {
                if let Some(vel) = world.get_strand_mut::<Velocity>(entity) {
                    vel.linear.x = vel_vec.x;
                    vel.linear.z = vel_vec.z;
                }
            }

            if let Some((start, end)) = flipbook_start_end {
                if let Some(flipbook) = world.get_strand_mut::<FlipbookStrand>(entity) {
                    if flipbook.start_frame != start || flipbook.end_frame != end {
                        flipbook.start_frame = start;
                        flipbook.end_frame = end;
                        flipbook.current_frame = start;
                    }
                }
            }
        }

        // 2. Spawn hitboxes triggered by attack states
        for (owner, team, pos, facing, combo) in attacks_to_spawn {
            spawn_melee_hitbox(world, owner, team, pos, facing, combo);
        }
    }
}

/// Resolves combat hitbox/hurtbox collisions, health degradation, knockback, and events.
pub struct CombatWeftSystem;

impl WeftSystem for CombatWeftSystem {
    fn name(&self) -> &'static str {
        "combat_resolution_weft"
    }

    fn reads(&self) -> &[&'static str] {
        &["HitboxStrand", "HurtboxStrand", "Transform", "NameStrand"]
    }

    fn writes(&self) -> &[&'static str] {
        &["HealthStrand", "Velocity", "ActionStateStrand", "HitboxStrand", "HurtboxStrand", "EventWeftStrand"]
    }

    fn weave(&mut self, world: &mut World, dt: f32) {
        // 1. Tick down i-frames on hurtboxes
        for (_, hurtbox) in world.query_mut::<HurtboxStrand>() {
            hurtbox.invulnerability_timer = (hurtbox.invulnerability_timer - dt).max(0.0);
        }

        // 2. Collect active hitboxes
        let hitboxes: Vec<(EntityId, HitboxStrand, Vec3)> = world
            .query::<HitboxStrand>()
            .filter_map(|(e, h)| {
                world.get_strand::<Transform>(e).map(|t| (e, h.clone(), t.translation + h.offset))
            })
            .collect();

        // 3. Collect vulnerable hurtbox targets
        let hurtboxes: Vec<(EntityId, HurtboxStrand, Vec3, String)> = world
            .query::<HurtboxStrand>()
            .filter_map(|(e, h)| {
                let trans = world.get_strand::<Transform>(e)?;
                let name = world.get_strand::<NameStrand>(e).map(|n| n.0.clone()).unwrap_or_else(|| format!("entity_{e}"));
                Some((e, h.clone(), trans.translation + h.offset, name))
            })
            .collect();

        let mut hits_to_apply: Vec<(EntityId, EntityId, f32, Vec3, f32, Vec3, String, String)> = Vec::new();

        for (hitbox_entity, mut hitbox, hit_pos) in hitboxes {
            for (target_entity, hurtbox, hurt_pos, target_name) in &hurtboxes {
                if *target_entity == hitbox.owner || hurtbox.team == hitbox.team {
                    continue;
                }
                if hurtbox.invulnerability_timer > 0.0 || hitbox.already_hit.contains(target_entity) {
                    continue;
                }

                if combat_shapes_overlap(&hitbox.shape, hit_pos, &hurtbox.shape, *hurt_pos) {
                    hitbox.already_hit.push(*target_entity);
                    let attacker_name = world
                        .get_strand::<NameStrand>(hitbox.owner)
                        .map(|n| n.0.clone())
                        .unwrap_or_else(|| format!("entity_{}", hitbox.owner));

                    hits_to_apply.push((
                        hitbox_entity,
                        *target_entity,
                        hitbox.damage,
                        hitbox.knockback,
                        hitbox.hitstun,
                        hit_pos,
                        attacker_name,
                        target_name.clone(),
                    ));
                }
            }

            // Sync already_hit array back to the hitbox component
            if let Some(h) = world.get_strand_mut::<HitboxStrand>(hitbox_entity) {
                h.already_hit = hitbox.already_hit;
            }
        }

        // 4. Apply damage, knockback, and publish events
        let mut events_to_publish: Vec<GameplayEvent> = Vec::new();

        for (_hitbox_e, target_e, damage, knockback, hitstun, hit_pos, attacker_name, target_name) in hits_to_apply {
            let mut is_dead = false;

            if let Some(health) = world.get_strand_mut::<HealthStrand>(target_e) {
                health.take_damage(damage);
                is_dead = !health.is_alive;
            }

            if let Some(hurtbox) = world.get_strand_mut::<HurtboxStrand>(target_e) {
                hurtbox.invulnerability_timer = 0.25; // 250ms i-frames
            }

            if let Some(vel) = world.get_strand_mut::<Velocity>(target_e) {
                vel.linear += knockback;
            }

            if let Some(action) = world.get_strand_mut::<ActionStateStrand>(target_e) {
                action.state = if is_dead {
                    ActionState::Dead
                } else {
                    ActionState::Hurt { timer: hitstun, max_time: hitstun }
                };
            }

            events_to_publish.push(GameplayEvent::CombatHit {
                attacker: attacker_name,
                target: target_name.clone(),
                damage,
                hit_pos,
                knockback,
            });

            events_to_publish.push(GameplayEvent::AudioCue {
                emitter: target_name,
                sound_id: "audio/combat_impact_blade.ogg".to_string(),
                stereo_gain: [0.75, 0.75],
            });
        }

        if let Some((_, bus)) = world.query_mut::<EventWeftStrand>().next() {
            for ev in events_to_publish {
                bus.publish(ev);
            }
        }

        // 5. Age and despawn expired hitboxes
        let mut expired_hitboxes: Vec<EntityId> = Vec::new();
        for (e, hitbox) in world.query_mut::<HitboxStrand>() {
            hitbox.lifetime -= dt;
            if hitbox.lifetime <= 0.0 {
                expired_hitboxes.push(e);
            }
        }

        for e in expired_hitboxes {
            world.despawn(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn melee_attack_damages_enemy_applies_knockback_and_publishes_event() {
        let mut world = World::new();

        // Event bus
        let bus_e = world.spawn();
        world.weave_strand(bus_e, EventWeftStrand::default());

        // Player attacker (Team 0) facing +Z
        let player = world.spawn();
        world
            .weave_strand(player, NameStrand("HeroWanderer".to_string()))
            .weave_strand(player, Transform::from_xyz(0.0, 0.0, 0.0))
            .weave_strand(player, Velocity::default())
            .weave_strand(
                player,
                ActionStateStrand {
                    state: ActionState::Attack {
                        combo_index: 1,
                        timer: 0.14,
                        max_time: 0.28,
                        has_spawned_hitbox: false,
                    },
                    facing_direction: Vec2::new(0.0, 1.0), // Facing +Z
                    ..Default::default()
                },
            )
            .weave_strand(player, HurtboxStrand::cylinder(0.5, 1.8, 0));

        // Enemy victim (Team 1) standing at (0.0, 0.0, 1.0)
        let enemy = world.spawn();
        world
            .weave_strand(enemy, NameStrand("ShadowBandit".to_string()))
            .weave_strand(enemy, Transform::from_xyz(0.0, 0.0, 1.0))
            .weave_strand(enemy, Velocity::default())
            .weave_strand(enemy, HealthStrand::new(100.0))
            .weave_strand(enemy, ActionStateStrand::default())
            .weave_strand(enemy, HurtboxStrand::cylinder(0.5, 1.8, 1));

        // 1. Run ActionStateWeftSystem: Spawns the attack hitbox in front of the player
        let mut action_sys = ActionStateWeftSystem;
        action_sys.weave(&mut world, 0.016);

        assert_eq!(world.query::<HitboxStrand>().count(), 1);

        // 2. Run CombatWeftSystem: Resolves damage, knockback, and event emission
        let mut combat_sys = CombatWeftSystem;
        combat_sys.weave(&mut world, 0.016);

        // Verify enemy took damage
        let enemy_health = world.get_strand::<HealthStrand>(enemy).unwrap();
        assert_eq!(enemy_health.current, 70.0); // 100 - 30 damage (combo 1: 20 + 10)

        // Verify enemy took knockback along +Z
        let enemy_vel = world.get_strand::<Velocity>(enemy).unwrap();
        assert!(enemy_vel.linear.z > 0.0);

        // Verify enemy entered Hurt state
        let enemy_action = world.get_strand::<ActionStateStrand>(enemy).unwrap();
        assert!(matches!(enemy_action.state, ActionState::Hurt { .. }));

        // Verify enemy hurtbox gained i-frames
        let enemy_hurt = world.get_strand::<HurtboxStrand>(enemy).unwrap();
        assert!(enemy_hurt.invulnerability_timer > 0.2);

        // Verify CombatHit event was published to EventWeft
        let bus = world.get_strand::<EventWeftStrand>(bus_e).unwrap();
        let hit_event = bus
            .current_events
            .iter()
            .find(|ev| matches!(ev, GameplayEvent::CombatHit { .. }));
        assert!(hit_event.is_some());
    }

    #[test]
    fn dash_dodge_roll_moves_character_and_recovers_to_idle() {
        let mut world = World::new();
        let player = world.spawn();
        world
            .weave_strand(player, Transform::from_xyz(0.0, 0.0, 0.0))
            .weave_strand(player, Velocity::default())
            .weave_strand(
                player,
                ActionStateStrand {
                    state: ActionState::Dash {
                        timer: 0.1,
                        max_time: 0.2,
                        direction: Vec2::new(1.0, 0.0), // Dash +X
                        speed: 10.0,
                    },
                    ..Default::default()
                },
            );

        let mut sys = ActionStateWeftSystem;
        sys.weave(&mut world, 0.05);

        // While dashing, velocity reflects dash speed
        let vel = world.get_strand::<Velocity>(player).unwrap();
        assert_eq!(vel.linear.x, 10.0);

        // Advance past dash duration
        sys.weave(&mut world, 0.06);
        let action = world.get_strand::<ActionStateStrand>(player).unwrap();
        assert_eq!(action.state, ActionState::Idle);

        let vel_after = world.get_strand::<Velocity>(player).unwrap();
        assert_eq!(vel_after.linear.x, 0.0);
    }
}
