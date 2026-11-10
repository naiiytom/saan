use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use glam::{Quat, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

use crate::ecs::{EntityId, World};
use crate::strands::{
    ActionStateStrand, AudioEmitter3D, BillboardMode, CameraProjection, CameraRig, Collider3D,
    ColliderShape, DirectionalLight, EventWeftStrand, FlipbookStrand, FogOfWarStrand, HealthStrand,
    HitboxStrand, HurtboxStrand, Material3D, Mesh3D, MeshPrimitive, NameStrand, ParticleEmitter3D,
    PointLight, ScriptStrand, Sprite25D, Transform, Velocity,
};

/// Serializable blueprint of a single entity and its interwoven 2.5D/3D strands.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityBlueprint {
    pub name: String,
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity: Option<Velocity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collider_3d: Option<Collider3D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_25d: Option<Sprite25D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flipbook: Option<FlipbookStrand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_3d: Option<Mesh3D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraRig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directional_light: Option<DirectionalLight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_light: Option<PointLight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_emitter: Option<AudioEmitter3D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<ScriptStrand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog_of_war: Option<FogOfWarStrand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub particle_emitter: Option<ParticleEmitter3D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthStrand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_state: Option<ActionStateStrand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurtbox: Option<HurtboxStrand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hitbox: Option<HitboxStrand>,
}

/// A `.saan` Scene Pattern describing an entire hybrid 2.5D + 3D diorama world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenePattern {
    pub version: u32,
    pub title: String,
    pub description: String,
    pub entities: Vec<EntityBlueprint>,
}

