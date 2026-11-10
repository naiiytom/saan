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

use egui::{Color32, ProgressBar, RichText, ScrollArea, Slider};
use glam::{Vec2, Vec3, Vec4};
use saan_core::{
    ActionState, ActionStateStrand, BillboardMode, EntityId, EventWeftStrand, HealthStrand,
    InputStrand, Loom, Material3D, Mesh3D, MeshPrimitive, NameStrand,
    ParticleEmitter3D, PointLight, ScenePattern, Sprite25D, Transform, Velocity, World,
};
use saan_render::gpu::GpuRenderer;
use saan_render::RenderLoom;
use saan_spatial::{GridMap3D, TileData, TileType};
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
                KeyCode::KeyI => {
                    self.editor_state.translate_selected(Vec3::new(0.0, 0.0, -0.5), &mut self.world);
                }
                KeyCode::KeyK => {
                    self.editor_state.translate_selected(Vec3::new(0.0, 0.0, 0.5), &mut self.world);
                }
                KeyCode::KeyJ => {
                    self.editor_state.translate_selected(Vec3::new(-0.5, 0.0, 0.0), &mut self.world);
                }
                KeyCode::KeyL => {
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

                if self.editor_state.simulation_mode == SimulationMode::Playing {
                    let _ = self.loom.step(&mut self.world, dt);
                }

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
        ui.label(format!("Tile: {:?}", state.brush_tile_type));
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

            // Despawn button
            ui.separator();
            if ui.button(RichText::new("🗑 Delete Entity").color(Color32::from_rgb(240, 80, 80))).clicked() {
                world.despawn(selected);
                state.select(None);
            }
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
        });

        if let Some((_, bus)) = world.query::<EventWeftStrand>().next() {
            ScrollArea::vertical().max_height(50.0).show(ui, |ui| {
                for ev in bus.current_events.iter().rev().take(6) {
                    ui.monospace(format!("~ {:?}", ev));
                }
            });
        }
    });

    // 5. Central Panel (Transparent Diorama Viewport Overlay)
    egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |_ui| {});
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
