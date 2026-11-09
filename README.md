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
| `saan_core` | **The Loom** | Custom ECS (`World`, `EntityId`, `Strand`), spatial math (`glam`), DAG system scheduler (`Loom`), tension validation, Fog of War, and `.saan` scene patterns |
| `saan_asset` | **The Thread** | Asset pipeline: Aseprite JSON spritesheet parser (`FlipbookStrand`), Wavefront `.obj` 3D mesh loader (`MeshAssetData`), and RGBA8 procedural texture generators |
| `saan_spatial` | **The Space** | 2.5D/3D collision (`Collider3D`: `Aabb3D`, `CylinderY`), stair/ledge step-up, raycast camera occlusion fade, dual nav (`GridMap3D` multi-height A* & `FlowField25D`), and `AgentController` squad commanding |
| `saan_script` | **The Mind** | Embedded Rhai scripting engine (`ScriptStrand`, `ScriptHost`, hot-reloadable gameplay logic, unit AI, and scenario triggers) |
| `saan_audio` | **The Resonance** | Pure-Rust `rodio` audio engine, 3D spatial panning, BGM/SFX buses, procedural waveform synthesis fallback, and headless-safe operation |
| `saan_render` | **The Mesh** | Hybrid 2.5D + 3D `wgpu` render graph: 2048x2048 directional depth shadow maps with 4-tap PCF, 3D meshes, depth-sorted billboard sprites, dynamic Fog of War grid texture, 3D particles, and tilt-shift DOF |
| `saan_editor` | **The Craft Loom** | Interactive diorama editor: live entity hierarchy, strand component inspector, 3D grid tile brush, simulation controls, and `.saan` serializer |
| `saan_cli` | **The Station** | CLI toolbelt: `saan init`, `saan inspect`, `saan weave`, `saan run`, `saan edit` |

---

## Quick Start

```powershell
# Build the workspace
cargo build

# Run all 23 unit & integration tests across all 8 crates
cargo test --workspace

# 1. Initialize the complete Diorama Sandbox scene (.saan)
cargo run -p saan_cli -- init my-diorama --template diorama_sandbox

# 2. Inspect the scene & validate Loom DAG tension (validating all 11 Weft systems)
cargo run -p saan_cli -- inspect my-diorama/scene.saan

# 3. Weave (simulate Loom systems, collisions, audio cues, and render batches headless)
cargo run -p saan_cli -- weave my-diorama/scene.saan --frames 3

# 4. Run the interactive winit + wgpu HD-2D & 3D viewport!
#    Controls: WASD/Arrows to pan, Q/E to orbit, R/F to zoom, Space to pause, Esc to quit
#    Woven Inspector: Tab/F1 to toggle HUD, 1/2 to cycle entity, [/] DOF focus, -/= blur, ,/. sun light
cargo run -p saan_cli -- run my-diorama/scene.saan

# 5. Launch the Saan Craft Editor (hierarchy, strand inspector, tile brush, .saan save)
#    Controls: Tab to cycle tool, T to cycle tile brush, P to paint tile, 1..3 to spawn prefabs
#              I/J/K/L to move selected entity, Space to play/pause simulation, S to save scene
cargo run -p saan_cli -- edit my-diorama/scene.saan
```

---

## Documentation

- [Technical Specifications](docs/TECHNICAL_SPECIFICATIONS.md)
- [Roadmap](docs/ROADMAP.md)
- [Phase 1 Implementation Plan](docs/IMPLEMENTATION_PLAN.md)

## License

MIT
