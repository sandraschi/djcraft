//! Stem operations shared by `djcraft-cli` and `djcraft-mcp`, so both surfaces behave the same.
//! Every function returns JSON for direct printing / MCP tool results.

use crate::shared::{library::stems_path, paths, SAMPLE_RATE};
use crate::{audio, cache, live, Quality, Separator};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;

/// Where the models are looked for and whether separation can run.
pub fn models_status() -> Value {
    let dir = paths::models_dir();
    let mut files: Vec<Value> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let size = e.metadata().ok()?.len();
                    Some(json!({ "file": name, "bytes": size }))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort_by(|a, b| a["file"].as_str().cmp(&b["file"].as_str()));
    json!({
        "models_dir": dir,
        "installed": paths::has_models(&dir),
        "files": files,
        "download": "https://github.com/sandraschi/djcraft/releases/tag/models-v1",
        "override_env": "DJCRAFT_MODELS",
    })
}

/// Cache state of one audio file.
pub fn stems_status(path: &Path) -> Result<Value> {
    let hash = live::content_hash(path)?;
    let cache_path = stems_path(&hash);
    let exists = cache_path.is_file();
    let partial = if exists {
        cache::read_partial_chunks(&cache_path).ok().flatten()
    } else {
        None
    };
    let model = if exists {
        cache::read_model_name(&cache_path).ok().flatten()
    } else {
        None
    };
    Ok(json!({
        "file": path,
        "content_hash": hash,
        "cache_path": cache_path,
        "cached": exists && partial.is_none(),
        "partial_chunks": partial.map(|c| c.len()),
        "model": model,
    }))
}

/// Separate a file completely (blocking) and cache the stems. A complete cache is reused unless
/// `force` is set. `progress` receives 0..1 and returns false to cancel.
pub fn separate_file(
    path: &Path,
    quality: Quality,
    force: bool,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<Value> {
    let hash = live::content_hash(path)?;
    let cache_path = stems_path(&hash);
    if !force && cache_path.is_file() && matches!(cache::read_partial_chunks(&cache_path), Ok(None))
    {
        return Ok(json!({
            "file": path,
            "cache_path": cache_path,
            "cached": true,
            "model": cache::read_model_name(&cache_path).ok().flatten(),
        }));
    }
    let t0 = Instant::now();
    let input = audio::decode_file(path, SAMPLE_RATE)
        .with_context(|| format!("decode {}", path.display()))?;
    let t_decode = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let sep = Separator::new(quality)?;
    let t_load = t1.elapsed().as_secs_f64();
    let t2 = Instant::now();
    let stems = sep.separate(&input, progress)?;
    let t_sep = t2.elapsed().as_secs_f64();
    std::fs::create_dir_all(paths::stems_dir())?;
    cache::save_stems_with_model(&cache_path, &stems, sep.model_name())?;
    let seconds = input.duration();
    Ok(json!({
        "file": path,
        "cache_path": cache_path,
        "cached": false,
        "model": sep.model_name(),
        "backend": sep.backend_name(),
        "audio_seconds": seconds,
        "decode_seconds": t_decode,
        "model_load_seconds": t_load,
        "separate_seconds": t_sep,
        "realtime_factor": if t_sep > 0.0 { seconds / t_sep } else { 0.0 },
    }))
}

/// Parse a quality name (`fast`, `high`, `ultra`, `best`).
pub fn parse_quality(s: &str) -> Result<Quality> {
    Quality::parse(s).with_context(|| format!("unknown quality '{s}' (fast, high, ultra, best)"))
}
