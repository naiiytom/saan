//! Unit selection, agent commanding, and movement execution across Action, RTS, and TBS modes.

use glam::{Vec2, Vec3};
use saan_core::{EntityId, Transform, Velocity, WeftSystem, World};
use serde::{Deserialize, Serialize};

use crate::navigation::GridMap3D;

/// An order issued to an RTS squad or tactical unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnitOrder {
    MoveTo(Vec3),
    Attack(EntityId),
    Hold,
    Interact(EntityId),
}

/// Agent controller mode governing how an entity moves and responds to player or AI input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ControllerMode {
    /// Action/Adventure single-hero direct movement (WASD/Gamepad vectors).
    DirectAction {
        move_speed: f32,
        sprint_multiplier: f32,
    },
    /// RTS unit squad member responding to mouse-click move/attack orders and flow fields.
    RtsSquad {
        squad_id: u32,
        move_speed: f32,
        current_order: Option<UnitOrder>,
        order_queue: Vec<UnitOrder>,
    },
    /// TBS grid actor acting during discrete strategy turns.
    TurnBasedActor {
        team_id: u32,
        action_points: u32,
        max_action_points: u32,
        move_cost_per_cell: u32,
        path: Vec<(i32, i32)>,
    },
}

/// Unified Agent Controller strand attached to heroes, RTS units, or TBS tactical pawns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentController {
    pub mode: ControllerMode,
}

impl AgentController {
    pub fn direct_action(speed: f32) -> Self {
        Self {
            mode: ControllerMode::DirectAction {
                move_speed: speed,
                sprint_multiplier: 1.6,
            },
        }
    }

    pub fn rts_squad(squad_id: u32, speed: f32) -> Self {
        Self {
            mode: ControllerMode::RtsSquad {
                squad_id,
                move_speed: speed,
                current_order: None,
                order_queue: Vec::new(),
            },
        }
    }

    pub fn turn_based_actor(team_id: u32, ap: u32) -> Self {
        Self {
            mode: ControllerMode::TurnBasedActor {
                team_id,
                action_points: ap,
                max_action_points: ap,
                move_cost_per_cell: 1,
                path: Vec::new(),
            },
        }
    }
}

/// Selection strand marking whether an entity is currently selected (e.g. by mouse drag-box).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SelectionStrand {
    pub is_selected: bool,
    pub selection_radius: f32,
    /// Team / faction ID (e.g. 0 = Player, 1 = Enemy).
    pub faction: u32,
}

impl Default for SelectionStrand {
    fn default() -> Self {
        Self {
            is_selected: false,
            selection_radius: 0.8,
            faction: 0,
        }
    }
}

/// Drag-box selection query in world coordinates.
pub fn select_entities_in_rect(
    world: &mut World,
    min_xz: Vec2,
    max_xz: Vec2,
    faction_filter: Option<u32>,
) -> Vec<EntityId> {
    let mut selected = Vec::new();

    let entities: Vec<(EntityId, Vec3, SelectionStrand)> = world
        .query::<SelectionStrand>()
        .filter_map(|(e, &sel)| {
            let pos = world.get_strand::<Transform>(e)?.translation;
            Some((e, pos, sel))
        })
        .collect();

    for (entity, pos, mut sel) in entities {
        let inside = pos.x >= min_xz.x
            && pos.x <= max_xz.x
            && pos.z >= min_xz.y
            && pos.z <= max_xz.y;

        let faction_match = faction_filter.map(|f| f == sel.faction).unwrap_or(true);

        if inside && faction_match {
            sel.is_selected = true;
            selected.push(entity);
        } else if faction_filter.is_some() {
            sel.is_selected = false;
        }

        if let Some(s) = world.get_strand_mut::<SelectionStrand>(entity) {
            *s = sel;
        }
    }

    selected
}

/// Weft system that updates agent velocities and paths according to their [`AgentController`].
pub struct AgentCommandingWeftSystem;

impl WeftSystem for AgentCommandingWeftSystem {
    fn name(&self) -> &'static str {
        "agent_commanding_weft"
    }

    fn reads(&self) -> &[&'static str] {
        &["AgentController", "GridMap3D", "Transform"]
    }

    fn writes(&self) -> &[&'static str] {
        &["AgentController", "Transform", "Velocity"]
    }

    fn weave(&mut self, world: &mut World, _dt: f32) {
        let grid_ref = world.query::<GridMap3D>().next().map(|(_, g)| g.clone());

        let mut agents: Vec<(EntityId, AgentController, Vec3)> = world
            .query::<AgentController>()
            .filter_map(|(e, c)| {
                let pos = world.get_strand::<Transform>(e)?.translation;
                Some((e, c.clone(), pos))
            })
            .collect();

        let mut vel_updates = Vec::new();
        let mut pos_updates = Vec::new();

        for (entity, controller, translation) in &mut agents {
            match &mut controller.mode {
                ControllerMode::DirectAction { .. } => {}
                ControllerMode::RtsSquad { move_speed, current_order, order_queue, .. } => {
                    if current_order.is_none() && !order_queue.is_empty() {
                        *current_order = Some(order_queue.remove(0));
                    }

                    if let Some(order) = current_order {
                        match order {
                            UnitOrder::MoveTo(target) => {
                                let diff = *target - *translation;
                                let dist_xz = Vec2::new(diff.x, diff.z).length();
                                if dist_xz < 0.25 {
                                    *current_order = None;
                                    vel_updates.push((*entity, Vec3::ZERO));
                                } else {
                                    let dir_flat = Vec3::new(diff.x, 0.0, diff.z).normalize_or_zero();
                                    vel_updates.push((*entity, dir_flat * *move_speed));
                                }
                            }
                            UnitOrder::Hold => {
                                vel_updates.push((*entity, Vec3::ZERO));
                            }
                            _ => {}
                        }
                    }
                }
                ControllerMode::TurnBasedActor { action_points, move_cost_per_cell, path, .. } => {
                    if !path.is_empty() && *action_points >= *move_cost_per_cell {
                        let next_cell = path.remove(0);
                        *action_points -= *move_cost_per_cell;
                        if let Some(ref grid) = grid_ref {
                            let world_dest = grid.grid_to_world(next_cell.0, next_cell.1);
                            pos_updates.push((*entity, world_dest));
                        }
                    }
                }
            }
        }

        // Apply updated controller states
        for (entity, controller, _) in agents {
            if let Some(c) = world.get_strand_mut::<AgentController>(entity) {
                *c = controller;
            }
        }

        // Apply velocities
        for (entity, vel) in vel_updates {
            if let Some(v) = world.get_strand_mut::<Velocity>(entity) {
                v.linear = vel;
            }
        }

        // Apply positions
        for (entity, pos) in pos_updates {
            if let Some(t) = world.get_strand_mut::<Transform>(entity) {
                t.translation = pos;
            }
        }
    }
}