impl ScenePattern {
    /// Creates a starter HD-2D & Diorama showcase scene (3D ground & shrine blocks + 2.5D billboarded hero, trees, and lantern light).
    pub fn default_hd2d_diorama(title: impl Into<String>) -> Self {
        Self {
            version: 1,
            title: title.into(),
            description: "HD-2D Diorama scene interweaving 3D terrain/architecture with 2.5D lit billboard sprites.".to_string(),
            entities: vec![
                EntityBlueprint {
                    name: "diorama_camera".to_string(),
                    transform: Transform::from_xyz(0.0, 6.5, 11.0),
                    velocity: None,
                    collider_3d: None,
                    sprite_25d: None,
                    flipbook: None,
                    mesh_3d: None,
                    camera: Some(CameraRig {
                        eye: Vec3::new(0.0, 6.5, 11.0),
                        target: Vec3::new(0.0, 1.0, 0.0),
                        up: Vec3::Y,
                        aspect_ratio: 16.0 / 9.0,
                        near: 0.1,
                        far: 150.0,
                        projection: CameraProjection::DioramaTilt {
                            fov_y_degrees: 32.0,
                            focus_distance: 11.5,
                            aperture_blur: 0.75,
                        },
                    }),
                    directional_light: None,
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "evening_sun".to_string(),
                    transform: Transform::from_xyz(10.0, 20.0, 8.0),
                    velocity: None,
                    collider_3d: None,
                    sprite_25d: None,
                    flipbook: None,
                    mesh_3d: None,
                    camera: None,
                    directional_light: Some(DirectionalLight {
                        direction: Vec3::new(-0.45, -0.8, -0.35).normalize(),
                        color: Vec3::new(1.0, 0.88, 0.72),
                        intensity: 2.2,
                        casts_shadows: true,
                    }),
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "mossy_courtyard_ground".to_string(),
                    transform: Transform::from_xyz(0.0, 0.0, 0.0),
                    velocity: None,
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::Aabb3D {
                            half_extents: Vec3::new(12.0, 0.1, 12.0),
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: None,
                    flipbook: None,
                    mesh_3d: Some(Mesh3D {
                        primitive: MeshPrimitive::GroundPlane {
                            width: 24.0,
                            depth: 24.0,
                        },
                        material: Material3D {
                            name: "moss_stone".to_string(),
                            base_color: Vec4::new(0.28, 0.36, 0.25, 1.0),
                            roughness: 0.85,
                            metallic: 0.0,
                        },
                        casts_shadow: false,
                        receives_shadow: true,
                    }),
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: Some(FogOfWarStrand::default_diorama()),
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "stone_altar_3d".to_string(),
                    transform: Transform::from_xyz(0.0, 0.75, -2.5),
                    velocity: None,
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::Aabb3D {
                            half_extents: Vec3::new(1.5, 0.75, 1.0),
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: true,
                    }),
                    sprite_25d: None,
                    flipbook: None,
                    mesh_3d: Some(Mesh3D {
                        primitive: MeshPrimitive::Cube {
                            size: Vec3::new(3.0, 1.5, 2.0),
                        },
                        material: Material3D {
                            name: "carved_laterite".to_string(),
                            base_color: Vec4::new(0.58, 0.34, 0.26, 1.0),
                            roughness: 0.9,
                            metallic: 0.05,
                        },
                        casts_shadow: true,
                        receives_shadow: true,
                    }),
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "wanderer_hero_25d".to_string(),
                    transform: Transform::from_xyz(0.0, 0.0, 1.5),
                    velocity: Some(Velocity {
                        linear: Vec3::new(0.5, 0.0, -0.25),
                        angular_y: 0.0,
                    }),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.45,
                            height: 1.8,
                        },
                        is_static: false,
                        max_step_height: 0.45,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/wanderer_walk_sheet.png".to_string(),
                        size: Vec2::new(1.2, 1.8),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/wanderer_walk_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::ONE,
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 0.25, 1.0],
                    }),
                    flipbook: Some(FlipbookStrand {
                        columns: 4,
                        rows: 1,
                        start_frame: 0,
                        end_frame: 3,
                        fps: 6.0,
                        current_frame: 0,
                        timer: 0.0,
                        looping: true,
                    }),
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/footsteps_moss.ogg".to_string(),
                        base_volume: 0.65,
                        max_distance: 16.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    script: Some(ScriptStrand {
                        script_path: "scripts/wanderer_ai.rhai".to_string(),
                        inline_source: Some(r#"
                            fn on_tick(dt) {
                                // Patrol loop when reaching boundary
                                if pos.x > 3.0 {
                                    vel.x = -0.5;
                                } else if pos.x < -3.0 {
                                    vel.x = 0.5;
                                }
                            }
                        "#.to_string()),
                        enabled: true,
                    }),
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "banyan_tree_canopy_25d".to_string(),
                    transform: Transform::from_xyz(-3.2, 0.0, -1.0),
                    velocity: None,
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.65,
                            height: 4.0,
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: true,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/banyan_tree.png".to_string(),
                        size: Vec2::new(3.5, 4.2),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/banyan_tree_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::ONE,
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                    }),
                    flipbook: None,
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/banyan_wind_leaves.ogg".to_string(),
                        base_volume: 0.5,
                        max_distance: 20.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "spirit_lantern_25d".to_string(),
                    transform: Transform::from_xyz(1.8, 1.4, -1.2)
                        .with_rotation(Quat::IDENTITY),
                    velocity: None,
                    collider_3d: None,
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/spirit_wisp.png".to_string(),
                        size: Vec2::new(0.6, 0.6),
                        pivot: Vec2::new(0.5, 0.5),
                        billboard: BillboardMode::Spherical,
                        normal_map_id: None,
                        casts_shadow: false,
                        tint: Vec4::new(1.0, 0.78, 0.35, 0.95),
                        depth_bias: 0.01,
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                    }),
                    flipbook: None,
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: Some(PointLight {
                        color: Vec3::new(1.0, 0.72, 0.3),
                        intensity: 4.5,
                        radius: 6.0,
                    }),
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/spirit_lantern_chime.ogg".to_string(),
                        base_volume: 0.9,
                        max_distance: 18.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    script: None,
                    fog_of_war: None,
                    particle_emitter: Some(ParticleEmitter3D::spirit_wisps()),
                    ..Default::default()
                },
            ],
        }
    }

    /// Creates the complete, playable Unified Diorama Sandbox scene.
    ///
    /// Interweaves all Saan engine capabilities:
    /// - Hero action unit with WASD movement and flipbook walking animation.
    /// - RTS squad units with selection and flow-field navigation.
    /// - Multi-height elevated stone ruins with stair/ledge step-up collision.
    /// - Dynamic Fog of War visibility grid with exploration clearing.
    /// - Directional evening sun casting hardware depth shadow maps with 4-tap PCF.
    /// - Glowing ancient spirit shrine with 3D particle emitter wisps and point lighting.
    /// - Embedded Rhai gameplay scripts and 3D spatial audio emitters.
    pub fn diorama_sandbox(title: impl Into<String>) -> Self {
        Self {
            version: 1,
            title: title.into(),
            description: "Unified 2.5D + 3D Diorama Sandbox combining Hero Action, RTS squads, TBS ruins, Fog of War, and Directional Shadows.".to_string(),
            entities: vec![
                EntityBlueprint {
                    name: "diorama_cinematic_camera".to_string(),
                    transform: Transform::from_xyz(0.0, 9.5, 14.0),
                    velocity: None,
                    collider_3d: None,
                    sprite_25d: None,
                    flipbook: None,
                    mesh_3d: None,
                    camera: Some(CameraRig {
                        eye: Vec3::new(0.0, 9.5, 14.0),
                        target: Vec3::new(0.0, 1.0, 0.0),
                        up: Vec3::Y,
                        aspect_ratio: 16.0 / 9.0,
                        near: 0.1,
                        far: 150.0,
                        projection: CameraProjection::DioramaTilt {
                            fov_y_degrees: 34.0,
                            focus_distance: 14.5,
                            aperture_blur: 0.75,
                        },
                    }),
                    directional_light: None,
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "evening_sun_shadow_caster".to_string(),
                    transform: Transform::from_xyz(12.0, 24.0, 10.0),
                    velocity: None,
                    collider_3d: None,
                    sprite_25d: None,
                    flipbook: None,
                    mesh_3d: None,
                    camera: None,
                    directional_light: Some(DirectionalLight {
                        direction: Vec3::new(-0.45, -0.85, -0.3).normalize(),
                        color: Vec3::new(1.0, 0.86, 0.70),
                        intensity: 2.4,
                        casts_shadows: true,
                    }),
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "temple_courtyard_ground".to_string(),
                    transform: Transform::from_xyz(0.0, 0.0, 0.0),
                    velocity: None,
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::Aabb3D {
                            half_extents: Vec3::new(16.0, 0.1, 16.0),
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: None,
                    flipbook: None,
                    mesh_3d: Some(Mesh3D {
                        primitive: MeshPrimitive::GroundPlane {
                            width: 32.0,
                            depth: 32.0,
                        },
                        material: Material3D {
                            name: "ancient_moss_flagstone".to_string(),
                            base_color: Vec4::new(0.26, 0.34, 0.24, 1.0),
                            roughness: 0.88,
                            metallic: 0.02,
                        },
                        casts_shadow: false,
                        receives_shadow: true,
                    }),
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: Some(FogOfWarStrand::new(64, 64, Vec2::new(-16.0, -16.0), Vec2::new(16.0, 16.0))),
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "elevated_ruins_plateau".to_string(),
                    transform: Transform::from_xyz(4.5, 1.0, -3.5),
                    velocity: None,
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::Aabb3D {
                            half_extents: Vec3::new(3.5, 1.0, 3.5),
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: true,
                    }),
                    sprite_25d: None,
                    flipbook: None,
                    mesh_3d: Some(Mesh3D {
                        primitive: MeshPrimitive::Cube {
                            size: Vec3::new(7.0, 2.0, 7.0),
                        },
                        material: Material3D {
                            name: "carved_laterite_shrine".to_string(),
                            base_color: Vec4::new(0.52, 0.32, 0.22, 1.0),
                            roughness: 0.92,
                            metallic: 0.05,
                        },
                        casts_shadow: true,
                        receives_shadow: true,
                    }),
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "ancient_spirit_shrine".to_string(),
                    transform: Transform::from_xyz(4.5, 2.6, -3.5),
                    velocity: None,
                    collider_3d: None,
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/spirit_wisp.png".to_string(),
                        size: Vec2::new(1.0, 1.0),
                        pivot: Vec2::new(0.5, 0.5),
                        billboard: BillboardMode::Spherical,
                        normal_map_id: None,
                        casts_shadow: false,
                        tint: Vec4::new(0.4, 0.85, 1.0, 0.98),
                        depth_bias: 0.02,
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                    }),
                    flipbook: None,
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: Some(PointLight {
                        color: Vec3::new(0.35, 0.75, 1.0),
                        intensity: 6.0,
                        radius: 8.5,
                    }),
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/spirit_lantern_chime.ogg".to_string(),
                        base_volume: 0.95,
                        max_distance: 22.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    script: Some(ScriptStrand {
                        script_path: "scripts/shrine_trigger.rhai".to_string(),
                        inline_source: Some(r#"
                            fn on_tick(dt) {
                                // Pulsing aura oscillation
                                if pos.y > 2.8 {
                                    vel.y = -0.15;
                                } else if pos.y < 2.4 {
                                    vel.y = 0.15;
                                }
                            }
                        "#.to_string()),
                        enabled: true,
                    }),
                    fog_of_war: None,
                    particle_emitter: Some(ParticleEmitter3D::spirit_wisps()),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "hero_wanderer_action".to_string(),
                    transform: Transform::from_xyz(0.0, 0.0, 2.0),
                    velocity: Some(Velocity {
                        linear: Vec3::new(0.0, 0.0, 0.0),
                        angular_y: 0.0,
                    }),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.45,
                            height: 1.8,
                        },
                        is_static: false,
                        max_step_height: 0.5,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/wanderer_walk_sheet.png".to_string(),
                        size: Vec2::new(1.3, 1.9),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/wanderer_walk_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::ONE,
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 0.25, 1.0],
                    }),
                    flipbook: Some(FlipbookStrand {
                        columns: 4,
                        rows: 1,
                        start_frame: 0,
                        end_frame: 3,
                        fps: 6.0,
                        current_frame: 0,
                        timer: 0.0,
                        looping: true,
                    }),
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/footsteps_moss.ogg".to_string(),
                        base_volume: 0.7,
                        max_distance: 18.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    script: Some(ScriptStrand {
                        script_path: "scripts/hero_controller.rhai".to_string(),
                        inline_source: Some(r#"
                            fn on_tick(dt) {
                                // Patrol or responsive velocity dampening
                                vel.x = vel.x * 0.92;
                                vel.z = vel.z * 0.92;
                            }
                        "#.to_string()),
                        enabled: true,
                    }),
                    fog_of_war: None,
                    particle_emitter: None,
                    health: Some(HealthStrand::new(100.0)),
                    action_state: Some(ActionStateStrand::default()),
                    hurtbox: Some(HurtboxStrand::cylinder(0.5, 1.8, 0)),
                    hitbox: None,
                },
                EntityBlueprint {
                    name: "rts_squad_unit_1".to_string(),
                    transform: Transform::from_xyz(-3.0, 0.0, 3.5),
                    velocity: Some(Velocity::default()),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.4,
                            height: 1.6,
                        },
                        is_static: false,
                        max_step_height: 0.4,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/wanderer_walk_sheet.png".to_string(),
                        size: Vec2::new(1.1, 1.6),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/wanderer_walk_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::new(0.75, 0.9, 1.0, 1.0),
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 0.25, 1.0],
                    }),
                    flipbook: Some(FlipbookStrand {
                        columns: 4,
                        rows: 1,
                        start_frame: 0,
                        end_frame: 3,
                        fps: 5.0,
                        current_frame: 0,
                        timer: 0.0,
                        looping: true,
                    }),
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "rts_squad_unit_2".to_string(),
                    transform: Transform::from_xyz(-4.5, 0.0, 4.0),
                    velocity: Some(Velocity::default()),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.4,
                            height: 1.6,
                        },
                        is_static: false,
                        max_step_height: 0.4,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/wanderer_walk_sheet.png".to_string(),
                        size: Vec2::new(1.1, 1.6),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/wanderer_walk_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::new(0.75, 0.9, 1.0, 1.0),
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 0.25, 1.0],
                    }),
                    flipbook: Some(FlipbookStrand {
                        columns: 4,
                        rows: 1,
                        start_frame: 0,
                        end_frame: 3,
                        fps: 5.0,
                        current_frame: 0,
                        timer: 0.0,
                        looping: true,
                    }),
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: None,
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "banyan_guardian_tree".to_string(),
                    transform: Transform::from_xyz(-5.5, 0.0, -2.5),
                    velocity: None,
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.8,
                            height: 4.8,
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: true,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/banyan_tree.png".to_string(),
                        size: Vec2::new(4.2, 5.0),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/banyan_tree_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::ONE,
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                    }),
                    flipbook: None,
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: None,
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/banyan_wind_leaves.ogg".to_string(),
                        base_volume: 0.6,
                        max_distance: 25.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    script: None,
                    fog_of_war: None,
                    particle_emitter: None,
                    ..Default::default()
                },
            ],
        }
    }

    /// Creates the flagship Action RPG Diorama scene ("arpg_shrine_clash").
    ///
    /// Interweaves:
    /// - Melee combo attacks (3-hit chain with lunges and hitboxes)
    /// - Dodge rolls with invulnerability frames
    /// - MagicaVoxel 3D Greedy-Meshed ancient shrine atop an elevated stone plateau
    /// - Dynamic 4-tap PCF directional shadows, volumetric fog-of-war, and tilt-shift depth-of-field
    /// - AI-driven Shadow Stalker enemies that chase and engage the hero
    /// - 3D spatial audio chimes, wind rustle, and footsteps
    pub fn arpg_shrine_clash(title: impl Into<String>) -> Self {
        Self {
            version: 1,
            title: title.into(),
            description: "Action RPG diorama showdown: Hero Wanderer battling Shadow Stalkers at the Sacred Ancient Voxel Shrine.".to_string(),
            entities: vec![
                EntityBlueprint {
                    name: "arpg_cinematic_camera".to_string(),
                    transform: Transform::from_xyz(0.0, 9.5, 14.5),
                    camera: Some(CameraRig {
                        eye: Vec3::new(0.0, 9.5, 14.5),
                        target: Vec3::new(0.0, 1.2, 0.0),
                        up: Vec3::Y,
                        aspect_ratio: 16.0 / 9.0,
                        near: 0.1,
                        far: 150.0,
                        projection: CameraProjection::DioramaTilt {
                            fov_y_degrees: 34.0,
                            focus_distance: 14.5,
                            aperture_blur: 0.75,
                        },
                    }),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "evening_sun".to_string(),
                    transform: Transform::from_xyz(12.0, 24.0, 10.0),
                    directional_light: Some(DirectionalLight {
                        direction: Vec3::new(-0.45, -0.85, -0.3).normalize(),
                        color: Vec3::new(1.0, 0.86, 0.70),
                        intensity: 2.4,
                        casts_shadows: true,
                    }),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "temple_courtyard_ground".to_string(),
                    transform: Transform::from_xyz(0.0, 0.0, 0.0),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::Aabb3D {
                            half_extents: Vec3::new(16.0, 0.1, 16.0),
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: false,
                    }),
                    mesh_3d: Some(Mesh3D {
                        primitive: MeshPrimitive::GroundPlane {
                            width: 32.0,
                            depth: 32.0,
                        },
                        material: Material3D {
                            name: "ancient_moss_flagstone".to_string(),
                            base_color: Vec4::new(0.26, 0.34, 0.24, 1.0),
                            roughness: 0.88,
                            metallic: 0.02,
                        },
                        casts_shadow: false,
                        receives_shadow: true,
                    }),
                    fog_of_war: Some(FogOfWarStrand::new(64, 64, Vec2::new(-16.0, -16.0), Vec2::new(16.0, 16.0))),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "elevated_shrine_plateau".to_string(),
                    transform: Transform::from_xyz(0.0, 0.6, -4.0),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::Aabb3D {
                            half_extents: Vec3::new(4.0, 0.6, 3.5),
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: true,
                    }),
                    mesh_3d: Some(Mesh3D {
                        primitive: MeshPrimitive::Cube {
                            size: Vec3::new(8.0, 1.2, 7.0),
                        },
                        material: Material3D {
                            name: "carved_laterite".to_string(),
                            base_color: Vec4::new(0.52, 0.32, 0.22, 1.0),
                            roughness: 0.92,
                            metallic: 0.05,
                        },
                        casts_shadow: true,
                        receives_shadow: true,
                    }),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "ancient_voxel_shrine".to_string(),
                    transform: Transform::from_xyz(0.0, 1.2, -4.0),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::Aabb3D {
                            half_extents: Vec3::new(1.4, 1.8, 1.4),
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: true,
                    }),
                    mesh_3d: Some(Mesh3D {
                        primitive: MeshPrimitive::Voxel {
                            path: "assets/voxels/ancient_shrine.vox".to_string(),
                            scale: 0.14,
                        },
                        material: Material3D {
                            name: "ancient_gold_wood".to_string(),
                            base_color: Vec4::ONE,
                            roughness: 0.7,
                            metallic: 0.1,
                        },
                        casts_shadow: true,
                        receives_shadow: true,
                    }),
                    point_light: Some(PointLight {
                        color: Vec3::new(1.0, 0.75, 0.35),
                        intensity: 5.0,
                        radius: 7.5,
                    }),
                    particle_emitter: Some(ParticleEmitter3D::spirit_wisps()),
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/spirit_lantern_chime.ogg".to_string(),
                        base_volume: 0.85,
                        max_distance: 20.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "hero_wanderer".to_string(),
                    transform: Transform::from_xyz(0.0, 0.0, 2.5),
                    velocity: Some(Velocity::default()),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.45,
                            height: 1.8,
                        },
                        is_static: false,
                        max_step_height: 0.5,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/wanderer_walk_sheet.png".to_string(),
                        size: Vec2::new(1.3, 1.9),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/wanderer_walk_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::ONE,
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 0.25, 1.0],
                    }),
                    flipbook: Some(FlipbookStrand {
                        columns: 4,
                        rows: 1,
                        start_frame: 0,
                        end_frame: 3,
                        fps: 6.0,
                        current_frame: 0,
                        timer: 0.0,
                        looping: true,
                    }),
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/footsteps_moss.ogg".to_string(),
                        base_volume: 0.7,
                        max_distance: 18.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    health: Some(HealthStrand::new(100.0)),
                    action_state: Some(ActionStateStrand::default()),
                    hurtbox: Some(HurtboxStrand::cylinder(0.5, 1.8, 0)),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "shadow_stalker_alpha".to_string(),
                    transform: Transform::from_xyz(3.5, 0.0, -0.5),
                    velocity: Some(Velocity::default()),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.45,
                            height: 1.6,
                        },
                        is_static: false,
                        max_step_height: 0.4,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/wanderer_walk_sheet.png".to_string(),
                        size: Vec2::new(1.2, 1.7),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/wanderer_walk_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::new(0.95, 0.35, 0.45, 1.0),
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 0.25, 1.0],
                    }),
                    flipbook: Some(FlipbookStrand {
                        columns: 4,
                        rows: 1,
                        start_frame: 0,
                        end_frame: 3,
                        fps: 5.0,
                        current_frame: 0,
                        timer: 0.0,
                        looping: true,
                    }),
                    health: Some(HealthStrand::new(60.0)),
                    action_state: Some(ActionStateStrand {
                        move_speed: 3.2,
                        ..ActionStateStrand::default()
                    }),
                    hurtbox: Some(HurtboxStrand::cylinder(0.45, 1.6, 1)),
                    script: Some(ScriptStrand {
                        script_path: "assets/scripts/enemy_chase_ai.rhai".to_string(),
                        inline_source: None,
                        enabled: true,
                    }),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "shadow_stalker_beta".to_string(),
                    transform: Transform::from_xyz(-3.5, 0.0, 0.5),
                    velocity: Some(Velocity::default()),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.45,
                            height: 1.6,
                        },
                        is_static: false,
                        max_step_height: 0.4,
                        fade_when_occluding_camera: false,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/wanderer_walk_sheet.png".to_string(),
                        size: Vec2::new(1.2, 1.7),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/wanderer_walk_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::new(0.95, 0.35, 0.45, 1.0),
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 0.25, 1.0],
                    }),
                    flipbook: Some(FlipbookStrand {
                        columns: 4,
                        rows: 1,
                        start_frame: 0,
                        end_frame: 3,
                        fps: 5.0,
                        current_frame: 0,
                        timer: 0.0,
                        looping: true,
                    }),
                    health: Some(HealthStrand::new(60.0)),
                    action_state: Some(ActionStateStrand {
                        move_speed: 3.0,
                        ..ActionStateStrand::default()
                    }),
                    hurtbox: Some(HurtboxStrand::cylinder(0.45, 1.6, 1)),
                    script: Some(ScriptStrand {
                        script_path: "assets/scripts/enemy_chase_ai.rhai".to_string(),
                        inline_source: None,
                        enabled: true,
                    }),
                    ..Default::default()
                },
                EntityBlueprint {
                    name: "banyan_guardian_tree".to_string(),
                    transform: Transform::from_xyz(-6.0, 0.0, -2.0),
                    collider_3d: Some(Collider3D {
                        shape: ColliderShape::CylinderY {
                            radius: 0.8,
                            height: 5.0,
                        },
                        is_static: true,
                        max_step_height: 0.0,
                        fade_when_occluding_camera: true,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/banyan_tree.png".to_string(),
                        size: Vec2::new(4.2, 5.0),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/banyan_tree_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::ONE,
                        depth_bias: 0.0,
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                    }),
                    audio_emitter: Some(AudioEmitter3D {
                        sound_id: "audio/banyan_wind_leaves.ogg".to_string(),
                        base_volume: 0.6,
                        max_distance: 25.0,
                        looping: true,
                        current_stereo_gain: [0.0, 0.0],
                    }),
                    ..Default::default()
                },
            ],
        }
    }

    /// Saves the scene pattern as formatted JSON to `path`.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, json)
    }

    /// Loads a `.saan` scene pattern from `path`.
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let raw = fs::read_to_string(path)?;
        serde_json::from_str(&raw)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Extracts a serializable [`ScenePattern`] from the live [`World`] state.
    pub fn extract_from_world(world: &World, title: impl Into<String>, description: impl Into<String>) -> Self {
        let mut entities = Vec::new();

        for (entity, transform) in world.query::<Transform>() {
            let name = world
                .get_strand::<NameStrand>(entity)
                .map(|n| n.0.clone())
                .unwrap_or_else(|| format!("entity_{entity}"));

            // Skip internal engine singleton entities like the event bus
            if name.starts_with("event_weft") || name.starts_with("__engine") {
                continue;
            }

            let velocity = world.get_strand::<Velocity>(entity).copied();
            let collider_3d = world.get_strand::<Collider3D>(entity).copied();
            let sprite_25d = world.get_strand::<Sprite25D>(entity).cloned();
            let flipbook = world.get_strand::<FlipbookStrand>(entity).cloned();
            let mesh_3d = world.get_strand::<Mesh3D>(entity).cloned();
            let camera = world.get_strand::<CameraRig>(entity).copied();
            let directional_light = world.get_strand::<DirectionalLight>(entity).copied();
            let point_light = world.get_strand::<PointLight>(entity).copied();
            let audio_emitter = world.get_strand::<AudioEmitter3D>(entity).cloned();
            let script = world.get_strand::<ScriptStrand>(entity).cloned();
            let fog_of_war = world.get_strand::<FogOfWarStrand>(entity).cloned();
            let particle_emitter = world.get_strand::<ParticleEmitter3D>(entity).cloned();
            let health = world.get_strand::<HealthStrand>(entity).cloned();
            let action_state = world.get_strand::<ActionStateStrand>(entity).cloned();
            let hurtbox = world.get_strand::<HurtboxStrand>(entity).cloned();
            let hitbox = world.get_strand::<HitboxStrand>(entity).cloned();

            entities.push(EntityBlueprint {
                name,
                transform: *transform,
                velocity,
                collider_3d,
                sprite_25d,
                flipbook,
                mesh_3d,
                camera,
                directional_light,
                point_light,
                audio_emitter,
                script,
                fog_of_war,
                particle_emitter,
                health,
                action_state,
                hurtbox,
                hitbox,
            });
        }

        Self {
            version: 1,
            title: title.into(),
            description: description.into(),
            entities,
        }
    }

    /// Hydrates this [`ScenePattern`] into a live [`World`], returning the spawned [`EntityId`]s.
    pub fn weave_into_world(&self, world: &mut World) -> Vec<EntityId> {
        let mut spawned = Vec::with_capacity(self.entities.len());
        for bp in &self.entities {
            let entity = world.spawn();
            world
                .weave_strand(entity, NameStrand(bp.name.clone()))
                .weave_strand(entity, bp.transform);

            if let Some(vel) = bp.velocity {
                world.weave_strand(entity, vel);
            }
            if let Some(col) = bp.collider_3d {
                world.weave_strand(entity, col);
            }
            if let Some(ref sprite) = bp.sprite_25d {
                world.weave_strand(entity, sprite.clone());
            }
            if let Some(ref flipbook) = bp.flipbook {
                world.weave_strand(entity, flipbook.clone());
            }
            if let Some(ref mesh) = bp.mesh_3d {
                world.weave_strand(entity, mesh.clone());
            }
            if let Some(cam) = bp.camera {
                world.weave_strand(entity, cam);
            }
            if let Some(dir_light) = bp.directional_light {
                world.weave_strand(entity, dir_light);
            }
            if let Some(pt_light) = bp.point_light {
                world.weave_strand(entity, pt_light);
            }
            if let Some(ref audio) = bp.audio_emitter {
                world.weave_strand(entity, audio.clone());
            }
            if let Some(ref script) = bp.script {
                world.weave_strand(entity, script.clone());
            }
            if let Some(ref fow) = bp.fog_of_war {
                world.weave_strand(entity, fow.clone());
            }
            if let Some(ref particles) = bp.particle_emitter {
                world.weave_strand(entity, particles.clone());
            }
            if let Some(ref health) = bp.health {
                world.weave_strand(entity, health.clone());
            }
            if let Some(ref action) = bp.action_state {
                world.weave_strand(entity, action.clone());
            }
            if let Some(ref hurtbox) = bp.hurtbox {
                world.weave_strand(entity, hurtbox.clone());
            }
            if let Some(ref hitbox) = bp.hitbox {
                world.weave_strand(entity, hitbox.clone());
            }
            spawned.push(entity);
        }

        // Ensure an EventWeftStrand bus exists in the world if not already present
        if world.query::<EventWeftStrand>().next().is_none() {
            let bus_entity = world.spawn();
            world
                .weave_strand(bus_entity, NameStrand("event_weft_bus".to_string()))
                .weave_strand(bus_entity, EventWeftStrand::default());
            spawned.push(bus_entity);
        }

        spawned
    }
}

