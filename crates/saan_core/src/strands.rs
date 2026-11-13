use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

use crate::EntityId;

/// Human-readable label strand for an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameStrand(pub String);

/// 3D spatial transform used by both 3D meshes and 2.5D sprites in the world.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Self {
            translation: Vec3::new(x, y, z),
            ..Default::default()
        }
    }

    pub fn with_scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    /// Computes the local-to-world 4x4 transformation matrix.
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// Linear & angular velocity strand for simple motion weaving.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular_y: f32,
}

/// Interactive input & frame timing strand woven into the [`World`] during `saan run`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct InputStrand {
    /// Camera orbit / movement input vector `(right, up, forward)` in `[-1.0, 1.0]`.
    pub move_axis: Vec3,
    /// Orbit yaw delta in radians for this frame.
    pub orbit_yaw: f32,
    /// Zoom delta (scroll or Q/E keys).
    pub zoom_delta: f32,
    /// Whether the simulation pause toggle was requested.
    pub paused: bool,
    /// Whether the primary attack action was triggered this frame.
    #[serde(default)]
    pub attack: bool,
    /// Whether the dash / dodge roll action was triggered this frame.
    #[serde(default)]
    pub dash: bool,
    /// Whether the ranged spell / projectile action was triggered this frame.
    #[serde(default)]
    pub cast_spell: bool,
}

/// How a 2.5D sprite orientates itself inside the 3D world relative to the active camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillboardMode {
    /// Fixed plane in 3D world space (e.g., floor decals, walls, paper diorama props).
    WorldPlane,
    /// Rotates around the Y-axis to face the camera while staying upright (classic HD-2D characters & trees).
    CylindricalY,
    /// Fully faces the camera on all axes (particles, floating icons).
    Spherical,
}

/// 2.5D Sprite strand for HD-2D & Diorama rendering.
/// Lives in 3D coordinates via [`Transform`], receives 3D lighting, and optionally casts 3D shadows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sprite25D {
    /// Texture or spritesheet identifier.
    pub texture_id: String,
    /// World-space width and height in meters/units.
    pub size: Vec2,
    /// Normalized pivot point `(0.5, 0.0)` = bottom-center anchored to the ground.
    pub pivot: Vec2,
    /// Orientation mode in 3D space.
    pub billboard: BillboardMode,
    /// Optional normal map for 3D directional/point lighting on pixel/vector art.
    pub normal_map_id: Option<String>,
    /// Whether this 2.5D sprite casts shadows onto 3D geometry and other sprites.
    pub casts_shadow: bool,
    /// RGBA tint multiplier.
    pub tint: Vec4,
    /// Depth bias (in view space) to prevent Z-fighting when layering flat cards.
    pub depth_bias: f32,
    /// Normalized UV sub-rectangle `[u_min, v_min, u_max, v_max]` into the sprite sheet/atlas.
    #[serde(default = "default_uv_rect")]
    pub uv_rect: [f32; 4],
}

fn default_uv_rect() -> [f32; 4] {
    [0.0, 0.0, 1.0, 1.0]
}

impl Default for Sprite25D {
    fn default() -> Self {
        Self {
            texture_id: "default_sprite".to_string(),
            size: Vec2::new(1.0, 1.0),
            pivot: Vec2::new(0.5, 0.0),
            billboard: BillboardMode::CylindricalY,
            normal_map_id: None,
            casts_shadow: true,
            tint: Vec4::ONE,
            depth_bias: 0.0,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
        }
    }
}

/// Sprite sheet / flipbook animation strand ("Animation Strand") that steps `Sprite25D::uv_rect` over time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlipbookStrand {
    /// Number of columns in the sprite sheet grid.
    pub columns: u32,
    /// Number of rows in the sprite sheet grid.
    pub rows: u32,
    /// Inclusive start frame index.
    pub start_frame: u32,
    /// Inclusive end frame index.
    pub end_frame: u32,
    /// Playback speed in frames per second.
    pub fps: f32,
    /// Current frame index.
    #[serde(default)]
    pub current_frame: u32,
    /// Accumulated frame timer in seconds.
    #[serde(default)]
    pub timer: f32,
    /// Whether the animation loops when reaching `end_frame`.
    pub looping: bool,
}

