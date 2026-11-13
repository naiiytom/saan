//! WGSL shaders for the Saan HD-2D & 3D Interlaced Render Pipeline.

/// Unified WGSL shader for Pass 1 (`ShadowMapPass`), Pass 2 (`OpaqueMesh3DPass`),
/// and Pass 3 (`Sprite25DInterlacePass`).
///
/// Interweaves 3D geometry and procedural/textured 2.5D billboard sprites in the same
/// depth-buffered world with directional + point lighting, PCF directional shadow maps,
/// dynamic Fog of War visibility, and tilt-shift diorama depth-of-field.
pub const HD2D_INTERLACE_WGSL: &str = r#"
struct SceneUniforms {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir_intensity: vec4<f32>,
    sun_color: vec4<f32>,
    point_lights_pos_radius: array<vec4<f32>, 4>,
    point_lights_color_intensity: array<vec4<f32>, 4>,
    diorama_params: vec4<f32>, // x: focus_dist, y: aperture_blur, z: time, w: bloom_strength
    fow_bounds: vec4<f32>,     // x: min_x, y: min_z, z: max_x, w: max_z
};

@group(0) @binding(0)
var<uniform> scene: SceneUniforms;

@group(1) @binding(0)
var hero_sheet_tex: texture_2d<f32>;
@group(1) @binding(1)
var hero_sheet_sampler: sampler;
@group(1) @binding(2)
var fow_tex: texture_2d<f32>;
@group(1) @binding(3)
var fow_sampler: sampler;
@group(1) @binding(4)
var shadow_tex: texture_depth_2d;
@group(1) @binding(5)
var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    /// kind: 0.0 = 3D Mesh, 1.0 = 2.5D Hero Sprite, 2.0 = 2.5D Tree Sprite, 3.0 = 2.5D Spirit Lantern/Particles, 4.0 = Ground Shadow
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
    // kind >= 5.0 is 2D NDC HUD overlay for Phase 6 Woven Inspector
    if (in.kind > 4.5) {
        out.clip_pos = vec4<f32>(in.world_pos.xy, 0.0, 1.0);
    } else {
        out.clip_pos = scene.view_proj * vec4<f32>(in.world_pos, 1.0);
    }
    out.world_pos = in.world_pos;
    out.world_normal = normalize(in.world_normal);
    out.uv = in.uv;
    out.color = in.color;
    out.kind = in.kind;
    return out;
}

@vertex
fn vs_shadow(in: VertexInput) -> @builtin(position) vec4<f32> {
    if (in.kind > 4.5) {
        return vec4<f32>(2.0, 2.0, 2.0, 1.0); // Cull HUD from shadow map
    }
    return scene.light_view_proj * vec4<f32>(in.world_pos, 1.0);
}

