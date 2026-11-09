//! # `saan_script` — Embedded Rhai Scripting Engine for Saan
//!
//! Provides hot-reloadable gameplay logic, unit AI, scenario triggers, and ability behaviors
//! written in Rhai and interwoven into the [`saan_core::Loom`] DAG scheduler.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use glam::Vec3;
use rhai::{Engine, AST, Scope};
use saan_core::{
    EntityId, EventWeftStrand, GameplayEvent, Loom, LoomError, ScriptStrand, Transform, Velocity,
    WeftSystem, World,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("Rhai compilation error: {0}")]
    Compile(#[from] rhai::ParseError),
    #[error("Rhai runtime error: {0}")]
    Runtime(#[from] Box<rhai::EvalAltResult>),
    #[error("Script I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Thread-safe registry for Rhai script ASTs with disk hot-reloading support.
#[derive(Default)]
pub struct ScriptHost {
    pub engine: Engine,
    asts: HashMap<String, (AST, Option<SystemTime>)>,
    scopes: HashMap<EntityId, Scope<'static>>,
}

impl ScriptHost {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        register_saan_api(&mut engine);
        Self {
            engine,
            asts: HashMap::new(),
            scopes: HashMap::new(),
        }
    }

    /// Loads or retrieves a compiled Rhai AST from disk or inline source.
    pub fn get_or_compile(&mut self, path_or_key: &str, inline_source: Option<&str>) -> Result<&AST, ScriptError> {
        let path = Path::new(path_or_key);

        if let Some(source) = inline_source {
            if !self.asts.contains_key(path_or_key) {
                let ast = self.engine.compile(source)?;
                self.asts.insert(path_or_key.to_string(), (ast, None));
            }
            return Ok(&self.asts.get(path_or_key).unwrap().0);
        }

        if path.exists() {
            let modified = fs::metadata(path).and_then(|m| m.modified()).ok();
            let needs_reload = match self.asts.get(path_or_key) {
                Some((_, prev_time)) => match (prev_time, modified) {
                    (Some(prev), Some(cur)) => cur > *prev,
                    _ => false,
                },
                None => true,
            };

            if needs_reload {
                let code = fs::read_to_string(path)?;
                let ast = self.engine.compile(&code)?;
                self.asts.insert(path_or_key.to_string(), (ast, modified));
            }

            return Ok(&self.asts.get(path_or_key).unwrap().0);
        }

        if !self.asts.contains_key(path_or_key) {
            let empty_ast = self.engine.compile("// empty default script\nfn on_tick(ctx) { return ctx; }")?;
            self.asts.insert(path_or_key.to_string(), (empty_ast, None));
        }

        Ok(&self.asts.get(path_or_key).unwrap().0)
    }

    /// Runs `on_tick(ctx)` or top-level statements on the entity's scope.
    pub fn execute_entity_tick(
        &mut self,
        entity: EntityId,
        ast: &AST,
        dt: f32,
        pos: Vec3,
        vel: Vec3,
        events: &[GameplayEvent],
    ) -> (Option<Vec3>, Option<Vec3>, Vec<String>) {
        let mut ctx = ScriptCtx::new(pos, vel, dt);
        let scope = self.scopes.entry(entity).or_insert_with(Scope::new);
        scope.set_value("ctx", ctx.clone());

        // 1. Run top-level statements if any
        let _ = self.engine.run_ast_with_scope(scope, ast);
        if let Some(c) = scope.get_value::<ScriptCtx>("ctx") {
            ctx = c;
        }

        // 2. Try calling on_tick(ctx) returning updated ctx
        if let Ok(returned_ctx) = self.engine.call_fn::<ScriptCtx>(scope, ast, "on_tick", (ctx.clone(),)) {
            ctx = returned_ctx;
        }

        // 3. Process events
        for ev in events {
            let tag = match ev {
                GameplayEvent::CollisionContact { dynamic_entity, obstacle_entity, .. } => {
                    format!("collision:{dynamic_entity}:{obstacle_entity}")
                }
                GameplayEvent::SteppedUp { entity, step_height } => {
                    format!("stepped_up:{entity}:{step_height}")
                }
                GameplayEvent::AudioCue { sound_id, .. } => {
                    format!("audio:{sound_id}")
                }
                GameplayEvent::Trigger { tag, .. } => tag.clone(),
            };
            if let Ok(returned_ctx) = self.engine.call_fn::<ScriptCtx>(scope, ast, "on_event", (ctx.clone(), tag)) {
                ctx = returned_ctx;
            }
        }

        let new_pos = (ctx.pos != pos).then_some(ctx.pos);
        let new_vel = (ctx.vel != vel).then_some(ctx.vel);

        (new_pos, new_vel, ctx.emitted_events)
    }
}

/// Context object passed directly into Rhai script functions (`on_tick(ctx)`).
#[derive(Debug, Clone)]
pub struct ScriptCtx {
    pub pos: Vec3,
    pub vel: Vec3,
    pub dt: f32,
    pub emitted_events: Vec<String>,
}

impl ScriptCtx {
    pub fn new(pos: Vec3, vel: Vec3, dt: f32) -> Self {
        Self {
            pos,
            vel,
            dt,
            emitted_events: Vec::new(),
        }
    }
}

/// Registers the Saan engine math, context and helper API into Rhai.
pub fn register_saan_api(engine: &mut Engine) {
    // Custom Vec3 operations
    engine.register_type_with_name::<Vec3>("Vec3")
        .register_fn("vec3", |x: rhai::FLOAT, y: rhai::FLOAT, z: rhai::FLOAT| Vec3::new(x as f32, y as f32, z as f32))
        .register_get_set("x", |v: &mut Vec3| v.x as rhai::FLOAT, |v: &mut Vec3, val: rhai::FLOAT| v.x = val as f32)
        .register_get_set("y", |v: &mut Vec3| v.y as rhai::FLOAT, |v: &mut Vec3, val: rhai::FLOAT| v.y = val as f32)
        .register_get_set("z", |v: &mut Vec3| v.z as rhai::FLOAT, |v: &mut Vec3, val: rhai::FLOAT| v.z = val as f32)
        .register_fn("+", |a: Vec3, b: Vec3| a + b)
        .register_fn("-", |a: Vec3, b: Vec3| a - b)
        .register_fn("*", |a: Vec3, s: rhai::FLOAT| a * (s as f32))
        .register_fn("length", |v: &mut Vec3| v.length() as rhai::FLOAT)
        .register_fn("normalize", |v: &mut Vec3| v.normalize_or_zero());

    // ScriptCtx operations
    engine.register_type_with_name::<ScriptCtx>("ScriptCtx")
        .register_get_set("pos", |ctx: &mut ScriptCtx| ctx.pos, |ctx: &mut ScriptCtx, val: Vec3| ctx.pos = val)
        .register_get_set("vel", |ctx: &mut ScriptCtx| ctx.vel, |ctx: &mut ScriptCtx, val: Vec3| ctx.vel = val)
        .register_get("dt", |ctx: &mut ScriptCtx| ctx.dt as rhai::FLOAT)
        .register_fn("set_pos", |ctx: &mut ScriptCtx, x: rhai::FLOAT, y: rhai::FLOAT, z: rhai::FLOAT| {
            ctx.pos = Vec3::new(x as f32, y as f32, z as f32);
        })
        .register_fn("set_vel", |ctx: &mut ScriptCtx, x: rhai::FLOAT, y: rhai::FLOAT, z: rhai::FLOAT| {
            ctx.vel = Vec3::new(x as f32, y as f32, z as f32);
        })
        .register_fn("emit", |ctx: &mut ScriptCtx, tag: &str| {
            ctx.emitted_events.push(tag.to_string());
        });

    // Logging helpers
    engine.register_fn("saan_log", |msg: &str| {
        println!("[Saan Rhai] {msg}");
    });
}

/// Weft system that executes Rhai gameplay scripts over entities each frame.
pub struct ScriptingWeftSystem {
    host: Arc<Mutex<ScriptHost>>,
}

impl Default for ScriptingWeftSystem {
    fn default() -> Self {
        Self {
            host: Arc::new(Mutex::new(ScriptHost::new())),
        }
    }
}

impl ScriptingWeftSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_host(host: Arc<Mutex<ScriptHost>>) -> Self {
        Self { host }
    }
}