impl Default for FlipbookStrand {
    fn default() -> Self {
        Self {
            columns: 4,
            rows: 1,
            start_frame: 0,
            end_frame: 3,
            fps: 8.0,
            current_frame: 0,
            timer: 0.0,
            looping: true,
        }
    }
}

impl FlipbookStrand {
    /// Computes the `[u_min, v_min, u_max, v_max]` UV rectangle for `current_frame`.
    pub fn current_uv_rect(&self) -> [f32; 4] {
        let cols = self.columns.max(1);
        let rows = self.rows.max(1);
        let col = self.current_frame % cols;
        let row = (self.current_frame / cols).min(rows - 1);

        let w = 1.0 / cols as f32;
        let h = 1.0 / rows as f32;
        let u0 = col as f32 * w;
        let v0 = row as f32 * h;
        [u0, v0, u0 + w, v0 + h]
    }
}

/// Built-in or external 3D mesh shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MeshPrimitive {
    /// Ground/terrain plane of `(width, depth)`.
    GroundPlane { width: f32, depth: f32 },
    /// 3D box of `(x, y, z)` extents.
    Cube { size: Vec3 },
    /// Voxel/diorama block or custom glTF asset path.
    Asset { path: String },
    /// Native MagicaVoxel (.vox) asset path and scale with automatic 3D greedy meshing.
    Voxel { path: String, scale: f32 },
}

/// Surface material properties for 3D geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material3D {
    pub name: String,
    pub base_color: Vec4,
    pub roughness: f32,
    pub metallic: f32,
}

impl Default for Material3D {
    fn default() -> Self {
        Self {
            name: "standard_clay".to_string(),
            base_color: Vec4::new(0.8, 0.78, 0.72, 1.0),
            roughness: 0.7,
            metallic: 0.0,
        }
    }
}

/// 3D Mesh strand that shares the world with [`Sprite25D`] entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mesh3D {
    pub primitive: MeshPrimitive,
    pub material: Material3D,
    pub casts_shadow: bool,
    pub receives_shadow: bool,
}

/// Projection mode for [`CameraRig`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CameraProjection {
    /// Standard 3D perspective projection.
    Perspective { fov_y_degrees: f32 },
    /// True orthographic 2.5D/isometric projection.
    Orthographic { vertical_size: f32 },
    /// HD-2D Diorama projection: telephoto/tilted perspective with shallow depth-of-field parameters.
    DioramaTilt {
        fov_y_degrees: f32,
        focus_distance: f32,
        aperture_blur: f32,
    },
}

/// Camera strand controlling how the 2.5D + 3D world is viewed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraRig {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
    pub projection: CameraProjection,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 6.0, 10.0),
            target: Vec3::new(0.0, 1.0, 0.0),
            up: Vec3::Y,
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 200.0,
            projection: CameraProjection::DioramaTilt {
                fov_y_degrees: 35.0,
                focus_distance: 11.5,
                aperture_blur: 0.65,
            },
        }
    }
}

impl CameraRig {
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        match self.projection {
            CameraProjection::Perspective { fov_y_degrees }
            | CameraProjection::DioramaTilt { fov_y_degrees, .. } => {
                Mat4::perspective_rh(fov_y_degrees.to_radians(), self.aspect_ratio, self.near, self.far)
            }
            CameraProjection::Orthographic { vertical_size } => {
                let half_h = vertical_size * 0.5;
                let half_w = half_h * self.aspect_ratio;
                Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, self.near, self.far)
            }
        }
    }

    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }
}

/// Directional sun/moon light strand.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub casts_shadows: bool,
}

/// Omnidirectional point light strand (e.g., lanterns, campfires, spells in HD-2D scenes).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointLight {
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
}

