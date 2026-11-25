#![forbid(unsafe_code)]
//! Smart Crates (live rule-based dynamic playlists), Duplicate Track Detection,
//! and Live Setlist History + `.txt` / `.cue` / `.csv` Tracklist Export.

use std::collections::HashMap;
use std::path::Path;

use djcraft_audio::AudioEngine;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{CrateQuery, CrateStore, CrateTrack, DjError};

/// Rule criteria for a dynamic Smart Crate.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SmartCrateRule {
    pub name: String,
    pub min_bpm: Option<f64>,
    pub max_bpm: Option<f64>,
    /// Exact or Camelot-compatible key filter (e.g. `"8A"`).
    pub camelot_key: Option<String>,
    /// When true and `camelot_key` is set, matches harmonically compatible keys (same, +/-1, A<->B).
    pub harmonic_compatible: bool,
    /// Case-insensitive substring match against genre.
    pub genre_contains: Option<String>,
    /// Minimum star rating (0..=5).
    pub min_rating: Option<i64>,
    /// Case-insensitive substring match against title or artist.
    pub text_contains: Option<String>,
}

/// A saved Smart Crate stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedSmartCrate {
    pub id: i64,
    pub name: String,
    pub rule: SmartCrateRule,
}

/// Group of duplicate tracks sharing the same normalized `artist + title` or filename.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub reason: String,
    pub tracks: Vec<CrateTrack>,
}

/// One logged track play during a live DJ set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetHistoryEntry {
    pub id: i64,
    pub session_id: String,
    pub deck: String,
    pub track_path: String,
    pub title: String,
    pub artist: String,
    pub bpm: f64,
    pub musical_key: String,
    pub played_at_sec: f64,
    pub played_at_utc: String,
}

fn now_utc_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

fn normalize_dup_key(artist: &str, title: &str) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect()
    };
    let a = clean(artist);
    let t = clean(title);
    if a.is_empty() {
        t
    } else {
        format!("{a}::{t}")
    }
}

