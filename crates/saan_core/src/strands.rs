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