/// 2.5D/3D collision shape attached to an entity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ColliderShape {
    /// Axis-aligned 3D bounding box with `half_extents` `(hx, hy, hz)` centered at `Transform::translation`.
    Aabb3D { half_extents: Vec3 },
    /// Upright cylinder with `radius` in XZ and total `height` along Y (bottom at `translation.y`).
    /// Ideal for 2.5D billboard sprites moving inside a 3D world.
    CylinderY { radius: f32, height: f32 },
}

/// Spatial collision & ground/stair-snapping strand for 2.5D sprites and 3D props.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Collider3D {
    pub shape: ColliderShape,
    /// Static colliders (floors, walls, altars) do not move when pushed; dynamic ones slide/step.
    pub is_static: bool,
    /// Maximum vertical step height (in meters) this entity can automatically climb when walking into a ledge/stair.
    #[serde(default = "default_step_height")]
    pub max_step_height: f32,
    /// Whether this 3D obstacle should fade/occlude when blocking the camera's line of sight to the hero.
    #[serde(default)]
    pub fade_when_occluding_camera: bool,
}

fn default_step_height() -> f32 {
    0.45
}

impl Collider3D {
    /// Computes the world-space `(min, max)` AABB for this collider at `translation`.
    pub fn world_aabb(&self, translation: Vec3) -> (Vec3, Vec3) {
        match self.shape {
            ColliderShape::Aabb3D { half_extents } => {
                (translation - half_extents, translation + half_extents)
            }
            ColliderShape::CylinderY { radius, height } => (
                Vec3::new(translation.x - radius, translation.y, translation.z - radius),
                Vec3::new(
                    translation.x + radius,
                    translation.y + height,
                    translation.z + radius,
                ),
            ),
        }
    }
}

/// 3D positional audio emitter strand ("Sound Strand") woven onto entities in a 2.5D/3D world.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioEmitter3D {
    /// Sound clip asset ID (e.g., `"audio/spirit_chime_loop.ogg"`).
    pub sound_id: String,
    /// Base source volume in `[0.0, 2.0]`.
    pub base_volume: f32,
    /// Maximum audible radius in meters (inverse-distance rolloff).
    pub max_distance: f32,
    /// Whether this sound emitter loops continuously.
    pub looping: bool,
    /// Runtime calculated stereo gain `(left_ear, right_ear)` based on 3D listener pose.
    #[serde(default)]
    pub current_stereo_gain: [f32; 2],
}

impl Default for AudioEmitter3D {
    fn default() -> Self {
        Self {
            sound_id: "audio/ambient_chime.ogg".to_string(),
            base_volume: 1.0,
            max_distance: 18.0,
            looping: true,
            current_stereo_gain: [0.0, 0.0],
        }
    }
}

/// Decoupled gameplay event emitted during a frame weave.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameplayEvent {
    /// Emitted when a dynamic entity collides or slides against a static obstacle.
    CollisionContact {
        dynamic_entity: String,
        obstacle_entity: String,
        contact_point: Vec3,
    },
    /// Emitted when a dynamic entity steps up onto a ledge or stair.
    SteppedUp {
        entity: String,
        step_height: f32,
    },
    /// Emitted when an audio emitter enters or leaves audible range.
    AudioCue {
        emitter: String,
        sound_id: String,
        stereo_gain: [f32; 2],
    },
    /// Custom named trigger (e.g., shrine activation, dialogue cue).
    Trigger {
        source: String,
        tag: String,
    },
    /// Emitted when an attack hitbox intersects an enemy hurtbox.
    CombatHit {
        attacker: String,
        target: String,
        damage: f32,
        hit_pos: Vec3,
        knockback: Vec3,
    },
}

/// Event bus strand (`EventWeft`) allowing systems to publish and subscribe to decoupled gameplay triggers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventWeftStrand {
    /// Events published during the current frame.
    pub current_events: Vec<GameplayEvent>,
    /// Total cumulative events processed across the session.
    pub total_published: u64,
}

impl EventWeftStrand {
    pub fn publish(&mut self, event: GameplayEvent) {
        self.total_published += 1;
        self.current_events.push(event);
    }

    pub fn drain_current(&mut self) -> Vec<GameplayEvent> {
        std::mem::take(&mut self.current_events)
    }
}

