//! # `saan_core` — The Loom (สาน)
//!
//! Core Entity-Component-System (ECS), 2.5D/3D spatial strands, `petgraph`-backed
//! DAG system scheduler (`Loom`), and `.saan` scene pattern serialization.

pub mod ecs;
pub mod loom;
pub mod pattern;
pub mod strands;

pub use ecs::{EntityId, Strand, World};
pub use loom::{
    CameraOrbitWeftSystem, FlipbookAnimationWeftSystem, HazardKind, Loom, LoomError,
    PositionalAudioWeftSystem, StrandHazard, TensionReport, VelocityKinematicsSystem, WeftNode,
    WeftSystem,
};
pub use pattern::{EntityBlueprint, SceneHotReloader, ScenePattern};
pub use strands::{
    AudioEmitter3D, BillboardMode, CameraProjection, CameraRig, Collider3D, ColliderShape,
    DirectionalLight, EventWeftStrand, FlipbookStrand, GameplayEvent, InputStrand,
    InspectorStrand, Material3D, Mesh3D, MeshPrimitive, NameStrand, PointLight, ScriptStrand,
    Sprite25D, Transform, Velocity,
};
