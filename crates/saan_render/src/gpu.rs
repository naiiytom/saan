//! GPU Geometry & `wgpu` Render Pipeline for the Saan HD-2D & 3D Render Loom.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use saan_asset::RgbaTexture;
use saan_core::{CameraProjection, MeshPrimitive};
use wgpu::util::DeviceExt;

use crate::shaders::HD2D_INTERLACE_WGSL;
use crate::{MeshDrawItem, RenderFrame, Sprite25DDrawItem};

/// Single interlaced vertex supporting both 3D meshes (`kind = 0.0`) and 2.5D sprites (`kind = 1..4`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InterlacedVertex {
    pub world_pos: [f32; 3],
    pub world_normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub kind: f32,
}

impl InterlacedVertex {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
        3 => Float32x4,
        4 => Float32,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InterlacedVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// Uniform buffer layout matching `SceneUniforms` in `HD2D_INTERLACE_WGSL`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuSceneUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub sun_dir_intensity: [f32; 4],
    pub sun_color: [f32; 4],
    pub point_light_pos_radius: [f32; 4],
    pub point_light_color_intensity: [f32; 4],
    pub diorama_params: [f32; 4],
}

impl GpuSceneUniforms {
    pub fn from_frame(frame: &RenderFrame, elapsed_secs: f32) -> Self {
        let (sun_dir, sun_col, sun_intensity) = frame
            .lighting
            .directional_lights
            .first()
            .map(|l| (l.direction, l.color, l.intensity))
            .unwrap_or((Vec3::new(-0.4, -0.8, -0.3).normalize(), Vec3::ONE, 1.5));

        let (pt_pos, pt_radius, pt_col, pt_intensity) = frame
            .lighting
            .point_lights
            .first()
            .map(|(pos, l)| (*pos, l.radius, l.color, l.intensity))
            .unwrap_or((Vec3::ZERO, 0.0, Vec3::ZERO, 0.0));

        let (focus_dist, aperture_blur) = match frame.camera.projection {
            CameraProjection::DioramaTilt {
                focus_distance,
                aperture_blur,
                ..
            } => (focus_distance, aperture_blur),
            _ => (12.0, 0.0),
        };

        Self {
            view_proj: frame.view_proj.to_cols_array_2d(),
            camera_pos: [frame.camera.eye.x, frame.camera.eye.y, frame.camera.eye.z, 1.0],
            sun_dir_intensity: [sun_dir.x, sun_dir.y, sun_dir.z, sun_intensity],
            sun_color: [sun_col.x, sun_col.y, sun_col.z, 1.0],
            point_light_pos_radius: [pt_pos.x, pt_pos.y, pt_pos.z, pt_radius],
            point_light_color_intensity: [pt_col.x, pt_col.y, pt_col.z, pt_intensity],
            diorama_params: [focus_dist, aperture_blur, elapsed_secs, 0.0],
        }
    }
}

/// Builds the vertex arrays for the 3D opaque pass and the 2.5D sprite + shadow interlace pass.
pub fn tessellate_frame(frame: &RenderFrame) -> (Vec<InterlacedVertex>, Vec<InterlacedVertex>) {
    let mut mesh_vertices = Vec::new();
    for item in &frame.meshes_3d {
        append_mesh_primitive(&mut mesh_vertices, item);
    }

    let mut sprite_vertices = Vec::new();

    // Pass 1/Shadow projection: ground contact shadow blobs under shadow-casting 2.5D sprites & 3D props
    for sprite in &frame.sprites_25d {
        if sprite.casts_shadow {
            append_ground_shadow_quad(&mut sprite_vertices, sprite);
        }
    }

    // Pass 3: Back-to-front depth-sorted 2.5D billboard sprites
    for sprite in &frame.sprites_25d {
        append_sprite_25d_quad(&mut sprite_vertices, sprite);
    }

    (mesh_vertices, sprite_vertices)
}

