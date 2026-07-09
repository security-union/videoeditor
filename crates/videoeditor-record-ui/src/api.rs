//! Thin fetch client over the videoeditor-record REST API.

use crate::types::{Episode, Review, TakeInfo, TakeResponse};
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, Headers, Request, RequestInit, Response};

fn err(v: JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{v:?}"))
}

async fn send(url: &str, method: &str, blob: Option<&Blob>) -> Result<Response, String> {
    let init = RequestInit::new();
    init.set_method(method);
    if let Some(b) = blob {
        let headers = Headers::new().map_err(err)?;
        headers.set("content-type", &b.type_()).map_err(err)?;
        init.set_headers(&headers);
        init.set_body(b);
    }
    let req = Request::new_with_str_and_init(url, &init).map_err(err)?;
    let window = web_sys::window().ok_or("no window")?;
    let resp: Response = JsFuture::from(window.fetch_with_request(&req))
        .await
        .map_err(err)?
        .dyn_into()
        .map_err(|_| "not a Response")?;
    if !resp.ok() {
        let text = JsFuture::from(resp.text().map_err(err)?)
            .await
            .map_err(err)?;
        return Err(text
            .as_string()
            .unwrap_or_else(|| format!("http {}", resp.status())));
    }
    Ok(resp)
}

async fn json<T: DeserializeOwned>(resp: Response) -> Result<T, String> {
    let text = JsFuture::from(resp.text().map_err(err)?)
        .await
        .map_err(err)?;
    serde_json::from_str(&text.as_string().ok_or("non-text body")?).map_err(|e| e.to_string())
}

pub async fn episode() -> Result<Episode, String> {
    json(send("/api/episode", "GET", None).await?).await
}

pub async fn takes(id: &str) -> Result<Vec<TakeInfo>, String> {
    json(send(&format!("/api/takes/{id}"), "GET", None).await?).await
}

/// Archive + analyze a fresh take (the server stores it permanently).
pub async fn review(id: &str, blob: &Blob) -> Result<Review, String> {
    json(send(&format!("/api/review/{id}"), "POST", Some(blob)).await?).await
}

/// Promote an archived take to the clip's audio.
pub async fn approve(id: &str, file: &str) -> Result<TakeResponse, String> {
    json(send(&format!("/api/approve/{id}/{file}"), "POST", None).await?).await
}

/// Fallback: upload + approve in one shot (review never reached the server).
pub async fn upload_take(id: &str, blob: &Blob) -> Result<TakeResponse, String> {
    json(send(&format!("/api/take/{id}"), "POST", Some(blob)).await?).await
}