impl WeftSystem for ScriptingWeftSystem {
    fn name(&self) -> &'static str {
        "rhai_scripting"
    }

    fn reads(&self) -> &[&'static str] {
        &["EventWeftStrand", "ScriptStrand"]
    }

    fn writes(&self) -> &[&'static str] {
        &["EventWeftStrand", "Transform", "Velocity"]
    }

    fn weave(&mut self, world: &mut World, dt: f32) {
        let mut scripts = Vec::new();

        for (entity, script) in world.query::<ScriptStrand>() {
            if !script.enabled {
                continue;
            }
            let pos = world.get_strand::<Transform>(entity).map(|t| t.translation).unwrap_or(Vec3::ZERO);
            let vel = world.get_strand::<Velocity>(entity).map(|v| v.linear).unwrap_or(Vec3::ZERO);
            scripts.push((entity, script.script_path.clone(), script.inline_source.clone(), pos, vel));
        }

        let events: Vec<GameplayEvent> = world
            .query::<EventWeftStrand>()
            .next()
            .map(|(_, bus)| bus.current_events.clone())
            .unwrap_or_default();

        let mut host = self.host.lock().unwrap();

        let mut transform_updates = Vec::new();
        let mut velocity_updates = Vec::new();
        let mut emitted_events = Vec::new();

        for (entity, path, inline, pos, vel) in scripts {
            let ast = match host.get_or_compile(&path, inline.as_deref()) {
                Ok(ast) => ast.clone(),
                Err(err) => {
                    eprintln!("Failed to compile Rhai script '{path}': {err}");
                    continue;
                }
            };

            let (new_pos, new_vel, emitted) =
                host.execute_entity_tick(entity, &ast, dt, pos, vel, &events);

            if let Some(p) = new_pos {
                transform_updates.push((entity, p));
            }
            if let Some(v) = new_vel {
                velocity_updates.push((entity, v));
            }
            for tag in emitted {
                emitted_events.push(GameplayEvent::Trigger {
                    source: entity.to_string(),
                    tag,
                });
            }
        }

        // Apply mutations back to ECS World
        for (entity, new_pos) in transform_updates {
            if let Some(transform) = world.get_strand_mut::<Transform>(entity) {
                transform.translation = new_pos;
            }
        }
        for (entity, new_vel) in velocity_updates {
            if let Some(vel) = world.get_strand_mut::<Velocity>(entity) {
                vel.linear = new_vel;
            }
        }
        if !emitted_events.is_empty() {
            if let Some((_, bus)) = world.query_mut::<EventWeftStrand>().next() {
                for ev in emitted_events {
                    bus.publish(ev);
                }
            }
        }
    }
}

