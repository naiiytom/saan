//! # `saan_asset` — The Shaving Layer (Asset Shavers)
//!
//! Parses and prepares raw 2.5D and 3D assets into ready-to-weave engine data:
//! - **Aseprite JSON Sprite Sheet Shaver (`AsepriteSheet`)**: Extracts frame rectangles, durations, and `FlipbookStrand` animations.
//! - **OBJ / 3D Mesh Shaver (`MeshAssetData`)**: Parses 3D vertex/normal/UV geometry from Wavefront `.obj` or embedded mesh descriptors.
//! - **Procedural Sprite Sheet Generator (`RgbaTexture`)**: Generates crisp RGBA8 pixel-art sprite sheets for HD-2D testing and fallback rendering.

pub mod vox;
pub use vox::{get_or_load_vox_mesh, VoxModel, VoxelPoint};

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use glam::{Vec2, Vec3};
use saan_core::FlipbookStrand;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("I/O error loading asset: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error in asset: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid 3D mesh format: {0}")]
    InvalidMesh(String),
    #[error("Invalid MagicaVoxel .vox format: {0}")]
    InvalidVox(String),
}

/// Raw RGBA8 texture data ready for GPU upload.
#[derive(Debug, Clone)]
pub struct RgbaTexture {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RgbaTexture {
    /// Generates a 4-frame (64x24) HD-2D Wanderer Hero walking sprite sheet in RGBA8 format.
    pub fn procedural_wanderer_sheet() -> Self {
        let frame_w = 16u32;
        let frame_h = 24u32;
        let frames = 4u32;
        let width = frame_w * frames;
        let height = frame_h;
        let mut pixels = vec![0u8; (width * height * 4) as usize];

        for frame in 0..frames {
            let bob = if frame % 2 == 1 { 1i32 } else { 0i32 };
            let stride = match frame {
                0 => -1i32,
                1 => 0i32,
                2 => 1i32,
                _ => 0i32,
            };

            for py in 0..frame_h {
                for px in 0..frame_w {
                    let y = (frame_h - 1 - py) as i32 - bob;
                    let x = px as f32;

                    let mut rgba = [0u8, 0, 0, 0];

                    // Woven bamboo hat
                    if (17..=20).contains(&y) && (x - 7.5).abs() <= (21.0 - y as f32) * 1.6 {
                        let checker = (px + py) % 2 == 0;
                        rgba = if checker {
                            [194, 148, 82, 255]
                        } else {
                            [158, 115, 61, 255]
                        };
                    } else if (14..17).contains(&y) && (x - 7.5).abs() <= 2.5 {
                        // Head & glowing eyes
                        if y == 15 && (px == 6 || px == 9) {
                            rgba = [245, 218, 115, 255];
                        } else {
                            rgba = [56, 46, 61, 255];
                        }
                    } else if (4..14).contains(&y)
                        && (x - 7.5).abs() <= (4.5 - (y - 4) as f32 * 0.15)
                    {
                        // Cloak & red woven sash
                        rgba = if (8..=9).contains(&y) {
                            [199, 71, 56, 255]
                        } else {
                            [46, 107, 133, 255]
                        };
                    } else if (0..4).contains(&y) {
                        // Walking boots with frame-dependent stride offset
                        let left_foot = 5.5 + stride as f32;
                        let right_foot = 9.5 - stride as f32;
                        if (x - left_foot).abs() <= 1.2 || (x - right_foot).abs() <= 1.2 {
                            rgba = [61, 41, 31, 255];
                        }
                    }

                    let out_x = frame * frame_w + px;
                    let idx = ((py * width + out_x) * 4) as usize;
                    pixels[idx..idx + 4].copy_from_slice(&rgba);
                }
            }
        }

        Self {
            width,
            height,
            pixels,
        }
    }
}

/// Rectangle inside a sprite sheet texture (in pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Frame entry in an Aseprite JSON spritesheet export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsepriteFrame {
    pub frame: PixelRect,
    #[serde(default = "default_duration_ms")]
    pub duration: u32,
}

