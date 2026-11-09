# Saan — Agent Context

This file gives AI agents working on this repo the context they need to contribute effectively.

## What Saan Is

**Saan (สาน)** is a **2.5D and 3D hybrid game engine** written in Rust, inspired by traditional Thai bamboo/rattan weaving (*สาน*). It treats game architecture as a woven structure:
- **Strands (`Strand`)**: Typed components and assets (transforms, 2D sprites with normal maps, 3D meshes, materials, cameras, lights).
- **The Loom (`Loom`)**: Custom lightweight Entity-Component-System (ECS) and directed acyclic graph (DAG) system scheduler (`petgraph`). Entities form the **Warp** (structural threads) and Systems form the **Weft** (active passes woven through components every frame).
- **Interlacing (`saan_render`)**: First-class **HD-2D & Diorama** hybrid rendering (`wgpu` + `glam`) where 2D pixel/vector sprites, billboards, and tilemaps coexist seamlessly inside a lit, shadow-casting 3D world alongside 3D meshes.
- **Patterns (`.saan`)**: Declarative scene and prefab descriptions that bind strands together.
- **Tension (`inspect`)**: Compile/load-time and runtime validation of the system & render DAGs (detecting read/write conflicts, cycles, unreferenced assets, and frame bottlenecks).

> Note: The previous data-lineage project that lived in this repository has been archived and moved to `D:\work\saan-lineage`.

## Docs

- `docs/TECHNICAL_SPECIFICATIONS.md` — architecture reference, weaving metaphor mapping, core components, and tech stack
- `docs/ROADMAP.md` — phased delivery plan for the Saan 2.5D/3D Game Engine
- `docs/IMPLEMENTATION_PLAN.md` — detailed work breakdown for Phase 1 (The Loom ECS, DAG Scheduler, Scene Pattern, and HD-2D Render Graph foundation)

## Architecture

```
saan_core     (The Loom)   — Custom ECS (Warp/Entities, Strands/Components), Weft DAG scheduler, Transform/Math, Scene Pattern (.saan)
saan_render   (The Mesh)   — Hybrid 2.5D (HD-2D sprites/billboards) + 3D mesh render graph, cameras, lighting, depth interlacing
saan_cli      (The Craft)  — CLI toolbelt (`saan init`, `saan inspect`, `saan weave`, `saan run`)
```

## Core Concepts in `saan_core`

1. **`EntityId` & `World` (The Warp):**
   - Lightweight generational entity IDs and typed component columns (`StrandStorage`).
   - Query API for fetching entities with combinations of 2.5D and 3D `Strand` components (e.g., `Transform3D`, `Sprite25D`, `Mesh3D`, `CameraRig`, `PointLight3D`).

2. **`WeftSystem` & `Loom` (The Weft & Scheduler):**
   - Systems declare their read/write component access (`reads()`, `writes()`) and explicit ordering dependencies.
   - `Loom` builds a `petgraph` DAG of systems and render passes, validates **Tension** (detecting cycles or conflicting read/write access on the same strand without an ordering edge), and executes systems in deterministic topological order.

3. **`Pattern` (`.saan` Scene Format):**
   - Serializable RON/JSON scene format describing entities and their interwoven strands (transforms, 2.5D sprites, 3D meshes, lights, camera rigs).

## Core Concepts in `saan_render`

1. **HD-2D & Diorama Interlacing:**
   - Unifies 2D sprites (`Sprite25D` with billboard modes: `WorldPlane`, `YAxisBillboard`, `FullBillboard`, normal maps, depth offset, and shadow casting) and 3D geometry (`Mesh3D`, `Material3D`) in a single depth-buffered pipeline.
2. **Render Loom (Render Graph):**
   - Built-in render passes woven as a DAG:
     `ShadowPass` → `Opaque3DPass` → `Interlace25DPass` (depth-sorted/alpha-tested 2.5D sprites + 3D lighting) → `PostProcessPass` (tilt-shift/DOF, bloom, vignette for diorama aesthetics).

## CLI Commands (`saan_cli`)

| Command | Phase 1 Status | Description |
|---|---|---|
| `saan init [path] [--force]` | Implemented | Scaffolds a new Saan game project & default `.saan` scene pattern |
| `saan inspect [scene]` | Implemented | Checks "tension" on the Loom DAG and validates a `.saan` scene pattern |
| `saan weave [scene]` | Implemented | Steps the Loom + Render Graph headless for verification & frame telemetry |
| `saan run [scene]` | Implemented | Opens interactive `winit` + `wgpu` window running the scene |

## Running Locally

```bash
cargo build                 # builds all workspace crates
cargo test --workspace      # runs all unit and integration tests
cargo run -p saan_cli -- inspect
```

## Agent skills

### Issue tracker

Issues and specs are tracked in GitHub Issues (`naiiytom/saan`) via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the five default canonical triage labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout (`CONTEXT.md` at the repo root and ADRs in `docs/adr/`). See `docs/agents/domain.md`.