/// Registers the Rhai scripting Weft system onto `loom` with proper dependency ordering.
pub fn register_scripting_systems(loom: &mut Loom) -> Result<(), LoomError> {
    loom.add_system(ScriptingWeftSystem::new())?;
    // Run scripting after input and before kinematics and collision
    loom.add_ordering("camera_orbit_input", "rhai_scripting")?;
    loom.add_ordering("rhai_scripting", "velocity_kinematics")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_inline_rhai_script_and_updates_velocity_and_pos() {
        let mut world = World::new();
        let entity = world.spawn();
        world
            .weave_strand(entity, Transform::from_xyz(0.0, 0.0, 0.0))
            .weave_strand(entity, Velocity::default())
            .weave_strand(
                entity,
                ScriptStrand {
                    script_path: "inline_test".to_string(),
                    inline_source: Some(r#"
                        fn on_tick(ctx) {
                            ctx.set_vel(5.0, 0.0, 0.0);
                            ctx.pos = ctx.pos + vec3(0.0, 0.0, 1.5);
                            return ctx;
                        }
                    "#.to_string()),
                    enabled: true,
                },
            );

        let mut sys = ScriptingWeftSystem::new();
        sys.weave(&mut world, 0.016);

        let t = world.get_strand::<Transform>(entity).unwrap();
        let v = world.get_strand::<Velocity>(entity).unwrap();

        assert!((t.translation.z - 1.5).abs() < 1e-4);
        assert!((v.linear.x - 5.0).abs() < 1e-4);
    }

    #[test]
    fn script_can_publish_gameplay_event() {
        let mut world = World::new();
        let bus_e = world.spawn();
        world.weave_strand(bus_e, EventWeftStrand::default());

        let entity = world.spawn();
        world.weave_strand(
            entity,
            ScriptStrand {
                script_path: "event_test".to_string(),
                inline_source: Some(r#"
                    fn on_tick(ctx) {
                        ctx.emit("quest:altar_discovered");
                        return ctx;
                    }
                "#.to_string()),
                enabled: true,
            },
        );

        let mut sys = ScriptingWeftSystem::new();
        sys.weave(&mut world, 0.016);

        let bus = world.get_strand::<EventWeftStrand>(bus_e).unwrap();
        assert_eq!(bus.current_events.len(), 1);
        match &bus.current_events[0] {
            GameplayEvent::Trigger { tag, .. } => assert_eq!(tag, "quest:altar_discovered"),
            _ => panic!("Expected Trigger event"),
        }
    }
}