/// Phase 6: In-Engine Woven Inspector (`InspectorStrand`) for live DAG telemetry and diorama parameter tweaking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectorStrand {
    /// Whether the on-screen Woven Inspector HUD overlay is visible (toggled via `Tab` or `F1`).
    pub visible: bool,
    /// Currently selected entity index in the live world inspector.
    pub selected_entity_idx: usize,
    /// Smoothed frames-per-second telemetry.
    pub fps: f32,
    /// Last frame delta-time in milliseconds.
    pub frame_ms: f32,
    /// Whether the Loom DAG is currently taut (zero cycles/hazards).
    pub loom_taut: bool,
    /// Count of active Weft systems in the Loom schedule.
    pub active_system_count: usize,
    /// Live Diorama Tilt-Shift focus distance override delta.
    pub focus_distance_delta: f32,
    /// Live Diorama Tilt-Shift aperture blur override delta.
    pub aperture_blur_delta: f32,
    /// Live Directional Sun intensity override delta.
    pub sun_intensity_delta: f32,
}

impl Default for InspectorStrand {
    fn default() -> Self {
        Self {
            visible: true,
            selected_entity_idx: 0,
            fps: 60.0,
            frame_ms: 16.6,
            loom_taut: true,
            active_system_count: 6,
            focus_distance_delta: 0.0,
            aperture_blur_delta: 0.0,
            sun_intensity_delta: 0.0,
        }
    }
}

/// Script strand binding an entity to a Rhai gameplay script file or inline code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptStrand {
    /// Path to the script file (e.g. `"scripts/hero_controller.rhai"`) or identifier.
    pub script_path: String,
    /// Inline Rhai script source code if not loading from disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_source: Option<String>,
    /// Whether the script is currently active.
    pub enabled: bool,
}

impl Default for ScriptStrand {
    fn default() -> Self {
        Self {
            script_path: "scripts/default_behavior.rhai".to_string(),
            inline_source: None,
            enabled: true,
        }
    }
}

/// Tactical 2.5D Fog of War visibility grid strand.
///
/// Tracks explored and visible cells across the diorama world plane (XZ).
/// Cells store visibility: 0 = unexplored (black), 128 = shrouded (explored but hidden), 255 = fully revealed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FogOfWarStrand {
    pub width: u32,
    pub height: u32,
    pub world_min: Vec2,
    pub world_max: Vec2,
    pub cells: Vec<u8>,
}

impl FogOfWarStrand {
    pub fn new(width: u32, height: u32, world_min: Vec2, world_max: Vec2) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            world_min,
            world_max,
            cells: vec![0; size],
        }
    }

    pub fn default_diorama() -> Self {
        Self::new(64, 64, Vec2::new(-12.0, -12.0), Vec2::new(12.0, 12.0))
    }

    /// Converts world XZ coordinate into grid (gx, gz) cell index.
    pub fn world_to_grid(&self, wx: f32, wz: f32) -> Option<(u32, u32)> {
        if wx < self.world_min.x || wx > self.world_max.x || wz < self.world_min.y || wz > self.world_max.y {
            return None;
        }
        let u = (wx - self.world_min.x) / (self.world_max.x - self.world_min.x);
        let v = (wz - self.world_min.y) / (self.world_max.y - self.world_min.y);
        let gx = (u * self.width as f32).clamp(0.0, (self.width - 1) as f32) as u32;
        let gz = (v * self.height as f32).clamp(0.0, (self.height - 1) as f32) as u32;
        Some((gx, gz))
    }

    /// Downgrades all currently revealed cells (255) to shrouded (128) before actors re-reveal current vision.
    pub fn shroud_visible(&mut self) {
        for cell in &mut self.cells {
            if *cell == 255 {
                *cell = 128;
            }
        }
    }

    /// Reveals a circular vision area centered at `(center_x, center_z)` with radius in meters.
    pub fn reveal_circle(&mut self, center_x: f32, center_z: f32, radius: f32) {
        let min_x = center_x - radius;
        let max_x = center_x + radius;
        let min_z = center_z - radius;
        let max_z = center_z + radius;

        let (g_min_x, g_min_z) = self.world_to_grid(min_x, min_z).unwrap_or((0, 0));
        let (g_max_x, g_max_z) = self
            .world_to_grid(max_x, max_z)
            .unwrap_or((self.width - 1, self.height - 1));

        let r2 = radius * radius;
        let w_span = self.world_max.x - self.world_min.x;
        let h_span = self.world_max.y - self.world_min.y;

        for gz in g_min_z..=g_max_z.min(self.height - 1) {
            let wz = self.world_min.y + (gz as f32 + 0.5) / self.height as f32 * h_span;
            let dz = wz - center_z;
            for gx in g_min_x..=g_max_x.min(self.width - 1) {
                let wx = self.world_min.x + (gx as f32 + 0.5) / self.width as f32 * w_span;
                let dx = wx - center_x;
                if dx * dx + dz * dz <= r2 {
                    let idx = (gz * self.width + gx) as usize;
                    if idx < self.cells.len() {
                        self.cells[idx] = 255;
                    }
                }
            }
        }
    }

    /// Samples visibility at world coordinate `(wx, wz)`. Returns 0 (unexplored), 128 (shrouded), or 255 (visible).
    pub fn get_visibility(&self, wx: f32, wz: f32) -> u8 {
        match self.world_to_grid(wx, wz) {
            Some((gx, gz)) => self.cells[(gz * self.width + gx) as usize],
            None => 0,
        }
    }
}

