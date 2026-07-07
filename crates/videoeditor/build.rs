//! Embed build provenance in `--version` so two different builds can never
//! masquerade as each other (a stale binary claiming to be "0.1.0" while the
//! source moved on is how template features silently "disappear").

use std::process::Command;

fn main() {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    // crates.io tarballs and nix store sources have no .git — plain version
    let build = match git(&["rev-parse", "--short=9", "HEAD"]) {
        Some(hash) => {
            let dirty = git(&["status", "--porcelain"])
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if dirty {
                format!(" ({hash}, dirty)")
            } else {
                format!(" ({hash})")
            }
        }
        None => String::new(),
    };
    println!("cargo:rustc-env=VIDEOEDITOR_BUILD_INFO={build}");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
}
