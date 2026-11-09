//! 3D Multi-Height GridMap and Pathfinding for TBS and RTS navigation.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// Type of surface terrain at a grid cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileType {
    Grass,
    Stone,
    Dirt,
    Water,
    Ledge,
    Ramp,
    Blocked,
}

impl Default for TileType {
    fn default() -> Self {
        Self::Grass
    }
}

/// Data for an individual cell in a 3D multi-height tilemap.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TileData {
    pub tile_type: TileType,
    /// Elevation in meters/units above baseline (e.g. 0.0, 1.0, 2.5).
    pub elevation: f32,
    /// Movement traversal cost multiplier (1.0 = normal, 2.0 = difficult terrain).
    pub cost: f32,
    /// Walkable flag (false for deep water, solid walls, etc.).
    pub walkable: bool,
}

impl Default for TileData {
    fn default() -> Self {
        Self {
            tile_type: TileType::Grass,
            elevation: 0.0,
            cost: 1.0,
            walkable: true,
        }
    }
}

/// 3D multi-height gridmap strand attached to scene terrain or tactical arenas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridMap3D {
    pub width: u32,
    pub depth: u32,
    pub cell_size: f32,
    pub origin: Vec3,
    pub tiles: Vec<TileData>,
}

impl GridMap3D {
    pub fn new(width: u32, depth: u32, cell_size: f32, origin: Vec3) -> Self {
        let total = (width * depth) as usize;
        Self {
            width,
            depth,
            cell_size,
            origin,
            tiles: vec![TileData::default(); total],
        }
    }

    #[inline]
    pub fn in_bounds(&self, gx: i32, gz: i32) -> bool {
        gx >= 0 && gx < self.width as i32 && gz >= 0 && gz < self.depth as i32
    }

    #[inline]
    fn idx(&self, gx: i32, gz: i32) -> usize {
        (gz as u32 * self.width + gx as u32) as usize
    }

    pub fn get_tile(&self, gx: i32, gz: i32) -> Option<&TileData> {
        if self.in_bounds(gx, gz) {
            Some(&self.tiles[self.idx(gx, gz)])
        } else {
            None
        }
    }

    pub fn set_tile(&mut self, gx: i32, gz: i32, tile: TileData) -> bool {
        if self.in_bounds(gx, gz) {
            let i = self.idx(gx, gz);
            self.tiles[i] = tile;
            true
        } else {
            false
        }
    }

    /// Converts integer grid coordinates `(gx, gz)` to 3D world space at the top surface of the tile.
    pub fn grid_to_world(&self, gx: i32, gz: i32) -> Vec3 {
        let elevation = self.get_tile(gx, gz).map(|t| t.elevation).unwrap_or(0.0);
        Vec3::new(
            self.origin.x + (gx as f32 + 0.5) * self.cell_size,
            self.origin.y + elevation,
            self.origin.z + (gz as f32 + 0.5) * self.cell_size,
        )
    }

    /// Converts 3D world coordinates to the nearest integer grid coordinates `(gx, gz)`.
    pub fn world_to_grid(&self, world_pos: Vec3) -> (i32, i32) {
        let rel_x = (world_pos.x - self.origin.x) / self.cell_size;
        let rel_z = (world_pos.z - self.origin.z) / self.cell_size;
        (rel_x.floor() as i32, rel_z.floor() as i32)
    }

    /// Discrete A* pathfinding across the 3D multi-height grid (for TBS and tactical units).
    /// Respects `max_jump_height` between adjacent cells and impassable tiles.
    pub fn find_path_a_star(
        &self,
        start: (i32, i32),
        goal: (i32, i32),
        max_jump_height: f32,
    ) -> Option<Vec<(i32, i32)>> {
        if !self.in_bounds(start.0, start.1) || !self.in_bounds(goal.0, goal.1) {
            return None;
        }
        if !self.get_tile(goal.0, goal.1)?.walkable {
            return None;
        }

        #[derive(Copy, Clone, PartialEq)]
        struct Node {
            cost: f32,
            pos: (i32, i32),
        }
        impl Eq for Node {}
        impl Ord for Node {
            fn cmp(&self, other: &Self) -> Ordering {
                other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
            }
        }
        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut g_score: HashMap<(i32, i32), f32> = HashMap::new();

        g_score.insert(start, 0.0);
        open_set.push(Node { cost: 0.0, pos: start });

        let neighbors = [(0, 1), (1, 0), (0, -1), (-1, 0)];

        while let Some(Node { pos, .. }) = open_set.pop() {
            if pos == goal {
                let mut path = vec![goal];
                let mut curr = goal;
                while let Some(&prev) = came_from.get(&curr) {
                    path.push(prev);
                    curr = prev;
                }
                path.reverse();
                return Some(path);
            }

            let current_elevation = self.get_tile(pos.0, pos.1)?.elevation;
            let current_g = *g_score.get(&pos).unwrap_or(&f32::INFINITY);

            for (dx, dz) in neighbors {
                let next = (pos.0 + dx, pos.1 + dz);
                let Some(next_tile) = self.get_tile(next.0, next.1) else {
                    continue;
                };

                if !next_tile.walkable {
                    continue;
                }

                let height_diff = (next_tile.elevation - current_elevation).abs();
                if height_diff > max_jump_height {
                    continue;
                }

                let tentative_g = current_g + self.cell_size * next_tile.cost;
                if tentative_g < *g_score.get(&next).unwrap_or(&f32::INFINITY) {
                    g_score.insert(next, tentative_g);
                    came_from.insert(next, pos);
                    let h = ((next.0 - goal.0).abs() + (next.1 - goal.1).abs()) as f32 * self.cell_size;
                    open_set.push(Node {
                        cost: tentative_g + h,
                        pos: next,
                    });
                }
            }
        }

        None
    }

