#![forbid(unsafe_code)]
//! Persistent SQLite Crate & Subcrate Library with Serato DJ (`.crate`) and
//! VirtualDJ (`.vdjfolder` / `.m3u8`) import/export, sorting, filtering, and batch audio analysis.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use djcraft_audio::dsp::{
    decode_audio_file, detect_beatgrid, has_sidecar_stems, stem_dir_for_audio,
};
use djcraft_audio::AudioEngine;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::command::run_dettato_sidecar;
use crate::DjError;

const SERATO_VERSION_HEADER: &str = "1.0/Serato ScratchLive Crate";
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "m4a", "aac"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateNode {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub full_path: String,
    pub track_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateTrack {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub genre: String,
    pub bpm: f64,
    pub musical_key: String,
    pub duration_seconds: f64,
    pub beatgrid_offset_sec: f64,
    pub tuning_cents: Option<f64>,
    pub drum_hits: Option<i64>,
    pub rating: i32,
    pub analyzed_at: Option<String>,
    #[serde(default)]
    pub crate_position: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrateSortField {
    ManualOrder,
    Title,
    Artist,
    Bpm,
    MusicalKey,
    Duration,
    Rating,
    Added,
}

impl CrateSortField {
    pub const ALL: [Self; 8] = [
        Self::ManualOrder,
        Self::Title,
        Self::Artist,
        Self::Bpm,
        Self::MusicalKey,
        Self::Duration,
        Self::Rating,
        Self::Added,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ManualOrder => "Gig Order (#)",
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Bpm => "BPM",
            Self::MusicalKey => "Key",
            Self::Duration => "Duration",
            Self::Rating => "Rating",
            Self::Added => "Added",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateQuery {
    pub crate_id: Option<i64>,
    pub include_subcrates: bool,
    pub search: String,
    pub bpm_min: Option<f64>,
    pub bpm_max: Option<f64>,
    pub key_filter: Option<String>,
    pub compatible_with_key: Option<String>,
    pub analyzed_only: bool,
    pub sort_by: CrateSortField,
    pub sort_asc: bool,
}

impl Default for CrateQuery {
    fn default() -> Self {
        Self {
            crate_id: None,
            include_subcrates: true,
            search: String::new(),
            bpm_min: None,
            bpm_max: None,
            key_filter: None,
            compatible_with_key: None,
            analyzed_only: false,
            sort_by: CrateSortField::ManualOrder,
            sort_asc: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub format: String,
    pub crate_id: i64,
    pub crate_path: String,
    pub imported_tracks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStemSummary {
    pub total: usize,
    pub stemmed: usize,
    pub skipped_existing: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

pub struct CrateStore {
    pub(crate) conn: Connection,
}

impl CrateStore {
    pub fn default_db_path() -> Option<PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("XDG_DATA_HOME"))
            .map(PathBuf::from)?;
        Some(base.join("djcraft").join("crates.db"))
    }

    pub fn open_default() -> Result<Self, DjError> {
        if let Some(path) = Self::default_db_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            Self::open(&path)
        } else {
            Self::open_in_memory()
        }
    }

    pub fn open(path: &Path) -> Result<Self, DjError> {
        let conn = Connection::open(path)
            .map_err(|e| DjError::Deck(format!("Cannot open crates DB: {e}")))?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, DjError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| DjError::Deck(format!("Cannot open in-memory crates DB: {e}")))?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), DjError> {
        self.conn
            .execute_batch(
                "
                PRAGMA foreign_keys = ON;
                CREATE TABLE IF NOT EXISTS crates (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    parent_id INTEGER REFERENCES crates(id) ON DELETE CASCADE,
                    full_path TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS tracks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT NOT NULL UNIQUE,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL DEFAULT '',
                    genre TEXT NOT NULL DEFAULT '',
                    bpm REAL NOT NULL DEFAULT 0.0,
                    musical_key TEXT NOT NULL DEFAULT '',
                    duration_seconds REAL NOT NULL DEFAULT 0.0,
                    beatgrid_offset_sec REAL NOT NULL DEFAULT 0.0,
                    tuning_cents REAL,
                    drum_hits INTEGER,
                    rating INTEGER NOT NULL DEFAULT 0,
                    analyzed_at TEXT,
                    added_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS crate_tracks (
                    crate_id INTEGER NOT NULL REFERENCES crates(id) ON DELETE CASCADE,
                    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                    position INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (crate_id, track_id)
                );
                CREATE TABLE IF NOT EXISTS track_cues (
                    track_path TEXT NOT NULL,
                    slot INTEGER NOT NULL,
                    position_seconds REAL NOT NULL,
                    PRIMARY KEY (track_path, slot)
                );
                CREATE TABLE IF NOT EXISTS track_cue_meta (
                    track_path TEXT NOT NULL,
                    slot INTEGER NOT NULL,
                    label TEXT NOT NULL DEFAULT '',
                    color_rgb TEXT,
                    behavior TEXT NOT NULL DEFAULT 'standard',
                    PRIMARY KEY (track_path, slot)
                );
                CREATE TABLE IF NOT EXISTS smart_crates (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    rule_json TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS set_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    deck TEXT NOT NULL,
                    track_path TEXT NOT NULL,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL DEFAULT '',
                    bpm REAL NOT NULL DEFAULT 0.0,
                    musical_key TEXT NOT NULL DEFAULT '',
                    played_at_sec REAL NOT NULL DEFAULT 0.0,
                    played_at_utc TEXT NOT NULL
                );
                ",
            )
            .map_err(|e| DjError::Deck(format!("Crates schema init failed: {e}")))?;
        Ok(())
    }

    /// Seed initial demo crate when the database has zero tracks so the UI is immediately usable.
    pub fn seed_demo_if_empty(&self) -> Result<(), DjError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);
        if count > 0 {
            return Ok(());
        }
        let crate_id = self.ensure_crate_path("Demo Crate")?;
        let demos = [
            (
                "demo://subterranean_dub",
                "Subterranean Dub",
                "Resonant Swarm",
                "Dub Techno",
                126.0,
                "8A",
                384.0,
            ),
            (
                "demo://solaris_flare",
                "Solaris Flare",
                "Antigravity Sound",
                "Melodic Techno",
                128.0,
                "9A",
                412.0,
            ),
            (
                "demo://danube_midnight",
                "Danube Midnight",
                "Vienna Modular",
                "Deep House",
                126.0,
                "8B",
                356.0,
            ),
            (
                "demo://velvet_acid_pulse",
                "Velvet Acid Pulse",
                "Klangwerk",
                "Acid House",
                132.0,
                "7A",
                398.0,
            ),
        ];
        for (path, title, artist, genre, bpm, key, dur) in demos {
            let tid = self.upsert_track(path, title, artist, genre, bpm, key, dur)?;
            self.add_track_to_crate(crate_id, tid)?;
        }
        Ok(())
    }

    /// Ensure a slash-separated crate hierarchy (e.g. `"House/Deep House"`) exists, returning leaf id.
    pub fn ensure_crate_path(&self, full_path: &str) -> Result<i64, DjError> {
        let segments: Vec<&str> = full_path
            .split('/')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if segments.is_empty() {
            return Err(DjError::Deck("Crate path cannot be empty".to_string()));
        }

        let mut parent_id: Option<i64> = None;
        let mut accum = String::new();
        let now = Utc::now().to_rfc3339();

        for seg in segments {
            if !accum.is_empty() {
                accum.push('/');
            }
            accum.push_str(seg);

            let existing: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM crates WHERE full_path = ?1",
                    params![&accum],
                    |r| r.get(0),
                )
                .ok();
            let id = if let Some(found) = existing {
                found
            } else {
                self.conn
                    .execute(
                        "INSERT INTO crates (name, parent_id, full_path, created_at) VALUES (?1, ?2, ?3, ?4)",
                        params![seg, parent_id, &accum, &now],
                    )
                    .map_err(|e| DjError::Deck(format!("Create crate '{accum}' failed: {e}")))?;
                self.conn.last_insert_rowid()
            };
            parent_id = Some(id);
        }

        parent_id.ok_or_else(|| DjError::Deck("Failed to resolve crate id".to_string()))
    }

    pub fn create_subcrate(&self, parent_id: Option<i64>, name: &str) -> Result<i64, DjError> {
        let clean = name.trim().replace('/', "-");
        if clean.is_empty() {
            return Err(DjError::Deck("Crate name cannot be empty".to_string()));
        }
        let full_path = match parent_id {
            Some(pid) => {
                let parent_path: String = self
                    .conn
                    .query_row(
                        "SELECT full_path FROM crates WHERE id = ?1",
                        params![pid],
                        |r| r.get(0),
                    )
                    .map_err(|e| DjError::Deck(format!("Parent crate {pid} not found: {e}")))?;
                format!("{parent_path}/{clean}")
            }
            None => clean,
        };
        self.ensure_crate_path(&full_path)
    }

    pub fn list_crates(&self) -> Result<Vec<CrateNode>, DjError> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT c.id, c.name, c.parent_id, c.full_path,
                       (SELECT COUNT(*) FROM crate_tracks ct WHERE ct.crate_id = c.id) AS cnt
                FROM crates c
                ORDER BY c.full_path COLLATE NOCASE ASC
                ",
            )
            .map_err(|e| DjError::Deck(format!("List crates failed: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(CrateNode {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    parent_id: r.get(2)?,
                    full_path: r.get(3)?,
                    track_count: r.get::<_, i64>(4)?.max(0) as usize,
                })
            })
            .map_err(|e| DjError::Deck(format!("Read crates failed: {e}")))?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| DjError::Deck(format!("Row error: {e}")))?);
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_track(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        genre: &str,
        bpm: f64,
        musical_key: &str,
        duration_seconds: f64,
    ) -> Result<i64, DjError> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "
                INSERT INTO tracks (path, title, artist, genre, bpm, musical_key, duration_seconds, added_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(path) DO UPDATE SET
                    title = CASE WHEN excluded.title != '' THEN excluded.title ELSE tracks.title END,
                    artist = CASE WHEN excluded.artist != '' THEN excluded.artist ELSE tracks.artist END,
                    genre = CASE WHEN excluded.genre != '' THEN excluded.genre ELSE tracks.genre END,
                    bpm = CASE WHEN excluded.bpm > 0 THEN excluded.bpm ELSE tracks.bpm END,
                    musical_key = CASE WHEN excluded.musical_key != '' THEN excluded.musical_key ELSE tracks.musical_key END,
                    duration_seconds = CASE WHEN excluded.duration_seconds > 0 THEN excluded.duration_seconds ELSE tracks.duration_seconds END
                ",
                params![path, title, artist, genre, bpm, musical_key, duration_seconds, now],
            )
            .map_err(|e| DjError::Deck(format!("Upsert track failed: {e}")))?;
        let track_id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE path = ?1",
                params![path],
                |r| r.get(0),
            )
            .map_err(|e| DjError::Deck(format!("Lookup track id failed: {e}")))?;
        // Opportunistically harvest ID3 metadata and Serato Markers2 cue points on import.
        let _ = self.import_id3_into_track(path);
        Ok(track_id)
    }

    pub fn add_track_to_crate(&self, crate_id: i64, track_id: i64) -> Result<(), DjError> {
        let next_pos: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM crate_tracks WHERE crate_id = ?1",
                params![crate_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        self.conn
            .execute(
                "INSERT OR IGNORE INTO crate_tracks (crate_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![crate_id, track_id, next_pos],
            )
            .map_err(|e| DjError::Deck(format!("Add track to crate failed: {e}")))?;
        Ok(())
    }

    /// Rename a crate and recursively update `full_path` for all descendant subcrates.
    pub fn rename_crate(&self, crate_id: i64, new_name: &str) -> Result<String, DjError> {
        let clean = new_name.trim().replace('/', "-");
        if clean.is_empty() {
            return Err(DjError::Deck("New crate name cannot be empty".to_string()));
        }
        let (old_path, parent_id): (String, Option<i64>) = self
            .conn
            .query_row(
                "SELECT full_path, parent_id FROM crates WHERE id = ?1",
                params![crate_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| DjError::Deck(format!("Crate {crate_id} not found: {e}")))?;

        let new_full_path = match parent_id {
            Some(pid) => {
                let ppath: String = self
                    .conn
                    .query_row(
                        "SELECT full_path FROM crates WHERE id = ?1",
                        params![pid],
                        |r| r.get(0),
                    )
                    .map_err(|e| DjError::Deck(format!("Parent crate {pid} not found: {e}")))?;
                format!("{ppath}/{clean}")
            }
            None => clean.clone(),
        };

        self.conn
            .execute(
                "UPDATE crates SET name = ?1, full_path = ?2 WHERE id = ?3",
                params![&clean, &new_full_path, crate_id],
            )
            .map_err(|e| DjError::Deck(format!("Rename crate failed: {e}")))?;

        let old_prefix = format!("{old_path}/");
        let new_prefix = format!("{new_full_path}/");
        let descendants = self.collect_descendant_crate_ids(crate_id)?;
        for did in descendants {
            if did == crate_id {
                continue;
            }
            if let Ok(dpath) = self.conn.query_row::<String, _, _>(
                "SELECT full_path FROM crates WHERE id = ?1",
                params![did],
                |r| r.get(0),
            ) {
                if let Some(suffix) = dpath.strip_prefix(&old_prefix) {
                    let updated = format!("{new_prefix}{suffix}");
                    let _ = self.conn.execute(
                        "UPDATE crates SET full_path = ?1 WHERE id = ?2",
                        params![updated, did],
                    );
                }
            }
        }
        Ok(new_full_path)
    }

    /// Delete a crate (and its subcrates via ON DELETE CASCADE).
    pub fn delete_crate(&self, crate_id: i64) -> Result<(), DjError> {
        self.conn
            .execute("DELETE FROM crates WHERE id = ?1", params![crate_id])
            .map_err(|e| DjError::Deck(format!("Delete crate failed: {e}")))?;
        Ok(())
    }

    /// Duplicate a crate and preserve its exact manual track order.
    pub fn duplicate_crate(&self, crate_id: i64, new_name: &str) -> Result<i64, DjError> {
        let parent_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT parent_id FROM crates WHERE id = ?1",
                params![crate_id],
                |r| r.get(0),
            )
            .map_err(|e| DjError::Deck(format!("Crate {crate_id} not found: {e}")))?;
        let new_id = self.create_subcrate(parent_id, new_name)?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT track_id, position FROM crate_tracks WHERE crate_id = ?1 ORDER BY position ASC",
            )
            .map_err(|e| DjError::Deck(format!("Read crate order failed: {e}")))?;
        let pairs: Vec<(i64, i64)> = stmt
            .query_map(params![crate_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| DjError::Deck(format!("Map crate order failed: {e}")))?
            .flatten()
            .collect();
        for (tid, pos) in pairs {
            let _ = self.conn.execute(
                "INSERT OR IGNORE INTO crate_tracks (crate_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![new_id, tid, pos],
            );
        }
        Ok(new_id)
    }

    pub fn remove_track_from_crate(&self, crate_id: i64, track_id: i64) -> Result<(), DjError> {
        self.conn
            .execute(
                "DELETE FROM crate_tracks WHERE crate_id = ?1 AND track_id = ?2",
                params![crate_id, track_id],
            )
            .map_err(|e| DjError::Deck(format!("Remove track failed: {e}")))?;
        self.normalize_crate_positions(crate_id)
    }

    pub fn delete_track(&self, track_id: i64) -> Result<(), DjError> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![track_id])
            .map_err(|e| DjError::Deck(format!("Delete track failed: {e}")))?;
        Ok(())
    }

    fn normalize_crate_positions(&self, crate_id: i64) -> Result<(), DjError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT track_id FROM crate_tracks WHERE crate_id = ?1 ORDER BY position ASC, track_id ASC",
            )
            .map_err(|e| DjError::Deck(format!("Read order failed: {e}")))?;
        let tids: Vec<i64> = stmt
            .query_map(params![crate_id], |r| r.get(0))
            .map_err(|e| DjError::Deck(format!("Map order failed: {e}")))?
            .flatten()
            .collect();
        for (idx, tid) in tids.into_iter().enumerate() {
            let _ = self.conn.execute(
                "UPDATE crate_tracks SET position = ?1 WHERE crate_id = ?2 AND track_id = ?3",
                params![idx as i64, crate_id, tid],
            );
        }
        Ok(())
    }

    /// Move `track_id` to `new_index` (0-based) within `crate_id` for persistent gig-prep ordering.
    pub fn reorder_crate_track(
        &self,
        crate_id: i64,
        track_id: i64,
        new_index: usize,
    ) -> Result<(), DjError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT track_id FROM crate_tracks WHERE crate_id = ?1 ORDER BY position ASC, track_id ASC",
            )
            .map_err(|e| DjError::Deck(format!("Read order failed: {e}")))?;
        let mut tids: Vec<i64> = stmt
            .query_map(params![crate_id], |r| r.get(0))
            .map_err(|e| DjError::Deck(format!("Map order failed: {e}")))?
            .flatten()
            .collect();
        let Some(old_idx) = tids.iter().position(|&id| id == track_id) else {
            return Err(DjError::Deck(format!(
                "Track {track_id} is not in crate {crate_id}"
            )));
        };
        tids.remove(old_idx);
        let insert_at = new_index.min(tids.len());
        tids.insert(insert_at, track_id);
        for (idx, tid) in tids.into_iter().enumerate() {
            self.conn
                .execute(
                    "UPDATE crate_tracks SET position = ?1 WHERE crate_id = ?2 AND track_id = ?3",
                    params![idx as i64, crate_id, tid],
                )
                .map_err(|e| DjError::Deck(format!("Update order failed: {e}")))?;
        }
        Ok(())
    }

    pub fn move_crate_track_by(
        &self,
        crate_id: i64,
        track_id: i64,
        delta: isize,
    ) -> Result<(), DjError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT track_id FROM crate_tracks WHERE crate_id = ?1 ORDER BY position ASC, track_id ASC",
            )
            .map_err(|e| DjError::Deck(format!("Read order failed: {e}")))?;
        let tids: Vec<i64> = stmt
            .query_map(params![crate_id], |r| r.get(0))
            .map_err(|e| DjError::Deck(format!("Map order failed: {e}")))?
            .flatten()
            .collect();
        let Some(old_idx) = tids.iter().position(|&id| id == track_id) else {
            return Ok(());
        };
        let target =
            (old_idx as isize + delta).clamp(0, tids.len().saturating_sub(1) as isize) as usize;
        self.reorder_crate_track(crate_id, track_id, target)
    }

    /// Add one or more OS audio files (e.g. from drag-and-drop or multi-file picker) to a crate.
    pub fn add_audio_files_to_crate(
        &self,
        crate_id: i64,
        paths: &[PathBuf],
    ) -> Result<usize, DjError> {
        let mut added = 0usize;
        for p in paths {
            if p.is_dir() {
                added += self.scan_audio_folder(crate_id, p)?;
            } else if p.is_file() && is_supported_audio(p) {
                let (artist, title) = infer_artist_title(p);
                let tid =
                    self.upsert_track(&p.to_string_lossy(), &title, &artist, "", 0.0, "", 0.0)?;
                self.add_track_to_crate(crate_id, tid)?;
                added += 1;
            }
        }
        Ok(added)
    }

    pub fn set_track_rating(&self, track_id: i64, rating: i32) -> Result<(), DjError> {
        self.conn
            .execute(
                "UPDATE tracks SET rating = ?1 WHERE id = ?2",
                params![rating.clamp(0, 5), track_id],
            )
            .map_err(|e| DjError::Deck(format!("Update rating failed: {e}")))?;
        Ok(())
    }

    fn collect_descendant_crate_ids(&self, root_id: i64) -> Result<Vec<i64>, DjError> {
        let all = self.list_crates()?;
        let mut ids = vec![root_id];
        let mut idx = 0;
        while idx < ids.len() {
            let cur = ids[idx];
            for node in &all {
                if node.parent_id == Some(cur) && !ids.contains(&node.id) {
                    ids.push(node.id);
                }
            }
            idx += 1;
        }
        Ok(ids)
    }

    /// Query tracks with hierarchical crate/subcrate scoping, multi-criteria filtering, and sorting.
    pub fn query_tracks(&self, q: &CrateQuery) -> Result<Vec<CrateTrack>, DjError> {
        let mut tracks = if let Some(cid) = q.crate_id {
            let crate_ids = if q.include_subcrates {
                self.collect_descendant_crate_ids(cid)?
            } else {
                vec![cid]
            };
            let id_set: HashSet<i64> = crate_ids.into_iter().collect();
            let mut stmt = self
                .conn
                .prepare(
                    "
                    SELECT t.id, t.path, t.title, t.artist, t.genre, t.bpm,
                           t.musical_key, t.duration_seconds, t.beatgrid_offset_sec,
                           t.tuning_cents, t.drum_hits, t.rating, t.analyzed_at,
                           ct.position
                    FROM tracks t
                    INNER JOIN crate_tracks ct ON ct.track_id = t.id
                    WHERE ct.crate_id = ?1
                    ORDER BY ct.position ASC, t.id ASC
                    ",
                )
                .map_err(|e| DjError::Deck(format!("Query crate tracks failed: {e}")))?;
            let mut seen = HashSet::new();
            let mut acc = Vec::new();
            for target_cid in id_set {
                let rows = stmt
                    .query_map(params![target_cid], map_track_row_with_pos)
                    .map_err(|e| DjError::Deck(format!("Map crate tracks failed: {e}")))?;
                for row in rows {
                    let t = row.map_err(|e| DjError::Deck(format!("Row error: {e}")))?;
                    if seen.insert(t.id) {
                        acc.push(t);
                    }
                }
            }
            acc
        } else {
            let mut stmt = self
                .conn
                .prepare(
                    "
                    SELECT id, path, title, artist, genre, bpm, musical_key, duration_seconds,
                           beatgrid_offset_sec, tuning_cents, drum_hits, rating, analyzed_at, 0
                    FROM tracks
                    ",
                )
                .map_err(|e| DjError::Deck(format!("Query all tracks failed: {e}")))?;
            let rows = stmt
                .query_map([], map_track_row_with_pos)
                .map_err(|e| DjError::Deck(format!("Map tracks failed: {e}")))?;
            let mut acc = Vec::new();
            for row in rows {
                acc.push(row.map_err(|e| DjError::Deck(format!("Row error: {e}")))?);
            }
            acc
        };

        let search_lc = q.search.trim().to_lowercase();
        let key_exact = q
            .key_filter
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_uppercase);
        let compat_key = q
            .compatible_with_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        tracks.retain(|t| {
            if !search_lc.is_empty() {
                let hay = format!("{} {} {} {}", t.title, t.artist, t.genre, t.path).to_lowercase();
                if !hay.contains(&search_lc) {
                    return false;
                }
            }
            if let Some(min_bpm) = q.bpm_min {
                if t.bpm < min_bpm {
                    return false;
                }
            }
            if let Some(max_bpm) = q.bpm_max {
                if t.bpm > max_bpm {
                    return false;
                }
            }
            if let Some(ref exact) = key_exact {
                if t.musical_key.trim().to_uppercase() != *exact {
                    return false;
                }
            }
            if let Some(ref_key) = compat_key {
                if !AudioEngine::is_camelot_compatible(ref_key, &t.musical_key) {
                    return false;
                }
            }
            !(q.analyzed_only && t.analyzed_at.is_none())
        });

        tracks.sort_by(|a, b| {
            let ord = match q.sort_by {
                CrateSortField::ManualOrder => a
                    .crate_position
                    .cmp(&b.crate_position)
                    .then(a.id.cmp(&b.id)),
                CrateSortField::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                CrateSortField::Artist => a.artist.to_lowercase().cmp(&b.artist.to_lowercase()),
                CrateSortField::Bpm => a.bpm.total_cmp(&b.bpm),
                CrateSortField::MusicalKey => {
                    let ka = AudioEngine::parse_camelot(&a.musical_key).unwrap_or((99, 'Z'));
                    let kb = AudioEngine::parse_camelot(&b.musical_key).unwrap_or((99, 'Z'));
                    ka.cmp(&kb)
                }
                CrateSortField::Duration => a.duration_seconds.total_cmp(&b.duration_seconds),
                CrateSortField::Rating => a.rating.cmp(&b.rating),
                CrateSortField::Added => a.id.cmp(&b.id),
            };
            if q.sort_asc {
                ord
            } else {
                ord.reverse()
            }
        });

        Ok(tracks)
    }

    /// Run built-in Symphonia + autocorrelation beatgrid analysis on a track and persist results.
    pub fn analyze_track_builtin(&self, track_id: i64) -> Result<CrateTrack, DjError> {
        let path_str: String = self
            .conn
            .query_row(
                "SELECT path FROM tracks WHERE id = ?1",
                params![track_id],
                |r| r.get(0),
            )
            .map_err(|e| DjError::Deck(format!("Track {track_id} not found: {e}")))?;
        let path = Path::new(&path_str);
        let decoded = decode_audio_file(path).map_err(DjError::Deck)?;
        let grid = detect_beatgrid(&decoded);
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "
                UPDATE tracks
                SET bpm = ?1, duration_seconds = ?2, beatgrid_offset_sec = ?3, analyzed_at = ?4
                WHERE id = ?5
                ",
                params![
                    grid.bpm,
                    decoded.duration_seconds,
                    grid.first_beat_sec,
                    now,
                    track_id
                ],
            )
            .map_err(|e| DjError::Deck(format!("Save analysis failed: {e}")))?;
        self.get_track(track_id)
    }

    /// Run local `dettato` audio-to-MIDI sidecar on a track and store `tuning_cents` + `drum_hits`.
    pub fn analyze_track_dettato(&self, track_id: i64) -> Result<CrateTrack, DjError> {
        let path_str: String = self
            .conn
            .query_row(
                "SELECT path FROM tracks WHERE id = ?1",
                params![track_id],
                |r| r.get(0),
            )
            .map_err(|e| DjError::Deck(format!("Track {track_id} not found: {e}")))?;
        let analysis = run_dettato_sidecar(&path_str, None, None)?;
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "
                UPDATE tracks
                SET tuning_cents = ?1, drum_hits = ?2, analyzed_at = COALESCE(analyzed_at, ?3)
                WHERE id = ?4
                ",
                params![
                    analysis.tuning_cents,
                    analysis.drum_hits as i64,
                    now,
                    track_id
                ],
            )
            .map_err(|e| DjError::Deck(format!("Save dettato analysis failed: {e}")))?;
        self.get_track(track_id)
    }

    pub fn get_track(&self, track_id: i64) -> Result<CrateTrack, DjError> {
        self.conn
            .query_row(
                "
                SELECT id, path, title, artist, genre, bpm, musical_key, duration_seconds,
                       beatgrid_offset_sec, tuning_cents, drum_hits, rating, analyzed_at
                FROM tracks WHERE id = ?1
                ",
                params![track_id],
                map_track_row,
            )
            .map_err(|e| DjError::Deck(format!("Track {track_id} not found: {e}")))
    }

    /// Recursively scan a local folder of audio files into a crate.
    pub fn scan_audio_folder(&self, crate_id: i64, dir: &Path) -> Result<usize, DjError> {
        if !dir.is_dir() {
            return Err(DjError::Deck(format!(
                "Folder not found: {}",
                dir.display()
            )));
        }
        let mut imported = 0usize;
        let mut stack = vec![dir.to_path_buf()];
        while let Some(cur) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&cur) else {
                continue;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let is_hidden = p
                        .file_name()
                        .and_then(|s| s.to_str())
                        .map(|n| n.starts_with('.'))
                        .unwrap_or(false);
                    if !is_hidden {
                        stack.push(p);
                    }
                } else if is_supported_audio(&p) {
                    let (artist, title) = infer_artist_title(&p);
                    let tid =
                        self.upsert_track(&p.to_string_lossy(), &title, &artist, "", 0.0, "", 0.0)?;
                    self.add_track_to_crate(crate_id, tid)?;
                    imported += 1;
                }
            }
        }
        Ok(imported)
    }

    /// Import a Serato DJ `.crate` file (preserving `%%` subcrate hierarchy from filename).
    pub fn import_serato_crate(&self, crate_file: &Path) -> Result<ImportSummary, DjError> {
        let bytes = std::fs::read(crate_file)
            .map_err(|e| DjError::Deck(format!("Cannot read {}: {e}", crate_file.display())))?;
        let drive_prefix = extract_windows_drive(crate_file);
        let paths = parse_serato_crate_bytes(&bytes, drive_prefix.as_deref())?;

        let stem = crate_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Serato Crate");
        let crate_path = stem.replace("%%", "/");
        let crate_id = self.ensure_crate_path(&crate_path)?;

        let mut imported = 0usize;
        for track_path in paths {
            let (artist, title) = infer_artist_title(Path::new(&track_path));
            let tid = self.upsert_track(&track_path, &title, &artist, "", 0.0, "", 0.0)?;
            self.add_track_to_crate(crate_id, tid)?;
            imported += 1;
        }

        Ok(ImportSummary {
            format: "serato".to_string(),
            crate_id,
            crate_path,
            imported_tracks: imported,
        })
    }

    /// Export a DJCraft crate (and optionally its subcrates) to a Serato DJ binary `.crate` file.
    pub fn export_serato_crate(&self, crate_id: i64, dest_file: &Path) -> Result<usize, DjError> {
        let tracks = self.query_tracks(&CrateQuery {
            crate_id: Some(crate_id),
            include_subcrates: true,
            ..Default::default()
        })?;
        let paths: Vec<String> = tracks
            .iter()
            .filter(|t| !t.path.starts_with("demo://"))
            .map(|t| t.path.clone())
            .collect();
        let bytes = encode_serato_crate_bytes(&paths);
        if let Some(parent) = dest_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(dest_file, bytes)
            .map_err(|e| DjError::Deck(format!("Cannot write {}: {e}", dest_file.display())))?;
        Ok(paths.len())
    }

    /// Import a VirtualDJ `.vdjfolder` XML file or `.m3u`/`.m3u8` playlist into a crate.
    pub fn import_vdj_file(&self, file_path: &Path) -> Result<ImportSummary, DjError> {
        let text = std::fs::read_to_string(file_path)
            .map_err(|e| DjError::Deck(format!("Cannot read {}: {e}", file_path.display())))?;
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let paths = if ext == "m3u" || ext == "m3u8" {
            parse_m3u(&text)
        } else {
            parse_vdj_folder_xml(&text)
        };

        let stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("VirtualDJ Crate");
        let crate_path = format!("VirtualDJ/{stem}");
        let crate_id = self.ensure_crate_path(&crate_path)?;

        let mut imported = 0usize;
        for track_path in paths {
            let (artist, title) = infer_artist_title(Path::new(&track_path));
            let tid = self.upsert_track(&track_path, &title, &artist, "", 0.0, "", 0.0)?;
            self.add_track_to_crate(crate_id, tid)?;
            imported += 1;
        }

        Ok(ImportSummary {
            format: if ext == "m3u" || ext == "m3u8" {
                "m3u".to_string()
            } else {
                "vdjfolder".to_string()
            },
            crate_id,
            crate_path,
            imported_tracks: imported,
        })
    }

    /// Export a DJCraft crate to VirtualDJ `.vdjfolder` XML or `.m3u8` (determined by extension).
    pub fn export_vdj_file(&self, crate_id: i64, dest_file: &Path) -> Result<usize, DjError> {
        let tracks: Vec<CrateTrack> = self
            .query_tracks(&CrateQuery {
                crate_id: Some(crate_id),
                include_subcrates: true,
                ..Default::default()
            })?
            .into_iter()
            .filter(|t| !t.path.starts_with("demo://"))
            .collect();

        let ext = dest_file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("vdjfolder")
            .to_lowercase();
        let content = if ext == "m3u" || ext == "m3u8" {
            encode_m3u(&tracks)
        } else {
            encode_vdj_folder_xml(&tracks)
        };

        if let Some(parent) = dest_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(dest_file, content)
            .map_err(|e| DjError::Deck(format!("Cannot write {}: {e}", dest_file.display())))?;
        Ok(tracks.len())
    }

    /// Batch-stem every real audio file in a crate (and its subcrates) into transparent
    /// `<parent>/.djcraft-stems/<stem>/{vocals,drums,bass,other}.wav` folders.
    pub fn batch_stem_crate(&self, crate_id: i64) -> Result<BatchStemSummary, DjError> {
        let tracks = self.query_tracks(&CrateQuery {
            crate_id: Some(crate_id),
            include_subcrates: true,
            ..Default::default()
        })?;
        let paths: Vec<PathBuf> = tracks
            .into_iter()
            .filter(|t| !t.path.starts_with("demo://"))
            .map(|t| PathBuf::from(t.path))
            .collect();
        Ok(batch_stem_paths(&paths))
    }

    /// Persist or clear a single hot-cue slot (0..4) for an audio track path in SQLite.
    pub fn set_track_hot_cue(
        &self,
        track_path: &str,
        slot: usize,
        position_seconds: Option<f64>,
    ) -> Result<(), DjError> {
        if slot >= 4 {
            return Err(DjError::Deck(format!(
                "Hot cue slot {slot} out of range (0..4)"
            )));
        }
        match position_seconds {
            Some(pos) => {
                self.conn
                    .execute(
                        "INSERT INTO track_cues (track_path, slot, position_seconds)
                         VALUES (?1, ?2, ?3)
                         ON CONFLICT(track_path, slot) DO UPDATE SET position_seconds = excluded.position_seconds",
                        params![track_path, slot as i64, pos.max(0.0)],
                    )
                    .map_err(|e| DjError::Deck(format!("Save hot cue failed: {e}")))?;
            }
            None => {
                self.conn
                    .execute(
                        "DELETE FROM track_cues WHERE track_path = ?1 AND slot = ?2",
                        params![track_path, slot as i64],
                    )
                    .map_err(|e| DjError::Deck(format!("Clear hot cue failed: {e}")))?;
            }
        }
        Ok(())
    }

    /// Load all 4 hot-cue positions for an audio track path from SQLite.
    pub fn get_track_hot_cues(&self, track_path: &str) -> Result<[Option<f64>; 4], DjError> {
        let mut cues = [None; 4];
        let mut stmt = self
            .conn
            .prepare("SELECT slot, position_seconds FROM track_cues WHERE track_path = ?1")
            .map_err(|e| DjError::Deck(format!("Query hot cues failed: {e}")))?;
        let rows = stmt
            .query_map(params![track_path], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
            })
            .map_err(|e| DjError::Deck(format!("Map hot cues failed: {e}")))?;
        for (slot, pos) in rows.flatten() {
            if (0..4).contains(&slot) {
                cues[slot as usize] = Some(pos);
            }
        }
        Ok(cues)
    }

    /// Persist custom label, RGB color, and behavior for a single hot-cue slot (0..4).
    pub fn set_track_hot_cue_meta(
        &self,
        track_path: &str,
        slot: usize,
        meta: &TrackCueMeta,
    ) -> Result<(), DjError> {
        if slot >= 4 {
            return Err(DjError::Deck(format!(
                "Hot cue slot {slot} out of range (0..4)"
            )));
        }
        let color_str = meta.color.map(|[r, g, b]| format!("{r},{g},{b}"));
        self.conn
            .execute(
                "INSERT INTO track_cue_meta (track_path, slot, label, color_rgb, behavior)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(track_path, slot) DO UPDATE SET
                   label = excluded.label,
                   color_rgb = excluded.color_rgb,
                   behavior = excluded.behavior",
                params![
                    track_path,
                    slot as i64,
                    meta.label,
                    color_str,
                    meta.behavior
                ],
            )
            .map_err(|e| DjError::Deck(format!("Save hot cue meta failed: {e}")))?;
        Ok(())
    }

    /// Load all 4 hot-cue metadata entries (label, color, behavior) for a track path.
    pub fn get_track_hot_cue_metas(&self, track_path: &str) -> Result<[TrackCueMeta; 4], DjError> {
        let mut metas: [TrackCueMeta; 4] = std::array::from_fn(|_| TrackCueMeta::default());
        let mut stmt = self
            .conn
            .prepare(
                "SELECT slot, label, color_rgb, behavior FROM track_cue_meta WHERE track_path = ?1",
            )
            .map_err(|e| DjError::Deck(format!("Query hot cue meta failed: {e}")))?;
        let rows = stmt
            .query_map(params![track_path], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| DjError::Deck(format!("Map hot cue meta failed: {e}")))?;
        for (slot, label, color_rgb, behavior) in rows.flatten() {
            if (0..4).contains(&slot) {
                let color = color_rgb.and_then(|s| {
                    let parts: Vec<u8> =
                        s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                    if parts.len() == 3 {
                        Some([parts[0], parts[1], parts[2]])
                    } else {
                        None
                    }
                });
                metas[slot as usize] = TrackCueMeta {
                    label,
                    color,
                    behavior,
                };
            }
        }
        Ok(metas)
    }

    /// Opportunistically read ID3v2 tags (`TBPM`, `TKEY`, `TIT2`, `TPE1`, `TCON`, and
    /// `Serato Markers2` GEOB cues) from `track_path`. Fills missing track metadata and
    /// populates hot cues only when DJCraft has no local cues saved for this track yet.
    pub fn import_id3_into_track(&self, track_path: &str) -> Result<usize, DjError> {
        let p = Path::new(track_path);
        if !p.is_file() {
            return Ok(0);
        }
        let info = crate::id3_cues::read_id3_track_info(p)?;
        let title = info.title.as_deref().unwrap_or("");
        let artist = info.artist.as_deref().unwrap_or("");
        let genre = info.genre.as_deref().unwrap_or("");
        let bpm = info.bpm.unwrap_or(0.0);
        let key = info.musical_key.as_deref().unwrap_or("");
        if !title.is_empty()
            || !artist.is_empty()
            || !genre.is_empty()
            || bpm > 0.0
            || !key.is_empty()
        {
            let _ = self.conn.execute(
                "UPDATE tracks SET
                    title = CASE WHEN title = '' AND ?2 != '' THEN ?2 ELSE title END,
                    artist = CASE WHEN artist = '' AND ?3 != '' THEN ?3 ELSE artist END,
                    genre = CASE WHEN genre = '' AND ?4 != '' THEN ?4 ELSE genre END,
                    bpm = CASE WHEN bpm <= 0 AND ?5 > 0 THEN ?5 ELSE bpm END,
                    musical_key = CASE WHEN musical_key = '' AND ?6 != '' THEN ?6 ELSE musical_key END
                 WHERE path = ?1",
                params![track_path, title, artist, genre, bpm, key],
            );
        }

        if info.cue_count == 0 {
            return Ok(0);
        }
        let existing = self.get_track_hot_cues(track_path)?;
        if existing.iter().any(Option::is_some) {
            return Ok(0);
        }
        let mut imported = 0usize;
        for (slot, (pos_opt, meta)) in info.cues.iter().zip(info.cue_metas.iter()).enumerate() {
            if let Some(pos) = pos_opt {
                self.set_track_hot_cue(track_path, slot, Some(*pos))?;
                if !meta.label.is_empty() || meta.color.is_some() {
                    self.set_track_hot_cue_meta(track_path, slot, meta)?;
                }
                imported += 1;
            }
        }
        Ok(imported)
    }

    /// Batch-read ID3v2 metadata & `Serato Markers2` cue points for every track in `crate_id`.
    pub fn batch_read_id3_cues_for_crate(
        &self,
        crate_id: i64,
    ) -> Result<crate::id3_cues::BatchId3Summary, DjError> {
        let tracks = self.query_tracks(&CrateQuery {
            crate_id: Some(crate_id),
            ..Default::default()
        })?;
        let mut sum = crate::id3_cues::BatchId3Summary {
            tracks_scanned: tracks.len(),
            ..Default::default()
        };
        for t in tracks {
            if !Path::new(&t.path).is_file() {
                sum.skipped += 1;
                continue;
            }
            match self.import_id3_into_track(&t.path) {
                Ok(n) if n > 0 => {
                    sum.tracks_updated += 1;
                    sum.cues_processed += n;
                }
                Ok(_) => sum.skipped += 1,
                Err(_) => sum.failed += 1,
            }
        }
        Ok(sum)
    }

    /// Batch-write DJCraft's saved hot cues & labels into `Serato Markers2` ID3v2 tags
    /// for every track in `crate_id` that has at least one cue point.
    pub fn batch_write_id3_cues_for_crate(
        &self,
        crate_id: i64,
    ) -> Result<crate::id3_cues::BatchId3Summary, DjError> {
        let tracks = self.query_tracks(&CrateQuery {
            crate_id: Some(crate_id),
            ..Default::default()
        })?;
        let mut sum = crate::id3_cues::BatchId3Summary {
            tracks_scanned: tracks.len(),
            ..Default::default()
        };
        for t in tracks {
            let p = Path::new(&t.path);
            if !p.is_file() {
                sum.skipped += 1;
                continue;
            }
            let cues = self.get_track_hot_cues(&t.path)?;
            if cues.iter().all(Option::is_none) {
                sum.skipped += 1;
                continue;
            }
            let metas = self.get_track_hot_cue_metas(&t.path)?;
            match crate::id3_cues::write_id3_hot_cues(p, &cues, &metas) {
                Ok(n) => {
                    sum.tracks_updated += 1;
                    sum.cues_processed += n;
                }
                Err(_) => sum.failed += 1,
            }
        }
        Ok(sum)
    }
}

