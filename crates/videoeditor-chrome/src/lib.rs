//! Minimal Chrome DevTools Protocol driver — videoeditor's capture device
//! (think `libavdevice`).
//!
//! Launching a Chrome process per frame is 1000x too slow (and Chrome 134 on
//! macOS hangs on exit in single-shot --screenshot mode). Instead: launch once
//! with --remote-debugging-port=0, connect via WebSocket, then per frame
//! `Runtime.evaluate(__sceneSeek(t))` + `Page.captureScreenshot`.

pub mod grab;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use std::fs;
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

pub struct Chrome {
    child: Child,
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
}

impl Chrome {
    pub fn launch(chrome_bin: &str, width: u32, height: u32) -> Result<Self> {
        let profile = std::env::temp_dir().join("videoeditor-chrome-profile");
        fs::create_dir_all(&profile)?;
        let port_file = profile.join("DevToolsActivePort");
        let _ = fs::remove_file(&port_file);

        let child = Command::new(chrome_bin)
            .args([
                "--headless=new",
                "--remote-debugging-port=0",
                "--disable-gpu",
                "--hide-scrollbars",
                "--no-first-run",
                "--no-default-browser-check",
                "--use-mock-keychain",
                "--password-store=basic",
                "--mute-audio",
                "--disable-extensions",
                "--disable-features=DialMediaRouteProvider,Translate",
                "--force-device-scale-factor=1",
            ])
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg(format!("--window-size={width},{height}"))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to launch {chrome_bin}"))?;

        // Chrome writes "<port>\n<browser ws path>" once DevTools is up.
        let deadline = Instant::now() + Duration::from_secs(120);
        let port: u16 = loop {
            if let Ok(s) = fs::read_to_string(&port_file) {
                if let Some(line) = s.lines().next() {
                    if let Ok(p) = line.trim().parse() {
                        break p;
                    }
                }
            }
            if Instant::now() > deadline {
                bail!("Chrome did not expose DevToolsActivePort within 120s");
            }
            std::thread::sleep(Duration::from_millis(200));
        };

        // Open a fresh page target (PUT required since Chrome 111).
        let resp: Value = ureq::request("PUT", &format!("http://127.0.0.1:{port}/json/new"))
            .call()
            .context("CDP /json/new")?
            .into_json()?;
        let ws_url = resp["webSocketDebuggerUrl"]
            .as_str()
            .context("no webSocketDebuggerUrl")?;

        let (ws, _) = tungstenite::connect(ws_url).context("CDP websocket connect")?;
        let mut chrome = Self {
            child,
            ws,
            next_id: 1,
        };

        chrome.cmd(
            "Emulation.setDeviceMetricsOverride",
            json!({ "width": width, "height": height, "deviceScaleFactor": 1, "mobile": false }),
        )?;
        Ok(chrome)
    }

    fn cmd(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({ "id": id, "method": method, "params": params });
        self.ws.send(Message::Text(msg.to_string()))?;
        loop {
            let reply = self.ws.read()?;
            if let Message::Text(text) = reply {
                let v: Value = serde_json::from_str(&text)?;
                if v["id"] == json!(id) {
                    if let Some(err) = v.get("error") {
                        bail!("CDP {method} error: {err}");
                    }
                    return Ok(v["result"].clone());
                }
                // else: CDP event or stale reply — skip
            }
        }
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        // Mark the current page so wait_ready can't be satisfied by the page
        // we are navigating AWAY from (it also defines __sceneInit).
        let _ = self.eval("window.__sceneStale = true");
        self.cmd("Page.navigate", json!({ "url": url }))?;
        self.wait_ready(
            "!window.__sceneStale && document.readyState === 'complete' && !!window.__sceneInit",
            url,
        )
    }

    /// Hand the scene its data (JSON injected via CDP — no URL-length limits,
    /// no file:// subresource policy) and wait for images to decode.
    pub fn init_scene(&mut self, data_json: &str) -> Result<()> {
        self.eval(&format!("window.__sceneInit({data_json})"))?;
        self.wait_ready(
            "Array.from(document.images).every(i => i.complete && (i.naturalWidth > 0 || !i.getAttribute('src')))",
            "scene images",
        )?;
        if std::env::var("VIDEOEDITOR_DEBUG").is_ok() {
            let probe = self.eval(
                "JSON.stringify({d: Object.keys(window.SCENE.d), imgs: Array.from(document.images)\
                 .map(i => [String(i.src).slice(0, 40), i.complete, i.naturalWidth])})",
            )?;
            println!("  debug: {probe}");
        }
        Ok(())
    }

    fn wait_ready(&mut self, expr: &str, what: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if self.eval(expr)? == json!(true) {
                return Ok(());
            }
            if Instant::now() > deadline {
                bail!("page never became ready: {what}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn eval(&mut self, expr: &str) -> Result<Value> {
        let r = self.cmd(
            "Runtime.evaluate",
            json!({ "expression": expr, "returnByValue": true }),
        )?;
        if let Some(ex) = r.get("exceptionDetails") {
            bail!("JS exception: {ex}");
        }
        Ok(r["result"]["value"].clone())
    }

    pub fn seek(&mut self, t_ms: f64) -> Result<()> {
        self.eval(&format!("window.__sceneSeek({t_ms})"))?;
        Ok(())
    }

    pub fn screenshot(&mut self, out: &Path) -> Result<()> {
        let r = self.cmd("Page.captureScreenshot", json!({ "format": "png" }))?;
        let data = r["data"].as_str().context("no screenshot data")?;
        fs::write(out, STANDARD.decode(data)?)?;
        Ok(())
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        let _ = self.ws.close(None);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Locate a Chrome/Chromium binary: `CHROME_BIN` → macOS system install →
/// common Linux binary names on PATH.
pub fn find_chrome() -> Result<String> {
    if let Ok(c) = std::env::var("CHROME_BIN") {
        return Ok(c);
    }
    let mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    if Path::new(mac).exists() {
        return Ok(mac.to_string());
    }
    for cand in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ] {
        if Command::new("which")
            .arg(cand)
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Ok(cand.to_string());
        }
    }
    bail!("Chrome not found — set CHROME_BIN")
}