fn push_quad(
    out: &mut Vec<InterlacedVertex>,
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    p3: Vec3,
    normal: Vec3,
    color: [f32; 4],
    kind: f32,
) {
    push_quad_uv(out, p0, p1, p2, p3, [0.0, 0.0, 1.0, 1.0], normal, color, kind);
}

#[allow(clippy::too_many_arguments)]
fn push_quad_uv(
    out: &mut Vec<InterlacedVertex>,
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    p3: Vec3,
    uv_rect: [f32; 4],
    normal: Vec3,
    color: [f32; 4],
    kind: f32,
) {
    let [u0, v0, u1, v1] = uv_rect;
    let n = normal.to_array();
    let v_bl = InterlacedVertex {
        world_pos: p0.to_array(),
        world_normal: n,
        uv: [u0, v1],
        color,
        kind,
    };
    let v_br = InterlacedVertex {
        world_pos: p1.to_array(),
        world_normal: n,
        uv: [u1, v1],
        color,
        kind,
    };
    let v_tr = InterlacedVertex {
        world_pos: p2.to_array(),
        world_normal: n,
        uv: [u1, v0],
        color,
        kind,
    };
    let v_tl = InterlacedVertex {
        world_pos: p3.to_array(),
        world_normal: n,
        uv: [u0, v0],
        color,
        kind,
    };
    out.extend_from_slice(&[v_bl, v_br, v_tr, v_bl, v_tr, v_tl]);
}

fn append_mesh_primitive(out: &mut Vec<InterlacedVertex>, item: &MeshDrawItem) {
    let m = item.world_matrix;
    let color = item.base_color.to_array();

    match item.primitive {
        MeshPrimitive::GroundPlane { width, depth } => {
            let hw = width * 0.5;
            let hd = depth * 0.5;
            let p0 = m.transform_point3(Vec3::new(-hw, 0.0, hd));
            let p1 = m.transform_point3(Vec3::new(hw, 0.0, hd));
            let p2 = m.transform_point3(Vec3::new(hw, 0.0, -hd));
            let p3 = m.transform_point3(Vec3::new(-hw, 0.0, -hd));
            push_quad(out, p0, p1, p2, p3, Vec3::Y, color, 0.0);
        }
        MeshPrimitive::Cube { size } => {
            let h = size * 0.5;
            let faces: [([Vec3; 4], Vec3); 6] = [
                // Front (+Z)
                (
                    [
                        Vec3::new(-h.x, -h.y, h.z),
                        Vec3::new(h.x, -h.y, h.z),
                        Vec3::new(h.x, h.y, h.z),
                        Vec3::new(-h.x, h.y, h.z),
                    ],
                    Vec3::Z,
                ),
                // Back (-Z)
                (
                    [
                        Vec3::new(h.x, -h.y, -h.z),
                        Vec3::new(-h.x, -h.y, -h.z),
                        Vec3::new(-h.x, h.y, -h.z),
                        Vec3::new(h.x, h.y, -h.z),
                    ],
                    Vec3::NEG_Z,
                ),
                // Top (+Y)
                (
                    [
                        Vec3::new(-h.x, h.y, h.z),
                        Vec3::new(h.x, h.y, h.z),
                        Vec3::new(h.x, h.y, -h.z),
                        Vec3::new(-h.x, h.y, -h.z),
                    ],
                    Vec3::Y,
                ),
                // Bottom (-Y)
                (
                    [
                        Vec3::new(-h.x, -h.y, -h.z),
                        Vec3::new(h.x, -h.y, -h.z),
                        Vec3::new(h.x, -h.y, h.z),
                        Vec3::new(-h.x, -h.y, h.z),
                    ],
                    Vec3::NEG_Y,
                ),
                // Right (+X)
                (
                    [
                        Vec3::new(h.x, -h.y, h.z),
                        Vec3::new(h.x, -h.y, -h.z),
                        Vec3::new(h.x, h.y, -h.z),
                        Vec3::new(h.x, h.y, h.z),
                    ],
                    Vec3::X,
                ),
                // Left (-X)
                (
                    [
                        Vec3::new(-h.x, -h.y, -h.z),
                        Vec3::new(-h.x, -h.y, h.z),
                        Vec3::new(-h.x, h.y, h.z),
                        Vec3::new(-h.x, h.y, -h.z),
                    ],
                    Vec3::NEG_X,
                ),
            ];

            for (corners, local_n) in faces {
                let p0 = m.transform_point3(corners[0]);
                let p1 = m.transform_point3(corners[1]);
                let p2 = m.transform_point3(corners[2]);
                let p3 = m.transform_point3(corners[3]);
                let world_n = m.transform_vector3(local_n).normalize_or_zero();
                push_quad(out, p0, p1, p2, p3, world_n, color, 0.0);
            }
        }
        MeshPrimitive::Asset { .. } => {
            let h = Vec3::splat(0.5);
            let p0 = m.transform_point3(Vec3::new(-h.x, 0.0, h.z));
            let p1 = m.transform_point3(Vec3::new(h.x, 0.0, h.z));
            let p2 = m.transform_point3(Vec3::new(h.x, 1.0, h.z));
            let p3 = m.transform_point3(Vec3::new(-h.x, 1.0, h.z));
            push_quad(out, p0, p1, p2, p3, Vec3::Z, color, 0.0);
        }
    }
}