/// Single active particle in the world.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleData {
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime_left: f32,
    pub max_lifetime: f32,
    pub size: f32,
    pub color: [f32; 4],
}

/// 3D/2.5D particle emitter strand for dust puffs, spirit wisps, sparks, and spell impacts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleEmitter3D {
    pub emission_rate: f32,
    pub particle_lifetime: f32,
    pub initial_velocity: Vec3,
    pub velocity_spread: Vec3,
    pub gravity: Vec3,
    pub start_color: [f32; 4],
    pub end_color: [f32; 4],
    pub size: f32,
    #[serde(default)]
    pub spawn_accumulator: f32,
    #[serde(skip)]
    pub particles: Vec<ParticleData>,
}

impl Default for ParticleEmitter3D {
    fn default() -> Self {
        Self::spirit_wisps()
    }
}

impl ParticleEmitter3D {
    pub fn spirit_wisps() -> Self {
        Self {
            emission_rate: 8.0,
            particle_lifetime: 1.6,
            initial_velocity: Vec3::new(0.0, 0.4, 0.0),
            velocity_spread: Vec3::new(0.3, 0.2, 0.3),
            gravity: Vec3::new(0.0, 0.1, 0.0),
            start_color: [1.0, 0.85, 0.45, 0.9],
            end_color: [0.9, 0.4, 0.1, 0.0],
            size: 0.25,
            spawn_accumulator: 0.0,
            particles: Vec::new(),
        }
    }

    pub fn step(&mut self, origin: Vec3, dt: f32) {
        for p in &mut self.particles {
            p.lifetime_left -= dt;
            p.velocity += self.gravity * dt;
            p.position += p.velocity * dt;
            let life_frac = (p.lifetime_left / p.max_lifetime).clamp(0.0, 1.0);
            for i in 0..4 {
                p.color[i] = self.end_color[i] + (self.start_color[i] - self.end_color[i]) * life_frac;
            }
        }
        self.particles.retain(|p| p.lifetime_left > 0.0);

        self.spawn_accumulator += self.emission_rate * dt;
        while self.spawn_accumulator >= 1.0 {
            self.spawn_accumulator -= 1.0;
            let offset_x = (self.particles.len() as f32 * 1.618).fract() * 2.0 - 1.0;
            let offset_z = (self.particles.len() as f32 * 2.414).fract() * 2.0 - 1.0;
            let vel = self.initial_velocity
                + Vec3::new(
                    offset_x * self.velocity_spread.x,
                    ((self.particles.len() as f32 * 0.73).fract() * 2.0 - 1.0) * self.velocity_spread.y,
                    offset_z * self.velocity_spread.z,
                );
            self.particles.push(ParticleData {
                position: origin + Vec3::new(offset_x * 0.1, 0.0, offset_z * 0.1),
                velocity: vel,
                lifetime_left: self.particle_lifetime,
                max_lifetime: self.particle_lifetime,
                size: self.size,
                color: self.start_color,
            });
        }
    }
}

