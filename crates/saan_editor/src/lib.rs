//! # `saan_editor` — The Craft Loom Interactive Diorama Editor
//!
//! Provides an integrated diorama scene editor for Saan:
//! - **Entity Hierarchy**: Live tree of entities woven into the world.
//! - **Strand Inspector**: Live inspection and mutation of `Transform`, `Sprite25D`, `Collider3D`, `Light`, `ScriptStrand`.
//! - **3D GridMap Tile Brush**: Placement and painting of terrain tiles and heights.
//! - **Simulation Controls**: Play / Pause / Single-frame Step.
//! - **Pattern Serializer**: Exports the live scene to `.saan` files.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use egui::{Color32, Pos2, ProgressBar, Rect, RichText, ScrollArea, Sense, Slider, Stroke};
use glam::{Quat, Vec2, Vec3, Vec4};
use saan_core::{
    ActionState, ActionStateStrand, BillboardMode, CameraRig, Collider3D, EntityId,
    EventWeftStrand, HealthStrand, InputStrand, Loom, Material3D, Mesh3D, MeshPrimitive,
    NameStrand, ParticleEmitter3D, PointLight, ScenePattern, ScriptStrand, Sprite25D, Transform,
    Velocity, World,
};
use saan_render::gpu::GpuRenderer;
use saan_render::RenderLoom;
use saan_spatial::{ray_intersects_aabb, GridMap3D, TileData, TileType};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Active tool in the editor viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    TileBrush,
    MoveGizmo,
}

/// Simulation execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationMode {
    Playing,
    Paused,
    StepOnce,
}

/// Editor configuration options for launching the window.
#[derive(Debug, Clone)]
pub struct EditorOptions {
    pub width: u32,
    pub height: u32,
    pub scene_path: Option<PathBuf>,
    pub max_frames: Option<u32>,
}

impl Default for EditorOptions {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            scene_path: None,
            max_frames: None,
        }
    }
}

const MAX_UNDO_SNAPSHOTS: usize = 32;

/// Core state and data model for the editor.
pub struct EditorState {
    pub scene_title: String,
    pub scene_path: Option<PathBuf>,
    pub selected_entity: Option<EntityId>,
    pub active_tool: EditorTool,
    pub brush_tile_type: TileType,
    pub brush_elevation: f32,
    pub brush_grid_pos: (i32, i32),
    pub simulation_mode: SimulationMode,
    pub grid_map: GridMap3D,
    pub status_message: String,
    pub cursor_world_pos: Vec3,
    pub active_gizmo_axis: Option<usize>,
    // ponytail: full-scene ScenePattern snapshots capped at 32; switch to per-command deltas if scenes exceed thousands of entities
    pub undo_stack: Vec<ScenePattern>,
    pub redo_stack: Vec<ScenePattern>,
}