fn default_duration_ms() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsepriteSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsepriteFrameTag {
    pub name: String,
    pub from: u32,
    pub to: u32,
    #[serde(default)]
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsepriteMeta {
    pub image: String,
    pub size: AsepriteSize,
    #[serde(default, rename = "frameTags")]
    pub frame_tags: Vec<AsepriteFrameTag>,
}

/// Parsed Aseprite sprite sheet descriptor (`*.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsepriteSheet {
    pub frames: BTreeMap<String, AsepriteFrame>,
    pub meta: AsepriteMeta,
}

impl AsepriteSheet {
    /// Parses an Aseprite JSON hash export from a string.
    pub fn from_json_str(json: &str) -> Result<Self, AssetError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Loads an Aseprite JSON spritesheet from disk.
    pub fn load_from_file(path: &Path) -> Result<Self, AssetError> {
        let raw = fs::read_to_string(path)?;
        Self::from_json_str(&raw)
    }

    /// Converts an Aseprite animation tag (or the full sheet if `tag_name` is `None`) into a [`FlipbookStrand`].
    pub fn to_flipbook_strand(&self, tag_name: Option<&str>) -> Option<FlipbookStrand> {
        let first_frame = self.frames.values().next()?;
        let fw = first_frame.frame.w.max(1);
        let fh = first_frame.frame.h.max(1);
        let columns = (self.meta.size.w / fw).max(1);
        let rows = (self.meta.size.h / fh).max(1);
        let total_frames = (self.frames.len() as u32).max(1);

        let (start_frame, end_frame) = if let Some(tag) = tag_name {
            let found = self.meta.frame_tags.iter().find(|t| t.name == tag)?;
            (found.from, found.to)
        } else {
            (0, total_frames - 1)
        };

        let avg_ms = self
            .frames
            .values()
            .map(|f| f.duration as f32)
            .sum::<f32>()
            / total_frames as f32;
        let fps = (1000.0 / avg_ms.max(1.0)).clamp(1.0, 60.0);

        Some(FlipbookStrand {
            columns,
            rows,
            start_frame,
            end_frame,
            fps,
            current_frame: start_frame,
            timer: 0.0,
            looping: true,
        })
    }
}

pub fn default_vertex_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

/// 3D vertex parsed by the 3D Mesh Shaver or Greedy Voxel Mesher.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShavedVertex3D {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    #[serde(default = "default_vertex_color")]
    pub color: [f32; 4],
}

/// Loaded 3D mesh geometry ready for tessellation in `saan_render`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeshAssetData {
    pub name: String,
    pub vertices: Vec<ShavedVertex3D>,
}

