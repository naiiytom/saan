//! # `saan_render` — The Interlaced 2.5D & 3D Render Loom
//!
//! Extracts [`Mesh3D`] and [`Sprite25D`] strands from [`saan_core::World`], computes
//! camera-aware 2.5D billboard transforms, sorts and interlaces depth, and schedules
//! the 4-stage HD-2D render graph:
//! `ShadowMapPass` -> `OpaqueMesh3DPass` -> `Sprite25DInterlacePass` -> `DioramaPostPass`.

pub mod gpu;
pub mod shaders;
pub mod window;

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use saan_core::{
    BillboardMode, CameraProjection, CameraRig, DirectionalLight, EntityId, Mesh3D, MeshPrimitive,
    NameStrand, PointLight, Sprite25D, Transform, World,
};
use serde::{Deserialize, Serialize};

/// A prepared 3D mesh draw item in the current frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshDrawItem {
    pub entity: EntityId,
    pub label: String,
    pub primitive: MeshPrimitive,
    pub world_matrix: Mat4,
    pub base_color: Vec4,
    pub roughness: f32,
    pub casts_shadow: bool,
    pub receives_shadow: bool,
    pub material_name: String,
}

/// A prepared 2.5D sprite draw item, with its camera-interlaced billboard transform and view depth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprite25DDrawItem {
    pub entity: EntityId,
    pub label: String,
    pub texture_id: String,
    pub billboard_mode: BillboardMode,
    pub size: Vec2,
    pub pivot: Vec2,
    pub tint: Vec4,
    pub uv_rect: [f32; 4],
    pub world_matrix: Mat4,
    pub world_normal: Vec3,
    pub camera_distance: f32,
    pub casts_shadow: bool,
    pub has_normal_map: bool,
}

/// Active lighting environment extracted for the frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameLighting {
    pub directional_lights: Vec<DirectionalLight>,
    pub point_lights: Vec<(Vec3, PointLight)>,
}

/// Complete extracted render frame uniting 3D geometry, depth-sorted 2.5D sprites, lights, and camera.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderFrame {
    pub camera: CameraRig,
    pub view_proj: Mat4,
    pub meshes_3d: Vec<MeshDrawItem>,
    /// 2.5D sprites sorted back-to-front by camera distance (with depth bias applied) for clean alpha/depth interlacing.
    pub sprites_25d: Vec<Sprite25DDrawItem>,
    pub shadow_casters_count: usize,
    pub lighting: FrameLighting,
    pub diorama_dof_enabled: bool,
    pub pass_order: Vec<String>,
}

/// Node in the Render Graph ("Render Loom").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPassNode {
    pub name: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// The Render Loom: manages the DAG of render passes that interweave 3D meshes and 2.5D sprites.
pub struct RenderLoom {
    graph: DiGraph<RenderPassNode, ()>,
}

impl Default for RenderLoom {
    fn default() -> Self {
        Self::hd2d_pipeline()
    }
}

impl RenderLoom {
    /// Constructs the canonical 4-pass HD-2D & Diorama render pipeline DAG.
    pub fn hd2d_pipeline() -> Self {
        let mut graph = DiGraph::new();

        let shadow = graph.add_node(RenderPassNode {
            name: "1_ShadowMapPass (3D Meshes + 2.5D Sprite Casters)".to_string(),
            inputs: vec!["LightSpaceCascades".to_string()],
            outputs: vec!["ShadowAtlasDepth".to_string()],
        });

        let opaque_3d = graph.add_node(RenderPassNode {
            name: "2_OpaqueMesh3DPass (Terrain & 3D Diorama Props)".to_string(),
            inputs: vec!["ShadowAtlasDepth".to_string()],
            outputs: vec!["SceneColorHDR".to_string(), "SceneDepthBuffer".to_string()],
        });

        let interlace_25d = graph.add_node(RenderPassNode {
            name: "3_Sprite25DInterlacePass (Billboarded & Normal-Lit 2.5D Sprites)".to_string(),
            inputs: vec![
                "ShadowAtlasDepth".to_string(),
                "SceneColorHDR".to_string(),
                "SceneDepthBuffer".to_string(),
            ],
            outputs: vec!["InterlacedColorHDR".to_string(), "InterlacedDepth".to_string()],
        });

        let diorama_post = graph.add_node(RenderPassNode {
            name: "4_DioramaPostPass (Tilt-Shift DOF, Bloom & Tonemap)".to_string(),
            inputs: vec!["InterlacedColorHDR".to_string(), "InterlacedDepth".to_string()],
            outputs: vec!["SwapchainFrame".to_string()],
        });

        graph.add_edge(shadow, opaque_3d, ());
        graph.add_edge(shadow, interlace_25d, ());
        graph.add_edge(opaque_3d, interlace_25d, ());
        graph.add_edge(interlace_25d, diorama_post, ());

        Self { graph }
    }

    /// Returns the topologically sorted render pass names.
    pub fn pass_schedule(&self) -> Vec<String> {
        toposort(&self.graph, None)
            .expect("RenderLoom graph must be a valid DAG")
            .into_iter()
            .map(|idx| self.graph[idx].name.clone())
            .collect()
    }