impl EditorState {
    pub fn new(title: impl Into<String>, path: Option<PathBuf>) -> Self {
        Self {
            scene_title: title.into(),
            scene_path: path,
            selected_entity: None,
            active_tool: EditorTool::Select,
            brush_tile_type: TileType::Grass,
            brush_elevation: 0.0,
            brush_grid_pos: (0, 0),
            simulation_mode: SimulationMode::Paused,
            grid_map: GridMap3D::new(32, 32, 1.0, Vec3::new(-16.0, 0.0, -16.0)),
            status_message: "Ready. [Space] Play/Pause | [S] Save | [Tab] Cycle Tool".to_string(),
            cursor_world_pos: Vec3::ZERO,
            active_gizmo_axis: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Selects an entity by ID, or deselects if `None`.
    pub fn select(&mut self, entity: Option<EntityId>) {
        self.selected_entity = entity;
        if let Some(id) = entity {
            self.status_message = format!("Selected entity {id}");
        } else {
            self.status_message = "No entity selected".to_string();
        }
    }

    /// Steps the Loom simulation when in `Playing` or `StepOnce` mode (automatically pausing after `StepOnce`).
    pub fn step_simulation_if_needed(&mut self, world: &mut World, loom: &mut Loom, dt: f32) -> bool {
        match self.simulation_mode {
            SimulationMode::Playing => {
                let _ = loom.step(world, dt);
                true
            }
            SimulationMode::StepOnce => {
                let _ = loom.step(world, dt);
                self.simulation_mode = SimulationMode::Paused;
                self.status_message = "Stepped 1 frame (Paused)".to_string();
                true
            }
            SimulationMode::Paused => false,
        }
    }

    /// Captures the current world state onto the undo stack and clears redo history.
    pub fn record_undo_snapshot(&mut self, world: &World) {
        if self.undo_stack.len() >= MAX_UNDO_SNAPSHOTS {
            self.undo_stack.remove(0);
        }
        let snapshot = ScenePattern::extract_from_world(world, &self.scene_title, "Undo Snapshot");
        self.undo_stack.push(snapshot);
        self.redo_stack.clear();
    }

    fn restore_snapshot(&mut self, snapshot: ScenePattern, world: &mut World) {
        let prev_name = self
            .selected_entity
            .and_then(|e| world.get_strand::<NameStrand>(e))
            .map(|n| n.0.clone());

        let mut fresh = World::new();
        snapshot.weave_into_world(&mut fresh);
        let input_e = fresh.spawn();
        fresh.weave_strand(input_e, InputStrand::default());

        self.selected_entity = prev_name.and_then(|target| {
            fresh
                .query::<NameStrand>()
                .find(|(_, n)| n.0 == target)
                .map(|(e, _)| e)
        });
        *world = fresh;
    }

    /// Restores the most recent undo snapshot if available.
    pub fn undo(&mut self, world: &mut World) -> bool {
        let Some(snapshot) = self.undo_stack.pop() else {
            self.status_message = "Nothing to undo".to_string();
            return false;
        };
        let current = ScenePattern::extract_from_world(world, &self.scene_title, "Redo Snapshot");
        self.redo_stack.push(current);
        self.restore_snapshot(snapshot, world);
        self.status_message = format!("Undo ({} remaining)", self.undo_stack.len());
        true
    }

    /// Re-applies the most recent undone snapshot if available.
    pub fn redo(&mut self, world: &mut World) -> bool {
        let Some(snapshot) = self.redo_stack.pop() else {
            self.status_message = "Nothing to redo".to_string();
            return false;
        };
        let current = ScenePattern::extract_from_world(world, &self.scene_title, "Undo Snapshot");
        self.undo_stack.push(current);
        self.restore_snapshot(snapshot, world);
        self.status_message = format!("Redo ({} remaining)", self.redo_stack.len());
        true
    }

    /// Duplicates the currently selected entity with a slight world-space offset.
    pub fn duplicate_selected(&mut self, world: &mut World) -> Option<EntityId> {
        let selected = self.selected_entity?;
        let selected_name = world.get_strand::<NameStrand>(selected)?.0.clone();
        let extracted = ScenePattern::extract_from_world(world, &self.scene_title, "");
        let mut bp = extracted.entities.into_iter().find(|e| e.name == selected_name)?.clone();

        self.record_undo_snapshot(world);
        bp.name = format!("{}_copy", bp.name);
        bp.transform.translation += Vec3::new(1.0, 0.0, 1.0);

        let single = ScenePattern {
            version: 1,
            title: self.scene_title.clone(),
            description: String::new(),
            entities: vec![bp],
        };
        let spawned = single.weave_into_world(world);
        let new_id = spawned.into_iter().next()?;
        self.select(Some(new_id));
        self.status_message = format!("Duplicated entity #{selected} -> #{new_id}");
        Some(new_id)
    }

    /// Unprojects normalized device coordinates `ndc` in `[-1.0, 1.0]` (`+Y` up) into a world-space `(origin, dir)` ray.
    pub fn screen_ray(ndc: Vec2, camera: &CameraRig) -> (Vec3, Vec3) {
        let inv_vp = camera.view_projection_matrix().inverse();
        let near = inv_vp.project_point3(Vec3::new(ndc.x, ndc.y, 0.0));
        let far = inv_vp.project_point3(Vec3::new(ndc.x, ndc.y, 1.0));
        let dir = (far - near).normalize_or_zero();
        (near, dir)
    }

    /// Intersects a world-space ray `(origin, dir)` with the horizontal plane `y = plane_y`.
    pub fn ray_ground_intersection(origin: Vec3, dir: Vec3, plane_y: f32) -> Option<Vec3> {
        if dir.y.abs() < 1e-5 {
            return None;
        }
        let t = (plane_y - origin.y) / dir.y;
        (t >= 0.0).then(|| origin + dir * t)
    }

    /// Projects a 3D world position onto 2D screen coordinates inside `viewport_rect`.
    pub fn project_world_to_screen(world_pos: Vec3, camera: &CameraRig, viewport_rect: Rect) -> Option<Pos2> {
        let clip = camera.view_projection_matrix() * world_pos.extend(1.0);
        if clip.w <= 0.05 {
            return None;
        }
        let ndc = clip. truncate() / clip.w;
        let x = viewport_rect.left() + (ndc.x * 0.5 + 0.5) * viewport_rect.width();
        let y = viewport_rect.top() + (0.5 - ndc.y * 0.5) * viewport_rect.height();
        Some(Pos2::new(x, y))
    }

    /// Picks the closest selectable entity intersected by the 3D ray `(origin, dir)`.
    pub fn pick_entity_along_ray(&mut self, origin: Vec3, dir: Vec3, world: &World) -> Option<EntityId> {
        let mut best: Option<(EntityId, f32)> = None;

        for (entity, transform) in world.query::<Transform>() {
            if world.get_strand::<CameraRig>(entity).is_some()
                || world.get_strand::<EventWeftStrand>(entity).is_some()
            {
                continue;
            }
            if let Some(mesh) = world.get_strand::<Mesh3D>(entity)
                && matches!(mesh.primitive, MeshPrimitive::GroundPlane { .. })
            {
                continue;
            }

            let pos = transform.translation;
            let (bmin, bmax) = if let Some(col) = world.get_strand::<Collider3D>(entity) {
                col.world_aabb(pos)
            } else if let Some(sprite) = world.get_strand::<Sprite25D>(entity) {
                let half_w = sprite.size.x * 0.5;
                (
                    Vec3::new(pos.x - half_w, pos.y, pos.z - half_w),
                    Vec3::new(pos.x + half_w, pos.y + sprite.size.y, pos.z + half_w),
                )
            } else if let Some(mesh) = world.get_strand::<Mesh3D>(entity) {
                let half = match mesh.primitive {
                    MeshPrimitive::Cube { size } => size * 0.5,
                    _ => Vec3::splat(0.75),
                };
                (pos - half, pos + half)
            } else {
                (pos - Vec3::splat(0.5), pos + Vec3::splat(0.5))
            };

            if let Some(t) = ray_intersects_aabb(origin, dir, 500.0, bmin, bmax)
                && best.is_none_or(|(_, best_t)| t < best_t)
            {
                best = Some((entity, t));
            }
        }

        let picked = best.map(|(e, _)| e);
        self.select(picked);
        picked
    }

    /// Updates cursor ground intersection and grid coordinates from a ray.
    pub fn update_cursor_from_ray(&mut self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        let hit = Self::ray_ground_intersection(origin, dir, self.brush_elevation)?;
        self.cursor_world_pos = hit;
        self.brush_grid_pos = self.grid_map.world_to_grid(hit);
        Some(hit)
    }

    /// Cycles through active editor tools (`Select` -> `TileBrush` -> `MoveGizmo`).
    pub fn cycle_tool(&mut self) {
        self.active_tool = match self.active_tool {
            EditorTool::Select => EditorTool::TileBrush,
            EditorTool::TileBrush => EditorTool::MoveGizmo,
            EditorTool::MoveGizmo => EditorTool::Select,
        };
        self.status_message = format!("Active Tool: {:?}", self.active_tool);
    }

    /// Cycles the active tile brush type (`Grass` -> `Stone` -> `Dirt` -> `Water` -> `Ledge` -> `Ramp` -> `Blocked`).
    pub fn cycle_tile_type(&mut self) {
        self.brush_tile_type = match self.brush_tile_type {
            TileType::Grass => TileType::Stone,
            TileType::Stone => TileType::Dirt,
            TileType::Dirt => TileType::Water,
            TileType::Water => TileType::Ledge,
            TileType::Ledge => TileType::Ramp,
            TileType::Ramp => TileType::Blocked,
            TileType::Blocked => TileType::Grass,
        };
        self.status_message = format!("Brush Tile: {:?}", self.brush_tile_type);
    }

    /// Paints a tile onto `grid_map` at `(x, z)`.
    pub fn paint_tile_at(&mut self, x: i32, z: i32) {
        let elev = self.brush_elevation;
        let walkable = self.brush_tile_type != TileType::Blocked && self.brush_tile_type != TileType::Water;
        self.grid_map.set_tile(
            x,
            z,
            TileData {
                tile_type: self.brush_tile_type,
                elevation: elev,
                cost: if self.brush_tile_type == TileType::Stone { 0.8 } else { 1.0 },
                walkable,
            },
        );
        self.status_message = format!("Painted {:?} at ({x}, {z}) elev={elev:.1}m", self.brush_tile_type);
    }

    /// Translates the currently selected entity by `delta` in world space.
    pub fn translate_selected(&mut self, delta: Vec3, world: &mut World) {
        let Some(entity) = self.selected_entity else {
            return;
        };
        if let Some(transform) = world.get_strand_mut::<Transform>(entity) {
            transform.translation += delta;
            self.status_message = format!(
                "Entity pos: [{:.2}, {:.2}, {:.2}]",
                transform.translation.x, transform.translation.y, transform.translation.z
            );
        }
    }

    /// Spawns a new entity from a prefab preset into the world.
    pub fn spawn_prefab(&mut self, kind: &str, world: &mut World) -> EntityId {
        self.record_undo_snapshot(world);
        let entity = world.spawn();
        let pos = self.cursor_world_pos;

        match kind {
            "sprite_hero" => {
                world
                    .weave_strand(entity, NameStrand("spawned_hero".to_string()))
                    .weave_strand(entity, Transform::from_xyz(pos.x, pos.y, pos.z))
                    .weave_strand(entity, Velocity::default())
                    .weave_strand(
                        entity,
                        Sprite25D {
                            texture_id: "sprites/wanderer_walk_sheet.png".to_string(),
                            size: Vec2::new(1.2, 1.8),
                            pivot: Vec2::new(0.5, 0.0),
                            billboard: BillboardMode::CylindricalY,
                            normal_map_id: Some("sprites/wanderer_walk_n.png".to_string()),
                            casts_shadow: true,
                            tint: Vec4::ONE,
                            depth_bias: 0.0,
                            uv_rect: [0.0, 0.0, 0.25, 1.0],
                        },
                    );
            }
            "spirit_lantern" => {
                world
                    .weave_strand(entity, NameStrand("spawned_lantern".to_string()))
                    .weave_strand(entity, Transform::from_xyz(pos.x, pos.y + 1.2, pos.z))
                    .weave_strand(
                        entity,
                        Sprite25D {
                            texture_id: "sprites/spirit_wisp.png".to_string(),
                            size: Vec2::new(0.6, 0.6),
                            pivot: Vec2::new(0.5, 0.5),
                            billboard: BillboardMode::Spherical,
                            normal_map_id: None,
                            casts_shadow: false,
                            tint: Vec4::new(1.0, 0.8, 0.3, 0.95),
                            depth_bias: 0.01,
                            uv_rect: [0.0, 0.0, 1.0, 1.0],
                        },
                    )
                    .weave_strand(
                        entity,
                        PointLight {
                            color: Vec3::new(1.0, 0.75, 0.3),
                            intensity: 4.0,
                            radius: 5.5,
                        },
                    )
                    .weave_strand(entity, ParticleEmitter3D::spirit_wisps());
            }
            "voxel_shrine" | "shrine" => {
                world
                    .weave_strand(entity, NameStrand(format!("voxel_shrine_{entity}")))
                    .weave_strand(entity, Transform::from_xyz(pos.x, pos.y, pos.z))
                    .weave_strand(
                        entity,
                        Mesh3D {
                            primitive: MeshPrimitive::Voxel {
                                path: "assets/voxels/ancient_shrine.vox".to_string(),
                                scale: 0.1,
                            },
                            material: Material3D {
                                name: "shrine_palette".to_string(),
                                base_color: Vec4::ONE,
                                roughness: 0.8,
                                metallic: 0.1,
                            },
                            casts_shadow: true,
                            receives_shadow: true,
                        },
                    )
                    .weave_strand(
                        entity,
                        PointLight {
                            color: Vec3::new(1.0, 0.85, 0.4),
                            intensity: 4.5,
                            radius: 6.0,
                        },
                    );
            }
            _ => {
                world
                    .weave_strand(entity, NameStrand(format!("prop_cube_{entity}")))
                    .weave_strand(entity, Transform::from_xyz(pos.x, pos.y + 0.5, pos.z))
                    .weave_strand(
                        entity,
                        Mesh3D {
                            primitive: MeshPrimitive::Cube {
                                size: Vec3::new(1.0, 1.0, 1.0),
                            },
                            material: Material3D {
                                name: "editor_stone".to_string(),
                                base_color: Vec4::new(0.6, 0.5, 0.4, 1.0),
                                roughness: 0.8,
                                metallic: 0.0,
                            },
                            casts_shadow: true,
                            receives_shadow: true,
                        },
                    );
            }
        }

        self.select(Some(entity));
        self.status_message = format!("Spawned {kind} at [{:.1}, {:.1}, {:.1}]", pos.x, pos.y, pos.z);
        entity
    }

    /// Saves the current live world back into a `.saan` scene pattern file.
    pub fn save_to_file(&mut self, world: &World) -> anyhow::Result<PathBuf> {
        let path = self
            .scene_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("scene.saan"));

        let pattern = ScenePattern::extract_from_world(
            world,
            &self.scene_title,
            "Edited and authored via Saan Craft Editor",
        );

        pattern.save_to_file(&path)?;
        self.status_message = format!("Successfully saved scene to {}", path.display());
        Ok(path)
    }
}

/// The interactive editor application handler.
pub struct SaanEditorApp {
    pub options: EditorOptions,
    pub world: World,
    pub loom: Loom,
    pub render_loom: RenderLoom,
    pub editor_state: EditorState,
    pub window: Option<Arc<Window>>,
    pub gpu: Option<GpuRenderer>,
    pub egui_ctx: egui::Context,
    pub egui_winit: Option<egui_winit::State>,
    pub egui_renderer: Option<egui_wgpu::Renderer>,
    pub frame_counter: u32,
    pub last_frame: Instant,
    pub input_entity: EntityId,
}

impl SaanEditorApp {
    pub fn new(pattern: ScenePattern, options: EditorOptions) -> Self {
        let mut world = World::new();
        pattern.weave_into_world(&mut world);

        let input_entity = world.spawn();
        world.weave_strand(input_entity, InputStrand::default());

        let mut loom = Loom::with_default_gameplay();
        let _ = saan_spatial::register_spatial_systems(&mut loom);
        let _ = saan_script::register_scripting_systems(&mut loom);
        let _ = saan_audio::register_audio_systems(&mut loom);

        let editor_state = EditorState::new(pattern.title.clone(), options.scene_path.clone());
        let render_loom = RenderLoom::hd2d_pipeline();
        let egui_ctx = egui::Context::default();

        Self {
            options,
            world,
            loom,
            render_loom,
            editor_state,
            window: None,
            gpu: None,
            egui_ctx,
            egui_winit: None,
            egui_renderer: None,
            frame_counter: 0,
            last_frame: Instant::now(),
            input_entity,
        }
    }
}

impl ApplicationHandler for SaanEditorApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let title = format!("Saan Craft Editor — {}", self.editor_state.scene_title);
        let window_attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(self.options.width, self.options.height));

        let window = Arc::new(event_loop.create_window(window_attrs).expect("Failed to create window"));
        let gpu = GpuRenderer::new(Arc::clone(&window)).expect("Failed to initialize wgpu renderer");

        let viewport_id = self.egui_ctx.viewport_id();
        let egui_winit = egui_winit::State::new(
            self.egui_ctx.clone(),
            viewport_id,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let egui_renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            gpu.config.format,
            None,
            1,
            false,
        );

        self.window = Some(window);
        self.gpu = Some(gpu);
        self.egui_winit = Some(egui_winit);
        self.egui_renderer = Some(egui_renderer);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        if let (Some(win), Some(egui_winit)) = (&self.window, &mut self.egui_winit) {
            let res = egui_winit.on_window_event(win, &event);
            if res.consumed {
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(ref mut gpu) = self.gpu {
                    gpu.resize(new_size.width, new_size.height);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match key {
                KeyCode::Escape => event_loop.exit(),
                KeyCode::Space => {
                    self.editor_state.simulation_mode = match self.editor_state.simulation_mode {
                        SimulationMode::Playing => SimulationMode::Paused,
                        SimulationMode::Paused => SimulationMode::Playing,
                        SimulationMode::StepOnce => SimulationMode::Playing,
                    };
                    self.editor_state.status_message =
                        format!("Simulation: {:?}", self.editor_state.simulation_mode);
                }
                KeyCode::Tab => {
                    self.editor_state.cycle_tool();
                }
                KeyCode::KeyT => {
                    self.editor_state.cycle_tile_type();
                }
                KeyCode::KeyP => {
                    let (gx, gz) = self.editor_state.brush_grid_pos;
                    self.editor_state.paint_tile_at(gx, gz);
                }
                KeyCode::KeyS => {
                    let _ = self.editor_state.save_to_file(&self.world);
                }
                KeyCode::KeyZ => {
                    self.editor_state.undo(&mut self.world);
                }
                KeyCode::KeyY => {
                    self.editor_state.redo(&mut self.world);
                }
                KeyCode::KeyD => {
                    self.editor_state.duplicate_selected(&mut self.world);
                }
                KeyCode::Delete | KeyCode::Backspace => {
                    if let Some(selected) = self.editor_state.selected_entity {
                        self.editor_state.record_undo_snapshot(&self.world);
                        self.world.despawn(selected);
                        self.editor_state.select(None);
                    }
                }
                KeyCode::KeyI => {
                    self.editor_state.record_undo_snapshot(&self.world);
                    self.editor_state.translate_selected(Vec3::new(0.0, 0.0, -0.5), &mut self.world);
                }
                KeyCode::KeyK => {
                    self.editor_state.record_undo_snapshot(&self.world);
                    self.editor_state.translate_selected(Vec3::new(0.0, 0.0, 0.5), &mut self.world);
                }
                KeyCode::KeyJ => {
                    self.editor_state.record_undo_snapshot(&self.world);
                    self.editor_state.translate_selected(Vec3::new(-0.5, 0.0, 0.0), &mut self.world);
                }
                KeyCode::KeyL => {
                    self.editor_state.record_undo_snapshot(&self.world);
                    self.editor_state.translate_selected(Vec3::new(0.5, 0.0, 0.0), &mut self.world);
                }
                KeyCode::Digit1 => {
                    self.editor_state.spawn_prefab("sprite_hero", &mut self.world);
                }
                KeyCode::Digit2 => {
                    self.editor_state.spawn_prefab("spirit_lantern", &mut self.world);
                }
                KeyCode::Digit3 => {
                    self.editor_state.spawn_prefab("stone_cube", &mut self.world);
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().clamp(0.001, 0.1);
                self.last_frame = now;
                self.frame_counter += 1;

                self.editor_state
                    .step_simulation_if_needed(&mut self.world, &mut self.loom, dt);

                if let (Some(gpu), Some(win), Some(egui_winit), Some(egui_renderer)) =
                    (&mut self.gpu, &self.window, &mut self.egui_winit, &mut self.egui_renderer)
                {
                    let frame = self.render_loom.extract_frame(&self.world);

                    let raw_input = egui_winit.take_egui_input(win);
                    let full_output = self.egui_ctx.run(raw_input, |ctx| {
                        build_editor_ui(ctx, &mut self.editor_state, &mut self.world, &self.loom);
                    });

                    egui_winit.handle_platform_output(win, full_output.platform_output);
                    let clipped_primitives = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

                    if let Ok(output) = gpu.surface.get_current_texture() {
                        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

                        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Saan Editor Composite Encoder"),
                        });

                        gpu.encode_frame(&frame, &mut encoder, &view, self.frame_counter as f32 * dt);

                        let screen_descriptor = egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [gpu.config.width, gpu.config.height],
                            pixels_per_point: win.scale_factor() as f32,
                        };

                        for (id, delta) in &full_output.textures_delta.set {
                            egui_renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
                        }
                        egui_renderer.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &clipped_primitives, &screen_descriptor);

                        {
                            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Egui Workstation Overlay Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });
                            let mut rpass = rpass.forget_lifetime();
                            egui_renderer.render(&mut rpass, &clipped_primitives, &screen_descriptor);
                        }

                        gpu.queue.submit(std::iter::once(encoder.finish()));
                        output.present();

                        for id in &full_output.textures_delta.free {
                            egui_renderer.free_texture(id);
                        }
                    }
                }

                if let Some(max) = self.options.max_frames {
                    if self.frame_counter >= max {
                        event_loop.exit();
                        return;
                    }
                }

                if let Some(ref win) = self.window {
                    win.request_redraw();
                }
            }
            _ => {}
        }
    }
}