/// Watches a `.saan` scene file on disk and hot-reloads its entities into [`World`] when modified.
#[derive(Debug, Clone)]
pub struct SceneHotReloader {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    tracked_entities: Vec<EntityId>,
}

impl SceneHotReloader {
    pub fn new(path: impl Into<PathBuf>, initial_entities: Vec<EntityId>) -> Self {
        let path = path.into();
        let last_modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
        Self {
            path,
            last_modified,
            tracked_entities: initial_entities,
        }
    }

    /// Checks if the `.saan` file has a newer modification timestamp; if so, reloads and re-weaves the scene.
    /// Returns `Ok(Some(new_title))` if a hot-reload occurred.
    pub fn poll_reload(&mut self, world: &mut World) -> std::io::Result<Option<String>> {
        let Ok(meta) = fs::metadata(&self.path) else {
            return Ok(None);
        };
        let Ok(modified) = meta.modified() else {
            return Ok(None);
        };

        let changed = match self.last_modified {
            Some(prev) => modified > prev,
            None => true,
        };

        if !changed {
            return Ok(None);
        }

        let pattern = ScenePattern::load_from_file(&self.path)?;
        for entity in self.tracked_entities.drain(..) {
            world.despawn(entity);
        }
        self.tracked_entities = pattern.weave_into_world(world);
        self.last_modified = Some(modified);
        Ok(Some(pattern.title))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hd2d_scene_hydrates_into_world() {
        let scene = ScenePattern::default_hd2d_diorama("Test Diorama");
        let mut world = World::new();
        let ids = scene.weave_into_world(&mut world);

        assert_eq!(ids.len(), 8);
        assert_eq!(world.entity_count(), 8);
        assert_eq!(world.query::<Sprite25D>().count(), 3);
        assert_eq!(world.query::<Mesh3D>().count(), 2);
        assert_eq!(world.query::<CameraRig>().count(), 1);
        assert_eq!(world.query::<AudioEmitter3D>().count(), 3);
        assert_eq!(world.query::<EventWeftStrand>().count(), 1);

        // Test extracting back to ScenePattern
        let extracted = ScenePattern::extract_from_world(&world, "Extracted Title", "Extracted Description");
        assert_eq!(extracted.title, "Extracted Title");
        assert_eq!(extracted.entities.len(), 7); // 7 user entities, excluding internal event bus
    }

    #[test]
    fn arpg_shrine_clash_scene_hydrates_and_extracts_combat_strands() {
        let scene = ScenePattern::arpg_shrine_clash("ARPG Test Clash");
        let mut world = World::new();
        let ids = scene.weave_into_world(&mut world);

        assert_eq!(ids.len(), 10); // 9 scene entities + 1 event bus
        assert_eq!(world.query::<HealthStrand>().count(), 3); // Hero + 2 enemies
        assert_eq!(world.query::<ActionStateStrand>().count(), 3);
        assert_eq!(world.query::<HurtboxStrand>().count(), 3);
        assert_eq!(world.query::<Mesh3D>().count(), 3); // Ground + Plateau + Voxel Shrine
        assert_eq!(world.query::<Sprite25D>().count(), 4); // Hero + 2 enemies + Tree

        // Extract back to ScenePattern
        let extracted = ScenePattern::extract_from_world(&world, "Extracted ARPG", "Extracted Clash");
        assert_eq!(extracted.entities.len(), 9);

        let hero = extracted.entities.iter().find(|e| e.name == "hero_wanderer").expect("Hero exists");
        assert!(hero.health.is_some());
        assert_eq!(hero.health.as_ref().unwrap().max, 100.0);
        assert!(hero.action_state.is_some());
        assert!(hero.hurtbox.is_some());
        assert_eq!(hero.hurtbox.as_ref().unwrap().team, 0);

        let stalker = extracted.entities.iter().find(|e| e.name == "shadow_stalker_alpha").expect("Enemy exists");
        assert!(stalker.health.is_some());
        assert_eq!(stalker.health.as_ref().unwrap().max, 60.0);
        assert!(stalker.hurtbox.is_some());
        assert_eq!(stalker.hurtbox.as_ref().unwrap().team, 1);
    }
}
