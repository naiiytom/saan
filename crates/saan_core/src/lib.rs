//! # `saan_core` — The Loom (สาน)
//!
//! Core Entity-Component-System (ECS), 2.5D/3D spatial strands, `petgraph`-backed
//! DAG system scheduler (`Loom`), and `.saan` scene pattern serialization.

pub mod ecs;
pub mod loom;
pub mod pattern;
pub mod strands;

pub use ecs::{EntityId, Strand, World};
pub use loom::{HazardKind, Loom, LoomError, StrandHazard, TensionReport, WeftNode, WeftSystem};
pub use pattern::{EntityBlueprint, ScenePattern};
pub use strands::{
    BillboardMode, CameraProjection, CameraRig, DirectionalLight, Material3D, Mesh3D,
    MeshPrimitive, NameStrand, PointLight, Sprite25D, Transform, Velocity,
};
