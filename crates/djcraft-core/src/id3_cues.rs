#![forbid(unsafe_code)]
//! ID3v2 tag & `Serato Markers2` GEOB cue-point reader/writer.
//!
//! Reads standard ID3v2 metadata (`TIT2`, `TPE1`, `TCON`, `TBPM`, `TKEY`) plus the de-facto
//! cross-DJ `Serato Markers2` GEOB frame (supported by Serato, VirtualDJ, Mixxx, Engine DJ,
//! and rekordbox). When writing cue points back, all other ID3 frames and non-CUE Serato
//! entries (`COLOR`, `LOOP`, `BPMLOCK`, `FLIP`) are preserved untouched.

use std::path::Path;

use id3::frame::{Content, EncapsulatedObject};
use id3::{Frame, Tag, TagLike, Version};
use serde::{Deserialize, Serialize};

use crate::{DjError, TrackCueMeta};

const SERATO_MARKERS2_DESC: &str = "Serato Markers2";
const SERATO_HEADER: [u8; 2] = [0x01, 0x01];

/// Decoded single cue entry from a `Serato Markers2` GEOB frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeratoCue {
    pub index: u8,
    pub position_ms: u32,
    pub color: [u8; 3],
    pub name: String,
}

/// Metadata and hot cues extracted from an audio file's ID3v2 header.
#[derive(Debug, Clone, Default)]
pub struct Id3TrackInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub bpm: Option<f64>,
    pub musical_key: Option<String>,
    pub cues: [Option<f64>; 4],
    pub cue_metas: [TrackCueMeta; 4],
    pub cue_count: usize,
}

/// Summary of a batch ID3 cue read or write across a crate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchId3Summary {
    pub tracks_scanned: usize,
    pub tracks_updated: usize,
    pub cues_processed: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Read ID3v2 metadata and `Serato Markers2` cue points from an audio file.
///
/// Only reads the ID3 tag header (no audio decoding). Returns `Ok(default)` when a file
/// has no ID3 tag.
pub fn read_id3_track_info(path: &Path) -> Result<Id3TrackInfo, DjError> {
    if !path.is_file() {
        return Ok(Id3TrackInfo::default());
    }
    let tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        Err(id3::Error {
            kind: id3::ErrorKind::NoTag,
            ..
        }) => return Ok(Id3TrackInfo::default()),
        Err(e) => {
            return Err(DjError::Deck(format!(
                "Cannot read ID3 tag from {}: {e}",
                path.display()
            )))
        }
    };

    let mut info = Id3TrackInfo {
        title: tag
            .title()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        artist: tag
            .artist()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        genre: tag
            .genre_parsed()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        bpm: tag
            .get("TBPM")
            .and_then(|f| f.content().text())
            .and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 20.0 && *v < 300.0),
        musical_key: tag
            .get("TKEY")
            .and_then(|f| f.content().text())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        ..Default::default()
    };

    for geob in tag.encapsulated_objects() {
        if geob.description.eq_ignore_ascii_case(SERATO_MARKERS2_DESC) {
            let parsed = parse_serato_markers2(&geob.data);
            for cue in parsed {
                let slot = cue.index as usize;
                if slot < 4 {
                    info.cues[slot] = Some(f64::from(cue.position_ms) / 1000.0);
                    info.cue_metas[slot] = TrackCueMeta {
                        label: cue.name,
                        color: Some(cue.color),
                        behavior: "standard".to_string(),
                    };
                }
            }
            break;
        }
    }
    info.cue_count = info.cues.iter().filter(|c| c.is_some()).count();
    Ok(info)
}