impl CrateStore {
    /// Save or update a named Smart Crate rule in SQLite.
    pub fn upsert_smart_crate(&self, rule: &SmartCrateRule) -> Result<i64, DjError> {
        let name = rule.name.trim();
        if name.is_empty() {
            return Err(DjError::Deck("Smart crate name cannot be empty".into()));
        }
        let rule_json = serde_json::to_string(rule)
            .map_err(|e| DjError::Deck(format!("Serialize smart crate rule failed: {e}")))?;
        self.conn
            .execute(
                "INSERT INTO smart_crates (name, rule_json, created_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(name) DO UPDATE SET rule_json = excluded.rule_json",
                params![name, rule_json, now_utc_iso()],
            )
            .map_err(|e| DjError::Deck(format!("Save smart crate failed: {e}")))?;
        let id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM smart_crates WHERE name = ?1",
                params![name],
                |r| r.get(0),
            )
            .map_err(|e| DjError::Deck(format!("Read smart crate id failed: {e}")))?;
        Ok(id)
    }

    /// List all saved Smart Crates.
    pub fn list_smart_crates(&self) -> Result<Vec<SavedSmartCrate>, DjError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, rule_json FROM smart_crates ORDER BY name ASC")
            .map_err(|e| DjError::Deck(format!("List smart crates failed: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                let id: i64 = r.get(0)?;
                let name: String = r.get(1)?;
                let json: String = r.get(2)?;
                Ok((id, name, json))
            })
            .map_err(|e| DjError::Deck(format!("Query smart crates failed: {e}")))?;

        let mut out = Vec::new();
        for row in rows.flatten() {
            if let Ok(rule) = serde_json::from_str::<SmartCrateRule>(&row.2) {
                out.push(SavedSmartCrate {
                    id: row.0,
                    name: row.1,
                    rule,
                });
            }
        }
        Ok(out)
    }

    /// Delete a saved Smart Crate by name.
    pub fn delete_smart_crate(&self, name: &str) -> Result<usize, DjError> {
        self.conn
            .execute("DELETE FROM smart_crates WHERE name = ?1", params![name])
            .map_err(|e| DjError::Deck(format!("Delete smart crate failed: {e}")))
    }

    /// Dynamically evaluate a [`SmartCrateRule`] against the entire track library.
    pub fn query_smart_crate(&self, rule: &SmartCrateRule) -> Result<Vec<CrateTrack>, DjError> {
        let all = self.query_tracks(&CrateQuery::default())?;
        let genre_q = rule
            .genre_contains
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_lowercase);
        let text_q = rule
            .text_contains
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_lowercase);
        let key_q = rule
            .camelot_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_uppercase);

        let matched = all
            .into_iter()
            .filter(|t| {
                if let Some(min_b) = rule.min_bpm {
                    if t.bpm < min_b - 1e-6 {
                        return false;
                    }
                }
                if let Some(max_b) = rule.max_bpm {
                    if t.bpm > max_b + 1e-6 {
                        return false;
                    }
                }
                if let Some(min_r) = rule.min_rating {
                    if i64::from(t.rating) < min_r {
                        return false;
                    }
                }
                if let Some(ref g) = genre_q {
                    if !t.genre.to_ascii_lowercase().contains(g.as_str()) {
                        return false;
                    }
                }
                if let Some(ref q) = text_q {
                    let hay = format!("{} {}", t.artist, t.title).to_ascii_lowercase();
                    if !hay.contains(q.as_str()) {
                        return false;
                    }
                }
                if let Some(ref k) = key_q {
                    let tk = t.musical_key.trim().to_ascii_uppercase();
                    if tk.is_empty() {
                        return false;
                    }
                    if rule.harmonic_compatible {
                        if !AudioEngine::is_camelot_compatible(k, &tk) {
                            return false;
                        }
                    } else if tk != *k {
                        return false;
                    }
                }
                true
            })
            .collect();
        Ok(matched)
    }

    /// Detect duplicate tracks in the library by normalized `artist + title`.
    pub fn find_duplicates(&self) -> Result<Vec<DuplicateGroup>, DjError> {
        let all = self.query_tracks(&CrateQuery::default())?;
        let mut buckets: HashMap<String, Vec<CrateTrack>> = HashMap::new();
        for t in all {
            let key = normalize_dup_key(&t.artist, &t.title);
            if !key.is_empty() {
                buckets.entry(key).or_default().push(t);
            }
        }
        let mut groups: Vec<DuplicateGroup> = buckets
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(k, tracks)| DuplicateGroup {
                reason: format!("Matching artist/title ({k})"),
                tracks,
            })
            .collect();
        groups.sort_by_key(|a| std::cmp::Reverse(a.tracks.len()));
        Ok(groups)
    }

    /// Log a played track into the live Setlist History table.
    #[allow(clippy::too_many_arguments)]
    pub fn log_set_play(
        &self,
        session_id: &str,
        deck: &str,
        track_path: &str,
        title: &str,
        artist: &str,
        bpm: f64,
        musical_key: &str,
        played_at_sec: f64,
    ) -> Result<i64, DjError> {
        self.conn
            .execute(
                "INSERT INTO set_history
                 (session_id, deck, track_path, title, artist, bpm, musical_key, played_at_sec, played_at_utc)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    session_id,
                    deck,
                    track_path,
                    title,
                    artist,
                    bpm,
                    musical_key,
                    played_at_sec.max(0.0),
                    now_utc_iso(),
                ],
            )
            .map_err(|e| DjError::Deck(format!("Log set history failed: {e}")))?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Retrieve Setlist History entries (optionally filtered by `session_id`).
    pub fn list_set_history(
        &self,
        session_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SetHistoryEntry>, DjError> {
        let lim = limit.clamp(1, 2000) as i64;
        let mut out = Vec::new();
        if let Some(sid) = session_id {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT id, session_id, deck, track_path, title, artist, bpm, musical_key, played_at_sec, played_at_utc
                     FROM set_history WHERE session_id = ?1 ORDER BY id ASC LIMIT ?2",
                )
                .map_err(|e| DjError::Deck(format!("Query set history failed: {e}")))?;
            let rows = stmt
                .query_map(params![sid, lim], map_history_row)
                .map_err(|e| DjError::Deck(format!("Map set history failed: {e}")))?;
            for r in rows.flatten() {
                out.push(r);
            }
        } else {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT id, session_id, deck, track_path, title, artist, bpm, musical_key, played_at_sec, played_at_utc
                     FROM set_history ORDER BY id ASC LIMIT ?1",
                )
                .map_err(|e| DjError::Deck(format!("Query set history failed: {e}")))?;
            let rows = stmt
                .query_map(params![lim], map_history_row)
                .map_err(|e| DjError::Deck(format!("Map set history failed: {e}")))?;
            for r in rows.flatten() {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// Clear all set history entries for a session (or all if `None`).
    pub fn clear_set_history(&self, session_id: Option<&str>) -> Result<usize, DjError> {
        if let Some(sid) = session_id {
            self.conn
                .execute(
                    "DELETE FROM set_history WHERE session_id = ?1",
                    params![sid],
                )
                .map_err(|e| DjError::Deck(format!("Clear set history failed: {e}")))
        } else {
            self.conn
                .execute("DELETE FROM set_history", [])
                .map_err(|e| DjError::Deck(format!("Clear set history failed: {e}")))
        }
    }
}

fn map_history_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SetHistoryEntry> {
    Ok(SetHistoryEntry {
        id: r.get(0)?,
        session_id: r.get(1)?,
        deck: r.get(2)?,
        track_path: r.get(3)?,
        title: r.get(4)?,
        artist: r.get(5)?,
        bpm: r.get(6)?,
        musical_key: r.get(7)?,
        played_at_sec: r.get(8)?,
        played_at_utc: r.get(9)?,
    })
}

fn fmt_mm_ss(sec: f64) -> String {
    let total = sec.max(0.0).round() as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m:02}:{s:02}")
}

