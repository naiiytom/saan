//! # MagicaVoxel `.vox` Parser & 3D Greedy Mesher
//!
//! Parses MagicaVoxel `.vox` RIFF chunk hierarchies (`MAIN`, `SIZE`, `XYZI`, `RGBA`)
//! and compiles them into optimized, depth-lit 3D meshes using 3D greedy meshing.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{LazyLock, RwLock};

use glam::{Vec2, Vec3};

use crate::{AssetError, MeshAssetData, ShavedVertex3D};

/// A single voxel point in MagicaVoxel coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelPoint {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    /// 1-based palette index (1..=255). 0 is empty/air.
    pub color_index: u8,
}

/// A parsed MagicaVoxel model containing dimensions, voxel points, and a 256-color RGBA palette.
#[derive(Debug, Clone)]
pub struct VoxModel {
    pub name: String,
    pub size_x: u32,
    pub size_y: u32,
    pub size_z: u32,
    pub voxels: Vec<VoxelPoint>,
    pub palette: [[u8; 4]; 256],
}

/// Global thread-safe cache for greedy-meshed voxel models.
static VOX_MESH_CACHE: LazyLock<RwLock<HashMap<String, MeshAssetData>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

impl VoxModel {
    /// Creates an empty voxel model with the given bounds and default palette.
    pub fn new(name: impl Into<String>, size_x: u32, size_y: u32, size_z: u32) -> Self {
        Self {
            name: name.into(),
            size_x,
            size_y,
            size_z,
            voxels: Vec::new(),
            palette: Self::default_palette(),
        }
    }

    /// Default 256-color RGBA palette with rich earth, stone, foliage, bamboo, and gold tones.
    pub fn default_palette() -> [[u8; 4]; 256] {
        let mut palette = [[255u8; 4]; 256];

        // 1..16: Grays / Stones
        for i in 1..=16 {
            let shade = ((i as f32 / 16.0) * 220.0 + 35.0) as u8;
            palette[i - 1] = [shade, shade, (shade as f32 * 1.05).min(255.0) as u8, 255];
        }

        // 17..32: Woods / Bamboo / Earth Browns
        for i in 17..=32 {
            let t = (i - 17) as f32 / 15.0;
            let r = (60.0 + t * 140.0) as u8;
            let g = (40.0 + t * 105.0) as u8;
            let b = (25.0 + t * 60.0) as u8;
            palette[i - 1] = [r, g, b, 255];
        }

        // 33..48: Greens / Moss / Foliage
        for i in 33..=48 {
            let t = (i - 33) as f32 / 15.0;
            let r = (25.0 + t * 65.0) as u8;
            let g = (65.0 + t * 155.0) as u8;
            let b = (30.0 + t * 75.0) as u8;
            palette[i - 1] = [r, g, b, 255];
        }

        // 49..64: Golds / Embers / Shrines
        for i in 49..=64 {
            let t = (i - 49) as f32 / 15.0;
            let r = (200.0 + t * 55.0) as u8;
            let g = (140.0 + t * 95.0) as u8;
            let b = (30.0 + t * 50.0) as u8;
            palette[i - 1] = [r, g, b, 255];
        }

        // 65..255: Smooth HSV rainbow sweep
        for i in 65..=255 {
            let hue = (i - 65) as f32 / 190.0 * 6.0;
            let x = (1.0 - (hue % 2.0 - 1.0).abs()).clamp(0.0, 1.0);
            let (r, g, b) = match hue as i32 {
                0 => (1.0, x, 0.0),
                1 => (x, 1.0, 0.0),
                2 => (0.0, 1.0, x),
                3 => (0.0, x, 1.0),
                4 => (x, 0.0, 1.0),
                _ => (1.0, 0.0, x),
            };
            palette[i - 1] = [(r * 230.0) as u8, (g * 230.0) as u8, (b * 230.0) as u8, 255];
        }

        palette
    }