/// Write DJCraft's 4 hot cues (with labels and RGB colors) into the file's ID3v2
/// `Serato Markers2` GEOB frame, preserving all other ID3 tags and non-CUE Serato entries.
/// Returns the number of cues written.
pub fn write_id3_hot_cues(
    path: &Path,
    cues: &[Option<f64>; 4],
    metas: &[TrackCueMeta; 4],
) -> Result<usize, DjError> {
    if !path.is_file() {
        return Err(DjError::Deck(format!(
            "Audio file not found: {}",
            path.display()
        )));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "mp3" | "aif" | "aiff" | "wav") {
        return Err(DjError::Deck(format!(
            "ID3v2 cue writing supports MP3/AIFF/WAV (got .{ext})"
        )));
    }

    let mut tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        Err(id3::Error {
            kind: id3::ErrorKind::NoTag,
            ..
        }) => Tag::new(),
        Err(e) => {
            return Err(DjError::Deck(format!(
                "Cannot read existing ID3 tag from {}: {e}",
                path.display()
            )))
        }
    };

    let existing_raw = tag
        .encapsulated_objects()
        .find(|g| g.description.eq_ignore_ascii_case(SERATO_MARKERS2_DESC))
        .map(|g| g.data.clone());

    let encoded = encode_serato_markers2(cues, metas, existing_raw.as_deref());
    let written_count = cues.iter().filter(|c| c.is_some()).count();

    // Remove only the previous `Serato Markers2` GEOB frame, keeping all other GEOBs intact.
    let kept_geobs: Vec<EncapsulatedObject> = tag
        .encapsulated_objects()
        .filter(|g| !g.description.eq_ignore_ascii_case(SERATO_MARKERS2_DESC))
        .cloned()
        .collect();
    tag.remove("GEOB");
    for g in kept_geobs {
        tag.add_frame(Frame::with_content("GEOB", Content::EncapsulatedObject(g)));
    }
    tag.add_frame(Frame::with_content(
        "GEOB",
        Content::EncapsulatedObject(EncapsulatedObject {
            mime_type: "application/octet-stream".to_string(),
            filename: String::new(),
            description: SERATO_MARKERS2_DESC.to_string(),
            data: encoded,
        }),
    ));

    tag.write_to_path(path, Version::Id3v24).map_err(|e| {
        DjError::Deck(format!(
            "Failed to write ID3v2 cue tags to {}: {e}",
            path.display()
        ))
    })?;
    Ok(written_count)
}

/// Parse `CUE` entries from raw `Serato Markers2` GEOB payload bytes.
pub fn parse_serato_markers2(raw: &[u8]) -> Vec<SeratoCue> {
    let inner = decode_serato_inner(raw);
    let mut cues = Vec::new();
    for (entry_type, payload) in parse_serato_entries(&inner) {
        if entry_type == "CUE" && payload.len() >= 12 {
            let index = payload[1];
            let position_ms = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]);
            let color = [payload[7], payload[8], payload[9]];
            let name_bytes = &payload[12..];
            let end = name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(name_bytes.len());
            let name = String::from_utf8_lossy(&name_bytes[..end])
                .trim()
                .to_string();
            cues.push(SeratoCue {
                index,
                position_ms,
                color,
                name,
            });
        }
    }
    cues
}

/// Encode DJCraft's 4 hot cues into raw `Serato Markers2` GEOB payload bytes.
/// Preserves any existing non-`CUE` entries (`COLOR`, `LOOP`, `BPMLOCK`, etc.) from `existing_raw`.
pub fn encode_serato_markers2(
    cues: &[Option<f64>; 4],
    metas: &[TrackCueMeta; 4],
    existing_raw: Option<&[u8]>,
) -> Vec<u8> {
    let default_colors: [[u8; 3]; 4] = [
        [204, 0, 0],   // Slot 1: red
        [0, 153, 255], // Slot 2: blue
        [0, 204, 102], // Slot 3: green
        [255, 170, 0], // Slot 4: amber
    ];

    let mut inner = Vec::new();
    inner.extend_from_slice(&SERATO_HEADER);

    // Keep existing non-CUE entries (COLOR, LOOP, BPMLOCK, etc.) untouched.
    if let Some(raw) = existing_raw {
        let prev_inner = decode_serato_inner(raw);
        for (entry_type, payload) in parse_serato_entries(&prev_inner) {
            if entry_type != "CUE" {
                push_serato_entry(&mut inner, &entry_type, &payload);
            }
        }
    }

    for (slot, (pos_opt, meta)) in cues.iter().zip(metas.iter()).enumerate() {
        let Some(pos_s) = pos_opt else { continue };
        let pos_ms = (pos_s.max(0.0) * 1000.0).round() as u32;
        let rgb = meta.color.unwrap_or(default_colors[slot]);
        let mut payload = Vec::with_capacity(13 + meta.label.len());
        payload.push(0x00);
        payload.push(slot as u8);
        payload.extend_from_slice(&pos_ms.to_be_bytes());
        payload.push(0x00);
        payload.extend_from_slice(&rgb);
        payload.push(0x00);
        payload.push(0x00);
        payload.extend_from_slice(meta.label.as_bytes());
        payload.push(0x00);
        push_serato_entry(&mut inner, "CUE", &payload);
    }

    // Serato terminates inner payload with a trailing 0x00 byte.
    inner.push(0x00);

    let b64 = base64_encode(&inner);
    let mut outer = Vec::with_capacity(2 + b64.len());
    outer.extend_from_slice(&SERATO_HEADER);
    outer.extend_from_slice(b64.as_bytes());
    outer
}

