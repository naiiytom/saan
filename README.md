# Saan (สาน)

**Saan** is a **2.5D and 3D hybrid game engine** written in Rust. Inspired by the traditional Thai craft of *สาน* (weaving and interlacing bamboo or rattan strips into a resilient, unified structure), Saan treats game architecture and rendering as an interwoven fabric.

- **The Loom (`saan_core`)**: A lightweight Entity-Component-System (ECS) and `petgraph`-powered DAG scheduler. Entities are **Warp** threads, components are **Strands**, and systems are **Weft** passes with compile/load-time **Tension** checking (automatically detecting dependency cycles and read/write data hazards).
- **The Mesh (`saan_render`)**: First-class **HD-2D & Diorama** hybrid rendering. 2D pixel/vector sprites (`Sprite25D` with cylindrical/spherical billboarding, normal maps, and 3D shadow casting) are depth-interlaced directly inside a 3D world alongside `Mesh3D` geometry, 3D lights, and diorama tilt-shift cameras.
- **The Craft (`saan_cli`)**: Developer CLI for initializing `.saan` scenes, inspecting Loom tension, and running headless or interactive weaves.

> Looking for the earlier **Saan Data Lineage** platform? It has been preserved in `../saan-lineage`.

---

## Workspace Crates

| Crate | Weaving Role | Description |
| :--- | :--- | :--- |
| `saan_core` | **The Loom** | Custom ECS (`World`, `EntityId`, `Strand`), spatial math (`glam`), DAG system scheduler (`Loom`), tension validation, and `.saan` scene patterns |
| `saan_asset` | **The Shaver** | Asset pipeline: Aseprite JSON spritesheet shaver (`FlipbookStrand`), `.obj` 3D mesh shaver (`MeshAssetData`), and RGBA8 sprite sheet textures |
| `saan_spatial` | **The Frame** | 2.5D/3D collision (`Collider3D`: `Aabb3D`, `CylinderY`), automatic stair/ledge step-up, and 3D raycast camera occlusion fade |
| `saan_render` | **The Mesh** | Hybrid 2.5D + 3D `wgpu` render graph (`ShadowPass` → `Opaque3DPass` → `Interlace25DPass` → `DioramaPostPass`), billboard math, and depth sorting |
| `saan_cli` | **The Craft** | CLI toolbelt: `saan init`, `saan inspect`, `saan weave`, `saan run` |

---

## Quick Start

```powershell
# Build the workspace
cargo build

# Run all unit & integration tests
cargo test --workspace

# 1. Initialize a starter HD-2D diorama scene (.saan)
cargo run -p saan_cli -- init my-diorama

# 2. Inspect the scene & validate Loom DAG tension (cycles & read/write hazards)
cargo run -p saan_cli -- inspect my-diorama/scene.saan

# 3. Weave (simulate & extract 2.5D/3D render batches headless for N frames)
cargo run -p saan_cli -- weave my-diorama/scene.saan --frames 3

# 4. Run the interactive winit + wgpu HD-2D & 3D window!
#    Controls: WASD/Arrows to pan, Q/E to orbit, R/F to zoom, Space to pause, Esc to quit
cargo run -p saan_cli -- run my-diorama/scene.saan
```

---

## Documentation

- [Technical Specifications](docs/TECHNICAL_SPECIFICATIONS.md)
- [Roadmap](docs/ROADMAP.md)
- [Phase 1 Implementation Plan](docs/IMPLEMENTATION_PLAN.md)

## License

MIT
