#![forbid(unsafe_code)]
//! AlphaTheta / Pioneer Rekordbox XML (`rekordbox.xml`) Import & Export.
//!
//! Pure Safe Rust parser and writer for `<DJ_PLAYLISTS Version="1.0.0">` libraries:
//! - `<COLLECTION><TRACK ...>` with `Location` URL decoding (`file://localhost/...`),
//!   `Name`, `Artist`, `Genre`, `AverageBpm`, `Tonality` (normalized to Camelot), `TotalTime`
//! - `<POSITION_MARK Type="0" Num="0..3" Start="..." Name="..." Red/Green/Blue="..." />`
//!   auto-imported into SQLite `track_cues` + `track_cue_meta`
//! - `<PLAYLISTS>` playlist `<NODE Type="1" Name="...">` -> DJCraft crates

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CrateQuery, CrateStore, CrateTrack, DjError, ImportSummary, TrackCueMeta};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RekordboxCue {
    pub slot: usize,
    pub start_sec: f64,
    pub name: String,
    pub color: Option<[u8; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RekordboxTrack {
    pub track_id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub genre: String,
    pub bpm: f64,
    pub musical_key: String,
    pub duration_seconds: f64,
    pub cues: Vec<RekordboxCue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RekordboxPlaylist {
    pub name: String,
    pub track_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RekordboxLibrary {
    pub tracks: Vec<RekordboxTrack>,
    pub playlists: Vec<RekordboxPlaylist>,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Percent-decode a Rekordbox `file://localhost/...` URI into a local filesystem path.
pub fn decode_rekordbox_location(uri: &str) -> String {
    let mut s = xml_unescape(uri.trim());
    for prefix in ["file://localhost/", "file:///"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let decoded = String::from_utf8_lossy(&out).into_owned();
    // On POSIX paths (no drive letter like "C:/"), restore leading '/'
    if decoded.len() >= 2 && decoded.as_bytes()[1] == b':' {
        decoded
    } else if !decoded.starts_with('/') && uri.starts_with("file://") {
        format!("/{decoded}")
    } else {
        decoded
    }
}

/// Encode a local path into a Rekordbox-compatible `file://localhost/...` URI.
pub fn encode_rekordbox_location(path: &str) -> String {
    let norm = path.replace('\\', "/");
    let stripped = norm.trim_start_matches('/');
    let mut encoded = String::from("file://localhost/");
    for b in stripped.bytes() {
        let keep = b.is_ascii_alphanumeric() || matches!(b, b'/' | b':' | b'-' | b'_' | b'.');
        if keep {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    encoded
}

/// Convert standard Rekordbox `Tonality` (e.g. `Am`, `C`, `F#m`, `Bb`, or already Camelot `8A`) to Camelot.
pub fn tonality_to_camelot(tonality: &str) -> String {
    let raw = tonality.trim();
    if raw.is_empty() {
        return String::new();
    }
    let up = raw.to_uppercase();
    // Already Camelot (1A..12B)
    if (up.ends_with('A') || up.ends_with('B')) && up.len() <= 3 {
        if let Ok(n) = up[..up.len() - 1].parse::<u8>() {
            if (1..=12).contains(&n) {
                return up;
            }
        }
    }
    match raw {
        "Abm" | "G#m" | "G#min" | "Abmin" => "1A",
        "Ebm" | "D#m" | "D#min" | "Ebmin" => "2A",
        "Bbm" | "A#m" | "A#min" | "Bbmin" => "3A",
        "Fm" | "Fmin" => "4A",
        "Cm" | "Cmin" => "5A",
        "Gm" | "Gmin" => "6A",
        "Dm" | "Dmin" => "7A",
        "Am" | "Amin" => "8A",
        "Em" | "Emin" => "9A",
        "Bm" | "Bmin" => "10A",
        "F#m" | "Gbm" | "F#min" | "Gbmin" => "11A",
        "C#m" | "Dbm" | "C#min" | "Dbmin" => "12A",
        "B" | "Bmaj" => "1B",
        "F#" | "Gb" | "F#maj" | "Gbmaj" => "2B",
        "Db" | "C#" | "Dbmaj" | "C#maj" => "3B",
        "Ab" | "G#" | "Abmaj" | "G#maj" => "4B",
        "Eb" | "D#" | "Ebmaj" | "D#maj" => "5B",
        "Bb" | "A#" | "Bbmaj" | "A#maj" => "6B",
        "F" | "Fmaj" => "7B",
        "C" | "Cmaj" => "8B",
        "G" | "Gmaj" => "9B",
        "D" | "Dmaj" => "10B",
        "A" | "Amaj" => "11B",
        "E" | "Emaj" => "12B",
        _ => raw,
    }
    .to_string()
}

fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let idx = attrs.find(&needle)?;
    let after = &attrs[idx + needle.len()..];
    let end = after.find('"')?;
    Some(xml_unescape(&after[..end]))
}

/// Parse an AlphaTheta / Pioneer `rekordbox.xml` document into tracks, cues, and playlists.
pub fn parse_rekordbox_xml(xml: &str) -> RekordboxLibrary {
    let mut lib = RekordboxLibrary::default();

    // 1. Parse <COLLECTION> ... </COLLECTION>
    let coll_slice = if let Some(s) = xml.find("<COLLECTION") {
        let end = xml[s..]
            .find("</COLLECTION>")
            .map(|e| s + e)
            .unwrap_or(xml.len());
        &xml[s..end]
    } else {
        xml
    };

    let mut rest = coll_slice;
    while let Some(idx) = rest.find("<TRACK ") {
        rest = &rest[idx + 7..];
        let Some(tag_end) = rest.find('>') else {
            break;
        };
        let self_closing = rest[..tag_end].trim_end().ends_with('/');
        let attrs = &rest[..tag_end];
        let track_id = extract_attr(attrs, "TrackID").unwrap_or_default();
        let path = extract_attr(attrs, "Location")
            .map(|u| decode_rekordbox_location(&u))
            .unwrap_or_default();
        let title = extract_attr(attrs, "Name").unwrap_or_default();
        let artist = extract_attr(attrs, "Artist").unwrap_or_default();
        let genre = extract_attr(attrs, "Genre").unwrap_or_default();
        let bpm = extract_attr(attrs, "AverageBpm")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(124.0);
        let musical_key = extract_attr(attrs, "Tonality")
            .map(|k| tonality_to_camelot(&k))
            .unwrap_or_default();
        let duration_seconds = extract_attr(attrs, "TotalTime")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);

        rest = &rest[tag_end + 1..];
        let mut cues = Vec::new();
        if !self_closing {
            if let Some(close_idx) = rest.find("</TRACK>") {
                let inner = &rest[..close_idx];
                let mut m_rest = inner;
                while let Some(m_idx) = m_rest.find("<POSITION_MARK ") {
                    m_rest = &m_rest[m_idx + 15..];
                    let Some(m_end) = m_rest.find('>') else {
                        break;
                    };
                    let m_attrs = &m_rest[..m_end];
                    // Num="0..3" are Hot Cues A..D; Num="-1" are memory cues
                    if let Some(num) =
                        extract_attr(m_attrs, "Num").and_then(|v| v.parse::<i32>().ok())
                    {
                        if (0..4).contains(&num) {
                            if let Some(start) =
                                extract_attr(m_attrs, "Start").and_then(|v| v.parse::<f64>().ok())
                            {
                                let name = extract_attr(m_attrs, "Name").unwrap_or_default();
                                let color = match (
                                    extract_attr(m_attrs, "Red").and_then(|v| v.parse::<u8>().ok()),
                                    extract_attr(m_attrs, "Green")
                                        .and_then(|v| v.parse::<u8>().ok()),
                                    extract_attr(m_attrs, "Blue")
                                        .and_then(|v| v.parse::<u8>().ok()),
                                ) {
                                    (Some(r), Some(g), Some(b)) => Some([r, g, b]),
                                    _ => None,
                                };
                                cues.push(RekordboxCue {
                                    slot: num as usize,
                                    start_sec: start.max(0.0),
                                    name,
                                    color,
                                });
                            }
                        }
                    }
                    m_rest = &m_rest[m_end + 1..];
                }
                rest = &rest[close_idx + 8..];
            }
        }

        if !path.trim().is_empty() {
            lib.tracks.push(RekordboxTrack {
                track_id,
                path,
                title,
                artist,
                genre,
                bpm,
                musical_key,
                duration_seconds,
                cues,
            });
        }
    }

    // 2. Parse <PLAYLISTS> ... </PLAYLISTS>
    if let Some(ps) = xml.find("<PLAYLISTS") {
        let pe = xml[ps..]
            .find("</PLAYLISTS>")
            .map(|e| ps + e)
            .unwrap_or(xml.len());
        let mut p_rest = &xml[ps..pe];
        while let Some(n_idx) = p_rest.find("<NODE ") {
            p_rest = &p_rest[n_idx + 6..];
            let Some(tag_end) = p_rest.find('>') else {
                break;
            };
            let attrs = &p_rest[..tag_end];
            let node_type = extract_attr(attrs, "Type").unwrap_or_default();
            let node_name = extract_attr(attrs, "Name").unwrap_or_default();
            p_rest = &p_rest[tag_end + 1..];

            if node_type == "1" && !node_name.is_empty() {
                if let Some(close_idx) = p_rest.find("</NODE>") {
                    let inner = &p_rest[..close_idx];
                    let mut track_ids = Vec::new();
                    let mut t_rest = inner;
                    while let Some(t_idx) = t_rest.find("<TRACK ") {
                        t_rest = &t_rest[t_idx + 7..];
                        let Some(te) = t_rest.find('>') else {
                            break;
                        };
                        if let Some(key) = extract_attr(&t_rest[..te], "Key") {
                            track_ids.push(key);
                        }
                        t_rest = &t_rest[te + 1..];
                    }
                    lib.playlists.push(RekordboxPlaylist {
                        name: node_name,
                        track_ids,
                    });
                    p_rest = &p_rest[close_idx + 7..];
                }
            }
        }
    }

    lib
}

/// Encode DJCraft tracks + SQLite hot cues as a Rekordbox 6/7 compatible `rekordbox.xml`.
pub fn encode_rekordbox_xml(
    tracks: &[CrateTrack],
    cues_by_path: &HashMap<String, ([Option<f64>; 4], [TrackCueMeta; 4])>,
    playlist_name: &str,
) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<DJ_PLAYLISTS Version=\"1.0.0\">\n  <PRODUCT Name=\"DJCraft\" Version=\"0.4.0\" Company=\"Antigravity Craft Collective\" />\n",
    );
    xml.push_str(&format!("  <COLLECTION Entries=\"{}\">\n", tracks.len()));
    for (idx, t) in tracks.iter().enumerate() {
        let tid = idx + 1;
        let loc = encode_rekordbox_location(&t.path);
        let dur = t.duration_seconds.round() as i64;
        xml.push_str(&format!(
            "    <TRACK TrackID=\"{tid}\" Name=\"{}\" Artist=\"{}\" Genre=\"{}\" TotalTime=\"{dur}\" AverageBpm=\"{:.2}\" Tonality=\"{}\" Location=\"{}\">\n",
            xml_escape(&t.title),
            xml_escape(&t.artist),
            xml_escape(&t.genre),
            t.bpm,
            xml_escape(&t.musical_key),
            xml_escape(&loc),
        ));
        if let Some((cue_secs, cue_metas)) = cues_by_path.get(&t.path) {
            for slot in 0..4 {
                if let Some(pos) = cue_secs[slot] {
                    let meta = &cue_metas[slot];
                    let color_attrs = if let Some([r, g, b]) = meta.color {
                        format!(" Red=\"{r}\" Green=\"{g}\" Blue=\"{b}\"")
                    } else {
                        String::new()
                    };
                    xml.push_str(&format!(
                        "      <POSITION_MARK Name=\"{}\" Type=\"0\" Start=\"{pos:.3}\" Num=\"{slot}\"{color_attrs} />\n",
                        xml_escape(&meta.label),
                    ));
                }
            }
        }
        xml.push_str("    </TRACK>\n");
    }
    xml.push_str(
        "  </COLLECTION>\n  <PLAYLISTS>\n    <NODE Type=\"0\" Name=\"ROOT\" Count=\"1\">\n",
    );
    xml.push_str(&format!(
        "      <NODE Name=\"{}\" Type=\"1\" KeyType=\"0\" Entries=\"{}\">\n",
        xml_escape(playlist_name),
        tracks.len()
    ));
    for idx in 0..tracks.len() {
        xml.push_str(&format!("        <TRACK Key=\"{}\" />\n", idx + 1));
    }
    xml.push_str("      </NODE>\n    </NODE>\n  </PLAYLISTS>\n</DJ_PLAYLISTS>\n");
    xml
}

/// Import an AlphaTheta / Pioneer `rekordbox.xml` file into a DJCraft `CrateStore`.
pub fn import_rekordbox_xml_file(
    store: &CrateStore,
    xml_path: &Path,
) -> Result<ImportSummary, DjError> {
    let xml = std::fs::read_to_string(xml_path)
        .map_err(|e| DjError::Deck(format!("Read rekordbox.xml failed: {e}")))?;
    let lib = parse_rekordbox_xml(&xml);
    let default_crate_name = xml_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Rekordbox Import");
    let root_crate_id = store.ensure_crate_path(&format!("Rekordbox/{default_crate_name}"))?;

    let mut id_map: HashMap<String, i64> = HashMap::new();
    let mut imported_tracks = 0usize;

    for tr in &lib.tracks {
        let title = if tr.title.is_empty() {
            Path::new(&tr.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        } else {
            tr.title.clone()
        };
        let db_id = store.upsert_track(
            &tr.path,
            &title,
            &tr.artist,
            &tr.genre,
            tr.bpm,
            &tr.musical_key,
            tr.duration_seconds,
        )?;
        let _ = store.add_track_to_crate(root_crate_id, db_id);
        if !tr.track_id.is_empty() {
            id_map.insert(tr.track_id.clone(), db_id);
        }
        for cue in &tr.cues {
            let _ = store.set_track_hot_cue(&tr.path, cue.slot, Some(cue.start_sec));
            let meta = TrackCueMeta {
                label: cue.name.clone(),
                color: cue.color,
                behavior: "standard".to_string(),
            };
            let _ = store.set_track_hot_cue_meta(&tr.path, cue.slot, &meta);
        }
        imported_tracks += 1;
    }

    let mut created_crates = 1usize;
    for pl in &lib.playlists {
        let cid = store.ensure_crate_path(&format!("Rekordbox/{}", pl.name))?;
        created_crates += 1;
        for tid in &pl.track_ids {
            if let Some(&db_id) = id_map.get(tid) {
                let _ = store.add_track_to_crate(cid, db_id);
            }
        }
    }

    Ok(ImportSummary {
        format: "rekordbox_xml".to_string(),
        crate_id: root_crate_id,
        crate_path: format!("Rekordbox/{default_crate_name} ({created_crates} crate(s))"),
        imported_tracks,
    })
}

/// Export a DJCraft crate (or full library when `crate_id` is `None`) to `rekordbox.xml`.
pub fn export_rekordbox_xml_file(
    store: &CrateStore,
    crate_id: Option<i64>,
    out_path: &Path,
) -> Result<usize, DjError> {
    let query = CrateQuery {
        crate_id,
        ..Default::default()
    };
    let tracks = store.query_tracks(&query)?;
    let mut cues_by_path = HashMap::new();
    for t in &tracks {
        let secs = store.get_track_hot_cues(&t.path).unwrap_or([None; 4]);
        let metas = store
            .get_track_hot_cue_metas(&t.path)
            .unwrap_or_else(|_| std::array::from_fn(|_| TrackCueMeta::default()));
        cues_by_path.insert(t.path.clone(), (secs, metas));
    }
    let playlist_name = out_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("DJCraft Export");
    let xml = encode_rekordbox_xml(&tracks, &cues_by_path, playlist_name);
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(out_path, xml)
        .map_err(|e| DjError::Deck(format!("Write rekordbox.xml failed: {e}")))?;
    Ok(tracks.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rekordbox_xml_roundtrip_cues_colors_and_playlists() {
        let sample = r#"<?xml version="1.0" encoding="UTF-8"?>
<DJ_PLAYLISTS Version="1.0.0">
  <COLLECTION Entries="2">
    <TRACK TrackID="101" Name="Midnight Pulse" Artist="Sandra" Genre="Techno" TotalTime="312" AverageBpm="130.00" Tonality="Am" Location="file://localhost/D:/Music/Midnight%20Pulse.flac">
      <POSITION_MARK Name="Drop 1" Type="0" Start="64.500" Num="0" Red="239" Green="68" Blue="68" />
      <POSITION_MARK Name="Break" Type="0" Start="142.250" Num="2" Red="56" Green="189" Blue="248" />
    </TRACK>
    <TRACK TrackID="102" Name="Solar Groove" Artist="Goliath" Genre="House" TotalTime="280" AverageBpm="124.50" Tonality="C" Location="file://localhost/D:/Music/Solar%20Groove.mp3" />
  </COLLECTION>
  <PLAYLISTS>
    <NODE Type="0" Name="ROOT" Count="1">
      <NODE Name="Peak Hour" Type="1" KeyType="0" Entries="2">
        <TRACK Key="101" />
        <TRACK Key="102" />
      </NODE>
    </NODE>
  </PLAYLISTS>
</DJ_PLAYLISTS>"#;

        let lib = parse_rekordbox_xml(sample);
        assert_eq!(lib.tracks.len(), 2);
        assert_eq!(lib.tracks[0].path, "D:/Music/Midnight Pulse.flac");
        assert_eq!(lib.tracks[0].musical_key, "8A");
        assert_eq!(lib.tracks[0].cues.len(), 2);
        assert_eq!(lib.tracks[0].cues[0].name, "Drop 1");
        assert_eq!(lib.tracks[0].cues[0].color, Some([239, 68, 68]));
        assert_eq!(lib.tracks[1].musical_key, "8B");
        assert_eq!(lib.playlists.len(), 1);
        assert_eq!(lib.playlists[0].name, "Peak Hour");
        assert_eq!(lib.playlists[0].track_ids, vec!["101", "102"]);

        // Full SQLite import -> export round-trip
        let store = CrateStore::open_in_memory().expect("db");
        let tmp = std::env::temp_dir().join("djcraft_rb_test.xml");
        let tmp_out = std::env::temp_dir().join("djcraft_rb_export.xml");
        std::fs::write(&tmp, sample).expect("write tmp");
        let summary = import_rekordbox_xml_file(&store, &tmp).expect("import");
        assert_eq!(summary.imported_tracks, 2);
        let cues = store
            .get_track_hot_cues("D:/Music/Midnight Pulse.flac")
            .expect("cues");
        assert_eq!(cues[0], Some(64.5));
        assert_eq!(cues[2], Some(142.25));
        let metas = store
            .get_track_hot_cue_metas("D:/Music/Midnight Pulse.flac")
            .expect("metas");
        assert_eq!(metas[0].label, "Drop 1");
        assert_eq!(metas[0].color, Some([239, 68, 68]));

        let exported = export_rekordbox_xml_file(&store, None, &tmp_out).expect("export");
        assert_eq!(exported, 2);
        let re_xml = std::fs::read_to_string(&tmp_out).expect("read export");
        let re_lib = parse_rekordbox_xml(&re_xml);
        assert_eq!(re_lib.tracks.len(), 2);
        assert_eq!(re_lib.tracks[0].cues.len(), 2);
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&tmp_out);
    }
}
