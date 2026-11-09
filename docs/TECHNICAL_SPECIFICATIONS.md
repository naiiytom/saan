# Saan (สาน) — Technical Specifications

## 1. Overview & Philosophy

**Saan (สาน)** is a modular **2.5D and 3D game engine** written in Rust. Named after the Thai word *สาน* (to weave or interlace bamboo/rattan strips into a strong, flexible basket or mesh), Saan is built around two foundational ideas:

1. **Architectural Weaving (The Loom):** Game objects are not rigid class hierarchies. They are collections of independent **Strands** (components) bound to **Warp** threads (entities) and processed by **Weft** threads (systems and render passes) woven into a validated Directed Acyclic Graph (DAG).
2. **Visual Interlacing (HD-2D & Diorama 2.5D + 3D):** Instead of separating 2D and 3D into incompatible engines, Saan interweaves 2D sprites, pixel art, and tilemaps directly into a 3D lit, shadow-casting world alongside 3D geometry and diorama post-processing (inspired by *Octopath Traveler*, *Cult of the Lamb*, and *Paper Mario*).

---

## 2. Weaving Vocabulary Mapping

| Thai Craft Term | Engine Term | Responsibility |
| :--- | :--- | :--- |
| **ตอก / เส้น (Strand)** | `Strand` (Component) | Pure data attached to an entity: `Transform`, `Sprite25D`, `Mesh3D`, `PointLight`, `CameraRig`, `Velocity`. |
| **เส้นยืน (Warp)** | `EntityId` / `World` | Structural vertical threads that hold strands together across frames. |
| **เส้นพุ่ง (Weft)** | `WeftNode` / `System` | Active horizontal passes that weave through matching strands each frame. |
| **กี่ทอ (The Loom)** | `Loom` | The ECS + `petgraph` DAG scheduler that orders systems, resolves dependencies, and runs frames. |
| **ลายสาน (Pattern)** | `ScenePattern` (`.saan`) | Declarative scene/prefab asset describing entities and their interwoven strands. |
| **ความตึง (Tension)** | `TensionReport` | Static and runtime validation of the Loom DAG (cycle detection, read/write strand hazards, orphan references). |

---

## 3. Core Architecture

### 3.1 `saan_core` — The Loom (ECS, Math, Scene Patterns, DAG Scheduler)

- **World & Strand Storage:**
  - Generational `EntityId` allocator.
  - Type-erased `Any` component tables (`StrandStorage<T>`) indexed by `TypeId` and `EntityId`.
  - Ergonomic queries over component combinations.
- **Built-in Spatial & Gameplay Strands:**
  - `Transform`: 3D translation (`Vec3`), rotation (`Quat`), scale (`Vec3`), and local-to-world matrix computation via `glam`.
  - `Sprite25D`: Texture/atlas ID, pixel size, pivot, `BillboardMode` (`FreePlane`, `CylindricalY`, `Spherical`), normal-map support, and 3D shadow casting toggle.
  - `Mesh3D`: Mesh primitive or asset handle (`Cube`, `Quad`, `DioramaTerrain`, or external glTF) + `Material3D` (albedo, roughness, metallic, emissive).
  - `CameraRig`: Supports `Perspective3D`, `Orthographic25D`, and `DioramaTilt` (pitched perspective with depth-of-field focal distance).
  - `DirectionalLight` & `PointLight`: 3D lighting that illuminates both 3D meshes and 2.5D sprites simultaneously.
- **The Loom Scheduler (`Loom`):**
  - Uses `petgraph::graph::DiGraph` to model execution order between systems.
  - Each system declares:
    - `name`: Unique identifier.
    - `reads`: Set of component names/types read.
    - `writes`: Set of component names/types mutated.
    - `after` / `before`: Explicit topological constraints.
  - **Tension Check (`Loom::inspect_tension`):**
    - Detects dependency cycles.
    - Detects *unconstrained data hazards* (two systems writing the same strand, or one reading and one writing, without a directed path between them in the DAG).
    - Produces a deterministic execution schedule.

### 3.2 `saan_render` — The Interlaced 2.5D/3D Render Graph

- **Hybrid 2.5D + 3D Pipeline Design:**
  - Rather than drawing 2D sprites as a flat overlay, `saan_render` extracts both `Mesh3D` and `Sprite25D` instances from `saan_core::World` into a unified `RenderFrame`.
  - **Billboard & Depth Interlacing:** Calculates world-space vertex positions and normals for `Sprite25D` quads based on the active `CameraRig`, sorting and interlacing them with 3D opaque and transparent geometry.
  - **Render Loom Passes:**
    1. `ShadowMapPass`: Renders shadow casters (both 3D meshes and shadow-casting 2.5D sprites).
    2. `OpaqueMesh3DPass`: Renders 3D environment geometry to color + depth buffers.
    3. `Sprite25DInterlacePass`: Renders depth-tested and depth-sorted 2.5D sprites with 3D lighting and normal-mapped shading.
    4. `DioramaPostPass`: Applies tilt-shift depth-of-field, bloom, and color grading.

### 3.3 `saan_cli` — The Craft Station

- `saan init [path] [--force]`: Creates a new Saan game project with a starter HD-2D diorama scene (`scene.saan`).
- `saan inspect [scene]`: Validates the scene pattern and checks Loom + Render Graph tension (cycles, strand conflicts, entity/component counts).
- `saan weave [scene] [--frames N]`: Runs the Loom and Render Graph for `N` frames, printing the woven execution plan and frame extraction summary.

---

## 4. Technology Stack

- **Language:** Rust (2021 edition)
- **Math:** `glam` (SIMD-accelerated vectors, quaternions, matrices)
- **Graph & Scheduling:** `petgraph` (DAG topological sort, reachability, cycle & hazard detection)
- **Serialization:** `serde` + `serde_json` (for `.saan` scene patterns and inspection telemetry)
- **Graphics (Phase 2 Windowed Backend):** `wgpu` + `winit`
- **CLI:** `clap` + `anyhow`
