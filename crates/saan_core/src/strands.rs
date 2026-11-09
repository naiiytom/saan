use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

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



