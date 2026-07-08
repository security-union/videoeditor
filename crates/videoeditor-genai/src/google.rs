//! Google Imagen stills via the Gemini API `:predict` endpoint.
//!
//! Safe-stills provider: honors a real aspect/size param, but takes NO
//! reference images, and its safety filter rejects named real people and
//! branded characters — route those requests to [`crate::xai`]. An empty
//! `predictions` array almost always means the filter fired, so the error
//! says so instead of "no images".
//!
//! Veo (video) rides the same API family and is this module's planned
//! second resident.

use crate::{AspectRatio, GeneratedImage, ImageRequest};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MODEL: &str = "imagen-4.0-generate-001";
pub const MAX_IMAGES_PER_REQUEST: u8 = 4;

/// The subset of [`AspectRatio`] Imagen accepts.
pub const SUPPORTED_ASPECTS: [AspectRatio; 5] = [
    AspectRatio::R1x1,
    AspectRatio::R3x4,
    AspectRatio::R4x3,
    AspectRatio::R9x16,
    AspectRatio::R16x9,
];

pub fn api_key() -> Result<String> {
    std::env::var("AI_STUDIO")
        .or_else(|_| std::env::var("GEMINI_API_KEY"))
        .context("set AI_STUDIO or GEMINI_API_KEY (a Google AI Studio key)")
}

#[derive(Serialize)]
struct PredictPayload<'a> {
    instances: [Instance<'a>; 1],
    parameters: Parameters<'a>,
}

#[derive(Serialize)]
struct Instance<'a> {
    prompt: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Parameters<'a> {
    sample_count: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PredictResponse {
    #[serde(default)]
    predictions: Vec<Prediction>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Prediction {
    bytes_base64_encoded: Option<String>,
}

/// Generate `req.n` stills. Rejects reference images (Imagen has no such
/// input — use grok) and aspect ratios outside [`SUPPORTED_ASPECTS`].
pub fn generate(req: &ImageRequest) -> Result<Vec<GeneratedImage>> {
    if req.n == 0 || req.n > MAX_IMAGES_PER_REQUEST {
        bail!(
            "imagen generates 1..={MAX_IMAGES_PER_REQUEST} images per request, got {}",
            req.n
        );
    }
    if !req.reference_images.is_empty() {
        bail!(
            "imagen takes no reference images — use the grok provider for reference-conditioned stills"
        );
    }
    if req.aspect != AspectRatio::Auto && !SUPPORTED_ASPECTS.contains(&req.aspect) {
        bail!(
            "imagen supports aspect ratios 1:1, 3:4, 4:3, 9:16, 16:9 (got {})",
            req.aspect
        );
    }

    let model = req.model.as_deref().unwrap_or(DEFAULT_MODEL);
    let payload = PredictPayload {
        instances: [Instance {
            prompt: &req.prompt,
        }],
        parameters: Parameters {
            sample_count: req.n,
            aspect_ratio: match req.aspect {
                AspectRatio::Auto => None,
                a => Some(a.as_str()),
            },
        },
    };

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:predict");
    let resp = ureq::post(&url)
        .set("x-goog-api-key", &api_key()?)
        .timeout(std::time::Duration::from_secs(300))
        .send_json(serde_json::to_value(&payload)?);
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            bail!("imagen {code}: {}", r.into_string().unwrap_or_default())
        }
        Err(e) => return Err(e.into()),
    };
    let parsed: PredictResponse = resp.into_json().context("parsing imagen response")?;
    if parsed.predictions.is_empty() {
        bail!(
            "imagen returned no images — likely a safety-filter rejection \
             (named real people, branded characters, explicit content). \
             Rewrite the prompt or switch to the grok provider."
        );
    }

    parsed
        .predictions
        .into_iter()
        .map(|p| {
            let b64 = p
                .bytes_base64_encoded
                .context("imagen prediction had no image bytes")?;
            Ok(GeneratedImage {
                bytes: crate::decode_b64(&b64)?,
                revised_prompt: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_uses_camel_case_fields() {
        let payload = PredictPayload {
            instances: [Instance { prompt: "a bun" }],
            parameters: Parameters {
                sample_count: 2,
                aspect_ratio: Some(AspectRatio::R9x16.as_str()),
            },
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["parameters"]["sampleCount"], 2);
        assert_eq!(v["parameters"]["aspectRatio"], "9:16");
        assert_eq!(v["instances"][0]["prompt"], "a bun");
    }

    #[test]
    fn refs_are_rejected() {
        let req = ImageRequest {
            prompt: "x".into(),
            model: None,
            n: 1,
            aspect: AspectRatio::Auto,
            reference_images: vec!["a.png".into()],
        };
        assert!(generate(&req).unwrap_err().to_string().contains("grok"));
    }
}
