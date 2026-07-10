//! Mirrors of the JSON types `videoeditor-record` serves. Kept as a copy —
//! a shared crate would drag the server's native deps (tiny_http, ffmpeg
//! shell-outs) into the wasm build. The e2e suite exercises every field,
//! so drift breaks loudly.

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Clip {
    pub id: String,
    pub scene: String,
    pub clip: String,
    pub text: String,
    pub at: f64,
    pub tempo: f64,
    pub scene_duration: f64,
    pub window: f64,
    pub take_duration: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Episode {
    pub title: String,
    pub clips: Vec<Clip>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Pause {
    pub at: f64,
    pub len: f64,
}

/// The coach's report on one take.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Review {
    pub take: u32,
    pub duration: f64,
    pub window: f64,
    pub fits: bool,
    pub mean_db: f64,
    pub max_db: f64,
    pub clipped: bool,
    pub transcript: Option<String>,
    pub accuracy_pct: Option<f64>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub added: Vec<String>,
    pub wps: Option<f64>,
    #[serde(default)]
    pub pauses: Vec<Pause>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub coaching: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TakeInfo {
    pub file: String,
    pub duration: f64,
    pub approved: bool,
    pub review: Option<Review>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TakeResponse {
    pub duration: f64,
    pub window: f64,
    pub fits: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}