/// Summary of a multi-threaded crate analysis sweep (`std::thread::scope`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelAnalysisReport {
    pub analyzed: usize,
    pub skipped: usize,
    pub threads_used: usize,
}

impl CrateStore {
    /// Analyze all local audio files in a crate (or full library when `crate_id` is `None`)
    /// in parallel across available CPU threads, computing BPM + Krumhansl-Schmuckler Camelot Key
    /// and persisting updated metadata into SQLite.
    pub fn analyze_crate_parallel(
        &self,
        crate_id: Option<i64>,
    ) -> Result<ParallelAnalysisReport, DjError> {
        let tracks = self.query_tracks(&CrateQuery {
            crate_id,
            ..Default::default()
        })?;
        let candidates: Vec<(i64, PathBuf)> = tracks
            .iter()
            .filter_map(|t| {
                let p = PathBuf::from(&t.path);
                if p.is_file() {
                    Some((t.id, p))
                } else {
                    None
                }
            })
            .collect();
        let skipped = tracks.len().saturating_sub(candidates.len());
        if candidates.is_empty() {
            return Ok(ParallelAnalysisReport {
                analyzed: 0,
                skipped,
                threads_used: 0,
            });
        }

        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(candidates.len())
            .max(1);
        let chunk_size = candidates.len().div_ceil(threads);
        let results: std::sync::Mutex<Vec<(i64, f64, String, f64)>> =
            std::sync::Mutex::new(Vec::with_capacity(candidates.len()));

        std::thread::scope(|s| {
            for chunk in candidates.chunks(chunk_size) {
                let out_ref = &results;
                s.spawn(move || {
                    for (tid, path) in chunk {
                        if let Ok(decoded) = djcraft_audio::dsp::decode_audio_file(path) {
                            let analysis = djcraft_audio::analyze_track_full(&decoded);
                            if let Ok(mut g) = out_ref.lock() {
                                g.push((
                                    *tid,
                                    analysis.beatgrid.bpm,
                                    analysis.key.camelot,
                                    decoded.duration_seconds,
                                ));
                            }
                        }
                    }
                });
            }
        });

        let computed = results.into_inner().unwrap_or_default();
        let mut analyzed = 0usize;
        for (tid, bpm, key, dur) in computed {
            let _ = self.conn.execute(
                "UPDATE tracks SET bpm = ?1, musical_key = ?2, duration_seconds = ?3 WHERE id = ?4",
                params![bpm, key, dur, tid],
            );
            analyzed += 1;
        }

        Ok(ParallelAnalysisReport {
            analyzed,
            skipped: tracks.len().saturating_sub(analyzed),
            threads_used: threads,
        })
    }
}

