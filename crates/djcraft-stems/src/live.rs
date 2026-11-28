//! Live stem separation for DJCraft decks (DJCraft code, built on the ported ClubCraft separator).
//!
//! [`start`] returns at once. A worker thread looks for a cached stem file (content hash), and
//! otherwise decodes the track, hands an empty [`StemTrack`] to the caller (the deck plays the
//! original and picks up stems chunk by chunk), then fills it with
//! [`Separator::separate_into`] starting at the playhead. One separation runs at a time per
//! process; models are loaded once per quality and reused. Finished results are cached as
//! `<data>/Stems/<hash>.stems.flac`; cancelled runs save what they have as a partial file.

use crate::audio;
use crate::cache;
use crate::shared::{library::stems_path, paths, StemTrack, SAMPLE_RATE};
use crate::{Cancelled, Quality, Separator};
use anyhow::{Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Where a live job stands.
#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    /// Waiting for another separation to finish.
    Queued,
    /// Decoding the track or loading the model.
    Loading,
    Separating,
    /// All chunks ready. `cached`: loaded from the stem cache, no separation was run.
    Done {
        cached: bool,
    },
    Cancelled,
    Failed(String),
}

struct JobState {
    cancel: AtomicBool,
    progress: AtomicU32,
    status: Mutex<JobStatus>,
}

/// Handle to a running or finished live separation.
#[derive(Clone)]
pub struct LiveJob {
    inner: Arc<JobState>,
}

impl LiveJob {
    fn new() -> Self {
        LiveJob {
            inner: Arc::new(JobState {
                cancel: AtomicBool::new(false),
                progress: AtomicU32::new(0f32.to_bits()),
                status: Mutex::new(JobStatus::Queued),
            }),
        }
    }

    /// Ask the worker to stop; chunks already published stay valid.
    pub fn cancel(&self) {
        self.inner.cancel.store(true, Ordering::Relaxed);
    }

    /// Separation progress 0..1.
    pub fn progress(&self) -> f32 {
        f32::from_bits(self.inner.progress.load(Ordering::Relaxed))
    }

    pub fn status(&self) -> JobStatus {
        self.inner
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| JobStatus::Failed("status lock poisoned".into()))
    }

    pub fn is_finished(&self) -> bool {
        matches!(
            self.status(),
            JobStatus::Done { .. } | JobStatus::Cancelled | JobStatus::Failed(_)
        )
    }

    fn set_status(&self, s: JobStatus) {
        if let Ok(mut g) = self.inner.status.lock() {
            *g = s;
        }
    }

    fn set_progress(&self, p: f32) {
        self.inner
            .progress
            .store(p.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}

/// Whether separation models are installed (see [`paths::models_dir`]).
pub fn models_installed() -> bool {
    paths::models_installed()
}

/// Stable content hash of a file (FNV-1a 64 over its bytes, plus the length), used as the stem
/// cache key so renamed or moved files keep their stems.
pub fn content_hash(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut len: u64 = 0;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        len += n as u64;
        for b in &buf[..n] {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    Ok(format!("{h:016x}{len:012x}"))
}

/// Cache file for a track, if a complete one exists.
pub fn cached_stems_path(path: &Path) -> Option<PathBuf> {
    let p = stems_path(&content_hash(path).ok()?);
    let complete = p.is_file() && matches!(cache::read_partial_chunks(&p), Ok(None));
    complete.then_some(p)
}

/// Loaded separators, one per quality (loading a model takes seconds).
type SeparatorCache = Mutex<Vec<(Quality, Arc<Separator>)>>;

fn separators() -> &'static SeparatorCache {
    static S: OnceLock<SeparatorCache> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}

/// One separation at a time in this process (the model saturates the CPU or GPU on its own).
fn heavy() -> &'static Mutex<()> {
    static H: OnceLock<Mutex<()>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(()))
}

fn separator(q: Quality) -> Result<Arc<Separator>> {
    let mut list = separators()
        .lock()
        .map_err(|_| anyhow::anyhow!("separator cache poisoned"))?;
    if let Some((_, s)) = list.iter().find(|(k, _)| *k == q) {
        return Ok(Arc::clone(s));
    }
    let s = Arc::new(Separator::new(q)?);
    log::info!(
        "djcraft stems: loaded {} on {}",
        s.model_name(),
        s.backend_name()
    );
    list.push((q, Arc::clone(&s)));
    Ok(s)
}

