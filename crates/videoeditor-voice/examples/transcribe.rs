//! Transcribe one audio file with the resolved STT backend and print the
//! normalized transcript JSON:
//!
//! ```sh
//! cargo run -p videoeditor-voice --example transcribe -- take.mp3
//! ```

use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let audio: PathBuf = std::env::args_os()
        .nth(1)
        .context("usage: transcribe <audio-file>")?
        .into();
    eprintln!("stt backend: {}", videoeditor_voice::stt_name());
    let transcript = videoeditor_voice::stt(&audio)?;
    println!("{}", serde_json::to_string_pretty(&transcript)?);
    Ok(())
}
