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

## Phase 3: Asset Shavers & Sprite Animation (Complete)
- [x] `saan_asset` crate: Aseprite JSON sprite sheet shaver (`AsepriteSheet` → `FlipbookStrand`), Wavefront `.obj` 3D mesh parser (`MeshAssetData`), and RGBA8 sprite sheet generator (`RgbaTexture`).
- [x] `FlipbookStrand` & `FlipbookAnimationWeftSystem` in `saan_core` + GPU texture/sampler binding in `saan_render` for animated 2.5D sprites.
- [x] Hot-reloading `.saan` scene patterns (`SceneHotReloader`) while `saan run` is active.

## Phase 4: 2.5D/3D Spatial Physics & Collision (Complete)
- [x] `saan_spatial` crate: 3D AABB (`Aabb3D`) and upright cylinder (`CylinderY`) colliders (`Collider3D`) tailored for 2.5D sprites moving in 3D space.
- [x] Ground-plane snapping, automatic stair/ledge step-up (`max_step_height`), and raycasting (`ray_intersects_aabb`) for foreground camera occlusion fade (`CameraOcclusionWeftSystem`).

## Phase 5: Audio & Scripting Strands (Complete)
- [x] 3D positional audio strand (`AudioEmitter3D`) and `PositionalAudioWeftSystem` computing stereo panning & inverse-distance attenuation relative to the active `CameraRig`.
- [x] Lightweight event bus (`EventWeftStrand` & `GameplayEvent`) woven into spatial collision and 3D audio systems for decoupled gameplay triggers.

## Phase 6: The Woven Inspector (Visual Editor) (Complete)
- [x] In-engine Woven Inspector HUD overlay (`InspectorStrand` + `append_woven_inspector_hud` in `saan_render::gpu`) to inspect the live Loom DAG nodes, tweak diorama tilt-shift DOF focus/blur and directional lighting parameters, and cycle entity strands in real time.