/// Persisted custom metadata (name, optional RGB color, trigger behavior) for one Hot Cue slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackCueMeta {
    pub label: String,
    pub color: Option<[u8; 3]>,
    pub behavior: String,
}

impl Default for TrackCueMeta {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: None,
            behavior: "standard".to_string(),
        }
    }
}

/// Resolve local `demucs` executable (`DEMUCS_BIN`, `stems-mcp/.venv`, or `PATH`).
pub fn resolve_demucs_binary() -> Option<PathBuf> {
    if let Ok(env_bin) = std::env::var("DEMUCS_BIN") {
        let p = PathBuf::from(env_bin);
        if p.is_file() {
            return Some(p);
        }
    }
    for candidate in [
        r"D:\Dev\repos\stems-mcp\.venv\Scripts\demucs.exe",
        r"D:\Dev\tools\demucs\.venv\Scripts\demucs.exe",
    ] {
        let p = PathBuf::from(candidate);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in ["demucs.exe", "demucs"] {
                let c = dir.join(name);
                if c.is_file() {
                    return Some(c);
                }
            }
        }
    }
    None
}

/// Separate one audio file transparently into `<parent>/.djcraft-stems/<stem>/{vocals,drums,bass,other}.wav`.
/// Skips immediately if all 4 stem WAVs already exist.
pub fn stem_audio_file_transparent(audio_path: &Path) -> Result<bool, DjError> {
    stem_audio_file_with_device(audio_path, false)
}