/// Builds the docked egui workstation interface for the Saan Craft Editor.
pub fn build_editor_ui(
    ctx: &egui::Context,
    state: &mut EditorState,
    world: &mut World,
    loom: &Loom,
) {
    // 1. Top Panel (Toolbar & Status)
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Saan Craft Loom").color(Color32::from_rgb(220, 180, 90)));
            ui.separator();

            let play_label = if state.simulation_mode == SimulationMode::Playing { "⏸ Pause" } else { "▶ Play" };
            if ui.button(play_label).clicked() {
                state.simulation_mode = match state.simulation_mode {
                    SimulationMode::Playing => SimulationMode::Paused,
                    SimulationMode::Paused | SimulationMode::StepOnce => SimulationMode::Playing,
                };
            }

            if ui.button("⏭ Step").clicked() {
                state.simulation_mode = SimulationMode::StepOnce;
            }

            ui.separator();
            ui.selectable_value(&mut state.active_tool, EditorTool::Select, "🔍 Select");
            ui.selectable_value(&mut state.active_tool, EditorTool::TileBrush, "🖌 Tile Brush");
            ui.selectable_value(&mut state.active_tool, EditorTool::MoveGizmo, "📐 Move Gizmo");

            ui.separator();
            if ui.add_enabled(!state.undo_stack.is_empty(), egui::Button::new("↩ Undo")).clicked() {
                state.undo(world);
            }
            if ui.add_enabled(!state.redo_stack.is_empty(), egui::Button::new("↪ Redo")).clicked() {
                state.redo(world);
            }
            if ui.add_enabled(state.selected_entity.is_some(), egui::Button::new("📋 Dup")).clicked() {
                state.duplicate_selected(world);
            }
            if ui.button("💾 Save .saan").clicked() {
                let _ = state.save_to_file(world);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tension = loom.inspect_tension();
                if tension.is_taut() {
                    ui.label(RichText::new(format!("● Taut ({} systems)", tension.system_count)).color(Color32::from_rgb(80, 200, 120)));
                } else {
                    ui.label(RichText::new(format!("⚠ Hazards ({})", tension.hazards.len())).color(Color32::from_rgb(240, 80, 80)));
                }
            });
        });
    });

    // 2. Left Panel (Outliner & Palettes)
    egui::SidePanel::left("left_panel").default_width(220.0).show(ctx, |ui| {
        ui.heading("Entity Outliner");
        let entities: Vec<(EntityId, String)> = world
            .query::<NameStrand>()
            .map(|(e, name)| (e, name.0.clone()))
            .collect();

        ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
            for (entity, name) in entities {
                let is_selected = state.selected_entity == Some(entity);
                if ui.selectable_label(is_selected, format!("#{}: {}", entity, name)).clicked() {
                    state.select(Some(entity));
                }
            }
        });

        ui.separator();
        ui.heading("Prefabs");
        ui.horizontal_wrapped(|ui| {
            if ui.button("+ Hero").clicked() {
                state.spawn_prefab("sprite_hero", world);
            }
            if ui.button("+ Shrine").clicked() {
                state.spawn_prefab("voxel_shrine", world);
            }
            if ui.button("+ Lantern").clicked() {
                state.spawn_prefab("spirit_lantern", world);
            }
            if ui.button("+ Cube").clicked() {
                state.spawn_prefab("stone_cube", world);
            }
        });

        ui.separator();
        ui.heading("3D Tile Palette");
        ui.label(format!("Tile: {:?} @ ({}, {})", state.brush_tile_type, state.brush_grid_pos.0, state.brush_grid_pos.1));
        ui.horizontal_wrapped(|ui| {
            for tile in [
                TileType::Grass,
                TileType::Stone,
                TileType::Dirt,
                TileType::Water,
                TileType::Ledge,
                TileType::Ramp,
                TileType::Blocked,
            ] {
                if ui.selectable_label(state.brush_tile_type == tile, format!("{:?}", tile)).clicked() {
                    state.brush_tile_type = tile;
                }
            }
        });
        ui.add(Slider::new(&mut state.brush_elevation, 0.0..=8.0).text("Elevation"));
        if ui.button("Paint Tile").clicked() {
            let (gx, gz) = state.brush_grid_pos;
            state.paint_tile_at(gx, gz);
        }
    });

    // 3. Right Panel (Strand Component Inspector)
    egui::SidePanel::right("right_panel").default_width(260.0).show(ctx, |ui| {
        ui.heading("Strand Inspector");
        if let Some(selected) = state.selected_entity {
            ui.label(format!("Entity: #{}", selected));

            // NameStrand
            if let Some(name) = world.get_strand_mut::<NameStrand>(selected) {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut name.0);
                });
            }

            // Transform
            if let Some(trans) = world.get_strand_mut::<Transform>(selected) {
                ui.collapsing("Transform", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("X");
                        ui.add(egui::DragValue::new(&mut trans.translation.x).speed(0.1));
                        ui.label("Y");
                        ui.add(egui::DragValue::new(&mut trans.translation.y).speed(0.1));
                        ui.label("Z");
                        ui.add(egui::DragValue::new(&mut trans.translation.z).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scale");
                        ui.add(egui::DragValue::new(&mut trans.scale.x).speed(0.05));
                        ui.add(egui::DragValue::new(&mut trans.scale.y).speed(0.05));
                        ui.add(egui::DragValue::new(&mut trans.scale.z).speed(0.05));
                    });
                    let (mut yaw, pitch, roll) = trans.rotation.to_euler(glam::EulerRot::YXZ);
                    let mut yaw_deg = yaw.to_degrees();
                    if ui.add(Slider::new(&mut yaw_deg, -180.0..=180.0).text("Yaw °")).changed() {
                        yaw = yaw_deg.to_radians();
                        trans.rotation = Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll);
                    }
                });
            }

            // Collider3D
            if let Some(collider) = world.get_strand_mut::<Collider3D>(selected) {
                ui.collapsing("Collider3D", |ui| {
                    ui.label(format!("Shape: {:?}", collider.shape));
                    ui.checkbox(&mut collider.is_static, "Static Obstacle");
                    ui.checkbox(&mut collider.fade_when_occluding_camera, "Occlusion Fade");
                    ui.add(Slider::new(&mut collider.max_step_height, 0.0..=1.5).text("Max Step (m)"));
                });
            }

            // PointLight
            if let Some(light) = world.get_strand_mut::<PointLight>(selected) {
                ui.collapsing("PointLight", |ui| {
                    ui.add(Slider::new(&mut light.intensity, 0.0..=15.0).text("Intensity"));
                    ui.add(Slider::new(&mut light.radius, 0.5..=25.0).text("Radius"));
                    let mut rgb = [light.color.x, light.color.y, light.color.z];
                    if ui.color_edit_button_rgb(&mut rgb).changed() {
                        light.color = Vec3::from_array(rgb);
                    }
                });
            }

            // ScriptStrand
            if let Some(script) = world.get_strand_mut::<ScriptStrand>(selected) {
                ui.collapsing("ScriptStrand", |ui| {
                    ui.checkbox(&mut script.enabled, "Enabled");
                    ui.text_edit_singleline(&mut script.script_path);
                });
            }

            // HealthStrand
            if let Some(health) = world.get_strand_mut::<HealthStrand>(selected) {
                ui.collapsing("Health", |ui| {
                    let frac = (health.current / health.max.max(1.0)).clamp(0.0, 1.0);
                    ui.add(ProgressBar::new(frac).text(format!("{:.0} / {:.0}", health.current, health.max)));
                    ui.horizontal(|ui| {
                        if ui.button("-20 Dmg").clicked() {
                            health.take_damage(20.0);
                        }
                        if ui.button("+20 Heal").clicked() {
                            health.heal(20.0);
                        }
                    });
                });
            }

            // ActionStateStrand
            if let Some(action) = world.get_strand_mut::<ActionStateStrand>(selected) {
                ui.collapsing("Action State", |ui| {
                    ui.label(format!("State: {:?}", action.state));
                    ui.label(format!("Facing: [{:.2}, {:.2}]", action.facing_direction.x, action.facing_direction.y));
                    if ui.button("Trigger Dash").clicked() {
                        action.state = ActionState::Dash {
                            timer: action.dash_duration,
                            max_time: action.dash_duration,
                            direction: action.facing_direction,
                            speed: action.dash_speed,
                        };
                    }
                    if ui.button("Trigger Attack").clicked() {
                        action.state = ActionState::Attack {
                            combo_index: 1,
                            timer: 0.28,
                            max_time: 0.28,
                            has_spawned_hitbox: false,
                        };
                    }
                });
            }

            // Sprite25D
            if let Some(sprite) = world.get_strand_mut::<Sprite25D>(selected) {
                ui.collapsing("Sprite25D", |ui| {
                    ui.label(format!("Texture: {}", sprite.texture_id));
                    ui.checkbox(&mut sprite.casts_shadow, "Casts Shadow");
                    ui.label(format!("Billboard: {:?}", sprite.billboard));
                });
            }

            // Mesh3D
            if let Some(mesh) = world.get_strand_mut::<Mesh3D>(selected) {
                ui.collapsing("Mesh3D", |ui| {
                    ui.label(format!("Primitive: {:?}", mesh.primitive));
                    ui.checkbox(&mut mesh.casts_shadow, "Casts Shadow");
                    ui.checkbox(&mut mesh.receives_shadow, "Receives Shadow");
                });
            }

            // Duplicate / Despawn buttons
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("📋 Duplicate").clicked() {
                    state.duplicate_selected(world);
                }
                if ui.button(RichText::new("🗑 Delete").color(Color32::from_rgb(240, 80, 80))).clicked() {
                    state.record_undo_snapshot(world);
                    world.despawn(selected);
                    state.select(None);
                }
            });
        } else {
            ui.label("No entity selected.\nSelect an entity from the Outliner or click in viewport.");
        }
    });

    // 4. Bottom Panel (Event Console & Status)
    egui::TopBottomPanel::bottom("bottom_panel").default_height(80.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Event Weft & Status");
            ui.separator();
            ui.label(&state.status_message);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.monospace(format!(
                    "Cursor: [{:.1}, {:.1}, {:.1}] | Grid: ({}, {})",
                    state.cursor_world_pos.x,
                    state.cursor_world_pos.y,
                    state.cursor_world_pos.z,
                    state.brush_grid_pos.0,
                    state.brush_grid_pos.1
                ));
            });
        });

        if let Some((_, bus)) = world.query::<EventWeftStrand>().next() {
            ScrollArea::vertical().max_height(50.0).show(ui, |ui| {
                for ev in bus.current_events.iter().rev().take(6) {
                    ui.monospace(format!("~ {:?}", ev));
                }
            });
        }
    });

    // 5. Central Panel (Interactive 3D Viewport: Ray Picking, Tile Brush, 3-Axis Move Gizmo, Camera Orbit/Pan)
    egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
        let viewport_rect = ui.max_rect();
        let response = ui.allocate_rect(viewport_rect, Sense::click_and_drag());
        let painter = ui.painter_at(viewport_rect);

        let camera = world
            .query::<CameraRig>()
            .next()
            .map(|(_, c)| *c)
            .unwrap_or_default();

        // Update 3D ground cursor from pointer hover position
        if let Some(hover_pos) = response.hover_pos().or_else(|| ctx.pointer_interact_pos())
            && viewport_rect.width() > 1.0
            && viewport_rect.height() > 1.0
        {
            let ndc = Vec2::new(
                ((hover_pos.x - viewport_rect.left()) / viewport_rect.width()) * 2.0 - 1.0,
                1.0 - ((hover_pos.y - viewport_rect.top()) / viewport_rect.height()) * 2.0,
            );
            let (origin, dir) = EditorState::screen_ray(ndc, &camera);
            state.update_cursor_from_ray(origin, dir);

            if response.clicked() && state.active_tool == EditorTool::Select {
                state.pick_entity_along_ray(origin, dir, world);
            }
        }

        // Right-drag camera orbit, Middle-drag camera pan, Scroll zoom
        if response.dragged_by(egui::PointerButton::Secondary) {
            let delta = response.drag_delta();
            if let Some((_, cam)) = world.query_mut::<CameraRig>().next() {
                let offset = cam.eye - cam.target;
                let yaw = Quat::from_rotation_y(-delta.x * 0.008);
                cam.eye = cam.target + yaw * offset;
                cam.eye.y = (cam.eye.y + delta.y * 0.04).clamp(1.5, 45.0);
            }
        } else if response.dragged_by(egui::PointerButton::Middle) {
            let delta = response.drag_delta();
            if let Some((_, cam)) = world.query_mut::<CameraRig>().next() {
                let forward = (cam.target - cam.eye). with_y(0.0).normalize_or_zero();
                let right = forward.cross(Vec3::Y).normalize_or_zero();
                let pan = (-right * delta.x + forward * delta.y) * 0.025;
                cam.eye += pan;
                cam.target += pan;
            }
        }
        if response.hovered() {
            let scroll_y = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > 0.1
                && let Some((_, cam)) = world.query_mut::<CameraRig>().next()
            {
                let to_eye = cam.eye - cam.target;
                let dist = (to_eye.length() - scroll_y * 0.02).clamp(3.0, 60.0);
                cam.eye = cam.target + to_eye.normalize_or_zero() * dist;
            }
        }

        // Tile Brush visual cell footprint + click/drag painting
        if state.active_tool == EditorTool::TileBrush {
            let (gx, gz) = state.brush_grid_pos;
            let center = state.grid_map.grid_to_world(gx, gz).with_y(state.brush_elevation);
            let half = state.grid_map.cell_size * 0.5;
            let corners = [
                Vec3::new(center.x - half, center.y, center.z - half),
                Vec3::new(center.x + half, center.y, center.z - half),
                Vec3::new(center.x + half, center.y, center.z + half),
                Vec3::new(center.x - half, center.y, center.z + half),
            ];
            let pts: Vec<Pos2> = corners
                .iter()
                .filter_map(|&c| EditorState::project_world_to_screen(c, &camera, viewport_rect))
                .collect();
            if pts.len() == 4 {
                for i in 0..4 {
                    painter.line_segment([pts[i], pts[(i + 1) % 4]], Stroke::new(2.0, Color32::from_rgb(90, 230, 140)));
                }
            }
            if response.clicked() || response.dragged_by(egui::PointerButton::Primary) {
                state.paint_tile_at(gx, gz);
            }
        }

        // Selected entity highlight & 3-Axis Move Gizmo (X Red, Y Green, Z Blue)
        if let Some(selected) = state.selected_entity
            && let Some(entity_pos) = world.get_strand::<Transform>(selected).map(|t| t.translation)
            && let Some(origin_2d) = EditorState::project_world_to_screen(entity_pos, &camera, viewport_rect)
        {
            painter.circle_stroke(origin_2d, 8.0, Stroke::new(2.0, Color32::from_rgb(250, 210, 80)));

            if state.active_tool == EditorTool::MoveGizmo {
                let axes = [
                    (0usize, Vec3::X, Color32::from_rgb(235, 75, 75), "X"),
                    (1usize, Vec3::Y, Color32::from_rgb(80, 220, 100), "Y"),
                    (2usize, Vec3::Z, Color32::from_rgb(80, 145, 245), "Z"),
                ];

                let mut axis_screen_dirs = [Vec2::ZERO; 3];
                for (idx, axis_vec, color, label) in axes {
                    if let Some(tip_2d) = EditorState::project_world_to_screen(entity_pos + axis_vec * 1.4, &camera, viewport_rect) {
                        let width = if state.active_gizmo_axis == Some(idx) { 4.0 } else { 2.5 };
                        painter.line_segment([origin_2d, tip_2d], Stroke::new(width, color));
                        painter.circle_filled(tip_2d, 5.0, color);
                        painter.text(tip_2d, egui::Align2::LEFT_BOTTOM, label, egui::FontId::monospace(12.0), color);

                        let d = Vec2::new(tip_2d.x - origin_2d.x, tip_2d.y - origin_2d.y);
                        axis_screen_dirs[idx] = d.normalize_or_zero();

                        if response.drag_started_by(egui::PointerButton::Primary)
                            && let Some(press_pos) = ctx.pointer_interact_pos()
                            && press_pos.distance(tip_2d) < 24.0
                        {
                            state.record_undo_snapshot(world);
                            state.active_gizmo_axis = Some(idx);
                        }
                    }
                }

                if response.drag_started_by(egui::PointerButton::Primary) && state.active_gizmo_axis.is_none() {
                    state.record_undo_snapshot(world);
                }

                if response.dragged_by(egui::PointerButton::Primary) {
                    let drag = response.drag_delta();
                    let drag_vec = Vec2::new(drag.x, drag.y);
                    let world_delta = match state.active_gizmo_axis {
                        Some(0) => Vec3::X * (drag_vec.dot(axis_screen_dirs[0]) * 0.02),
                        Some(1) => Vec3::Y * (-drag.y * 0.02),
                        Some(2) => Vec3::Z * (drag_vec.dot(axis_screen_dirs[2]) * 0.02),
                        _ => Vec3::new(drag.x * 0.02, 0.0, drag.y * 0.02),
                    };
                    state.translate_selected(world_delta, world);
                }

                if response.drag_stopped() {
                    state.active_gizmo_axis = None;
                }
            }
        }
    });
}

