//! Build script to optionally embed an icon into the Windows EXE for pantones-gui.
//! Uses GNU windres when building for Windows targets from non-Windows hosts.
//!
//! If windres is not available, the build will proceed without embedding the icon.

use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    // Only run for Windows targets (detect via CARGO_CFG_TARGET_OS at runtime for cross-compiles)
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    // Try to locate a Windows-compatible windres (GNU toolchain) on the host.
    // Prefer the WINDRES env var, then a couple of common names/paths.
    let mut windres_candidates: Vec<String> = Vec::new();
    if let Ok(w) = env::var("WINDRES") {
        windres_candidates.push(w);
    }
    windres_candidates.extend([
        "x86_64-w64-mingw32-windres".to_string(),
        "/usr/bin/x86_64-w64-mingw32-windres".to_string(),
        "/usr/local/bin/x86_64-w64-mingw32-windres".to_string(),
        "windres".to_string(),
    ]);
    let windres_candidates = windres_candidates.as_slice();

    let windres_path = windres_candidates
        .iter()
        .filter_map(|p| {
            if Path::new(p).exists() {
                Some((*p).to_string())
            } else {
                None
            }
        })
        .next();

    let windres_path = match windres_path {
        Some(p) => p,
        None => {
            println!("cargo:warning=windres not found; skipping icon embedding");
            return;
        }
    };

    // Prepare icon resources
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let icon_rc = Path::new("assets/icon.rc");
    let icon_o = Path::new(&out_dir).join("icon.o");

    // Try to compile the icon.rc into a COFF object
    let status = Command::new(&windres_path)
        .args([
            "-i",
            icon_rc.to_str().unwrap(),
            "-O",
            "coff",
            "-o",
            icon_o.to_str().unwrap(),
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg={}", icon_o.display());
        }
        Ok(s) => {
            println!(
                "cargo:warning=windres failed with exit code {}. Icon embedding skipped.",
                s.code().unwrap_or(-1)
            );
        }
        Err(e) => {
            println!(
                "cargo:warning=failed to run windres: {}. Icon embedding skipped.",
                e
            );
        }
    }
}