/// Separate one audio file into transparent sidecar stems, optionally requesting CUDA GPU acceleration
/// (`-d cuda`, e.g. Goliath's RTX 4090) with automatic fallback to CPU if CUDA fails.
pub fn stem_audio_file_with_device(audio_path: &Path, prefer_cuda: bool) -> Result<bool, DjError> {
    if !audio_path.is_file() {
        return Err(DjError::Sidecar(format!(
            "Audio file not found: {}",
            audio_path.display()
        )));
    }
    if has_sidecar_stems(audio_path) {
        return Ok(false); // already stemmed
    }
    let demucs = resolve_demucs_binary().ok_or_else(|| {
        DjError::Sidecar(
            "demucs CLI not found. Set DEMUCS_BIN or install demucs in stems-mcp/.venv."
                .to_string(),
        )
    })?;

    let target_dir = stem_dir_for_audio(audio_path);
    let tmp_out = target_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("_demucs_tmp");
    std::fs::create_dir_all(&tmp_out)
        .map_err(|e| DjError::Sidecar(format!("Cannot create stem temp dir: {e}")))?;

    let run_demucs = |device: Option<&str>| -> Result<std::process::Output, DjError> {
        let mut cmd = std::process::Command::new(&demucs);
        cmd.args(["-n", "htdemucs"]);
        if let Some(dev) = device {
            cmd.args(["-d", dev]);
        }
        cmd.arg("-o").arg(&tmp_out).arg(audio_path);
        cmd.output()
            .map_err(|e| DjError::Sidecar(format!("Failed to launch demucs: {e}")))
    };

    let output = if prefer_cuda {
        let gpu_out = run_demucs(Some("cuda"))?;
        if gpu_out.status.success() {
            gpu_out
        } else {
            run_demucs(None)?
        }
    } else {
        run_demucs(None)?
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DjError::Sidecar(format!(
            "demucs failed ({}): {}",
            output.status,
            stderr.trim()
        )));
    }

    let stem_name = audio_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");
    let produced = tmp_out.join("htdemucs").join(stem_name);
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| DjError::Sidecar(format!("Cannot create {}: {e}", target_dir.display())))?;
    for wav in ["vocals.wav", "drums.wav", "bass.wav", "other.wav"] {
        let src_wav = produced.join(wav);
        let dst_wav = target_dir.join(wav);
        if src_wav.is_file() {
            std::fs::copy(&src_wav, &dst_wav)
                .map_err(|e| DjError::Sidecar(format!("Failed to move {wav}: {e}")))?;
        }
    }
    let _ = std::fs::remove_dir_all(&tmp_out);
    Ok(true)
}