    /// Flood-fills all tiles reachable within `max_movement_points` (for TBS movement radius visualization).
    pub fn reachable_tiles(
        &self,
        start: (i32, i32),
        max_movement_points: f32,
        max_jump_height: f32,
    ) -> HashSet<(i32, i32)> {
        let mut reachable = HashSet::new();
        let mut distances: HashMap<(i32, i32), f32> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();

        distances.insert(start, 0.0);
        queue.push_back(start);
        reachable.insert(start);

        let neighbors = [(0, 1), (1, 0), (0, -1), (-1, 0)];

        while let Some(curr) = queue.pop_front() {
            let current_dist = distances[&curr];
            let current_elevation = self.get_tile(curr.0, curr.1).map(|t| t.elevation).unwrap_or(0.0);

            for (dx, dz) in neighbors {
                let next = (curr.0 + dx, curr.1 + dz);
                let Some(tile) = self.get_tile(next.0, next.1) else {
                    continue;
                };
                if !tile.walkable || (tile.elevation - current_elevation).abs() > max_jump_height {
                    continue;
                }

                let new_dist = current_dist + self.cell_size * tile.cost;
                if new_dist <= max_movement_points {
                    if let Some(&prev_dist) = distances.get(&next) {
                        if new_dist < prev_dist {
                            distances.insert(next, new_dist);
                            queue.push_back(next);
                        }
                    } else {
                        distances.insert(next, new_dist);
                        reachable.insert(next);
                        queue.push_back(next);
                    }
                }
            }
        }

        reachable
    }
}

/// 2.5D Flow-Field vector field for guiding multiple RTS swarm units simultaneously towards a target.
#[derive(Debug, Clone)]
pub struct FlowField25D {
    pub width: u32,
    pub depth: u32,
    pub cell_size: f32,
    pub origin: Vec3,
    pub directions: Vec<Vec2>,
}

impl FlowField25D {
    /// Generates a continuous FlowField heading towards `target_grid_pos`.
    pub fn build(grid: &GridMap3D, target: (i32, i32)) -> Self {
        let mut integration: Vec<f32> = vec![f32::INFINITY; (grid.width * grid.depth) as usize];
        let mut directions: Vec<Vec2> = vec![Vec2::ZERO; (grid.width * grid.depth) as usize];

        if !grid.in_bounds(target.0, target.1) {
            return Self {
                width: grid.width,
                depth: grid.depth,
                cell_size: grid.cell_size,
                origin: grid.origin,
                directions,
            };
        }

        // Dijkstra wave from target
        let target_idx = grid.idx(target.0, target.1);
        integration[target_idx] = 0.0;
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(target);

        let neighbors = [(0, 1), (1, 0), (0, -1), (-1, 0), (1, 1), (-1, 1), (1, -1), (-1, -1)];

        while let Some((cx, cz)) = queue.pop_front() {
            let curr_cost = integration[grid.idx(cx, cz)];
            let curr_elev = grid.get_tile(cx, cz).map(|t| t.elevation).unwrap_or(0.0);

            for (dx, dz) in neighbors {
                let (nx, nz) = (cx + dx, cz + dz);
                if !grid.in_bounds(nx, nz) {
                    continue;
                }
                let next_idx = grid.idx(nx, nz);
                let Some(tile) = grid.get_tile(nx, nz) else {
                    continue;
                };
                if !tile.walkable || (tile.elevation - curr_elev).abs() > 0.6 {
                    continue;
                }

                let step_dist = if dx != 0 && dz != 0 { 1.414 } else { 1.0 } * grid.cell_size * tile.cost;
                let new_cost = curr_cost + step_dist;
                if new_cost < integration[next_idx] {
                    integration[next_idx] = new_cost;
                    queue.push_back((nx, nz));
                }
            }
        }

        // Derive vector gradients from integration field
        for gz in 0..grid.depth as i32 {
            for gx in 0..grid.width as i32 {
                let idx = grid.idx(gx, gz);
                if integration[idx].is_infinite() || (gx == target.0 && gz == target.1) {
                    continue;
                }

                let mut best_dir = Vec2::ZERO;
                let mut min_val = integration[idx];

                for (dx, dz) in neighbors {
                    let (nx, nz) = (gx + dx, gz + dz);
                    if grid.in_bounds(nx, nz) {
                        let val = integration[grid.idx(nx, nz)];
                        if val < min_val {
                            min_val = val;
                            best_dir = Vec2::new(dx as f32, dz as f32);
                        }
                    }
                }

                directions[idx] = best_dir.normalize_or_zero();
            }
        }

        Self {
            width: grid.width,
            depth: grid.depth,
            cell_size: grid.cell_size,
            origin: grid.origin,
            directions,
        }
    }

    /// Samples the normalized 2D steering vector `(vx, vz)` for an entity at `world_pos`.
    pub fn sample_steering(&self, world_pos: Vec3) -> Vec2 {
        let rel_x = (world_pos.x - self.origin.x) / self.cell_size;
        let rel_z = (world_pos.z - self.origin.z) / self.cell_size;
        let gx = rel_x.floor() as i32;
        let gz = rel_z.floor() as i32;

        if gx >= 0 && gx < self.width as i32 && gz >= 0 && gz < self.depth as i32 {
            let idx = (gz as u32 * self.width + gx as u32) as usize;
            self.directions[idx]
        } else {
            Vec2::ZERO
        }
    }
}
