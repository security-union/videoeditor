//! Leptos (CSR) front end for `videoeditor record` — the narration
//! recorder's teleprompter, takes rail, and coach UI. Built to static
//! assets with trunk; `videoeditor-record` embeds the committed dist and
//! serves it, so the shipped CLI stays a single self-contained binary.

pub mod api;
pub mod app;
pub mod audio;
pub mod types;

pub use app::App;