/// Start live separation of `path` with the playhead at `start_sec`. `on_track` is called once
/// (on the worker thread) with the track to attach to the deck: a complete one from the cache,
/// or an empty one that fills while it plays.
pub fn start<F>(path: PathBuf, start_sec: f64, quality: Quality, on_track: F) -> LiveJob
where
    F: FnOnce(Arc<StemTrack>) + Send + 'static,
{
    let job = LiveJob::new();
    let worker = job.clone();
    let spawned = std::thread::Builder::new()
        .name("djcraft-stems".into())
        .spawn(move || {
            worker.set_status(JobStatus::Loading);
            match run(&worker, &path, start_sec, quality, on_track) {
                Ok(status) => worker.set_status(status),
                Err(e) => {
                    log::warn!("djcraft stems: {} failed: {e:#}", path.display());
                    worker.set_status(JobStatus::Failed(format!("{e:#}")));
                }
            }
        });
    if let Err(e) = spawned {
        job.set_status(JobStatus::Failed(format!("cannot start stem worker: {e}")));
    }
    job
}

fn run<F>(
    job: &LiveJob,
    path: &Path,
    start_sec: f64,
    quality: Quality,
    on_track: F,
) -> Result<JobStatus>
where
    F: FnOnce(Arc<StemTrack>),
{
    let cache_path = stems_path(&content_hash(path)?);
    let partial = if cache_path.is_file() {
        match cache::read_partial_chunks(&cache_path) {
            Ok(None) => {
                let track = Arc::new(cache::load_stem_track(&cache_path)?);
                job.set_progress(1.0);
                on_track(track);
                return Ok(JobStatus::Done { cached: true });
            }
            Ok(Some(_)) => cache::load_stem_track(&cache_path).ok(),
            Err(_) => None,
        }
    } else {
        None
    };

    let audio = audio::decode_file(path, SAMPLE_RATE)?;
    let model = quality.options().model;
    let track = match partial {
        Some(t) if t.frames == audio.frames() => Arc::new(t),
        _ => Arc::new(StemTrack::new(audio.frames(), model.clone())),
    };
    on_track(Arc::clone(&track));
    if track.is_complete() {
        return Ok(JobStatus::Done { cached: true });
    }

    job.set_status(JobStatus::Queued);
    let _heavy = heavy()
        .lock()
        .map_err(|_| anyhow::anyhow!("separation lock poisoned"))?;
    if job.inner.cancel.load(Ordering::Relaxed) {
        return Ok(JobStatus::Cancelled);
    }
    job.set_status(JobStatus::Loading);
    let sep = separator(quality)?;
    job.set_status(JobStatus::Separating);
    let start_frame =
        ((start_sec.max(0.0) * SAMPLE_RATE as f64) as usize).min(audio.frames().saturating_sub(1));
    let res = sep.separate_into(
        &audio,
        &track,
        start_frame,
        &mut |p| job.set_progress(p),
        &job.inner.cancel,
    );
    std::fs::create_dir_all(paths::stems_dir())?;
    match res {
        Ok(()) => {
            job.set_progress(1.0);
            if let Err(e) = cache::save_stem_track_as(&cache_path, &track, &model) {
                log::warn!(
                    "djcraft stems: cannot cache {}: {e:#}",
                    cache_path.display()
                );
            }
            Ok(JobStatus::Done { cached: false })
        }
        Err(e) if e.downcast_ref::<Cancelled>().is_some() => {
            if track.ready_fraction() > 0.0 {
                let _ = cache::save_partial_stem_track(&cache_path, &track, &model);
            }
            Ok(JobStatus::Cancelled)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_stable_and_content_based() {
        let dir = std::env::temp_dir().join(format!("djcraft-stems-hash-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let a = dir.join("a.bin");
        let b = dir.join("b.bin");
        let c = dir.join("c.bin");
        let _ = std::fs::write(&a, b"same bytes");
        let _ = std::fs::write(&b, b"same bytes");
        let _ = std::fs::write(&c, b"other bytes");
        let ha = content_hash(&a).ok();
        assert!(ha.is_some());
        assert_eq!(ha, content_hash(&b).ok(), "same content, same key");
        assert_ne!(
            ha,
            content_hash(&c).ok(),
            "different content, different key"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
