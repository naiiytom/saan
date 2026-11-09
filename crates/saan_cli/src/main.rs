use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use saan_core::{EventWeftStrand, Loom, ScenePattern, World};
use saan_render::window::{run_windowed, WindowOptions};
use saan_render::RenderLoom;

#[derive(Parser)]
#[command(
    name = "saan",
    version,
    about = "Saan (สาน) — 2.5D & 3D Hybrid Game Engine Craft Station"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Saan HD-2D & 3D diorama scene project (.saan)
    Init {
        /// Target directory or .saan file path (default: ./scene.saan)
        #[arg(default_value = "scene.saan")]
        path: PathBuf,
        /// Overwrite existing scene file if present
        #[arg(long)]
        force: bool,
        /// Scene template ("diorama_sandbox" or "default")
        #[arg(long, default_value = "diorama_sandbox")]
        template: String,
    },
    /// Inspect a .saan scene pattern and check Loom + Render Graph tension
    Inspect {
        /// Path to the .saan scene pattern file
        #[arg(default_value = "scene.saan")]
        scene: PathBuf,
    },
    /// Weave (simulate) the Loom ECS & 2.5D/3D Render Graph headless for N frames
    Weave {
        /// Path to the .saan scene pattern file
        #[arg(default_value = "scene.saan")]
        scene: PathBuf,
        /// Number of frames to step
        #[arg(long, default_value_t = 1)]
        frames: u32,
        /// Fixed delta-time per frame in seconds
        #[arg(long, default_value_t = 0.016)]
        dt: f32,
    },
    /// Launch an interactive winit + wgpu window running the 2.5D + 3D scene
    Run {
        /// Path to the .saan scene pattern file (if missing, runs the default HD-2D Diorama scene)
        #[arg(default_value = "scene.saan")]
        scene: PathBuf,
        /// Window width in logical pixels
        #[arg(long, default_value_t = 1280)]
        width: u32,
        /// Window height in logical pixels
        #[arg(long, default_value_t = 720)]
        height: u32,
        /// Optional maximum number of rendered frames before auto-closing (useful for verification)
        #[arg(long)]
        max_frames: Option<u32>,
    },
    /// Launch the interactive Saan Craft Editor (hierarchy, strand inspector, tile brush, .saan save)
    Edit {
        /// Path to the .saan scene pattern file (if missing, loads the default HD-2D Diorama scene)
        #[arg(default_value = "scene.saan")]
        scene: PathBuf,
        /// Window width in logical pixels
        #[arg(long, default_value_t = 1280)]
        width: u32,
        /// Window height in logical pixels
        #[arg(long, default_value_t = 720)]
        height: u32,
        /// Optional maximum number of rendered frames before auto-closing (useful for verification)
        #[arg(long)]
        max_frames: Option<u32>,
    },
}

