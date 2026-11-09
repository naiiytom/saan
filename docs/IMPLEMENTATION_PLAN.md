# Phase 1 Implementation Plan — The Loom & 2.5D/3D Spine

## Goal
Build the foundational Rust workspace for the **Saan (สาน) 2.5D & 3D Game Engine**, proving out the weaving architecture:
1. **`saan_core`**: Custom ECS (`World`, `EntityId`, `Strand` components), 3D/2.5D spatial math (`glam`), `Loom` system DAG scheduler (`petgraph`) with tension/hazard inspection, and `.saan` scene file serialization.
2. **`saan_render`**: Hybrid 2.5D + 3D render graph and frame extractor that weaves 3D meshes (`Mesh3D`), 2.5D sprites (`Sprite25D` with billboard orientation & depth sorting), 3D lights (`PointLight`), and diorama cameras (`CameraRig`) into a deterministic render queue.
3. **`saan_cli`**: Command-line tool (`saan init`, `saan inspect`, `saan weave`) with full integration tests.

---

## Crate Layout

```text
Cargo.toml                          # Workspace manifest
crates/
  saan_core/
    Cargo.toml
    src/
      lib.rs                        # Public exports
      ecs.rs                        # EntityId, World, Strand storage & queries
      strands.rs                    # Transform, Sprite25D, BillboardMode, Mesh3D, PointLight, CameraRig, Velocity
      loom.rs                       # WeftSystem trait, Loom DAG scheduler, TensionReport (cycles & hazards)
      pattern.rs                    # ScenePattern (.saan file load/save & World hydration)
  saan_render/
    Cargo.toml
    src/
      lib.rs                        # RenderLoom, RenderFrame, 2.5D billboard math, depth sorting, 4-stage HD-2D pass graph
  saan_cli/
    Cargo.toml
    src/
      main.rs                       # CLI entry point (`init`, `inspect`, `weave`)
```

---

## Verification Criteria
- `cargo test --workspace` passes all unit and CLI tests.
- `saan init` creates a valid HD-2D diorama `.saan` scene file.
- `saan inspect` validates both the `.saan` scene and the Loom system + render DAG tension.
- `saan weave` steps the simulation and outputs the interlaced 3D mesh + 2.5D depth-sorted sprite draw batches.
