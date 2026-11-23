# Saan (สาน)

Saan is a 2.5D and 3D hybrid game engine modeled after traditional Thai bamboo and rattan weaving (*สาน*), where entities, components, systems, and render passes interlace into a single depth-sorted diorama fabric.

## Language

### The Loom & Architecture

**Warp**:
A generational entity identity that acts as a vertical structural thread holding strands together across frames.
_Avoid_: GameObject, Actor, Node

**Strand**:
A pure, typed data component attached to a Warp thread (such as a transform, 2.5D sprite, 3D mesh, collider, or health pool).
_Avoid_: Property, Behaviour, Attribute

**Weft**:
An active system pass that weaves horizontally through matching Strands each frame, declaring its read and write Strand access.
_Avoid_: Controller, Manager, Updater

**Loom**:
The directed acyclic graph (DAG) scheduler that orders Weft passes and Render passes and executes them deterministically every frame.
_Avoid_: WorldRunner, PipelineManager, Dispatcher

**Tension**:
The structural validity of a Loom schedule, requiring zero dependency cycles and zero unsequenced read/write or write/write hazards on any Strand.
_Avoid_: ScheduleValidation, ConflictCheck

**Pattern**:
A declarative `.saan` scene or prefab blueprint describing Warp entities and their interwoven Strands.
_Avoid_: LevelFile, MapFile, SceneGraph

**Shaver**:
An asset loader that strips raw external formats (Aseprite JSON, Wavefront OBJ, MagicaVoxel `.vox`, glTF 2.0) into engine-ready geometry, flipbooks, or textures.
_Avoid_: Importer, Converter

### Rendering & Combat

**Interlacing**:
Depth-buffered hybrid rendering where 2D billboard sprites and 3D meshes coexist in the same 3D coordinate space, cast shadows onto one another, and share clustered 3D lighting.
_Avoid_: 2DOverlay, CanvasLayer

**Diorama Tilt**:
A pitched perspective camera projection paired with tilt-shift depth-of-field blur, rim lighting, and soft bloom.
_Avoid_: IsometricCamera, PostEffectStack

**Hitbox**:
A transient damaging volume spawned by a melee swing or spell projectile that applies damage, knockback, and hitstun to opposing Hurtboxes.
_Avoid_: AttackTrigger, DamageZone

**Hurtbox**:
A vulnerable volume attached to a combatant that receives hits from opposing teams and tracks invulnerability frames.
_Avoid_: CollisionBox, HitReceiver