pub fn batch_stem_paths(paths: &[PathBuf]) -> BatchStemSummary {
    let mut summary = BatchStemSummary {
        total: paths.len(),
        stemmed: 0,
        skipped_existing: 0,
        failed: 0,
        errors: Vec::new(),
    };
    for p in paths {
        match stem_audio_file_transparent(p) {
            Ok(true) => summary.stemmed += 1,
            Ok(false) => summary.skipped_existing += 1,
            Err(e) => {
                summary.failed += 1;
                if summary.errors.len() < 5 {
                    summary.errors.push(format!("{}: {e}", p.display()));
                }
            }
        }
    }
    summary
}

/// Recursively batch-stem every audio file in `dir` (skipping `.djcraft-stems` and dot dirs).
pub fn batch_stem_folder(dir: &Path) -> Result<BatchStemSummary, DjError> {
    if !dir.is_dir() {
        return Err(DjError::Deck(format!(
            "Folder not found: {}",
            dir.display()
        )));
    }
    let mut paths = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&cur) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let is_hidden = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n.starts_with('.'))
                    .unwrap_or(false);
                if !is_hidden {
                    stack.push(p);
                }
            } else if is_supported_audio(&p) {
                paths.push(p);
            }
        }
    }
    Ok(batch_stem_paths(&paths))
}