/// Dynamic health pool strand for characters, enemies, and destructibles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthStrand {
    pub current: f32,
    pub max: f32,
    pub is_alive: bool,
    pub invulnerable: bool,
}

impl HealthStrand {
    pub fn new(max: f32) -> Self {
        Self {
            current: max,
            max,
            is_alive: true,
            invulnerable: false,
        }
    }

    pub fn take_damage(&mut self, amount: f32) -> bool {
        if self.invulnerable || !self.is_alive {
            return false;
        }
        self.current = (self.current - amount).max(0.0);
        if self.current <= 0.0 {
            self.is_alive = false;
        }
        true
    }

    pub fn heal(&mut self, amount: f32) {
        if self.is_alive {
            self.current = (self.current + amount).min(self.max);
        }
    }
}

impl Default for HealthStrand {
    fn default() -> Self {
        Self::new(100.0)
    }
}

/// Geometric shape for combat hitboxes and hurtboxes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CombatShape {
    CylinderY { radius: f32, height: f32 },
    Aabb { half_extents: Vec3 },
    Sphere { radius: f32 },
}

/// Vulnerable hurtbox strand attached to damagable entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HurtboxStrand {
    pub shape: CombatShape,
    pub offset: Vec3,
    pub team: u8,
    pub invulnerability_timer: f32,
    pub poise: f32,
}

impl HurtboxStrand {
    pub fn cylinder(radius: f32, height: f32, team: u8) -> Self {
        Self {
            shape: CombatShape::CylinderY { radius, height },
            offset: Vec3::ZERO,
            team,
            invulnerability_timer: 0.0,
            poise: 10.0,
        }
    }
}

impl Default for HurtboxStrand {
    fn default() -> Self {
        Self::cylinder(0.5, 1.8, 0)
    }
}

/// Active damaging hitbox strand spawned during attacks or attached to melee swings / projectiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitboxStrand {
    pub shape: CombatShape,
    pub offset: Vec3,
    pub team: u8,
    pub owner: EntityId,
    pub damage: f32,
    pub knockback: Vec3,
    pub hitstun: f32,
    pub lifetime: f32,
    pub already_hit: Vec<EntityId>,
    #[serde(default)]
    pub destroy_on_hit: bool,
}

impl HitboxStrand {
    pub fn new(owner: EntityId, team: u8, damage: f32, knockback: Vec3, shape: CombatShape, lifetime: f32) -> Self {
        Self {
            shape,
            offset: Vec3::ZERO,
            team,
            owner,
            damage,
            knockback,
            hitstun: 0.2,
            lifetime,
            already_hit: Vec::new(),
            destroy_on_hit: false,
        }
    }
}

/// Character Action State for real-time 2.5D combat.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ActionState {
    Idle,
    Move { speed: f32 },
    Dash { timer: f32, max_time: f32, direction: Vec2, speed: f32 },
    Attack { combo_index: u32, timer: f32, max_time: f32, has_spawned_hitbox: bool },
    Hurt { timer: f32, max_time: f32 },
    Dead,
}

/// Action State strand managing inputs, dodge rolls, melee combos, and flipbook sync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionStateStrand {
    pub state: ActionState,
    pub facing_direction: Vec2,
    pub buffered_dash: Option<Vec2>,
    pub buffered_attack: bool,
    #[serde(default)]
    pub buffered_spell: bool,
    pub move_speed: f32,
    pub dash_speed: f32,
    pub dash_duration: f32,
}

impl Default for ActionStateStrand {
    fn default() -> Self {
        Self {
            state: ActionState::Idle,
            facing_direction: Vec2::new(0.0, -1.0),
            buffered_dash: None,
            buffered_attack: false,
            buffered_spell: false,
            move_speed: 6.0,
            dash_speed: 14.0,
            dash_duration: 0.22,
        }
    }
}

