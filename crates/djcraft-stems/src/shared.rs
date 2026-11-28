//! DJCraft side of the ClubCraft `clubcraft-core` items the ported separator uses:
//! the progressive stem buffer, the internal sample rate, and the data / model folders.

pub use djcraft_audio::stem_track::{StemKind, StemTrack, STEM_CHANNELS, STEM_CHUNK_FRAMES};

/// Internal separation sample rate (the rate the Demucs models are trained on).
pub const SAMPLE_RATE: u32 = 44_100;

pub mod stems {
    pub use djcraft_audio::stem_track::StemChunk;
}

pub mod paths {
    use std::path::{Path, PathBuf};

    fn local_data_root() -> PathBuf {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Root of DJCraft separation data (`DJCRAFT_HOME` overrides; default
    /// `%LOCALAPPDATA%\djcraft`).
    pub fn data_dir() -> PathBuf {
        if let Some(p) = std::env::var_os("DJCRAFT_HOME") {
            return PathBuf::from(p);
        }
        local_data_root().join("djcraft")
    }

    /// Cached stem files (`<content hash>.stems.flac`, 8 channels).
    pub fn stems_dir() -> PathBuf {
        data_dir().join("Stems")
    }

    /// Where model packs are installed; survives app updates.
    pub fn user_models_dir() -> PathBuf {
        data_dir().join("Models")
    }

    /// Whether `dir` holds separation weights, not only JSON manifests.
    pub fn has_models(dir: &Path) -> bool {
        let manifest = dir.join("htdemucs.json").exists() || dir.join("htdemucs_ft.json").exists();
        manifest
            && ["htdemucs", "htdemucs_ft_0"]
                .iter()
                .any(|m| dir.join(format!("{m}.onnx")).exists())
    }

    /// Separation models: `DJCRAFT_MODELS`, then the user models folder, then `<exe dir>/models`.
    pub fn models_dir() -> PathBuf {
        if let Some(p) = std::env::var_os("DJCRAFT_MODELS") {
            return PathBuf::from(p);
        }
        let mut cands = vec![user_models_dir()];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                cands.push(dir.join("models"));
            }
        }
        cands
            .iter()
            .find(|c| has_models(c))
            .cloned()
            .unwrap_or_else(user_models_dir)
    }

    /// Whether stem separation can run.
    pub fn models_installed() -> bool {
        has_models(&models_dir())
    }
}

pub mod library {
    use std::path::PathBuf;

    /// Stem cache file for a track content hash.
    pub fn stems_path(hash: &str) -> PathBuf {
        super::paths::stems_dir().join(format!("{hash}.stems.flac"))
    }
}
