//! Generative-asset clients: typed bindings for the image APIs of xAI
//! (Grok Imagine) and Google (Imagen), so episodes can optionally generate
//! stills — memes, logos-in-costume, backdrops — straight from the CLI.
//! Video generation (Grok Imagine video, Google Veo) is the planned next
//! tenant of this crate; the module split anticipates it.
//!
//! Provider fit (learned in production, keep in mind when routing):
//! - **Grok** accepts up to [`xai::MAX_REFERENCE_IMAGES`] reference images
//!   (data-URL encoded) and is lenient about brand mascots and named people.
//! - **Imagen** honors a real aspect/size API param but takes NO reference
//!   images, and its safety filter rejects named real people — requests
//!   that need either belong to Grok.

pub mod google;
pub mod xai;

use anyhow::{Context, Result, bail};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// One still-image request, provider-agnostic. Each backend consumes the
/// fields it supports and rejects what it can't honor (no silent drops).
#[derive(Clone, Debug)]
pub struct ImageRequest {
    pub prompt: String,
    /// Explicit model id; `None` = the provider's default.
    pub model: Option<String>,
    /// Variants to generate (grok ≤ 10, imagen ≤ 4).
    pub n: u8,
    pub aspect: AspectRatio,
    /// Local images the model should condition on (grok only).
    pub reference_images: Vec<PathBuf>,
}

/// A generated still plus whatever the provider said about it.
#[derive(Debug)]
pub struct GeneratedImage {
    pub bytes: Vec<u8>,
    /// Grok rewrites prompts before rendering; surfaced for prompt QC.
    pub revised_prompt: Option<String>,
}

/// Aspect ratios the xAI image endpoint enumerates (probed 2026-07-07 via
/// its 400 on an unknown variant). Imagen accepts the subset noted on
/// [`google::SUPPORTED_ASPECTS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum AspectRatio {
    R1x1,
    R3x4,
    R4x3,
    R9x16,
    R16x9,
    R2x3,
    R3x2,
    R9x19_5,
    R19_5x9,
    R9x20,
    R20x9,
    R1x2,
    R2x1,
    /// Let the model pick framing from the prompt.
    Auto,
}

impl AspectRatio {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::R1x1 => "1:1",
            Self::R3x4 => "3:4",
            Self::R4x3 => "4:3",
            Self::R9x16 => "9:16",
            Self::R16x9 => "16:9",
            Self::R2x3 => "2:3",
            Self::R3x2 => "3:2",
            Self::R9x19_5 => "9:19.5",
            Self::R19_5x9 => "19.5:9",
            Self::R9x20 => "9:20",
            Self::R20x9 => "20:9",
            Self::R1x2 => "1:2",
            Self::R2x1 => "2:1",
            Self::Auto => "auto",
        }
    }
}

impl fmt::Display for AspectRatio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AspectRatio {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "1:1" => Self::R1x1,
            "3:4" => Self::R3x4,
            "4:3" => Self::R4x3,
            "9:16" => Self::R9x16,
            "16:9" => Self::R16x9,
            "2:3" => Self::R2x3,
            "3:2" => Self::R3x2,
            "9:19.5" => Self::R9x19_5,
            "19.5:9" => Self::R19_5x9,
            "9:20" => Self::R9x20,
            "20:9" => Self::R20x9,
            "1:2" => Self::R1x2,
            "2:1" => Self::R2x1,
            "auto" => Self::Auto,
            other => bail!(
                "unknown aspect ratio {other:?} (expected one of 1:1, 3:4, 4:3, \
                 9:16, 16:9, 2:3, 3:2, 9:19.5, 19.5:9, 9:20, 20:9, 1:2, 2:1, auto)"
            ),
        })
    }
}

/// Encode a local image as a `data:` URL for a JSON body.
pub(crate) fn data_url(image: &Path) -> Result<String> {
    use base64::Engine as _;
    let bytes = std::fs::read(image).with_context(|| format!("reading {}", image.display()))?;
    let mime = match image
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "jpeg",
        Some("webp") => "webp",
        _ => "png",
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:image/{mime};base64,{b64}"))
}

/// Download a provider-hosted asset. xAI's CDN 403s UA-less requests
/// (observed June 2026), so send a browser-ish User-Agent.
pub(crate) fn download(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url)
        .set("user-agent", "Mozilla/5.0 (videoeditor)")
        .timeout(std::time::Duration::from_secs(300))
        .call()
        .with_context(|| format!("downloading {url}"))?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut resp.into_reader(), &mut bytes)?;
    Ok(bytes)
}

pub(crate) fn decode_b64(b64: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;
    Ok(base64::engine::general_purpose::STANDARD.decode(b64)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_ratio_roundtrips() {
        for s in [
            "1:1", "3:4", "4:3", "9:16", "16:9", "2:3", "3:2", "9:19.5", "19.5:9", "9:20", "20:9",
            "1:2", "2:1", "auto",
        ] {
            assert_eq!(s.parse::<AspectRatio>().unwrap().as_str(), s);
        }
        assert!("21:9".parse::<AspectRatio>().is_err());
    }
}