/// Launches the interactive Saan Craft Editor window.
pub fn run_editor(pattern: ScenePattern, options: EditorOptions) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SaanEditorApp::new(pattern, options);
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_state_manages_tools_and_selection() {
        let mut state = EditorState::new("Test Scene", None);
        assert_eq!(state.active_tool, EditorTool::Select);
        assert_eq!(state.simulation_mode, SimulationMode::Paused);

        state.cycle_tool();
        assert_eq!(state.active_tool, EditorTool::TileBrush);

        state.cycle_tool();
        assert_eq!(state.active_tool, EditorTool::MoveGizmo);

        state.cycle_tile_type();
        assert_eq!(state.brush_tile_type, TileType::Stone);
    }

    #[test]
    fn editor_paints_tiles_and_spawns_prefabs() {
        let mut state = EditorState::new("Test Scene", None);
        state.paint_tile_at(3, 4);
        assert!(state.grid_map.get_tile(3, 4).is_some());

        let mut world = World::new();
        let entity = state.spawn_prefab("sprite_hero", &mut world);
        assert_eq!(state.selected_entity, Some(entity));
        assert!(world.get_strand::<Sprite25D>(entity).is_some());

        let shrine_entity = state.spawn_prefab("voxel_shrine", &mut world);
        assert_eq!(state.selected_entity, Some(shrine_entity));
        let mesh = world.get_strand::<Mesh3D>(shrine_entity).unwrap();
        assert!(matches!(mesh.primitive, MeshPrimitive::Voxel { .. }));

        state.select(Some(entity));
        state.translate_selected(Vec3::new(1.0, 0.0, 2.0), &mut world);
        let trans = world.get_strand::<Transform>(entity).unwrap();
        assert_eq!(trans.translation, Vec3::new(1.0, 0.0, 2.0));
    }

    #[test]
    fn editor_supports_step_once_undo_redo_duplicate_and_ray_picking() {
        let mut state = EditorState::new("Editor Features Test", None);
        let mut world = World::new();
        let mut loom = Loom::with_default_gameplay();

        let hero = state.spawn_prefab("sprite_hero", &mut world);
        world.get_strand_mut::<Velocity>(hero).unwrap().linear = Vec3::new(2.0, 0.0, 0.0);

        // StepOnce steps 1 frame then returns to Paused
        state.simulation_mode = SimulationMode::StepOnce;
        assert!(state.step_simulation_if_needed(&mut world, &mut loom, 0.5));
        assert_eq!(state.simulation_mode, SimulationMode::Paused);
        assert!((world.get_strand::<Transform>(hero).unwrap().translation.x - 1.0).abs() < 1e-4);

        // Ray picking selects the hero at (1.0, 0.5, 0.0)
        state.select(None);
        let picked = state.pick_entity_along_ray(Vec3::new(1.0, 0.5, 5.0), Vec3::NEG_Z, &world);
        assert_eq!(picked, Some(hero));

        // Duplicate selected creates a copy and Undo/Redo restores entity counts
        let count_before = world.query::<Transform>().count();
        let dup = state.duplicate_selected(&mut world);
        assert!(dup.is_some());
        assert_eq!(world.query::<Transform>().count(), count_before + 1);

        assert!(state.undo(&mut world));
        assert_eq!(world.query::<Transform>().count(), count_before);

        assert!(state.redo(&mut world));
        assert_eq!(world.query::<Transform>().count(), count_before + 1);
    }

    #[test]
    fn egui_workstation_ui_builds_and_tessellates_cleanly() {
        let pattern = ScenePattern::default_hd2d_diorama("Egui Test Scene");
        let mut world = World::new();
        pattern.weave_into_world(&mut world);

        let mut loom = Loom::with_default_gameplay();
        let _ = saan_spatial::register_spatial_systems(&mut loom);

        let mut state = EditorState::new("Egui Test Scene", None);
        // Select first entity to populate the inspector
        let first_e = world.query::<NameStrand>().next().map(|(e, _)| e);
        state.select(first_e);

        let ctx = egui::Context::default();
        let raw_input = egui::RawInput::default();
        let full_output = ctx.run(raw_input, |ctx| {
            build_editor_ui(ctx, &mut state, &mut world, &loom);
        });

        assert!(!full_output.shapes.is_empty(), "Egui UI should emit panel shapes");
        let primitives = ctx.tessellate(full_output.shapes, 1.0);
        assert!(!primitives.is_empty(), "Egui should tessellate into clipped primitives");
    }
}