impl MeshAssetData {
    /// Parses a Wavefront `.obj` 3D mesh string into triangle vertices (`v`, `vn`, `vt`, `f`).
    pub fn from_obj_str(name: impl Into<String>, obj_source: &str) -> Result<Self, AssetError> {
        let mut positions: Vec<Vec3> = Vec::new();
        let mut normals: Vec<Vec3> = Vec::new();
        let mut uvs: Vec<Vec2> = Vec::new();
        let mut vertices: Vec<ShavedVertex3D> = Vec::new();

        for line in obj_source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            match parts.next() {
                Some("v") => {
                    let x: f32 = parts
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| AssetError::InvalidMesh("Missing v.x".into()))?;
                    let y: f32 = parts
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| AssetError::InvalidMesh("Missing v.y".into()))?;
                    let z: f32 = parts
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| AssetError::InvalidMesh("Missing v.z".into()))?;
                    positions.push(Vec3::new(x, y, z));
                }
                Some("vn") => {
                    let x: f32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let y: f32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
                    let z: f32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    normals.push(Vec3::new(x, y, z).normalize_or_zero());
                }
                Some("vt") => {
                    let u: f32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let v: f32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    uvs.push(Vec2::new(u, v));
                }
                Some("f") => {
                    let mut face_verts = Vec::new();
                    for token in parts {
                        let mut indices = token.split('/');
                        let vi: usize = indices
                            .next()
                            .and_then(|s| s.parse().ok())
                            .ok_or_else(|| AssetError::InvalidMesh("Invalid face vertex".into()))?;
                        let ti: Option<usize> = indices.next().and_then(|s| s.parse().ok());
                        let ni: Option<usize> = indices.next().and_then(|s| s.parse().ok());

                        let pos = *positions.get(vi.saturating_sub(1)).ok_or_else(|| {
                            AssetError::InvalidMesh(format!("Vertex index {vi} out of bounds"))
                        })?;
                        let uv = ti
                            .and_then(|idx| uvs.get(idx.saturating_sub(1)).copied())
                            .unwrap_or(Vec2::ZERO);
                        let normal = ni
                            .and_then(|idx| normals.get(idx.saturating_sub(1)).copied())
                            .unwrap_or(Vec3::Y);

                        face_verts.push(ShavedVertex3D {
                            position: pos,
                            normal,
                            uv,
                            color: [1.0, 1.0, 1.0, 1.0],
                        });
                    }

                    // Fan triangulation for triangles & quads
                    for i in 1..face_verts.len().saturating_sub(1) {
                        vertices.push(face_verts[0]);
                        vertices.push(face_verts[i]);
                        vertices.push(face_verts[i + 1]);
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            name: name.into(),
            vertices,
        })
    }

    /// Parses a glTF 2.0 JSON (`.gltf`) asset with embedded data-URI base64 buffers into triangle vertices.
    pub fn from_gltf_str(name: impl Into<String>, gltf_json: &str) -> Result<Self, AssetError> {
        #[derive(Deserialize)]
        struct GltfRoot {
            #[serde(default)]
            buffers: Vec<GltfBuffer>,
            #[serde(default, rename = "bufferViews")]
            buffer_views: Vec<GltfBufferView>,
            #[serde(default)]
            accessors: Vec<GltfAccessor>,
            #[serde(default)]
            meshes: Vec<GltfMesh>,
        }
        #[derive(Deserialize)]
        struct GltfBuffer {
            #[serde(default)]
            uri: Option<String>,
        }
        #[derive(Deserialize)]
        struct GltfBufferView {
            buffer: usize,
            #[serde(default, rename = "byteOffset")]
            byte_offset: usize,
            #[serde(rename = "byteLength")]
            byte_length: usize,
            #[serde(default, rename = "byteStride")]
            byte_stride: Option<usize>,
        }
        #[derive(Deserialize)]
        struct GltfAccessor {
            #[serde(rename = "bufferView")]
            buffer_view: usize,
            #[serde(default, rename = "byteOffset")]
            byte_offset: usize,
            #[serde(rename = "componentType")]
            component_type: u32,
            count: usize,
        }
        #[derive(Deserialize)]
        struct GltfMesh {
            #[serde(default)]
            primitives: Vec<GltfPrimitive>,
        }
        #[derive(Deserialize)]
        struct GltfPrimitive {
            attributes: BTreeMap<String, usize>,
            #[serde(default)]
            indices: Option<usize>,
        }

        let root: GltfRoot = serde_json::from_str(gltf_json)?;
        let mut raw_buffers: Vec<Vec<u8>> = Vec::with_capacity(root.buffers.len());
        for buf in &root.buffers {
            let uri = buf.uri.as_deref().unwrap_or("");
            if let Some((_, b64)) = uri.split_once(";base64,") {
                raw_buffers.push(decode_base64(b64)?);
            } else {
                return Err(AssetError::InvalidMesh(
                    "glTF buffer must use embedded base64 data URI".into(),
                ));
            }
        }

        let slice_for = |acc: &GltfAccessor, elem_bytes: usize| -> Result<(&[u8], usize), AssetError> {
            let view = root
                .buffer_views
                .get(acc.buffer_view)
                .ok_or_else(|| AssetError::InvalidMesh("Invalid bufferView index".into()))?;
            let buf = raw_buffers
                .get(view.buffer)
                .ok_or_else(|| AssetError::InvalidMesh("Invalid buffer index".into()))?;
            let start = view.byte_offset + acc.byte_offset;
            let end = (view.byte_offset + view.byte_length).min(buf.len());
            if start > end {
                return Err(AssetError::InvalidMesh("Accessor offset out of bounds".into()));
            }
            let stride = view.byte_stride.unwrap_or(elem_bytes).max(elem_bytes);
            Ok((&buf[start..end], stride))
        };

        let read_vec3 = |acc_idx: usize| -> Result<Vec<Vec3>, AssetError> {
            let acc = root
                .accessors
                .get(acc_idx)
                .ok_or_else(|| AssetError::InvalidMesh("Missing VEC3 accessor".into()))?;
            let (bytes, stride) = slice_for(acc, 12)?;
            let mut out = Vec::with_capacity(acc.count);
            for i in 0..acc.count {
                let off = i * stride;
                if off + 12 > bytes.len() {
                    break;
                }
                let x = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
                let y = f32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap());
                let z = f32::from_le_bytes(bytes[off + 8..off + 12].try_into().unwrap());
                out.push(Vec3::new(x, y, z));
            }
            Ok(out)
        };

        let read_vec2 = |acc_idx: usize| -> Result<Vec<Vec2>, AssetError> {
            let acc = root
                .accessors
                .get(acc_idx)
                .ok_or_else(|| AssetError::InvalidMesh("Missing VEC2 accessor".into()))?;
            let (bytes, stride) = slice_for(acc, 8)?;
            let mut out = Vec::with_capacity(acc.count);
            for i in 0..acc.count {
                let off = i * stride;
                if off + 8 > bytes.len() {
                    break;
                }
                let u = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
                let v = f32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap());
                out.push(Vec2::new(u, v));
            }
            Ok(out)
        };

        let mut vertices = Vec::new();
        for mesh in &root.meshes {
            for prim in &mesh.primitives {
                let pos_idx = *prim
                    .attributes
                    .get("POSITION")
                    .ok_or_else(|| AssetError::InvalidMesh("glTF primitive missing POSITION".into()))?;
                let positions = read_vec3(pos_idx)?;
                let normals = prim
                    .attributes
                    .get("NORMAL")
                    .and_then(|&i| read_vec3(i).ok())
                    .unwrap_or_default();
                let uvs = prim
                    .attributes
                    .get("TEXCOORD_0")
                    .and_then(|&i| read_vec2(i).ok())
                    .unwrap_or_default();

                let indices: Vec<usize> = if let Some(idx_acc_i) = prim.indices {
                    let acc = root
                        .accessors
                        .get(idx_acc_i)
                        .ok_or_else(|| AssetError::InvalidMesh("Missing index accessor".into()))?;
                    let elem_size = match acc.component_type {
                        5121 => 1,
                        5123 => 2,
                        5125 => 4,
                        _ => return Err(AssetError::InvalidMesh("Unsupported glTF index componentType".into())),
                    };
                    let (bytes, stride) = slice_for(acc, elem_size)?;
                    let mut out = Vec::with_capacity(acc.count);
                    for i in 0..acc.count {
                        let off = i * stride;
                        if off + elem_size > bytes.len() {
                            break;
                        }
                        let idx = match acc.component_type {
                            5121 => bytes[off] as usize,
                            5123 => u16::from_le_bytes(bytes[off..off + 2].try_into().unwrap()) as usize,
                            _ => u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize,
                        };
                        out.push(idx);
                    }
                    out
                } else {
                    (0..positions.len()).collect()
                };

                for tri in indices.chunks_exact(3) {
                    let p0 = positions.get(tri[0]).copied().unwrap_or(Vec3::ZERO);
                    let p1 = positions.get(tri[1]).copied().unwrap_or(Vec3::ZERO);
                    let p2 = positions.get(tri[2]).copied().unwrap_or(Vec3::ZERO);
                    let flat_n = (p1 - p0).cross(p2 - p0).normalize_or_zero();

                    for &vi in tri {
                        let position = positions.get(vi).copied().unwrap_or(Vec3::ZERO);
                        let normal = normals.get(vi).copied().unwrap_or(flat_n);
                        let uv = uvs.get(vi).copied().unwrap_or(Vec2::ZERO);
                        vertices.push(ShavedVertex3D {
                            position,
                            normal,
                            uv,
                            color: [1.0, 1.0, 1.0, 1.0],
                        });
                    }
                }
            }
        }

        Ok(Self {
            name: name.into(),
            vertices,
        })
    }
}

