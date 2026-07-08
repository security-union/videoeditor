mod analyze;
mod assets;
mod catalog;
mod pack;
mod render;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "videoeditor",
    version = concat!(env!("CARGO_PKG_VERSION"), env!("VIDEOEDITOR_BUILD_INFO")),
    about = "Scripted short-video renderer: script.md in, rendered vertical video out",
    after_help = "AI agents (Claude Code, etc.): run `videoeditor guide` for the embedded \
                  director's guide — the full production workflow, script grammar, and \
                  template-authoring pointers. Scaffolds (`new`, `pack init`) drop CLAUDE.md \
                  files that wire up your session automatically."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse an episode's script.md and print the resolved plan as JSON
    Parse { episode: PathBuf },
    /// Generate narration clips via ElevenLabs (skips existing clips)
    Tts {
        episode: PathBuf,
        /// Regenerate only this clip (scene/clip name)
        #[arg(long)]
        clip: Option<String>,
        /// Regenerate even if the clip already exists
        #[arg(long)]
        force: bool,
    },
    /// Render every scene to video via headless Chrome + ffmpeg
    Render {
        episode: PathBuf,
        /// Render only this scene
        #[arg(long)]
        scene: Option<String>,
    },
    /// Concat scenes and mix narration + clip audio + music into final.mp4
    Assemble { episode: PathBuf },
    /// Full pipeline: tts + render + assemble
    Build { episode: PathBuf },
    /// Analyze a reference video: transcript (ElevenLabs STT) + scene cuts
    Analyze {
        video: PathBuf,
        /// Output directory (default: <video dir>/analysis)
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Scene-cut detection threshold (0..1)
        #[arg(long, default_value_t = 0.12)]
        threshold: f32,
    },
    /// Scaffold a new episode directory from a format skeleton
    New {
        dir: PathBuf,
        #[arg(long, default_value = "meme-benchmark")]
        format: String,
    },
    /// Template packs: bring-your-own scene templates (see `pack init --help`)
    Pack {
        #[command(subcommand)]
        cmd: PackCmd,
    },
    /// List the template repertoire: every scene template visible from here
    /// (or from an episode), with descriptions and data keys
    Templates {
        /// Episode dir — include its packs and episode-local templates
        episode: Option<PathBuf>,
    },
    /// Render visual contact sheets of templates from their built-in demo
    /// data (all of them by default, or one by name)
    Preview {
        /// Template name (omit to preview the whole repertoire)
        template: Option<String>,
        /// Output directory for the PNG sheets
        #[arg(short, long, default_value = "template-previews")]
        out: PathBuf,
        /// Episode dir — resolve templates the way this episode would
        #[arg(long)]
        episode: Option<PathBuf>,
    },
    /// Print the embedded director's guide: production workflow, script.md
    /// grammar, template authoring — written for AI agents and humans alike
    Guide,
    /// Generate a still image with a generative model — xAI Grok Imagine
    /// (XAI_API_KEY; accepts reference images) or Google Imagen
    /// (AI_STUDIO/GEMINI_API_KEY; safety-filtered, no references)
    Image {
        /// What to render. With --ref, describe how the referenced
        /// subject should appear
        prompt: String,
        /// Output PNG path (with -n > 1: name_v1.png ... name_vN.png)
        #[arg(short, long)]
        out: PathBuf,
        /// Provider to use
        #[arg(long, value_enum, default_value_t = ImageProvider::Grok)]
        provider: ImageProvider,
        /// Reference image to condition on (repeatable, grok only, ≤ 7)
        #[arg(long = "ref")]
        refs: Vec<PathBuf>,
        /// Number of variants (grok ≤ 10, imagen ≤ 4)
        #[arg(short = 'n', long, default_value_t = 1)]
        count: u8,
        /// Aspect ratio: 1:1, 3:4, 4:3, 9:16, 16:9, 2:3, 3:2, 1:2, 2:1,
        /// auto, ... (imagen: first five only)
        #[arg(long, default_value = "auto")]
        aspect: String,
        /// Model id override (defaults: grok-imagine-image-quality /
        /// imagen-4.0-generate-001)
        #[arg(long)]
        model: Option<String>,
    },
    /// Research helper: fetch a URL through YOUR running Chrome (logged-in
    /// sessions bypass bot walls) and print the page text
    Grab {
        url: String,
        /// Chrome remote-debugging port (start Chrome with --remote-debugging-port)
        #[arg(long, default_value_t = 9222)]
        port: u16,
        /// Seconds to let the page load/render
        #[arg(long, default_value_t = 6.0)]
        wait: f64,
        /// Optional CSS selector — print matching elements instead of the whole body
        #[arg(long)]
        selector: Option<String>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ImageProvider {
    /// xAI Grok Imagine — reference images, lenient with mascots/people
    Grok,
    /// Google Imagen — aspect-true, safety-filtered, no reference images
    Imagen,
}

#[derive(Subcommand)]
enum PackCmd {
    /// Scaffold a self-contained template pack (vendors the scene runtime)
    Init { dir: PathBuf },
    /// Show an episode's template resolution: layers + where each scene's
    /// template comes from
    List { episode: PathBuf },
}

fn load(episode: &Path) -> Result<videoeditor_timeline::Episode> {
    videoeditor_timeline::load(episode, &assets::find_root()?)
}

/// Template resolution layers for catalog commands: an episode's layers when
/// one is given, otherwise cwd + $VIDEOEDITOR_PACK_PATH + built-ins.
fn catalog_roots(episode: Option<&Path>) -> Result<Vec<std::path::PathBuf>> {
    match episode {
        Some(ep) => Ok(load(ep)?.template_roots),
        None => {
            let pack_path = std::env::var("VIDEOEDITOR_PACK_PATH").ok();
            videoeditor_timeline::template_roots(
                &std::env::current_dir()?,
                &[],
                pack_path.as_deref(),
                &assets::find_root()?,
            )
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Parse { episode } => {
            let ep = load(&episode)?;
            println!("{}", serde_json::to_string_pretty(&ep)?);
        }
        Cmd::Tts {
            episode,
            clip,
            force,
        } => {
            let ep = load(&episode)?;
            videoeditor_voice::run(&ep, clip.as_deref(), force)?;
        }
        Cmd::Render { episode, scene } => {
            let ep = load(&episode)?;
            render::run(&ep, scene.as_deref())?;
        }
        Cmd::Assemble { episode } => {
            let ep = load(&episode)?;
            videoeditor_media::assemble::run(&ep)?;
        }
        Cmd::Build { episode } => {
            let ep = load(&episode)?;
            videoeditor_voice::run(&ep, None, false)?;
            render::run(&ep, None)?;
            videoeditor_media::assemble::run(&ep)?;
        }
        Cmd::Analyze {
            video,
            out,
            threshold,
        } => {
            analyze::run(&video, out.as_deref(), threshold)?;
        }
        Cmd::New { dir, format } => {
            assets::scaffold(&dir, &format)?;
        }
        Cmd::Pack { cmd } => match cmd {
            PackCmd::Init { dir } => pack::init(&dir)?,
            PackCmd::List { episode } => pack::list(&load(&episode)?)?,
        },
        Cmd::Guide => {
            print!("{}", include_str!("../guide.md"));
        }
        Cmd::Templates { episode } => {
            catalog::list(&catalog_roots(episode.as_deref())?)?;
        }
        Cmd::Preview {
            template,
            out,
            episode,
        } => {
            catalog::preview(
                &catalog_roots(episode.as_deref())?,
                template.as_deref(),
                &out,
            )?;
        }
        Cmd::Image {
            prompt,
            out,
            provider,
            refs,
            count,
            aspect,
            model,
        } => {
            let req = videoeditor_genai::ImageRequest {
                prompt,
                model,
                n: count,
                aspect: aspect.parse()?,
                reference_images: refs,
            };
            let images = match provider {
                ImageProvider::Grok => videoeditor_genai::xai::generate(&req)?,
                ImageProvider::Imagen => videoeditor_genai::google::generate(&req)?,
            };
            for (i, img) in images.iter().enumerate() {
                let path = if images.len() == 1 {
                    out.clone()
                } else {
                    let stem = out.file_stem().unwrap_or_default().to_string_lossy();
                    let ext = out.extension().unwrap_or_default().to_string_lossy();
                    out.with_file_name(format!("{stem}_v{}.{ext}", i + 1))
                };
                if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&path, &img.bytes)?;
                println!("image: wrote {}", path.display());
                if let Some(rp) = &img.revised_prompt {
                    println!("image:   revised prompt: {rp}");
                }
            }
        }
        Cmd::Grab {
            url,
            port,
            wait,
            selector,
        } => {
            videoeditor_chrome::grab::run(&url, port, wait, selector.as_deref())?;
        }
    }
    Ok(())
}
