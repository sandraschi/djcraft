//! xtask: ci / layers / assets gates for DJCraft.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> i32 {
    match Command::new(cmd).args(args).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("xtask: failed to run {cmd}: {e}");
            1
        }
    }
}

fn ci() -> i32 {
    let steps: &[(&str, &[&str])] = &[
        ("cargo", &["fmt", "--all", "--check"]),
        (
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("cargo", &["test", "--workspace"]),
    ];
    for (cmd, args) in steps {
        let code = run(cmd, args);
        if code != 0 {
            return code;
        }
    }
    for check in [layers, assets] {
        let code = check();
        if code != 0 {
            return code;
        }
    }
    0
}

fn layers() -> i32 {
    for rel in [
        "crates/djcraft-audio/Cargo.toml",
        "crates/djcraft-core/Cargo.toml",
        "crates/djcraft-stems/Cargo.toml",
    ] {
        let toml = match std::fs::read_to_string(rel) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("xtask layers: cannot read {rel}: {e}");
                return 1;
            }
        };
        if toml.contains("egui") || toml.contains("eframe") {
            eprintln!("xtask layers: {rel} must not depend on egui/eframe");
            return 1;
        }
    }
    // ONNX Runtime must stay out of the engine, the core and the browser build.
    for rel in [
        "crates/djcraft-audio/Cargo.toml",
        "crates/djcraft-core/Cargo.toml",
        "crates/djcraft-wasm/Cargo.toml",
    ] {
        match std::fs::read_to_string(rel) {
            Ok(toml) if toml.contains("djcraft-stems") || toml.contains("ort =") => {
                eprintln!("xtask layers: {rel} must not depend on djcraft-stems / ort");
                return 1;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("xtask layers: cannot read {rel}: {e}");
                return 1;
            }
        }
    }
    println!(
        "xtask layers: ok (audio/core/stems UI-free; ORT only in djcraft-stems and its users)"
    );
    0
}

fn assets() -> i32 {
    let out = match Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            eprintln!(
                "xtask assets: git ls-files failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            );
            return 1;
        }
        Err(e) => {
            eprintln!("xtask assets: git ls-files failed: {e}");
            return 1;
        }
    };
    let md = match std::fs::read_to_string("ATTRIBUTION.md") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("xtask assets: cannot read ATTRIBUTION.md: {e}");
            return 1;
        }
    };
    let exts = [
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "icns", "svg", "pdf",
        "ttf", "otf", "woff", "woff2",
    ];
    let mut missing = Vec::new();
    let mut count = 0usize;
    for raw in out.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = String::from_utf8_lossy(raw);
        let ext = std::path::Path::new(path.as_ref())
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if path.starts_with("docs/images/") || exts.contains(&ext.as_str()) {
            count += 1;
            if !md.contains(&format!("`{path}`")) {
                missing.push(path.into_owned());
            }
        }
    }
    if missing.is_empty() {
        println!("xtask assets: ok ({count} asset files attributed in ATTRIBUTION.md)");
        0
    } else {
        eprintln!(
            "xtask assets: {} asset(s) missing from ATTRIBUTION.md:\n  {}",
            missing.len(),
            missing.join("\n  ")
        );
        1
    }
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "ci".to_string());
    let code = match arg.as_str() {
        "ci" => ci(),
        "layers" => layers(),
        "assets" => assets(),
        other => {
            eprintln!("xtask: unknown command '{other}' (expected: ci, layers, assets)");
            2
        }
    };
    std::process::exit(code);
}
