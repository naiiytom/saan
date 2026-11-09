# Saan (สาน) — Game Engine Roadmap

Saan is developed in six structured phases, growing from the core **Loom ECS & 2.5D/3D Interlacing Spine** into a full interactive HD-2D & 3D game engine.

---

## Phase 1: The Loom & 2.5D/3D Interlacing Spine (Active)
- [x] **Workspace & Architecture Reset:** Move legacy data-lineage project to `saan-lineage`; initialize game engine workspace (`saan_core`, `saan_render`, `saan_cli`).
- [x] **The Warp (`World` & `Strand` ECS):** Generational entities, typed component storage, built-in 2.5D (`Sprite25D`, `BillboardMode`) and 3D (`Transform`, `Mesh3D`, `PointLight`, `CameraRig`) strands.
- [x] **The Weft (`Loom` DAG Scheduler):** System registration, read/write strand declarations, topological scheduling, and **Tension Inspection** (cycle detection & unconstrained read/write hazard detection).
- [x] **Scene Pattern (`.saan`):** Declarative scene serialization/deserialization for hybrid 2.5D/3D scenes.
- [x] **The Render Loom (`saan_render`):** Scene extraction, camera view-projection math, 2.5D billboard matrix generation, depth-sorted interlacing queue, and 4-pass HD-2D render graph (`ShadowPass` → `Opaque3DPass` → `Interlace25DPass` → `DioramaPostPass`).
- [x] **The Craft CLI (`saan_cli`):** `saan init`, `saan inspect`, and `saan weave`.

## Phase 2: Windowed wgpu Renderer & Interactive Viewport (Complete)
- [x] `winit` window lifecycle & input strand (`InputStrand` keyboard controls + `CameraOrbitWeftSystem`).
- [x] `wgpu` GPU pipeline implementation for the 4 Render Loom passes (`gpu.rs`).
- [x] WGSL shaders (`shaders.rs`) for 3D meshes + lit, shadow-casting, normal-beveled 2.5D sprites and diorama tilt-shift DOF.
- [x] `saan run` command to launch an interactive window from any `.saan` scene.

## Phase 3: Asset Shavers & Sprite Animation
- [ ] Texture atlas & Aseprite spritesheet loader (`FlipbookStrand`).
- [ ] glTF 2.0 3D environment & prop loader.
- [ ] Hot-reloading `.saan` scene patterns and WGSL shaders while running.

## Phase 4: 2.5D/3D Spatial Physics & Collision
- [ ] `saan_spatial` crate: 3D AABB, cylinder, and capsule colliders tailored for 2.5D sprites moving in 3D space.
- [ ] Ground-plane snapping, step-up stairs/slopes, and raycasting for camera occlusion fade.

## Phase 5: Audio & Scripting Strands
- [ ] 3D positional audio (`kira` / `rodio`) woven into `Transform` strands.
- [ ] Lightweight event bus (`EventWeft`) for decoupled gameplay triggers.

## Phase 6: The Woven Inspector (Visual Editor)
- [ ] In-engine overlay (`egui`) to inspect the live Loom DAG, tweak diorama tilt-shift/lighting parameters, and edit entity strands in real time.