    /// Parses a MagicaVoxel `.vox` byte buffer into a [`VoxModel`].
    pub fn from_bytes(name: impl Into<String>, bytes: &[u8]) -> Result<Self, AssetError> {
        if bytes.len() < 8 {
            return Err(AssetError::InvalidVox(
                "Buffer too short for VOX header".into(),
            ));
        }

        if &bytes[0..4] != b"VOX " {
            return Err(AssetError::InvalidVox("Missing 'VOX ' magic header".into()));
        }

        let mut offset = 8;
        let mut model_name = name.into();
        let mut size_x = 16u32;
        let mut size_y = 16u32;
        let mut size_z = 16u32;
        let mut voxels = Vec::new();
        let mut palette = Self::default_palette();

        // Read MAIN chunk
        if offset + 12 > bytes.len() {
            return Err(AssetError::InvalidVox("Missing MAIN chunk header".into()));
        }
        let main_id = &bytes[offset..offset + 4];
        if main_id != b"MAIN" {
            return Err(AssetError::InvalidVox("Root chunk must be MAIN".into()));
        }

        let main_content_len = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let main_children_len = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as usize;
        offset += 12 + main_content_len;

        let end_of_children = (offset + main_children_len).min(bytes.len());

        while offset + 12 <= end_of_children {
            let chunk_id = &bytes[offset..offset + 4];
            let content_len = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
            let children_len = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as usize;
            offset += 12;

            if offset + content_len > bytes.len() {
                break;
            }
            let chunk_content = &bytes[offset..offset + content_len];

            match chunk_id {
                b"SIZE" if chunk_content.len() >= 12 => {
                    size_x = u32::from_le_bytes(chunk_content[0..4].try_into().unwrap()).max(1);
                    size_y = u32::from_le_bytes(chunk_content[4..8].try_into().unwrap()).max(1);
                    size_z = u32::from_le_bytes(chunk_content[8..12].try_into().unwrap()).max(1);
                }
                b"XYZI" if chunk_content.len() >= 4 => {
                    let num_voxels = u32::from_le_bytes(chunk_content[0..4].try_into().unwrap()) as usize;
                    let mut cur = 4;
                    for _ in 0..num_voxels {
                        if cur + 4 > chunk_content.len() {
                            break;
                        }
                        voxels.push(VoxelPoint {
                            x: chunk_content[cur],
                            y: chunk_content[cur + 1],
                            z: chunk_content[cur + 2],
                            color_index: chunk_content[cur + 3],
                        });
                        cur += 4;
                    }
                }
                b"RGBA" if chunk_content.len() >= 1024 => {
                    for i in 0..256 {
                        let idx = i * 4;
                        palette[i] = [
                            chunk_content[idx],
                            chunk_content[idx + 1],
                            chunk_content[idx + 2],
                            chunk_content[idx + 3],
                        ];
                    }
                }
                _ => {}
            }

            offset += content_len + children_len;
        }

        if model_name.is_empty() {
            model_name = "vox_model".to_string();
        }

        Ok(Self {
            name: model_name,
            size_x,
            size_y,
            size_z,
            voxels,
            palette,
        })
    }

    /// Loads a `.vox` file from disk.
    pub fn load_from_file(path: &Path) -> Result<Self, AssetError> {
        let bytes = fs::read(path)?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("vox_model");
        Self::from_bytes(name, &bytes)
    }

    /// Encodes this model into valid MagicaVoxel `.vox` binary byte format.
    pub fn encode_vox_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        // Header
        out.extend_from_slice(b"VOX ");
        out.extend_from_slice(&150u32.to_le_bytes());

        // SIZE chunk
        let mut size_content = Vec::new();
        size_content.extend_from_slice(&self.size_x.to_le_bytes());
        size_content.extend_from_slice(&self.size_y.to_le_bytes());
        size_content.extend_from_slice(&self.size_z.to_le_bytes());

        // XYZI chunk
        let mut xyzi_content = Vec::new();
        xyzi_content.extend_from_slice(&(self.voxels.len() as u32).to_le_bytes());
        for v in &self.voxels {
            xyzi_content.push(v.x);
            xyzi_content.push(v.y);
            xyzi_content.push(v.z);
            xyzi_content.push(v.color_index);
        }

        // RGBA chunk
        let mut rgba_content = Vec::new();
        for col in &self.palette {
            rgba_content.extend_from_slice(col);
        }

        let children_len = (12 + size_content.len()) + (12 + xyzi_content.len()) + (12 + rgba_content.len());