// Quantizes UV into crisp HD-2D pixel grid coordinates
fn pixel_uv(uv: vec2<f32>, grid: f32) -> vec2<f32> {
    return (floor(uv * grid) + vec2<f32>(0.5, 0.5)) / grid;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.kind > 4.5) {
        return in.color;
    }

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
        // 3.0: Spirit Lantern / Particles Wisp (Spherical 2.5D billboard with warm core glow)
        let centered = (in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
        let r = length(centered);
        if (r > 1.0) {
            discard;
        }
        let glow = pow(1.0 - r, 1.8);
        emissive = in.color.rgb * (2.2 * glow);
        base_rgba = vec4<f32>(in.color.rgb, clamp(glow * 1.3, 0.2, 0.98) * in.color.a);
    } else {
        // 0.0: 3D Mesh (subtle woven tile grid on courtyard stone & altar)
        let grid = abs(fract(in.world_pos.xz * 0.5) - vec2<f32>(0.5, 0.5));
        let line = min(grid.x, grid.y);
        let mortar = smoothstep(0.02, 0.06, line);
        base_rgba = vec4<f32>(in.color.rgb * (0.82 + 0.18 * mortar), in.color.a);
    }

    // Hardware Directional Depth Shadow mapping (PCF 4-tap)
    let light_clip = scene.light_view_proj * vec4<f32>(in.world_pos, 1.0);
    let shadow_coords = light_clip.xyz / light_clip.w;
    let shadow_uv = shadow_coords.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let current_depth = shadow_coords.z - 0.003;
    var shadow_factor = 1.0;

    if (shadow_uv.x >= 0.0 && shadow_uv.x <= 1.0 && shadow_uv.y >= 0.0 && shadow_uv.y <= 1.0 && current_depth <= 1.0) {
        let texel = 1.0 / 2048.0;
        var s = 0.0;
        s += textureSampleCompare(shadow_tex, shadow_sampler, shadow_uv + vec2<f32>(-texel, -texel), current_depth);
        s += textureSampleCompare(shadow_tex, shadow_sampler, shadow_uv + vec2<f32>(texel, -texel), current_depth);
        s += textureSampleCompare(shadow_tex, shadow_sampler, shadow_uv + vec2<f32>(-texel, texel), current_depth);
        s += textureSampleCompare(shadow_tex, shadow_sampler, shadow_uv + vec2<f32>(texel, texel), current_depth);
        shadow_factor = s * 0.25;
    }

    // 3D Directional Sun Lighting with hardware shadow
    let sun_dir = normalize(-scene.sun_dir_intensity.xyz);
    let sun_ndl = max(dot(normal, sun_dir), 0.0);
    let ambient = vec3<f32>(0.22, 0.25, 0.32);
    var lit = base_rgba.rgb * (ambient + scene.sun_color.rgb * (sun_ndl * scene.sun_dir_intensity.w * 0.45 * shadow_factor));

    // 3D Clustered Point Lights (up to 4 simultaneous dynamic point lights: lanterns, shrines, spell orbs)
    for (var i: u32 = 0u; i < 4u; i = i + 1u) {
        let pt_pos_r = scene.point_lights_pos_radius[i];
        let pt_col_i = scene.point_lights_color_intensity[i];
        let radius = pt_pos_r.w;
        if (radius > 0.001) {
            let to_pt = pt_pos_r.xyz - in.world_pos;
            let dist = length(to_pt);
            if (dist < radius) {
                let pt_dir = to_pt / max(dist, 0.0001);
                let atten = pow(1.0 - clamp(dist / radius, 0.0, 1.0), 2.0);
                let pt_ndl = max(dot(normal, pt_dir), 0.25); // wrap lighting for 2.5D cards
                lit += base_rgba.rgb * pt_col_i.rgb * (atten * pt_ndl * pt_col_i.w * 0.45);
            }
        }
    }

    // Subtle HD-2D Rim Lighting + Soft Bloom Highlight Boost
    let view_dir = normalize(scene.camera_pos.xyz - in.world_pos);
    let rim = pow(1.0 - max(dot(normal, view_dir), 0.0), 3.0) * 0.14;
    lit += scene.sun_color.rgb * (rim * shadow_factor) + emissive;
    let luma = dot(lit, vec3<f32>(0.2126, 0.7152, 0.0722));
    if (luma > 0.85) {
        lit += (lit - vec3<f32>(0.85)) * scene.diorama_params.w;
    }

    // Tactical Fog of War 2D Grid sampling
    let fow_uv = (in.world_pos.xz - scene.fow_bounds.xy) / (scene.fow_bounds.zw - scene.fow_bounds.xy);
    var fow_vis = 1.0;
    if (fow_uv.x >= 0.0 && fow_uv.x <= 1.0 && fow_uv.y >= 0.0 && fow_uv.y <= 1.0) {
        let sample_fow = textureSample(fow_tex, fow_sampler, fow_uv).r;
        fow_vis = mix(0.12, 1.0, sample_fow);
    }
    lit = lit * fow_vis;

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