fn append_sprite_25d_quad(out: &mut Vec<InterlacedVertex>, sprite: &Sprite25DDrawItem) {
    let m = sprite.world_matrix;
    let px = sprite.pivot.x;
    let py = sprite.pivot.y;

    // Local card coordinates anchored around `pivot`
    let x0 = -px;
    let x1 = 1.0 - px;
    let y0 = -py;
    let y1 = 1.0 - py;

    let p0 = m.transform_point3(Vec3::new(x0, y0, 0.0));
    let p1 = m.transform_point3(Vec3::new(x1, y0, 0.0));
    let p2 = m.transform_point3(Vec3::new(x1, y1, 0.0));
    let p3 = m.transform_point3(Vec3::new(x0, y1, 0.0));

    let kind = if sprite.label.contains("tree") || sprite.texture_id.contains("tree") {
        2.0
    } else if sprite.label.contains("lantern") || sprite.label.contains("spirit") {
        3.0
    } else {
        1.0
    };

    push_quad_uv(
        out,
        p0,
        p1,
        p2,
        p3,
        sprite.uv_rect,
        sprite.world_normal,
        sprite.tint.to_array(),
        kind,
    );
}

fn append_ground_shadow_quad(out: &mut Vec<InterlacedVertex>, sprite: &Sprite25DDrawItem) {
    let center = sprite.world_matrix.transform_point3(Vec3::ZERO);
    let rx = sprite.size.x * 0.48;
    let rz = sprite.size.x * 0.35;
    let y = 0.015; // Slightly above ground plane to prevent Z-fighting
    let offset_x = 0.25; // Skewed away from evening sun

    let p0 = Vec3::new(center.x - rx + offset_x, y, center.z + rz);
    let p1 = Vec3::new(center.x + rx + offset_x, y, center.z + rz);
    let p2 = Vec3::new(center.x + rx + offset_x, y, center.z - rz);
    let p3 = Vec3::new(center.x - rx + offset_x, y, center.z - rz);

    push_quad(out, p0, p1, p2, p3, Vec3::Y, [0.0, 0.0, 0.0, 0.5], 4.0);
}

/// Active `wgpu` state executing the Saan 2.5D + 3D interlaced pipeline.
pub struct GpuRenderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_view: wgpu::TextureView,
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,
    pub opaque_3d_pipeline: wgpu::RenderPipeline,
    pub interlace_25d_pipeline: wgpu::RenderPipeline,
}