fn push_serato_entry(buf: &mut Vec<u8>, entry_type: &str, payload: &[u8]) {
    buf.extend_from_slice(entry_type.as_bytes());
    buf.push(0x00);
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);
}

fn decode_serato_inner(raw: &[u8]) -> Vec<u8> {
    if raw.len() < 3 || raw[0] != 0x01 || raw[1] != 0x01 {
        return Vec::new();
    }
    // Strip trailing NUL padding if present before base64 decoding.
    let mut b64_slice = &raw[2..];
    while b64_slice.last() == Some(&0) {
        b64_slice = &b64_slice[..b64_slice.len() - 1];
    }
    base64_decode(b64_slice)
}

fn parse_serato_entries(inner: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    if inner.len() < 2 || inner[0] != 0x01 || inner[1] != 0x01 {
        return out;
    }
    let mut pos = 2usize;
    while pos < inner.len() {
        if inner[pos] == 0 {
            break;
        }
        let Some(nul_rel) = inner[pos..].iter().position(|&b| b == 0) else {
            break;
        };
        let entry_type = String::from_utf8_lossy(&inner[pos..pos + nul_rel]).to_string();
        pos += nul_rel + 1;
        if pos + 4 > inner.len() {
            break;
        }
        let len = u32::from_be_bytes([inner[pos], inner[pos + 1], inner[pos + 2], inner[pos + 3]])
            as usize;
        pos += 4;
        if pos + len > inner.len() {
            break;
        }
        out.push((entry_type, inner[pos..pos + len].to_vec()));
        pos += len;
    }
    out
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = if chunk.len() > 1 {
            u32::from(chunk[1])
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            u32::from(chunk[2])
        } else {
            0
        };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(input: &[u8]) -> Vec<u8> {
    let mut vals = Vec::with_capacity(input.len());
    for &b in input {
        if b.is_ascii_whitespace() || b == b'=' {
            continue;
        }
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        vals.push(v);
    }
    let mut out = Vec::with_capacity((vals.len() * 3) / 4);
    let mut i = 0;
    while i + 1 < vals.len() {
        let v0 = u32::from(vals[i]);
        let v1 = u32::from(vals[i + 1]);
        out.push(((v0 << 2) | (v1 >> 4)) as u8);
        if i + 2 < vals.len() {
            let v2 = u32::from(vals[i + 2]);
            out.push((((v1 & 0x0F) << 4) | (v2 >> 2)) as u8);
            if i + 3 < vals.len() {
                let v3 = u32::from(vals[i + 3]);
                out.push((((v2 & 0x03) << 6) | v3) as u8);
            }
        }
        i += 4;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serato_markers2_round_trips_named_colored_cues_and_preserves_other_entries() {
        // Build an existing payload with a COLOR entry so we verify preservation.
        let mut seed_inner = Vec::new();
        seed_inner.extend_from_slice(&SERATO_HEADER);
        push_serato_entry(&mut seed_inner, "COLOR", &[0x00, 0x11, 0x22, 0x33]);
        seed_inner.push(0x00);
        let mut existing_raw = Vec::new();
        existing_raw.extend_from_slice(&SERATO_HEADER);
        existing_raw.extend_from_slice(base64_encode(&seed_inner).as_bytes());

        let cues = [Some(12.5), Some(64.25), None, Some(180.0)];
        let metas = [
            TrackCueMeta {
                label: "Intro".to_string(),
                color: Some([255, 40, 40]),
                behavior: "standard".to_string(),
            },
            TrackCueMeta {
                label: "Main Drop".to_string(),
                color: Some([0, 200, 255]),
                behavior: "standard".to_string(),
            },
            TrackCueMeta::default(),
            TrackCueMeta {
                label: "Outro".to_string(),
                color: Some([120, 255, 80]),
                behavior: "standard".to_string(),
            },
        ];

        let raw = encode_serato_markers2(&cues, &metas, Some(&existing_raw));
        let parsed = parse_serato_markers2(&raw);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].index, 0);
        assert_eq!(parsed[0].position_ms, 12_500);
        assert_eq!(parsed[0].name, "Intro");
        assert_eq!(parsed[0].color, [255, 40, 40]);
        assert_eq!(parsed[1].index, 1);
        assert_eq!(parsed[1].position_ms, 64_250);
        assert_eq!(parsed[1].name, "Main Drop");
        assert_eq!(parsed[2].index, 3);
        assert_eq!(parsed[2].position_ms, 180_000);
        assert_eq!(parsed[2].name, "Outro");

        // Verify COLOR entry was preserved alongside CUE entries.
        let inner = decode_serato_inner(&raw);
        let entries = parse_serato_entries(&inner);
        assert!(entries
            .iter()
            .any(|(t, p)| t == "COLOR" && p == &[0x00, 0x11, 0x22, 0x33]));
    }

    #[test]
    fn id3_file_round_trip_writes_and_auto_imports_into_crate_store() {
        let mut mp3 = std::env::temp_dir();
        mp3.push(format!("djcraft_id3_cue_test_{}.mp3", std::process::id()));
        let _ = std::fs::remove_file(&mp3);
        // Minimal valid ID3v2.4 tag with title + BPM + dummy MP3 frame bytes.
        std::fs::write(&mp3, [0xFFu8, 0xFB, 0x90, 0x00, 0x00, 0x00]).ok();
        let mut seed_tag = Tag::new();
        seed_tag.set_title("Vienna Night");
        seed_tag.set_artist("DJ Test");
        seed_tag.add_frame(Frame::text("TBPM", "128"));
        seed_tag.add_frame(Frame::text("TKEY", "8A"));
        assert!(seed_tag.write_to_path(&mp3, Version::Id3v24).is_ok());

        let cues = [Some(16.0), Some(48.5), None, None];
        let metas = [
            TrackCueMeta {
                label: "Drop 1".to_string(),
                color: Some([255, 0, 80]),
                behavior: "standard".to_string(),
            },
            TrackCueMeta {
                label: "Break".to_string(),
                color: Some([0, 180, 255]),
                behavior: "standard".to_string(),
            },
            TrackCueMeta::default(),
            TrackCueMeta::default(),
        ];
        assert_eq!(write_id3_hot_cues(&mp3, &cues, &metas).ok(), Some(2));

        let info = read_id3_track_info(&mp3).expect("read back");
        assert_eq!(info.title.as_deref(), Some("Vienna Night"));
        assert_eq!(info.bpm, Some(128.0));
        assert_eq!(info.musical_key.as_deref(), Some("8A"));
        assert_eq!(info.cue_count, 2);
        assert_eq!(info.cue_metas[0].label, "Drop 1");
        assert_eq!(info.cue_metas[1].label, "Break");

        // Verify CrateStore::upsert_track automatically imports ID3 cues into SQLite.
        let store = crate::CrateStore::open_in_memory().expect("mem db");
        let path_str = mp3.to_string_lossy().into_owned();
        let _ = store
            .upsert_track(&path_str, "", "", "", 0.0, "", 0.0)
            .expect("upsert");
        let loaded_cues = store.get_track_hot_cues(&path_str).expect("cues");
        assert_eq!(loaded_cues[0], Some(16.0));
        assert_eq!(loaded_cues[1], Some(48.5));
        let loaded_metas = store.get_track_hot_cue_metas(&path_str).expect("metas");
        assert_eq!(loaded_metas[0].label, "Drop 1");
        assert_eq!(loaded_metas[0].color, Some([255, 0, 80]));

        let _ = std::fs::remove_file(&mp3);
    }
}
