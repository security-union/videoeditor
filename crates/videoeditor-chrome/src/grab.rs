//! `videoeditor grab <url>` — research helper: fetch a page THROUGH THE USER'S
//! OWN running Chrome (logged-in sessions, no bot walls) and print its text.
//!
//! Requires Chrome started with a debugging port, e.g.:
//!   "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
//!       --remote-debugging-port=9222
//! (Quit Chrome first; this relaunches it with your normal profile.)

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::net::TcpStream;
use std::time::{Duration, Instant};
use tungstenite::{Message, WebSocket};

pub fn run(url: &str, port: u16, wait_secs: f64, selector: Option<&str>) -> Result<()> {
    // open a new tab (PUT required since Chrome 111)
    let resp = ureq::request(
        "PUT",
        &format!("http://127.0.0.1:{port}/json/new?{}", urlencode(url)),
    )
    .call()
    .with_context(|| {
        format!(
            "no Chrome DevTools on port {port} — start Chrome with \
             --remote-debugging-port={port} (quit it first)"
        )
    })?;
    let tab: Value = resp.into_json()?;
    let tab_id = tab["id"].as_str().context("no tab id")?.to_string();
    let ws_url = tab["webSocketDebuggerUrl"]
        .as_str()
        .context("no webSocketDebuggerUrl")?;

    let (mut ws, _) = tungstenite::connect(ws_url)?;
    // give the page time to load + render (SPAs need real time here)
    let deadline = Instant::now() + Duration::from_secs_f64(wait_secs.max(2.0));
    let mut id = 0u64;
    loop {
        let ready = eval(&mut ws, &mut id, "document.readyState === 'complete'")?;
        if ready == json!(true) || Instant::now() > deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    // settle a moment for client-side rendering, then extract
    std::thread::sleep(Duration::from_millis(((wait_secs.max(2.0)) * 300.0) as u64));
    let expr = match selector {
        Some(sel) => format!(
            "Array.from(document.querySelectorAll({sel:?})).map(e => e.innerText).join('\\n---\\n')"
        ),
        None => "document.body.innerText".to_string(),
    };
    let text = eval(&mut ws, &mut id, &expr)?;
    println!("{}", text.as_str().unwrap_or_default());

    let _ = ureq::get(&format!("http://127.0.0.1:{port}/json/close/{tab_id}")).call();
    Ok(())
}

fn eval(
    ws: &mut WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
    id: &mut u64,
    expr: &str,
) -> Result<Value> {
    *id += 1;
    let msg = json!({ "id": *id, "method": "Runtime.evaluate",
        "params": { "expression": expr, "returnByValue": true } });
    ws.send(Message::Text(msg.to_string()))?;
    loop {
        if let Message::Text(text) = ws.read()? {
            let v: Value = serde_json::from_str(&text)?;
            if v["id"] == json!(*id) {
                return Ok(v["result"]["result"]["value"].clone());
            }
        }
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