impl GpuRenderer {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(window: std::sync::Arc<winit::window::Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found for wgpu"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Saan HD-2D Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_view = Self::create_depth_view(&device, width, height);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Saan HD2D Interlace Shader"),
            source: wgpu::ShaderSource::Wgsl(HD2D_INTERLACE_WGSL.into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SceneUniforms Buffer"),
            size: std::mem::size_of::<GpuSceneUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Upload the 4-frame RGBA8 Wanderer Hero sprite sheet texture via saan_asset
        let sheet_img = RgbaTexture::procedural_wanderer_sheet();
        let sheet_size = wgpu::Extent3d {
            width: sheet_img.width,
            height: sheet_img.height,
            depth_or_array_layers: 1,
        };
        let sheet_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Wanderer Flipbook Sprite Sheet"),
            size: sheet_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &sheet_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &sheet_img.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * sheet_img.width),
                rows_per_image: Some(sheet_img.height),
            },
            sheet_size,
        );
        let sheet_view = sheet_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let pixel_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Crisp HD-2D Nearest Pixel Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SceneUniforms + SpriteSheet Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SceneUniforms BindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&sheet_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&pixel_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Saan HD-2D Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Pass 2: Opaque 3D Mesh Pipeline (depth write ON, cull backfaces)
        let opaque_3d_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("2_OpaqueMesh3DPass Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[InterlacedVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Pass 3: 2.5D Sprite Interlace Pipeline (alpha blending + depth testing against 3D world)
        let interlace_25d_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("3_Sprite25DInterlacePass Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[InterlacedVertex::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Self::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            depth_view,
            uniform_buffer,
            uniform_bind_group,
            opaque_3d_pipeline,
            interlace_25d_pipeline,
        })
    }

    fn create_depth_view(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Saan Interlaced Depth Buffer"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = Self::create_depth_view(&self.device, width, height);
        }
    }

    pub fn render(&mut self, frame: &RenderFrame, elapsed_secs: f32) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let uniforms = GpuSceneUniforms::from_frame(frame, elapsed_secs);
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let (mesh_verts, sprite_verts) = tessellate_frame(frame);

        let mesh_vb = (!mesh_verts.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("3D Mesh Vertex Buffer"),
                    contents: bytemuck::cast_slice(&mesh_verts),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });

        let sprite_vb = (!sprite_verts.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("2.5D Sprite Vertex Buffer"),
                    contents: bytemuck::cast_slice(&sprite_verts),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Saan Frame Command Encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Saan HD-2D Unified Depth Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.04,
                            g: 0.05,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Pass 2: Draw 3D Opaque Geometry
            if let Some(ref vb) = mesh_vb {
                rpass.set_pipeline(&self.opaque_3d_pipeline);
                rpass.set_vertex_buffer(0, vb.slice(..));
                rpass.draw(0..mesh_verts.len() as u32, 0..1);
            }

            // Pass 1 (ground shadows) + Pass 3 (depth-sorted 2.5D sprites) + Pass 4 (Diorama DOF shader)
            if let Some(ref vb) = sprite_vb {
                rpass.set_pipeline(&self.interlace_25d_pipeline);
                rpass.set_vertex_buffer(0, vb.slice(..));
                rpass.draw(0..sprite_verts.len() as u32, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderLoom;
    use saan_core::{ScenePattern, World};

    #[test]
    fn tessellates_3d_meshes_and_25d_sprites_with_shadows() {
        let pattern = ScenePattern::default_hd2d_diorama("Tessellation Test");
        let mut world = World::new();
        pattern.weave_into_world(&mut world);

        let frame = RenderLoom::hd2d_pipeline().extract_frame(&world);
        let (mesh_verts, sprite_verts) = tessellate_frame(&frame);

        // 1 GroundPlane (6 verts) + 1 Cube (36 verts) = 42 vertices
        assert_eq!(mesh_verts.len(), 42);
        // 2 shadow quads (12 verts) + 3 sprite quads (18 verts) = 30 vertices
        assert_eq!(sprite_verts.len(), 30);
    }
}