fn map_track_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CrateTrack> {
    Ok(CrateTrack {
        id: r.get(0)?,
        path: r.get(1)?,
        title: r.get(2)?,
        artist: r.get(3)?,
        genre: r.get(4)?,
        bpm: r.get(5)?,
        musical_key: r.get(6)?,
        duration_seconds: r.get(7)?,
        beatgrid_offset_sec: r.get(8)?,
        tuning_cents: r.get(9)?,
        drum_hits: r.get(10)?,
        rating: r.get(11)?,
        analyzed_at: r.get(12)?,
        crate_position: 0,
    })
}

fn map_track_row_with_pos(r: &rusqlite::Row<'_>) -> rusqlite::Result<CrateTrack> {
    let mut t = map_track_row(r)?;
    t.crate_position = r.get(13)?;
    Ok(t)
}

fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn infer_artist_title(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .trim();
    if let Some((artist, title)) = stem.split_once(" - ") {
        (artist.trim().to_string(), title.trim().to_string())
    } else {
        ("Unknown Artist".to_string(), stem.to_string())
    }
}

fn extract_windows_drive(path: &Path) -> Option<String> {
    let s = path.to_string_lossy();
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        Some(format!("{}:\\", bytes[0] as char))
    } else {
        None
    }
}

fn encode_utf16be(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 2);
    for unit in s.encode_utf16() {
        out.extend_from_slice(&unit.to_be_bytes());
    }
    out
}