fn resolve_scene_path(path: PathBuf) -> PathBuf {
    if path.extension().and_then(|e| e.to_str()) == Some("saan") {
        path
    } else {
        path.join("scene.saan")
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            path,
            force,
            template,
        } => {
            let scene_path = resolve_scene_path(path);
            if scene_path.exists() && !force {
                bail!(
                    "Scene file '{}' already exists (use --force to overwrite)",
                    scene_path.display()
                );
            }

            let title = scene_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("Saan HD-2D Diorama");

            let pattern = if template == "default" {
                ScenePattern::default_hd2d_diorama(title)
            } else {
                ScenePattern::diorama_sandbox(title)
            };
            pattern
                .save_to_file(&scene_path)
                .with_context(|| format!("Failed to write scene to {}", scene_path.display()))?;

            println!(
                "Initialized Saan 2.5D/3D scene pattern at {}",
                scene_path.display()
            );
            println!("  Title:    {}", pattern.title);
            println!("  Entities: {} interwoven warp threads", pattern.entities.len());
        }

        Commands::Inspect { scene } => {
            let scene_path = resolve_scene_path(scene);
            let pattern = ScenePattern::load_from_file(&scene_path)
                .with_context(|| format!("Failed to load scene '{}'", scene_path.display()))?;

            let mut world = World::new();
            pattern.weave_into_world(&mut world);

            let mut loom = Loom::with_default_gameplay();
            saan_spatial::register_spatial_systems(&mut loom)?;
            saan_script::register_scripting_systems(&mut loom)?;
            saan_audio::register_audio_systems(&mut loom)?;
            let tension = loom.inspect_tension();
            let render_loom = RenderLoom::hd2d_pipeline();
            let frame = render_loom.extract_frame(&world);

            println!("=== Saan Loom & Scene Tension Report ===");
            println!("Scene:         {} (v{})", pattern.title, pattern.version);
            println!("Live Entities: {}", world.entity_count());
            println!("Strand Counts:");
            for (strand, count) in world.strand_counts() {
                println!("  - {:<18} {}", strand, count);
            }
            println!("\nGameplay Loom Tension:");
            println!("  - Taut (Valid):     {}", tension.is_taut());
            println!("  - Systems:          {:?}", tension.execution_order);
            println!("  - Hazards:          {}", tension.hazards.len());
            for h in &tension.hazards {
                println!(
                    "    * {:?}: {} <-> {} on strand '{}'",
                    h.kind, h.system_a, h.system_b, h.strand
                );
            }

            println!("\n2.5D + 3D Render Loom Schedule:");
            for pass in &frame.pass_order {
                println!("  -> {pass}");
            }
            println!(
                "  - 3D Mesh Batches:        {}",
                frame.meshes_3d.len()
            );
            println!(
                "  - 2.5D Sprite Interlaces: {} (back-to-front sorted)",
                frame.sprites_25d.len()
            );
            println!(
                "  - Shadow Casters:         {}",
                frame.shadow_casters_count
            );

            if !tension.is_taut() {
                bail!("Loom tension check failed (cycles or strand hazards detected)");
            }
        }

        Commands::Weave { scene, frames, dt } => {
            let scene_path = resolve_scene_path(scene);
            let pattern = ScenePattern::load_from_file(&scene_path)
                .with_context(|| format!("Failed to load scene '{}'", scene_path.display()))?;

            let mut world = World::new();
            pattern.weave_into_world(&mut world);

            let mut loom = Loom::with_default_gameplay();
            saan_spatial::register_spatial_systems(&mut loom)?;
            saan_script::register_scripting_systems(&mut loom)?;
            saan_audio::register_audio_systems(&mut loom)?;
            let render_loom = RenderLoom::hd2d_pipeline();

            for frame_idx in 1..=frames {
                let executed = loom.step(&mut world, dt)?;
                let render_frame = render_loom.extract_frame(&world);

                println!(
                    "[Frame {frame_idx}/{frames}] Weft systems: {:?} | 3D Meshes: {} | 2.5D Sprites: {}",
                    executed,
                    render_frame.meshes_3d.len(),
                    render_frame.sprites_25d.len()
                );
                for sprite in &render_frame.sprites_25d {
                    println!(
                        "   * 2.5D Sprite '{:<24}' mode={:?} cam_dist={:.2}m shadow={}",
                        sprite.label,
                        sprite.billboard_mode,
                        sprite.camera_distance,
                        sprite.casts_shadow
                    );
                }

                if let Some((_, bus)) = world.query_mut::<EventWeftStrand>().next() {
                    let events = bus.drain_current();
                    for ev in events {
                        println!("   ~ EventWeft: {ev:?}");
                    }
                }
            }
        }

        Commands::Run {
            scene,
            width,
            height,
            max_frames,
        } => {
            let scene_path = resolve_scene_path(scene);
            let pattern = if scene_path.exists() {
                ScenePattern::load_from_file(&scene_path)
                    .with_context(|| format!("Failed to load scene '{}'", scene_path.display()))?
            } else {
                println!(
                    "No '{}' found; weaving built-in HD-2D Diorama showcase scene...",
                    scene_path.display()
                );
                ScenePattern::default_hd2d_diorama("Saan HD-2D Courtyard")
            };

            println!("Launching Saan wgpu + winit viewport: '{}'", pattern.title);
            println!("Controls: [WASD / Arrows] Pan | [Q / E] Orbit | [R / F] Zoom | [Space] Pause | [Esc] Quit");
            println!("Woven Inspector: [Tab / F1] Toggle HUD | [1 / 2] Cycle Entity | [[ / ]] DOF Focus | [- / =] DOF Blur | [, / .] Sun Light");

            let watch_path = scene_path.exists().then_some(scene_path);
            run_windowed(
                pattern,
                WindowOptions {
                    width,
                    height,
                    max_frames,
                    watch_path,
                },
            )?;
        }

        Commands::Edit {
            scene,
            width,
            height,
            max_frames,
        } => {
            let scene_path = resolve_scene_path(scene);
            let pattern = if scene_path.exists() {
                ScenePattern::load_from_file(&scene_path)
                    .with_context(|| format!("Failed to load scene '{}'", scene_path.display()))?
            } else {
                println!(
                    "No '{}' found; creating default HD-2D Diorama scene for editor...",
                    scene_path.display()
                );
                ScenePattern::default_hd2d_diorama("Saan HD-2D Diorama")
            };

            println!("Launching Saan Craft Editor: '{}'", pattern.title);
            println!("Tools: [Tab] Cycle Tool | [T] Cycle Tile Brush | [P] Paint Tile | [1..3] Spawn Prefab");
            println!("Entity: [I/J/K/L] Translate Selected | [Space] Play/Pause Sim | [S] Save Scene | [Esc] Quit");

            saan_editor::run_editor(
                pattern,
                saan_editor::EditorOptions {
                    width,
                    height,
                    scene_path: Some(scene_path),
                    max_frames,
                },
            )?;
        }
    }

    Ok(())
}
