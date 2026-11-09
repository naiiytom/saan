//! Interactive `winit` + `wgpu` viewport runner for `saan run`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use glam::Vec3;
use saan_core::{CameraRig, EntityId, InputStrand, Loom, SceneHotReloader, ScenePattern, World};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::gpu::GpuRenderer;
use crate::RenderLoom;

/// Options for launching the interactive Saan window.
#[derive(Debug, Clone)]
pub struct WindowOptions {
    pub width: u32,
    pub height: u32,
    pub max_frames: Option<u32>,
    pub watch_path: Option<PathBuf>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            max_frames: None,
            watch_path: None,
        }
    }
}

struct SaanApp {
    title: String,
    options: WindowOptions,
    world: World,
    input_entity: EntityId,
    hot_reloader: Option<SceneHotReloader>,
    loom: Loom,
    render_loom: RenderLoom,
    window: Option<Arc<Window>>,
    gpu: Option<GpuRenderer>,
    pressed_keys: HashSet<KeyCode>,
    start_instant: Instant,
    last_frame: Instant,
    frame_counter: u32,
    paused: bool,
}

impl SaanApp {
    fn new(pattern: &ScenePattern, options: WindowOptions) -> Self {
        let mut world = World::new();
        let spawned = pattern.weave_into_world(&mut world);

        let hot_reloader = options
            .watch_path
            .as_ref()
            .map(|path| SceneHotReloader::new(path.clone(), spawned));

        let input_entity = world.spawn();
        world.weave_strand(input_entity, InputStrand::default());

        let loom = Loom::with_default_gameplay();
        let render_loom = RenderLoom::hd2d_pipeline();
        let now = Instant::now();

        Self {
            title: format!("Saan (สาน) — {} [WASD: Pan | Q/E: Orbit | R/F: Zoom | Space: Pause | Esc: Quit]", pattern.title),
            options,
            world,
            input_entity,
            hot_reloader,
            loom,
            render_loom,
            window: None,
            gpu: None,
            pressed_keys: HashSet::new(),
            start_instant: now,
            last_frame: now,
            frame_counter: 0,
            paused: false,
        }
    }

    fn sync_input_strand(&mut self) {
        let mut move_axis = Vec3::ZERO;
        let mut orbit_yaw = 0.0;
        let mut zoom_delta = 0.0;

        if self.pressed_keys.contains(&KeyCode::KeyA) || self.pressed_keys.contains(&KeyCode::ArrowLeft) {
            move_axis.x -= 1.0;
        }
        if self.pressed_keys.contains(&KeyCode::KeyD) || self.pressed_keys.contains(&KeyCode::ArrowRight) {
            move_axis.x += 1.0;
        }
        if self.pressed_keys.contains(&KeyCode::KeyW) || self.pressed_keys.contains(&KeyCode::ArrowUp) {
            move_axis.z += 1.0;
        }
        if self.pressed_keys.contains(&KeyCode::KeyS) || self.pressed_keys.contains(&KeyCode::ArrowDown) {
            move_axis.z -= 1.0;
        }
        if self.pressed_keys.contains(&KeyCode::KeyQ) {
            orbit_yaw -= 1.4;
        }
        if self.pressed_keys.contains(&KeyCode::KeyE) {
            orbit_yaw += 1.4;
        }
        if self.pressed_keys.contains(&KeyCode::KeyR) {
            zoom_delta += 0.8;
        }
        if self.pressed_keys.contains(&KeyCode::KeyF) {
            zoom_delta -= 0.8;
        }

        if let Some(input) = self.world.get_strand_mut::<InputStrand>(self.input_entity) {
            input.move_axis = move_axis;
            input.orbit_yaw = orbit_yaw;
            input.zoom_delta = zoom_delta;
            input.paused = self.paused;
        }
    }
}

impl ApplicationHandler for SaanApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(LogicalSize::new(self.options.width, self.options.height));

        match event_loop.create_window(attrs) {
            Ok(win) => {
                let win = Arc::new(win);
                match GpuRenderer::new(Arc::clone(&win)) {
                    Ok(gpu) => {
                        win.request_redraw();
                        self.window = Some(win);
                        self.gpu = Some(gpu);
                    }
                    Err(err) => {
                        eprintln!("Failed to initialize wgpu renderer: {err}");
                        event_loop.exit();
                    }
                }
            }
            Err(err) => {
                eprintln!("Failed to create window: {err}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size.width, size.height);
                }
                if size.width > 0 && size.height > 0 {
                    let aspect = size.width as f32 / size.height as f32;
                    for (_, cam) in self.world.query_mut::<CameraRig>() {
                        cam.aspect_ratio = aspect;
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                match state {
                    ElementState::Pressed => {
                        if code == KeyCode::Escape {
                            event_loop.exit();
                            return;
                        }
                        if code == KeyCode::Space {
                            self.paused = !self.paused;
                        }
                        self.pressed_keys.insert(code);
                    }
                    ElementState::Released => {
                        self.pressed_keys.remove(&code);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().clamp(0.001, 0.1);
                self.last_frame = now;
                let elapsed = (now - self.start_instant).as_secs_f32();

                self.sync_input_strand();
                if self.frame_counter % 30 == 0 {
                    if let Some(reloader) = &mut self.hot_reloader {
                        if let Ok(Some(new_title)) = reloader.poll_reload(&mut self.world) {
                            println!("Hot-reloaded .saan scene pattern: '{new_title}'");
                        }
                    }
                }
                let sim_dt = if self.paused { 0.0 } else { dt };
                let _ = self.loom.step(&mut self.world, sim_dt);

                let frame = self.render_loom.extract_frame(&self.world);

                if let Some(gpu) = &mut self.gpu {
                    match gpu.render(&frame, elapsed) {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            gpu.resize(gpu.config.width, gpu.config.height);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            event_loop.exit();
                            return;
                        }
                        Err(wgpu::SurfaceError::Timeout) => {}
                    }
                }

                self.frame_counter += 1;
                if let Some(max) = self.options.max_frames {
                    if self.frame_counter >= max {
                        println!(
                            "Completed {} rendered frames in wgpu viewport; exiting.",
                            self.frame_counter
                        );
                        event_loop.exit();
                        return;
                    }
                }

                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
            _ => {}
        }
    }
}

/// Launches the interactive `winit` + `wgpu` window for the given [`ScenePattern`].
pub fn run_windowed(pattern: ScenePattern, options: WindowOptions) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = SaanApp::new(&pattern, options);
    event_loop.run_app(&mut app)?;
    Ok(())
}