fn decode_utf16be(bytes: &[u8]) -> Result<String, DjError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(DjError::Deck(
            "Odd UTF-16BE byte length in Serato crate".to_string(),
        ));
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units)
        .map_err(|e| DjError::Deck(format!("Invalid UTF-16BE in Serato crate: {e}")))
}

fn push_tlv(buf: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    buf.extend_from_slice(tag);
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);
}

/// Encode a list of track paths into Serato DJ binary `.crate` format (`vrsn`/`otrk`/`ptrk`).
pub fn encode_serato_crate_bytes(paths: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, b"vrsn", &encode_utf16be(SERATO_VERSION_HEADER));
    for raw in paths {
        // Serato stores forward-slash paths relative to volume root (strip leading "C:\" or "/")
        let normalized = raw.replace('\\', "/");
        let stripped = if normalized.len() >= 3
            && normalized.as_bytes()[0].is_ascii_alphabetic()
            && normalized.as_bytes()[1] == b':'
            && normalized.as_bytes()[2] == b'/'
        {
            &normalized[3..]
        } else {
            normalized.trim_start_matches('/')
        };
        let mut otrk_body = Vec::new();
        push_tlv(&mut otrk_body, b"ptrk", &encode_utf16be(stripped));
        push_tlv(&mut out, b"otrk", &otrk_body);
    }
    out
}