        // MAIN chunk header
        out.extend_from_slice(b"MAIN");
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(children_len as u32).to_le_bytes());

        // Write SIZE
        out.extend_from_slice(b"SIZE");
        out.extend_from_slice(&(size_content.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&size_content);

        // Write XYZI
        out.extend_from_slice(b"XYZI");
        out.extend_from_slice(&(xyzi_content.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&xyzi_content);

        // Write RGBA
        out.extend_from_slice(b"RGBA");
        out.extend_from_slice(&(rgba_content.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&rgba_content);

        out
    }

    /// Converts this voxel model into an optimized 3D mesh using 3D greedy meshing.
    ///
    /// - `scale`: world-space units per voxel (e.g. `0.1` for 10cm voxels).
    /// - `pivot`: normalized origin anchor (e.g. `Vec3::new(0.5, 0.0, 0.5)` for bottom-center).
    pub fn to_greedy_mesh(&self, scale: f32, pivot: Vec3) -> MeshAssetData {
        // Map MagicaVoxel coordinate axes to Saan 3D engine coordinate axes:
        // Vox X -> Saan X (width)
        // Vox Y -> Saan Z (depth)
        // Vox Z -> Saan Y (height)
        let dim_x = self.size_x as usize;
        let dim_y = self.size_z as usize; // Height
        let dim_z = self.size_y as usize; // Depth

        if dim_x == 0 || dim_y == 0 || dim_z == 0 || self.voxels.is_empty() {
            return MeshAssetData {
                name: self.name.clone(),
                vertices: Vec::new(),
            };
        }

        // Fill 3D density grid (0 = air, 1..=255 = palette index)
        let mut grid = vec![0u8; dim_x * dim_y * dim_z];
        let idx_fn = |x: usize, y: usize, z: usize| -> usize {
            (z * dim_y + y) * dim_x + x
        };

        for v in &self.voxels {
            let sx = v.x as usize;
            let sy = v.z as usize; // Vox Z is Saan height Y
            let sz = v.y as usize; // Vox Y is Saan depth Z
            if sx < dim_x && sy < dim_y && sz < dim_z {
                grid[idx_fn(sx, sy, sz)] = v.color_index;
            }
        }

        let pivot_offset = Vec3::new(
            dim_x as f32 * pivot.x,
            dim_y as f32 * pivot.y,
            dim_z as f32 * pivot.z,
        );

        let mut vertices: Vec<ShavedVertex3D> = Vec::new();

        let get_color = |palette_idx: u8| -> [f32; 4] {
            if palette_idx == 0 {
                return [1.0, 1.0, 1.0, 1.0];
            }
            let c = self.palette[(palette_idx - 1) as usize];
            [
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                c[3] as f32 / 255.0,
            ]
        };

        let mut push_quad = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, normal: Vec3, color: [f32; 4]| {
            let q0 = (p0 - pivot_offset) * scale;
            let q1 = (p1 - pivot_offset) * scale;
            let q2 = (p2 - pivot_offset) * scale;
            let q3 = (p3 - pivot_offset) * scale;

            vertices.push(ShavedVertex3D { position: q0, normal, uv: Vec2::ZERO, color });
            vertices.push(ShavedVertex3D { position: q1, normal, uv: Vec2::ZERO, color });
            vertices.push(ShavedVertex3D { position: q2, normal, uv: Vec2::ZERO, color });

            vertices.push(ShavedVertex3D { position: q0, normal, uv: Vec2::ZERO, color });
            vertices.push(ShavedVertex3D { position: q2, normal, uv: Vec2::ZERO, color });
            vertices.push(ShavedVertex3D { position: q3, normal, uv: Vec2::ZERO, color });
        };

        // 1. Top (+Y) and Bottom (-Y) faces
        for y in 0..dim_y {
            // +Y Top
            let mut mask = vec![0u8; dim_x * dim_z];
            for z in 0..dim_z {
                for x in 0..dim_x {
                    let c = grid[idx_fn(x, y, z)];
                    if c > 0 && (y + 1 == dim_y || grid[idx_fn(x, y + 1, z)] == 0) {
                        mask[z * dim_x + x] = c;
                    }
                }
            }
            greedy_mesh_2d(&mut mask, dim_x, dim_z, |u, v, w, h, col_idx| {
                let y_pos = (y + 1) as f32;
                let p0 = Vec3::new(u as f32, y_pos, (v + h) as f32);
                let p1 = Vec3::new((u + w) as f32, y_pos, (v + h) as f32);
                let p2 = Vec3::new((u + w) as f32, y_pos, v as f32);
                let p3 = Vec3::new(u as f32, y_pos, v as f32);
                push_quad(p0, p1, p2, p3, Vec3::Y, get_color(col_idx));
            });

            // -Y Bottom
            let mut mask = vec![0u8; dim_x * dim_z];
            for z in 0..dim_z {
                for x in 0..dim_x {
                    let c = grid[idx_fn(x, y, z)];
                    if c > 0 && (y == 0 || grid[idx_fn(x, y - 1, z)] == 0) {
                        mask[z * dim_x + x] = c;
                    }
                }
            }
            greedy_mesh_2d(&mut mask, dim_x, dim_z, |u, v, w, h, col_idx| {
                let y_pos = y as f32;
                let p0 = Vec3::new(u as f32, y_pos, v as f32);
                let p1 = Vec3::new((u + w) as f32, y_pos, v as f32);
                let p2 = Vec3::new((u + w) as f32, y_pos, (v + h) as f32);
                let p3 = Vec3::new(u as f32, y_pos, (v + h) as f32);
                push_quad(p0, p1, p2, p3, Vec3::NEG_Y, get_color(col_idx));
            });
        }

        // 2. Right (+X) and Left (-X) faces
        for x in 0..dim_x {
            // +X Right
            let mut mask = vec![0u8; dim_z * dim_y];
            for y in 0..dim_y {
                for z in 0..dim_z {
                    let c = grid[idx_fn(x, y, z)];
                    if c > 0 && (x + 1 == dim_x || grid[idx_fn(x + 1, y, z)] == 0) {
                        mask[y * dim_z + z] = c;
                    }
                }
            }
            greedy_mesh_2d(&mut mask, dim_z, dim_y, |u, v, w, h, col_idx| {
                let x_pos = (x + 1) as f32;
                let p0 = Vec3::new(x_pos, v as f32, u as f32);
                let p1 = Vec3::new(x_pos, v as f32, (u + w) as f32);
                let p2 = Vec3::new(x_pos, (v + h) as f32, (u + w) as f32);
                let p3 = Vec3::new(x_pos, (v + h) as f32, u as f32);
                push_quad(p0, p1, p2, p3, Vec3::X, get_color(col_idx));
            });

            // -X Left
            let mut mask = vec![0u8; dim_z * dim_y];
            for y in 0..dim_y {
                for z in 0..dim_z {
                    let c = grid[idx_fn(x, y, z)];
                    if c > 0 && (x == 0 || grid[idx_fn(x - 1, y, z)] == 0) {
                        mask[y * dim_z + z] = c;
                    }
                }
            }
            greedy_mesh_2d(&mut mask, dim_z, dim_y, |u, v, w, h, col_idx| {
                let x_pos = x as f32;
                let p0 = Vec3::new(x_pos, v as f32, (u + w) as f32);
                let p1 = Vec3::new(x_pos, v as f32, u as f32);
                let p2 = Vec3::new(x_pos, (v + h) as f32, u as f32);
                let p3 = Vec3::new(x_pos, (v + h) as f32, (u + w) as f32);
                push_quad(p0, p1, p2, p3, Vec3::NEG_X, get_color(col_idx));
            });
        }

        // 3. Front (+Z) and Back (-Z) faces
        for z in 0..dim_z {
            // +Z Front
            let mut mask = vec![0u8; dim_x * dim_y];
            for y in 0..dim_y {
                for x in 0..dim_x {
                    let c = grid[idx_fn(x, y, z)];
                    if c > 0 && (z + 1 == dim_z || grid[idx_fn(x, y, z + 1)] == 0) {
                        mask[y * dim_x + x] = c;
                    }
                }
            }
            greedy_mesh_2d(&mut mask, dim_x, dim_y, |u, v, w, h, col_idx| {
                let z_pos = (z + 1) as f32;
                let p0 = Vec3::new(u as f32, v as f32, z_pos);
                let p1 = Vec3::new((u + w) as f32, v as f32, z_pos);
                let p2 = Vec3::new((u + w) as f32, (v + h) as f32, z_pos);
                let p3 = Vec3::new(u as f32, (v + h) as f32, z_pos);
                push_quad(p0, p1, p2, p3, Vec3::Z, get_color(col_idx));
            });

            // -Z Back
            let mut mask = vec![0u8; dim_x * dim_y];
            for y in 0..dim_y {
                for x in 0..dim_x {
                    let c = grid[idx_fn(x, y, z)];
                    if c > 0 && (z == 0 || grid[idx_fn(x, y, z - 1)] == 0) {
                        mask[y * dim_x + x] = c;
                    }
                }
            }
            greedy_mesh_2d(&mut mask, dim_x, dim_y, |u, v, w, h, col_idx| {
                let z_pos = z as f32;
                let p0 = Vec3::new((u + w) as f32, v as f32, z_pos);
                let p1 = Vec3::new(u as f32, v as f32, z_pos);
                let p2 = Vec3::new(u as f32, (v + h) as f32, z_pos);
                let p3 = Vec3::new((u + w) as f32, (v + h) as f32, z_pos);
                push_quad(p0, p1, p2, p3, Vec3::NEG_Z, get_color(col_idx));
            });
        }

        MeshAssetData {
            name: self.name.clone(),
            vertices,
        }
    }

    /// Generates a handcrafted ancient Thai/Diorama Stone Shrine with 4 corner pillars, stepped base, and golden relic.
    pub fn procedural_ancient_shrine() -> Self {
        let size_x = 16u32;
        let size_y = 16u32;
        let size_z = 20u32;
        let mut model = Self::new("ancient_shrine", size_x, size_y, size_z);

        let stone_dark = 2u8;
        let stone_light = 7u8;
        let wood = 22u8;
        let moss = 38u8;
        let gold_relic = 54u8;

        // 1. Stepped stone foundation (layers z = 0..3)
        for z in 0..3 {
            let margin = z;
            for x in margin..(size_x - margin) {
                for y in margin..(size_y - margin) {
                    let col = if (x + y + z) % 3 == 0 { moss } else { stone_dark };
                    model.voxels.push(VoxelPoint {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                        color_index: col,
                    });
                }
            }
        }

        // 2. Four corner pillars (z = 3..14)
        for z in 3..14 {
            let corners = [(3, 3), (12, 3), (3, 12), (12, 12)];
            for (cx, cy) in corners {
                for dx in 0..2 {
                    for dy in 0..2 {
                        model.voxels.push(VoxelPoint {
                            x: (cx + dx) as u8,
                            y: (cy + dy) as u8,
                            z: z as u8,
                            color_index: stone_light,
                        });
                    }
                }
            }
        }

        // 3. Central altar table with golden relic (z = 3..7)
        for z in 3..5 {
            for x in 6..10 {
                for y in 6..10 {
                    model.voxels.push(VoxelPoint {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                        color_index: wood,
                    });
                }
            }
        }
        // Floating glowing relic (z = 6..8)
        for z in 6..8 {
            for x in 7..9 {
                for y in 7..9 {
                    model.voxels.push(VoxelPoint {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                        color_index: gold_relic,
                    });
                }
            }
        }

        // 4. Overhanging wooden roof (z = 14..18)
        for (i, z) in (14..18).enumerate() {
            let inset = i as u32;
            for x in (2 + inset)..(15 - inset) {
                for y in (2 + inset)..(15 - inset) {
                    model.voxels.push(VoxelPoint {
                        x: x as u8,
                        y: y as u8,
                        z: z as u8,
                        color_index: if z == 17 { gold_relic } else { wood },
                    });
                }
            }
        }

        model
    }
}

/// 2D Greedy Meshing algorithm on a rectangular mask.
/// Finds adjacent identical non-zero values and expands them into maximum-area quads.
fn greedy_mesh_2d<F>(mask: &mut [u8], dim_u: usize, dim_v: usize, mut on_quad: F)
where
    F: FnMut(usize, usize, usize, usize, u8),
{
    for v in 0..dim_v {
        for u in 0..dim_u {
            let col = mask[v * dim_u + u];
            if col == 0 {
                continue;
            }

            // Expand width along u
            let mut w = 1;
            while u + w < dim_u && mask[v * dim_u + (u + w)] == col {
                w += 1;
            }

            // Expand height along v
            let mut h = 1;
            'grow_h: while v + h < dim_v {
                for du in 0..w {
                    if mask[(v + h) * dim_u + (u + du)] != col {
                        break 'grow_h;
                    }
                }
                h += 1;
            }

            // Clear merged region in mask
            for dv in 0..h {
                for du in 0..w {
                    mask[(v + dv) * dim_u + (u + du)] = 0;
                }
            }

            on_quad(u, v, w, h, col);
        }
    }
}

