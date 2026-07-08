//! xAI Grok Imagine images — the OpenAI-compatible
//! `POST /v1/images/generations` endpoint, with multi-reference editing.
//!
//! Gotchas baked in from production use:
//! - Request `b64_json`: xAI's asset CDN has started 403-ing plain URL
//!   downloads; the `url` branch survives only as a fallback (with a
//!   browser-ish User-Agent).
//! - `reference_images` is an array of data-URL *strings* (unlike the
//!   video endpoint, which wraps each in `{"url": ...}`); at most
//!   [`MAX_REFERENCE_IMAGES`] are honored.

use crate::{GeneratedImage, ImageRequest};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MODEL: &str = "grok-imagine-image-quality";
pub const MAX_REFERENCE_IMAGES: usize = 7;
pub const MAX_IMAGES_PER_REQUEST: u8 = 10;

const IMAGES_URL: &str = "https://api.x.ai/v1/images/generations";

pub fn api_key() -> Result<String> {
    std::env::var("XAI_API_KEY").context("set XAI_API_KEY (your xAI API key)")
}

#[derive(Serialize)]
struct ImagesPayload<'a> {
    model: &'a str,
    prompt: &'a str,
    n: u8,
    response_format: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reference_images: Vec<String>,
}

#[derive(Deserialize)]
struct ImagesResponse {
    #[serde(default)]
    data: Vec<ImageDatum>,
}

#[derive(Deserialize)]
struct ImageDatum {
    url: Option<String>,
    b64_json: Option<String>,
    revised_prompt: Option<String>,
}

/// Generate `req.n` stills, optionally conditioned on reference images.
pub fn generate(req: &ImageRequest) -> Result<Vec<GeneratedImage>> {
    if req.n == 0 || req.n > MAX_IMAGES_PER_REQUEST {
        bail!(
            "grok generates 1..={MAX_IMAGES_PER_REQUEST} images per request, got {}",
            req.n
        );
    }
    if req.reference_images.len() > MAX_REFERENCE_IMAGES {
        bail!(
            "grok honors at most {MAX_REFERENCE_IMAGES} reference images, got {}",
            req.reference_images.len()
        );
    }
    let refs = req
        .reference_images
        .iter()
        .map(|p| crate::data_url(p))
        .collect::<Result<Vec<_>>>()?;

    let payload = ImagesPayload {
        model: req.model.as_deref().unwrap_or(DEFAULT_MODEL),
        prompt: &req.prompt,
        n: req.n,
        response_format: "b64_json",
        aspect_ratio: match req.aspect {
            crate::AspectRatio::Auto => None,
            a => Some(a.as_str()),
        },
        reference_images: refs,
    };

    let resp = ureq::post(IMAGES_URL)
        .set("authorization", &format!("Bearer {}", api_key()?))
        .timeout(std::time::Duration::from_secs(300))
        .send_json(serde_json::to_value(&payload)?);
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            bail!("grok image {code}: {}", r.into_string().unwrap_or_default())
        }
        Err(e) => return Err(e.into()),
    };
    let parsed: ImagesResponse = resp.into_json().context("parsing grok image response")?;
    if parsed.data.is_empty() {
        bail!("grok returned no images");
    }

    parsed
        .data
        .into_iter()
        .map(|d| {
            let bytes = match (&d.b64_json, &d.url) {
                (Some(b64), _) => crate::decode_b64(b64)?,
                (None, Some(url)) => crate::download(url)?,
                (None, None) => bail!("grok image had neither b64_json nor url"),
            };
            Ok(GeneratedImage {
                bytes,
                revised_prompt: d.revised_prompt,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AspectRatio;

    #[test]
    fn payload_omits_empty_optionals() {
        let payload = ImagesPayload {
            model: DEFAULT_MODEL,
            prompt: "a bun",
            n: 1,
            response_format: "b64_json",
            aspect_ratio: None,
            reference_images: vec![],
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert!(v.get("aspect_ratio").is_none());
        assert!(v.get("reference_images").is_none());
    }

    #[test]
    fn payload_carries_refs_as_plain_strings() {
        let payload = ImagesPayload {
            model: DEFAULT_MODEL,
            prompt: "a bun",
            n: 2,
            response_format: "b64_json",
            aspect_ratio: Some(AspectRatio::R1x1.as_str()),
            reference_images: vec!["data:image/png;base64,AAAA".into()],
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["aspect_ratio"], "1:1");
        assert_eq!(v["reference_images"][0], "data:image/png;base64,AAAA");
    }
}