/// Parse a Serato DJ binary `.crate` file (`vrsn`/`otrk`/`ptrk` UTF-16BE TLV).
pub fn parse_serato_crate_bytes(
    bytes: &[u8],
    drive_prefix: Option<&str>,
) -> Result<Vec<String>, DjError> {
    let mut tracks = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= bytes.len() {
        let tag = &bytes[pos..pos + 4];
        let len = u32::from_be_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        pos += 8;
        if pos + len > bytes.len() {
            return Err(DjError::Deck(
                "Truncated Serato .crate TLV record".to_string(),
            ));
        }
        let body = &bytes[pos..pos + len];
        pos += len;

        if tag == b"otrk" {
            let mut inner = 0usize;
            while inner + 8 <= body.len() {
                let itag = &body[inner..inner + 4];
                let ilen = u32::from_be_bytes([
                    body[inner + 4],
                    body[inner + 5],
                    body[inner + 6],
                    body[inner + 7],
                ]) as usize;
                inner += 8;
                if inner + ilen > body.len() {
                    break;
                }
                let ibody = &body[inner..inner + ilen];
                inner += ilen;
                if itag == b"ptrk" {
                    let rel = decode_utf16be(ibody)?;
                    let full = if let Some(prefix) = drive_prefix {
                        if rel.len() >= 2 && rel.as_bytes()[1] == b':' {
                            rel
                        } else {
                            format!("{prefix}{}", rel.replace('/', "\\"))
                        }
                    } else {
                        rel
                    };
                    tracks.push(full);
                }
            }
        }
    }
    Ok(tracks)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Encode tracks as a VirtualDJ `.vdjfolder` XML file.
pub fn encode_vdj_folder_xml(tracks: &[CrateTrack]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<VirtualFolder noDuplicates=\"yes\">\n",
    );
    for t in tracks {
        xml.push_str(&format!("  <song path=\"{}\" />\n", xml_escape(&t.path)));
    }
    xml.push_str("</VirtualFolder>\n");
    xml
}

/// Parse `<song path="..." />` entries from a VirtualDJ `.vdjfolder` XML document.
pub fn parse_vdj_folder_xml(xml: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut rest = xml;
    while let Some(idx) = rest.find("<song") {
        rest = &rest[idx + 5..];
        let Some(end_tag) = rest.find('>') else {
            break;
        };
        let attrs = &rest[..end_tag];
        if let Some(p_idx) = attrs.find("path=\"") {
            let after = &attrs[p_idx + 6..];
            if let Some(quote_end) = after.find('"') {
                let raw = xml_unescape(&after[..quote_end]);
                if !raw.trim().is_empty() {
                    paths.push(raw);
                }
            }
        }
        rest = &rest[end_tag + 1..];
    }
    paths
}

/// Encode tracks as an `#EXTM3U` playlist compatible with VirtualDJ and Rekordbox.
pub fn encode_m3u(tracks: &[CrateTrack]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for t in tracks {
        let dur = t.duration_seconds.round() as i64;
        out.push_str(&format!(
            "#EXTINF:{dur},{} - {}\n{}\n",
            t.artist, t.title, t.path
        ));
    }
    out
}

/// Parse track paths from an `.m3u` / `.m3u8` file.
pub fn parse_m3u(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToString::to_string)
        .collect()
}
