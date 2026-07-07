mod analyze;
mod assets;
mod pack;
mod render;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "videoeditor",
    version,
    about = "Scripted short-video renderer: script.md in, rendered vertical video out"
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