/// Retrieves or loads and greedy-meshes a `.vox` file from disk (or built-in procedural shrine).
pub fn get_or_load_vox_mesh(path: impl AsRef<Path>, scale: f32) -> MeshAssetData {
    let path_ref = path.as_ref();
    let cache_key = format!("{}::scale_{}", path_ref.display(), scale);

    if let Ok(guard) = VOX_MESH_CACHE.read() {
        if let Some(cached) = guard.get(&cache_key) {
            return cached.clone();
        }
    }

    let model = if path_ref.as_os_str().is_empty()
        || path_ref.to_string_lossy().contains("shrine")
        || !path_ref.exists()
    {
        // Try reading if file exists, else procedural shrine fallback
        if path_ref.exists() {
            VoxModel::load_from_file(path_ref).unwrap_or_else(|_| VoxModel::procedural_ancient_shrine())
        } else {
            VoxModel::procedural_ancient_shrine()
        }
    } else {
        VoxModel::load_from_file(path_ref).unwrap_or_else(|_| VoxModel::procedural_ancient_shrine())
    };

    let mesh = model.to_greedy_mesh(scale, Vec3::new(0.5, 0.0, 0.5));

    if let Ok(mut guard) = VOX_MESH_CACHE.write() {
        guard.insert(cache_key, mesh.clone());
    }

    mesh
}