fn fmt_cue_mm_ss_ff(sec: f64) -> String {
    let total_frames = (sec.max(0.0) * 75.0).round() as u64;
    let ff = total_frames % 75;
    let total_sec = total_frames / 75;
    let ss = total_sec % 60;
    let mm = total_sec / 60;
    format!("{mm:02}:{ss:02}:{ff:02}")
}

/// Format a setlist as Mixcloud / YouTube timestamped plain text (`00:00 Artist - Title [128 BPM | 8A]`).
pub fn format_setlist_txt(entries: &[SetHistoryEntry]) -> String {
    let mut out = String::new();
    for (i, e) in entries.iter().enumerate() {
        let who = if e.artist.trim().is_empty() {
            e.title.clone()
        } else {
            format!("{} - {}", e.artist.trim(), e.title.trim())
        };
        out.push_str(&format!(
            "{:02}. [{}] {} ({:.1} BPM | {} | {})\n",
            i + 1,
            fmt_mm_ss(e.played_at_sec),
            who,
            e.bpm,
            if e.musical_key.is_empty() {
                "-"
            } else {
                e.musical_key.as_str()
            },
            e.deck
        ));
    }
    out
}

/// Format a setlist as a standard CD/DAW `.cue` sheet.
pub fn format_setlist_cue(
    entries: &[SetHistoryEntry],
    set_title: &str,
    audio_file: &str,
) -> String {
    let mut out = format!(
        "REM GENERATOR \"DJCraft Pro\"\nTITLE \"{}\"\nFILE \"{}\" WAVE\n",
        set_title.replace('"', "'"),
        audio_file.replace('"', "'")
    );
    for (i, e) in entries.iter().enumerate() {
        let idx = (i + 1).min(99);
        out.push_str(&format!(
            "  TRACK {idx:02} AUDIO\n    TITLE \"{}\"\n    PERFORMER \"{}\"\n    INDEX 01 {}\n",
            e.title.replace('"', "'"),
            e.artist.replace('"', "'"),
            fmt_cue_mm_ss_ff(e.played_at_sec)
        ));
    }
    out
}

