use std::fs;
use std::path::Path;

use glam::{Quat, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

use crate::ecs::{EntityId, World};
use crate::strands::{
    BillboardMode, CameraProjection, CameraRig, DirectionalLight, Material3D, Mesh3D,
    MeshPrimitive, NameStrand, PointLight, Sprite25D, Transform, Velocity,
};

/// Serializable blueprint of a single entity and its interwoven 2.5D/3D strands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityBlueprint {
    pub name: String,
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity: Option<Velocity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_25d: Option<Sprite25D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_3d: Option<Mesh3D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraRig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directional_light: Option<DirectionalLight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_light: Option<PointLight>,
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
                    sprite_25d: None,
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
                },
                EntityBlueprint {
                    name: "evening_sun".to_string(),
                    transform: Transform::from_xyz(10.0, 20.0, 8.0),
                    velocity: None,
                    sprite_25d: None,
                    mesh_3d: None,
                    camera: None,
                    directional_light: Some(DirectionalLight {
                        direction: Vec3::new(-0.45, -0.8, -0.35).normalize(),
                        color: Vec3::new(1.0, 0.88, 0.72),
                        intensity: 2.2,
                        casts_shadows: true,
                    }),
                    point_light: None,
                },
                EntityBlueprint {
                    name: "mossy_courtyard_ground".to_string(),
                    transform: Transform::from_xyz(0.0, 0.0, 0.0),
                    velocity: None,
                    sprite_25d: None,
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
                },
                EntityBlueprint {
                    name: "stone_altar_3d".to_string(),
                    transform: Transform::from_xyz(0.0, 0.75, -2.5),
                    velocity: None,
                    sprite_25d: None,
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
                },
                EntityBlueprint {
                    name: "wanderer_hero_25d".to_string(),
                    transform: Transform::from_xyz(0.0, 0.0, 1.5),
                    velocity: Some(Velocity {
                        linear: Vec3::new(0.5, 0.0, -0.25),
                        angular_y: 0.0,
                    }),
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/wanderer_idle.png".to_string(),
                        size: Vec2::new(1.2, 1.8),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/wanderer_idle_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::ONE,
                        depth_bias: 0.0,
                    }),
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: None,
                },
                EntityBlueprint {
                    name: "banyan_tree_canopy_25d".to_string(),
                    transform: Transform::from_xyz(-3.2, 0.0, -1.0),
                    velocity: None,
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/banyan_tree.png".to_string(),
                        size: Vec2::new(3.5, 4.2),
                        pivot: Vec2::new(0.5, 0.0),
                        billboard: BillboardMode::CylindricalY,
                        normal_map_id: Some("sprites/banyan_tree_n.png".to_string()),
                        casts_shadow: true,
                        tint: Vec4::ONE,
                        depth_bias: 0.0,
                    }),
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: None,
                },
                EntityBlueprint {
                    name: "spirit_lantern_25d".to_string(),
                    transform: Transform::from_xyz(1.8, 1.4, -1.2)
                        .with_rotation(Quat::IDENTITY),
                    velocity: None,
                    sprite_25d: Some(Sprite25D {
                        texture_id: "sprites/spirit_wisp.png".to_string(),
                        size: Vec2::new(0.6, 0.6),
                        pivot: Vec2::new(0.5, 0.5),
                        billboard: BillboardMode::Spherical,
                        normal_map_id: None,
                        casts_shadow: false,
                        tint: Vec4::new(1.0, 0.78, 0.35, 0.95),
                        depth_bias: 0.01,
                    }),
                    mesh_3d: None,
                    camera: None,
                    directional_light: None,
                    point_light: Some(PointLight {
                        color: Vec3::new(1.0, 0.72, 0.3),
                        intensity: 4.5,
                        radius: 6.0,
                    }),
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
            if let Some(ref sprite) = bp.sprite_25d {
                world.weave_strand(entity, sprite.clone());
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
            spawned.push(entity);
        }
        spawned
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

        assert_eq!(ids.len(), 7);
        assert_eq!(world.entity_count(), 7);
        assert_eq!(world.query::<Sprite25D>().count(), 3);
        assert_eq!(world.query::<Mesh3D>().count(), 2);
        assert_eq!(world.query::<CameraRig>().count(), 1);
    }
}