    /// Extracts and interlaces a [`RenderFrame`] from the current [`World`] state.
    pub fn extract_frame(&self, world: &World) -> RenderFrame {
        let camera = world
            .query::<CameraRig>()
            .next()
            .map(|(_, &cam)| cam)
            .unwrap_or_default();

        let view_proj = camera.view_projection_matrix();

        // Extract 3D meshes
        let mut meshes_3d = Vec::new();
        for (entity, mesh) in world.query::<Mesh3D>() {
            let transform = world
                .get_strand::<Transform>(entity)
                .copied()
                .unwrap_or_default();
            let label = world
                .get_strand::<NameStrand>(entity)
                .map(|n| n.0.clone())
                .unwrap_or_else(|| entity.to_string());

            meshes_3d.push(MeshDrawItem {
                entity,
                label,
                primitive: mesh.primitive.clone(),
                world_matrix: transform.matrix(),
                base_color: mesh.material.base_color,
                roughness: mesh.material.roughness,
                casts_shadow: mesh.casts_shadow,
                receives_shadow: mesh.receives_shadow,
                material_name: mesh.material.name.clone(),
            });
        }

        // Extract and orient 2.5D sprites in 3D space
        let mut sprites_25d = Vec::new();
        for (entity, sprite) in world.query::<Sprite25D>() {
            let transform = world
                .get_strand::<Transform>(entity)
                .copied()
                .unwrap_or_default();
            let label = world
                .get_strand::<NameStrand>(entity)
                .map(|n| n.0.clone())
                .unwrap_or_else(|| entity.to_string());

            let (world_matrix, world_normal) =
                compute_sprite_25d_transform(&transform, sprite, &camera);
            let raw_dist = transform.translation.distance(camera.eye);
            let camera_distance = (raw_dist - sprite.depth_bias).max(0.0);

            sprites_25d.push(Sprite25DDrawItem {
                entity,
                label,
                texture_id: sprite.texture_id.clone(),
                billboard_mode: sprite.billboard,
                size: sprite.size,
                pivot: sprite.pivot,
                tint: sprite.tint,
                uv_rect: sprite.uv_rect,
                world_matrix,
                world_normal,
                camera_distance,
                casts_shadow: sprite.casts_shadow,
                has_normal_map: sprite.normal_map_id.is_some(),
            });
        }

        // Sort 2.5D sprites back-to-front (largest distance first) for proper translucent interlacing
        sprites_25d.sort_by(|a, b| {
            b.camera_distance
                .partial_cmp(&a.camera_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let shadow_casters_count = meshes_3d.iter().filter(|m| m.casts_shadow).count()
            + sprites_25d.iter().filter(|s| s.casts_shadow).count();

        let directional_lights = world
            .query::<DirectionalLight>()
            .map(|(_, &l)| l)
            .collect();

        let point_lights = world
            .query::<PointLight>()
            .map(|(e, &l)| {
                let pos = world
                    .get_strand::<Transform>(e)
                    .map(|t| t.translation)
                    .unwrap_or(Vec3::ZERO);
                (pos, l)
            })
            .collect();

        let diorama_dof_enabled = matches!(camera.projection, CameraProjection::DioramaTilt { .. });

        RenderFrame {
            camera,
            view_proj,
            meshes_3d,
            sprites_25d,
            shadow_casters_count,
            lighting: FrameLighting {
                directional_lights,
                point_lights,
            },
            diorama_dof_enabled,
            pass_order: self.pass_schedule(),
        }
    }
}

/// Computes the 4x4 world matrix and surface normal for a [`Sprite25D`] card based on its [`BillboardMode`].
pub fn compute_sprite_25d_transform(
    transform: &Transform,
    sprite: &Sprite25D,
    camera: &CameraRig,
) -> (Mat4, Vec3) {
    let rotation = match sprite.billboard {
        BillboardMode::WorldPlane => transform.rotation,
        BillboardMode::CylindricalY => {
            let mut to_cam = camera.eye - transform.translation;
            to_cam.y = 0.0;
            if to_cam.length_squared() > 1e-6 {
                let forward = to_cam.normalize();
                let yaw = forward.x.atan2(forward.z);
                Quat::from_rotation_y(yaw)
            } else {
                transform.rotation
            }
        }
        BillboardMode::Spherical => {
            let to_cam = (camera.eye - transform.translation).normalize_or_zero();
            if to_cam.length_squared() > 1e-6 {
                Quat::from_rotation_arc(Vec3::Z, to_cam)
            } else {
                transform.rotation
            }
        }
    };

    let scale = Vec3::new(
        transform.scale.x * sprite.size.x,
        transform.scale.y * sprite.size.y,
        1.0,
    );
    let normal = (rotation * Vec3::Z).normalize_or_zero();
    let matrix = Mat4::from_scale_rotation_translation(scale, rotation, transform.translation);
    (matrix, normal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use saan_core::ScenePattern;

    #[test]
    fn extracts_and_depth_sorts_hd2d_scene() {
        let pattern = ScenePattern::default_hd2d_diorama("Unit Test Scene");
        let mut world = World::new();
        pattern.weave_into_world(&mut world);

        let render_loom = RenderLoom::hd2d_pipeline();
        let frame = render_loom.extract_frame(&world);

        assert_eq!(frame.pass_order.len(), 4);
        assert_eq!(frame.meshes_3d.len(), 2);
        assert_eq!(frame.sprites_25d.len(), 3);
        assert!(frame.diorama_dof_enabled);
        assert_eq!(frame.shadow_casters_count, 3); // 1 3D altar + 2 shadow-casting 2.5D sprites

        // Verify back-to-front sorting of 2.5D sprites
        for pair in frame.sprites_25d.windows(2) {
            assert!(pair[0].camera_distance >= pair[1].camera_distance);
        }
    }
}