/// Format a setlist as RFC-4180 CSV.
pub fn format_setlist_csv(entries: &[SetHistoryEntry]) -> String {
    let mut out = String::from("index,timestamp,deck,artist,title,bpm,key,path\n");
    let esc = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    for (i, e) in entries.iter().enumerate() {
        out.push_str(&format!(
            "{},{},{},{},{},{:.2},{},{}\n",
            i + 1,
            fmt_mm_ss(e.played_at_sec),
            esc(&e.deck),
            esc(&e.artist),
            esc(&e.title),
            e.bpm,
            esc(&e.musical_key),
            esc(&e.track_path),
        ));
    }
    out
}

/// Export setlist history to `.txt`, `.cue`, or `.csv` (inferred from extension or `format`).
pub fn export_setlist_file(
    entries: &[SetHistoryEntry],
    format: &str,
    set_title: &str,
    out_path: &Path,
) -> Result<usize, DjError> {
    let fmt = format.trim().to_ascii_lowercase();
    let content = match fmt.as_str() {
        "cue" => format_setlist_cue(entries, set_title, "djcraft_mix.wav"),
        "csv" => format_setlist_csv(entries),
        _ => format_setlist_txt(entries),
    };
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(out_path, content)
        .map_err(|e| DjError::Deck(format!("Failed to write setlist export: {e}")))?;
    Ok(entries.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_crates_duplicate_detection_and_setlist_exports_work() {
        let store = CrateStore::open_in_memory().expect("mem db");
        store
            .upsert_track(
                "a.flac",
                "Night Drive",
                "Alpha",
                "Techno",
                128.0,
                "8A",
                320.0,
            )
            .expect("t1");
        store
            .upsert_track(
                "b.flac",
                "Night Drive",
                "Alpha",
                "Techno",
                128.0,
                "8A",
                320.0,
            )
            .expect("t2 dup");
        store
            .upsert_track(
                "c.flac",
                "Morning Mist",
                "Beta",
                "House",
                122.0,
                "9A",
                290.0,
            )
            .expect("t3");

        let rule = SmartCrateRule {
            name: "Peak 8A Harmonic".into(),
            min_bpm: Some(125.0),
            max_bpm: Some(132.0),
            camelot_key: Some("8A".into()),
            harmonic_compatible: true,
            genre_contains: Some("tech".into()),
            min_rating: None,
            text_contains: None,
        };
        store.upsert_smart_crate(&rule).expect("save smart");
        assert_eq!(store.list_smart_crates().expect("list").len(), 1);
        let hits = store.query_smart_crate(&rule).expect("query");
        assert_eq!(hits.len(), 2);

        let dups = store.find_duplicates().expect("dups");
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].tracks.len(), 2);

        store
            .log_set_play(
                "gig1",
                "Deck A",
                "a.flac",
                "Night Drive",
                "Alpha",
                128.0,
                "8A",
                0.0,
            )
            .expect("log1");
        store
            .log_set_play(
                "gig1",
                "Deck B",
                "c.flac",
                "Morning Mist",
                "Beta",
                122.0,
                "9A",
                185.0,
            )
            .expect("log2");
        let hist = store.list_set_history(Some("gig1"), 50).expect("hist");
        assert_eq!(hist.len(), 2);

        let txt = format_setlist_txt(&hist);
        assert!(txt.contains("[03:05] Beta - Morning Mist"));
        let cue = format_setlist_cue(&hist, "Vienna Live", "mix.wav");
        assert!(cue.contains("INDEX 01 03:05:00"));
        let csv = format_setlist_csv(&hist);
        assert!(csv.contains("\"Alpha\",\"Night Drive\""));
    }
}