fn decode_base64(input: &str) -> Result<Vec<u8>, AssetError> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for b in input.bytes() {
        if b.is_ascii_whitespace() || b == b'=' {
            continue;
        }
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(AssetError::InvalidMesh("Invalid base64 character in glTF buffer".into())),
        };
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

static MESH_ASSET_CACHE: OnceLock<Mutex<HashMap<String, Arc<MeshAssetData>>>> = OnceLock::new();

/// Loads and caches a 3D mesh asset (`.gltf`, `.obj`, or `.vox`) by path.
pub fn get_or_load_mesh_asset(path: &str) -> Arc<MeshAssetData> {
    let cache = MESH_ASSET_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(existing) = guard.get(path)
    {
        return Arc::clone(existing);
    }

    let loaded = if path.ends_with(".vox") {
        Arc::new(get_or_load_vox_mesh(path, 0.1))
    } else {
        let candidates = [path.to_string(), format!("../../{path}")];
        let mut parsed = None;
        for cand in &candidates {
            if let Ok(raw) = fs::read_to_string(cand) {
                parsed = if path.ends_with(".gltf") {
                    MeshAssetData::from_gltf_str(path, &raw).ok()
                } else {
                    MeshAssetData::from_obj_str(path, &raw).ok()
                };
                if parsed.is_some() {
                    break;
                }
            }
        }
        Arc::new(parsed.unwrap_or_else(|| {
            MeshAssetData::from_obj_str(
                path,
                "v -0.5 0.0 0.5\nv 0.5 0.0 0.5\nv 0.0 1.2 0.0\nf 1 2 3",
            )
            .unwrap_or_default()
        }))
    };

    if let Ok(mut guard) = cache.lock() {
        guard.insert(path.to_string(), Arc::clone(&loaded));
    }
    loaded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aseprite_json_into_flipbook_strand() {
        let sample = r#"{
            "frames": {
                "hero_0": { "frame": { "x": 0, "y": 0, "w": 16, "h": 24 }, "duration": 125 },
                "hero_1": { "frame": { "x": 16, "y": 0, "w": 16, "h": 24 }, "duration": 125 },
                "hero_2": { "frame": { "x": 32, "y": 0, "w": 16, "h": 24 }, "duration": 125 },
                "hero_3": { "frame": { "x": 48, "y": 0, "w": 16, "h": 24 }, "duration": 125 }
            },
            "meta": {
                "image": "wanderer_walk_sheet.png",
                "size": { "w": 64, "h": 24 },
                "frameTags": [
                    { "name": "walk", "from": 0, "to": 3, "direction": "forward" }
                ]
            }
        }"#;

        let sheet = AsepriteSheet::from_json_str(sample).unwrap();
        let flipbook = sheet.to_flipbook_strand(Some("walk")).unwrap();

        assert_eq!(flipbook.columns, 4);
        assert_eq!(flipbook.rows, 1);
        assert_eq!(flipbook.start_frame, 0);
        assert_eq!(flipbook.end_frame, 3);
        assert!((flipbook.fps - 8.0).abs() < 1e-4);
    }

    #[test]
    fn parses_wavefront_obj_mesh() {
        let obj = "
            v -1.0 0.0 1.0
            v 1.0 0.0 1.0
            v 1.0 0.0 -1.0
            v -1.0 0.0 -1.0
            vn 0.0 1.0 0.0
            f 1//1 2//1 3//1 4//1
        ";
        let mesh = MeshAssetData::from_obj_str("quad", obj).unwrap();
        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.vertices[0].normal, Vec3::Y);
    }

    #[test]
    fn generates_procedural_wanderer_rgba_sheet() {
        let tex = RgbaTexture::procedural_wanderer_sheet();
        assert_eq!(tex.width, 64);
        assert_eq!(tex.height, 24);
        assert_eq!(tex.pixels.len(), 64 * 24 * 4);
    }

    #[test]
    fn parses_and_greedy_meshes_vox_cube() {
        let mut model = VoxModel::new("solid_slab", 4, 4, 1);
        // Create a 4x4x1 flat slab of 16 voxels with identical color index 10
        for x in 0..4 {
            for y in 0..4 {
                model.voxels.push(VoxelPoint {
                    x,
                    y,
                    z: 0,
                    color_index: 10,
                });
            }
        }

        // Test roundtrip encoding and parsing
        let encoded = model.encode_vox_bytes();
        let parsed = VoxModel::from_bytes("parsed_slab", &encoded).unwrap();
        assert_eq!(parsed.size_x, 4);
        assert_eq!(parsed.size_y, 4);
        assert_eq!(parsed.size_z, 1);
        assert_eq!(parsed.voxels.len(), 16);

        // Greedy mesh: A 4x4 flat top face would be 16 quads (32 triangles) naive.
        // With greedy meshing, the top face merges into 1 single quad (2 triangles)!
        // Bottom face: 1 quad (2 triangles).
        // 4 side faces (each 4x1): 4 quads (8 triangles).
        // Total quads = 1 + 1 + 4 = 6 quads (12 triangles = 36 vertices)!
        let mesh = parsed.to_greedy_mesh(0.1, Vec3::new(0.5, 0.0, 0.5));
        assert_eq!(mesh.vertices.len(), 36);

        // Top normal vertices have normal +Y
        let top_verts: Vec<_> = mesh.vertices.iter().filter(|v| v.normal == Vec3::Y).collect();
        assert_eq!(top_verts.len(), 6); // 1 quad = 2 triangles = 6 vertices!
    }

    #[test]
    fn procedural_ancient_shrine_greedy_meshes_cleanly() {
        let shrine = VoxModel::procedural_ancient_shrine();
        assert!(!shrine.voxels.is_empty());

        let bytes = shrine.encode_vox_bytes();
        assert!(!bytes.is_empty());

        let candidates = ["assets/voxels", "../../assets/voxels"];
        for dir in candidates {
            if Path::new(dir).exists() {
                let p = Path::new(dir).join("ancient_shrine.vox");
                let _ = fs::write(p, &bytes);
            }
        }

        let mesh = shrine.to_greedy_mesh(0.1, Vec3::new(0.5, 0.0, 0.5));
        assert!(!mesh.vertices.is_empty());
        // Verify all vertices have valid normals and finite coordinates
        for v in &mesh.vertices {
            assert!(v.position.is_finite());
            assert!(v.normal.is_finite());
            assert!(v.color[3] > 0.0);
        }
    }

    #[test]
    fn parses_gltf_2_0_embedded_buffer_mesh() {
        // 3 vertices (f32x3 = 36 bytes) + 3 u16 indices (6 bytes) = 42 bytes
        // v0=(0,0,0), v1=(1,0,0), v2=(0,1,0), indices=[0,1,2]
        // 16 'A's = 12 zero bytes (v0), then "AACAPwAAAAAAAAAA" = (1.0, 0.0, 0.0) (v1),
        // then "AAAAAACAPwAAAAAA" = (0.0, 1.0, 0.0) (v2), then "AAEAAgAA" = u16 [0, 1, 2]
        let gltf = r#"{
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAABAAIA",
                "byteLength": 42
            }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
                { "buffer": 0, "byteOffset": 36, "byteLength": 6 }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" },
                { "bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR" }
            ],
            "meshes": [{
                "primitives": [{
                    "attributes": { "POSITION": 0 },
                    "indices": 1
                }]
            }]
        }"#;

        let mesh = MeshAssetData::from_gltf_str("gltf_tri", gltf).unwrap();
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.vertices[1].position, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(mesh.vertices[2].position, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(mesh.vertices[0].normal, Vec3::Z);
    }
}
