//! WGSL shaders for the Saan HD-2D & 3D Interlaced Render Pipeline.

/// Unified WGSL shader for Pass 2 (`OpaqueMesh3DPass`) and Pass 3 (`Sprite25DInterlacePass`).
///
/// Interweaves 3D geometry and procedural/textured 2.5D billboard sprites in the same
/// depth-buffered world with directional + point lighting and ground shadow projection.
pub const HD2D_INTERLACE_WGSL: &str = r#"
struct SceneUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir_intensity: vec4<f32>,
    sun_color: vec4<f32>,
    point_light_pos_radius: vec4<f32>,
    point_light_color_intensity: vec4<f32>,
    diorama_params: vec4<f32>, // x: focus_dist, y: aperture_blur, z: time, w: unused
};

@group(0) @binding(0)
var<uniform> scene: SceneUniforms;
@group(0) @binding(1)
var hero_sheet_tex: texture_2d<f32>;
@group(0) @binding(2)
var hero_sheet_sampler: sampler;

struct VertexInput {
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    /// kind: 0.0 = 3D Mesh, 1.0 = 2.5D Hero Sprite, 2.0 = 2.5D Tree Sprite, 3.0 = 2.5D Spirit Lantern, 4.0 = Ground Shadow
    @location(4) kind: f32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) kind: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = scene.view_proj * vec4<f32>(in.world_pos, 1.0);
    out.world_pos = in.world_pos;
    out.world_normal = normalize(in.world_normal);
    out.uv = in.uv;
    out.color = in.color;
    out.kind = in.kind;
    return out;
}

// Quantizes UV into crisp HD-2D pixel grid coordinates
fn pixel_uv(uv: vec2<f32>, grid: f32) -> vec2<f32> {
    return (floor(uv * grid) + vec2<f32>(0.5, 0.5)) / grid;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var base_rgba = in.color;
    var normal = normalize(in.world_normal);
    var emissive = vec3<f32>(0.0);

    // Pass 1/Shadow projection on the courtyard floor
    if (in.kind > 3.5) {
        let centered = (in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
        let r2 = dot(centered, centered);
        if (r2 > 1.0) {
            discard;
        }
        let alpha = (1.0 - r2) * 0.48;
        return vec4<f32>(0.03, 0.04, 0.06, alpha);
    }

    // Sampled RGBA8 Sprite Sheet + normal bevels for 2.5D Sprite Strands
    if (in.kind > 0.5 && in.kind < 1.5) {
        // 1.0: Wanderer Hero 2.5D Sprite sampled from 4-frame Flipbook Sprite Sheet texture!
        let sampled = textureSample(hero_sheet_tex, hero_sheet_sampler, in.uv);
        if (sampled.a < 0.5) {
            discard;
        }
        // Highlight glowing eyes on the sprite sheet
        if (sampled.r > 0.9 && sampled.g > 0.8) {
            emissive = vec3<f32>(0.6, 0.45, 0.15);
        }
        let local_u = fract(in.uv.x * 4.0);
        let nx = (local_u - 0.5) * 1.4;
        let ny = (0.5 - in.uv.y) * 0.6;
        normal = normalize(normal + vec3<f32>(nx, ny, 0.25));
        base_rgba = vec4<f32>(sampled.rgb * in.color.rgb, sampled.a * in.color.a);
    } else if (in.kind >= 1.5 && in.kind < 2.5) {
        // 2.0: Banyan Tree 2.5D Sprite (pixelated trunk + layered leafy canopy)
        let p = pixel_uv(in.uv, 32.0);
        let cx = p.x - 0.5;
        let cy = 1.0 - p.y; // 0 bottom, 1 top

        var alpha = 0.0;
        var col = vec3<f32>(0.2, 0.5, 0.25);

        let d1 = length(vec2<f32>(cx * 1.15, (cy - 0.68) * 1.35));
        let d2 = length(vec2<f32>((cx + 0.18) * 1.3, (cy - 0.56) * 1.5));
        let d3 = length(vec2<f32>((cx - 0.18) * 1.3, (cy - 0.58) * 1.5));

        if (d1 < 0.34 || d2 < 0.26 || d3 < 0.26) {
            alpha = 1.0;
            let shade = clamp((cy - 0.4) * 1.8 - abs(cx) * 0.9, 0.0, 1.0);
            col = mix(vec3<f32>(0.14, 0.29, 0.18), vec3<f32>(0.34, 0.58, 0.28), shade);
        } else if (cy < 0.52 && abs(cx) < (0.09 + (0.52 - cy) * 0.14)) {
            alpha = 1.0;
            col = vec3<f32>(0.36, 0.24, 0.16);
        }

        if (alpha < 0.5) {
            discard;
        }
        normal = normalize(normal + vec3<f32>(cx * 1.5, (cy - 0.5) * 1.2, 0.3));
        base_rgba = vec4<f32>(col * in.color.rgb, in.color.a);
    } else if (in.kind >= 2.5 && in.kind < 3.5) {
        // 3.0: Spirit Lantern Wisp (Spherical 2.5D billboard with warm core glow)
        let centered = (in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
        let r = length(centered);
        if (r > 1.0) {
            discard;
        }
        let glow = pow(1.0 - r, 1.8);
        emissive = in.color.rgb * (2.2 * glow);
        base_rgba = vec4<f32>(in.color.rgb, clamp(glow * 1.3, 0.2, 0.98));
    } else {
        // 0.0: 3D Mesh (subtle woven tile grid on courtyard stone & altar)
        let grid = abs(fract(in.world_pos.xz * 0.5) - vec2<f32>(0.5, 0.5));
        let line = min(grid.x, grid.y);
        let mortar = smoothstep(0.02, 0.06, line);
        base_rgba = vec4<f32>(in.color.rgb * (0.82 + 0.18 * mortar), in.color.a);
    }

    // 3D Directional Sun Lighting
    let sun_dir = normalize(-scene.sun_dir_intensity.xyz);
    let sun_ndl = max(dot(normal, sun_dir), 0.0);
    let ambient = vec3<f32>(0.22, 0.25, 0.32);
    var lit = base_rgba.rgb * (ambient + scene.sun_color.rgb * (sun_ndl * scene.sun_dir_intensity.w * 0.45));

    // 3D Point Light (Spirit Lantern illuminates both 3D ground/altar AND 2.5D sprites)
    let to_pt = scene.point_light_pos_radius.xyz - in.world_pos;
    let dist = length(to_pt);
    let radius = max(scene.point_light_pos_radius.w, 0.001);
    if (dist < radius) {
        let pt_dir = to_pt / max(dist, 0.0001);
        let atten = pow(1.0 - clamp(dist / radius, 0.0, 1.0), 2.0);
        let pt_ndl = max(dot(normal, pt_dir), 0.25); // wrap lighting for 2.5D cards
        lit += base_rgba.rgb * scene.point_light_color_intensity.rgb * (atten * pt_ndl * scene.point_light_color_intensity.w * 0.45);
    }

    lit += emissive;

    // Pass 4: Diorama Tilt-Shift Depth-of-Field & Vignette atmospheric desaturation
    let cam_dist = distance(scene.camera_pos.xyz, in.world_pos);
    let focus_dist = scene.diorama_params.x;
    let dof_strength = scene.diorama_params.y;
    let coc = clamp(abs(cam_dist - focus_dist) / 10.0, 0.0, 1.0) * dof_strength;
    let fog_color = vec3<f32>(0.14, 0.16, 0.24);
    let final_rgb = mix(lit, fog_color, coc * 0.55);

    return vec4<f32>(final_rgb, base_rgba.a);
}
"#;
