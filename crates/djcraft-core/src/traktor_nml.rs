#![forbid(unsafe_code)]
//! Native Instruments Traktor Pro 4 (`collection.nml`) Import & Export.
//!
//! Pure Safe Rust parser and writer for `<NML VERSION="19">` libraries:
//! - `<COLLECTION><ENTRY TITLE="..." ARTIST="...">`
//!   - `<LOCATION DIR="/:Music/:Sets/:" FILE="track.flac" VOLUME="D:" />`
//!   - `<INFO GENRE="..." PLAYTIME="320" />`
//!   - `<TEMPO BPM="128.000000" />`
//!   - `<MUSICAL_KEY VALUE="0..23" />` (Traktor OpenKey index -> Camelot)
//!   - `<CUE_V2 NAME="..." TYPE="0" START="16000.0" HOTCUE="0..3" />` (ms -> sec)
//! - `<PLAYLISTS><NODE TYPE="PLAYLIST" NAME="..."><PLAYLIST><ENTRY><PRIMARYKEY KEY="..." /></ENTRY></PLAYLIST></NODE>`

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CrateQuery, CrateStore, CrateTrack, DjError, ImportSummary, TrackCueMeta};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraktorCue {
    pub slot: usize,
    pub start_sec: f64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraktorTrack {
    pub primary_key: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub genre: String,
    pub bpm: f64,
    pub musical_key: String,
    pub duration_seconds: f64,
    pub cues: Vec<TraktorCue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraktorPlaylist {
    pub name: String,
    pub primary_keys: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TraktorLibrary {
    pub tracks: Vec<TraktorTrack>,
    pub playlists: Vec<TraktorPlaylist>,
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

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(xml_unescape(&rest[..end]))
}

/// Convert Traktor's `<MUSICAL_KEY VALUE="0..23">` index into standard Camelot notation.
///
/// Traktor encodes 0..11 as major keys (C, Db, D, Eb, E, F, Gb, G, Ab, A, Bb, B)
/// and 12..23 as minor keys (Cm, Dbm, Dm, Ebm, Em, Fm, Gbm, Gm, Abm, Am, Bbm, Bm).
pub fn traktor_key_to_camelot(value: u32) -> &'static str {
    match value {
        0 => "8B",   // C major
        1 => "3B",   // Db major
        2 => "10B",  // D major
        3 => "5B",   // Eb major
        4 => "12B",  // E major
        5 => "7B",   // F major
        6 => "2B",   // Gb major
        7 => "9B",   // G major
        8 => "4B",   // Ab major
        9 => "11B",  // A major
        10 => "6B",  // Bb major
        11 => "1B",  // B major
        12 => "5A",  // C minor
        13 => "12A", // Db minor
        14 => "7A",  // D minor
        15 => "2A",  // Eb minor
        16 => "9A",  // E minor
        17 => "4A",  // F minor
        18 => "11A", // Gb minor
        19 => "6A",  // G minor
        20 => "1A",  // Ab minor
        21 => "8A",  // A minor
        22 => "3A",  // Bb minor
        23 => "10A", // B minor
        _ => "",
    }
}

/// Convert Camelot notation back into Traktor's `<MUSICAL_KEY VALUE="0..23">` index.
pub fn camelot_to_traktor_key(camelot: &str) -> Option<u32> {
    match camelot.trim().to_ascii_uppercase().as_str() {
        "8B" => Some(0),
        "3B" => Some(1),
        "10B" => Some(2),
        "5B" => Some(3),
        "12B" => Some(4),
        "7B" => Some(5),
        "2B" => Some(6),
        "9B" => Some(7),
        "4B" => Some(8),
        "11B" => Some(9),
        "6B" => Some(10),
        "1B" => Some(11),
        "5A" => Some(12),
        "12A" => Some(13),
        "7A" => Some(14),
        "2A" => Some(15),
        "9A" => Some(16),
        "4A" => Some(17),
        "11A" => Some(18),
        "6A" => Some(19),
        "1A" => Some(20),
        "8A" => Some(21),
        "3A" => Some(22),
        "10A" => Some(23),
        _ => None,
    }
}

/// Decode Traktor `VOLUME`, `DIR` (`/:Music/:Club/:`), and `FILE` into a local filesystem path.
pub fn decode_traktor_location(volume: &str, dir: &str, file: &str) -> String {
    let clean_dir = dir.replace("/:", "/");
    let vol = volume.trim();
    if vol.ends_with(':') && vol.len() == 2 {
        let mut p = format!("{vol}{clean_dir}{file}");
        p = p.replace("//", "/");
        p.replace('/', "\\")
    } else {
        let mut p = format!("{clean_dir}{file}");
        while p.contains("//") {
            p = p.replace("//", "/");
        }
        p
    }
}

/// Encode a local filesystem path into Traktor `(volume, dir, file, primary_key)`.
pub fn encode_traktor_location(path: &str) -> (String, String, String, String) {
    let norm = path.replace('\\', "/");
    let (vol, rest) = if norm.len() >= 2 && norm.as_bytes()[1] == b':' {
        (norm[..2].to_string(), norm[2..].to_string())
    } else {
        (String::new(), norm)
    };
    let (raw_dir, file) = match rest.rfind('/') {
        Some(idx) => (&rest[..=idx], &rest[idx + 1..]),
        None => ("/", rest.as_str()),
    };
    let mut traktor_dir = String::new();
    for part in raw_dir.split('/') {
        if part.is_empty() {
            continue;
        }
        traktor_dir.push_str("/:");
        traktor_dir.push_str(part);
    }
    traktor_dir.push_str("/:");
    let primary_key = format!("{vol}{traktor_dir}{file}");
    (vol, traktor_dir, file.to_string(), primary_key)
}

/// Parse a Traktor Pro `collection.nml` string into tracks, cues, and playlists.
pub fn parse_traktor_nml(xml: &str) -> TraktorLibrary {
    let mut lib = TraktorLibrary::default();

    // 1. Parse <COLLECTION> ... </COLLECTION>
    if let Some(c_start) = xml.find("<COLLECTION") {
        let c_end = xml[c_start..]
            .find("</COLLECTION>")
            .map(|o| c_start + o)
            .unwrap_or(xml.len());
        let coll = &xml[c_start..c_end];
        let mut pos = 0usize;
        while let Some(rel) = coll[pos..].find("<ENTRY ") {
            let entry_start = pos + rel;
            let Some(end_rel) = coll[entry_start..].find("</ENTRY>") else {
                break;
            };
            let entry_block = &coll[entry_start..entry_start + end_rel];
            let header_end = entry_block.find('>').unwrap_or(entry_block.len());
            let header = &entry_block[..header_end];

            let title = extract_attr(header, "TITLE").unwrap_or_default();
            let artist = extract_attr(header, "ARTIST").unwrap_or_default();

            let mut volume = String::new();
            let mut dir = String::new();
            let mut file = String::new();
            if let Some(loc_s) = entry_block.find("<LOCATION ") {
                if let Some(loc_e) = entry_block[loc_s..].find('>') {
                    let loc_tag = &entry_block[loc_s..loc_s + loc_e];
                    volume = extract_attr(loc_tag, "VOLUME").unwrap_or_default();
                    dir = extract_attr(loc_tag, "DIR").unwrap_or_default();
                    file = extract_attr(loc_tag, "FILE").unwrap_or_default();
                }
            }

            let mut genre = String::new();
            let mut duration_seconds = 0.0_f64;
            if let Some(info_s) = entry_block.find("<INFO ") {
                if let Some(info_e) = entry_block[info_s..].find('>') {
                    let info_tag = &entry_block[info_s..info_s + info_e];
                    genre = extract_attr(info_tag, "GENRE").unwrap_or_default();
                    if let Some(pt) = extract_attr(info_tag, "PLAYTIME") {
                        duration_seconds = pt.parse::<f64>().unwrap_or(0.0);
                    }
                }
            }

            let mut bpm = 0.0_f64;
            if let Some(t_s) = entry_block.find("<TEMPO ") {
                if let Some(t_e) = entry_block[t_s..].find('>') {
                    let t_tag = &entry_block[t_s..t_s + t_e];
                    if let Some(b) = extract_attr(t_tag, "BPM") {
                        bpm = b.parse::<f64>().unwrap_or(0.0);
                    }
                }
            }

            let mut musical_key = String::new();
            if let Some(k_s) = entry_block.find("<MUSICAL_KEY ") {
                if let Some(k_e) = entry_block[k_s..].find('>') {
                    let k_tag = &entry_block[k_s..k_s + k_e];
                    if let Some(v) = extract_attr(k_tag, "VALUE") {
                        if let Ok(idx) = v.parse::<u32>() {
                            musical_key = traktor_key_to_camelot(idx).to_string();
                        }
                    }
                }
            }

            let mut cues = Vec::new();
            let mut cpos = 0usize;
            while let Some(cs) = entry_block[cpos..].find("<CUE_V2 ") {
                let tag_s = cpos + cs;
                let Some(tag_e_rel) = entry_block[tag_s..].find('>') else {
                    break;
                };
                let ctag = &entry_block[tag_s..tag_s + tag_e_rel];
                if let Some(hc_str) = extract_attr(ctag, "HOTCUE") {
                    if let Ok(hc) = hc_str.parse::<i32>() {
                        if (0..4).contains(&hc) {
                            let start_ms = extract_attr(ctag, "START")
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            let name = extract_attr(ctag, "NAME").unwrap_or_default();
                            cues.push(TraktorCue {
                                slot: hc as usize,
                                start_sec: (start_ms / 1000.0).max(0.0),
                                name,
                            });
                        }
                    }
                }
                cpos = tag_s + tag_e_rel + 1;
            }

            if !file.is_empty() {
                let path = decode_traktor_location(&volume, &dir, &file);
                let primary_key = format!("{volume}{dir}{file}");
                let fallback_title = if title.is_empty() {
                    file.clone()
                } else {
                    title
                };
                lib.tracks.push(TraktorTrack {
                    primary_key,
                    path,
                    title: fallback_title,
                    artist,
                    genre,
                    bpm,
                    musical_key,
                    duration_seconds,
                    cues,
                });
            }

            pos = entry_start + end_rel + "</ENTRY>".len();
        }
    }

    // 2. Parse <PLAYLISTS> ... </PLAYLISTS>
    if let Some(p_start) = xml.find("<PLAYLISTS") {
        let p_end = xml[p_start..]
            .find("</PLAYLISTS>")
            .map(|o| p_start + o)
            .unwrap_or(xml.len());
        let pls = &xml[p_start..p_end];
        let mut pos = 0usize;
        while let Some(rel) = pls[pos..].find("<NODE ") {
            let ns = pos + rel;
            let Some(ne_rel) = pls[ns..].find('>') else {
                break;
            };
            let ntag = &pls[ns..ns + ne_rel];
            let node_type = extract_attr(ntag, "TYPE").unwrap_or_default();
            let node_name = extract_attr(ntag, "NAME").unwrap_or_default();
            if node_type.eq_ignore_ascii_case("PLAYLIST") && !node_name.is_empty() {
                let block_end = pls[ns..]
                    .find("</PLAYLIST>")
                    .map(|o| ns + o)
                    .unwrap_or(pls.len());
                let block = &pls[ns..block_end];
                let mut keys = Vec::new();
                let mut kpos = 0usize;
                while let Some(kr) = block[kpos..].find("<PRIMARYKEY ") {
                    let ks = kpos + kr;
                    let Some(ke) = block[ks..].find('>') else {
                        break;
                    };
                    if let Some(k) = extract_attr(&block[ks..ks + ke], "KEY") {
                        keys.push(k);
                    }
                    kpos = ks + ke + 1;
                }
                if !keys.is_empty() {
                    lib.playlists.push(TraktorPlaylist {
                        name: node_name,
                        primary_keys: keys,
                    });
                }
                pos = block_end + 1;
            } else {
                pos = ns + ne_rel + 1;
            }
        }
    }

    lib
}

/// Map from track path to `(slot, position_sec, optional_meta)` cue tuples.
pub type TraktorCuesMap = HashMap<String, Vec<(usize, f64, Option<TrackCueMeta>)>>;

/// Encode DJCraft tracks and hot cues into a Traktor Pro 4 `collection.nml` document.
pub fn encode_traktor_nml(
    tracks: &[CrateTrack],
    cues_by_path: &TraktorCuesMap,
    playlist_name: &str,
) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n\
         <NML VERSION=\"19\">\n\
         <HEAD COMPANY=\"DJCraft\" PROGRAM=\"DJCraft Pro\"/>\n",
    );
    out.push_str(&format!("<COLLECTION ENTRIES=\"{}\">\n", tracks.len()));

    let mut primary_keys = Vec::with_capacity(tracks.len());
    for t in tracks {
        let (vol, dir, file, pk) = encode_traktor_location(&t.path);
        primary_keys.push(pk);
        out.push_str(&format!(
            "  <ENTRY TITLE=\"{}\" ARTIST=\"{}\">\n",
            xml_escape(&t.title),
            xml_escape(&t.artist)
        ));
        out.push_str(&format!(
            "    <LOCATION DIR=\"{}\" FILE=\"{}\" VOLUME=\"{}\"/>\n",
            xml_escape(&dir),
            xml_escape(&file),
            xml_escape(&vol)
        ));
        out.push_str(&format!(
            "    <INFO GENRE=\"{}\" PLAYTIME=\"{}\"/>\n",
            xml_escape(&t.genre),
            t.duration_seconds.round().max(0.0) as u64
        ));
        if t.bpm > 0.0 {
            out.push_str(&format!("    <TEMPO BPM=\"{:.6}\"/>\n", t.bpm));
        }
        if let Some(k_idx) = camelot_to_traktor_key(&t.musical_key) {
            out.push_str(&format!("    <MUSICAL_KEY VALUE=\"{k_idx}\"/>\n"));
        }
        if let Some(cues) = cues_by_path.get(&t.path) {
            for (slot, sec, meta) in cues {
                let name = meta
                    .as_ref()
                    .map(|m| m.label.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Hot Cue");
                out.push_str(&format!(
                    "    <CUE_V2 NAME=\"{}\" DISPL_ORDER=\"0\" TYPE=\"0\" START=\"{:.3}\" LEN=\"0.000\" REPEATS=\"-1\" HOTCUE=\"{slot}\"/>\n",
                    xml_escape(name),
                    sec * 1000.0
                ));
            }
        }
        out.push_str("  </ENTRY>\n");
    }
    out.push_str("</COLLECTION>\n<PLAYLISTS>\n  <NODE TYPE=\"FOLDER\" NAME=\"$ROOT\">\n    <SUBNODES COUNT=\"1\">\n");
    out.push_str(&format!(
        "      <NODE TYPE=\"PLAYLIST\" NAME=\"{}\">\n        <PLAYLIST ENTRIES=\"{}\" TYPE=\"LIST\">\n",
        xml_escape(playlist_name),
        primary_keys.len()
    ));
    for pk in primary_keys {
        out.push_str(&format!(
            "          <ENTRY><PRIMARYKEY TYPE=\"TRACK\" KEY=\"{}\"/></ENTRY>\n",
            xml_escape(&pk)
        ));
    }
    out.push_str(
        "        </PLAYLIST>\n      </NODE>\n    </SUBNODES>\n  </NODE>\n</PLAYLISTS>\n</NML>\n",
    );
    out
}

/// Import a Traktor Pro `collection.nml` file into `CrateStore` (crates, tracks, and hot cues).
pub fn import_traktor_nml_file(
    store: &CrateStore,
    nml_path: &Path,
) -> Result<ImportSummary, DjError> {
    let xml = std::fs::read_to_string(nml_path)
        .map_err(|e| DjError::Deck(format!("Failed to read Traktor NML: {e}")))?;
    let lib = parse_traktor_nml(&xml);

    let default_crate_name = nml_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Traktor Import");
    let default_crate_id = store.ensure_crate_path(default_crate_name)?;

    let mut key_to_tid: HashMap<String, i64> = HashMap::new();
    let mut tracks_imported = 0usize;

    for t in &lib.tracks {
        let tid = store.upsert_track(
            &t.path,
            &t.title,
            &t.artist,
            &t.genre,
            t.bpm,
            &t.musical_key,
            t.duration_seconds,
        )?;
        key_to_tid.insert(t.primary_key.clone(), tid);
        let _ = store.add_track_to_crate(default_crate_id, tid);
        tracks_imported += 1;

        for c in &t.cues {
            let _ = store.set_track_hot_cue(&t.path, c.slot, Some(c.start_sec));
            if !c.name.is_empty() {
                let _ = store.set_track_hot_cue_meta(
                    &t.path,
                    c.slot,
                    &TrackCueMeta {
                        label: c.name.clone(),
                        color: None,
                        behavior: "standard".to_string(),
                    },
                );
            }
        }
    }

    for pl in &lib.playlists {
        let full = format!("{default_crate_name}/{}", pl.name);
        if let Ok(cid) = store.ensure_crate_path(&full) {
            for pk in &pl.primary_keys {
                if let Some(&tid) = key_to_tid.get(pk) {
                    let _ = store.add_track_to_crate(cid, tid);
                }
            }
        }
    }

    Ok(ImportSummary {
        format: "traktor_nml".to_string(),
        crate_id: default_crate_id,
        crate_path: default_crate_name.to_string(),
        imported_tracks: tracks_imported,
    })
}

/// Export a crate (or all library tracks if `crate_id` is `None`) to Traktor Pro `collection.nml`.
pub fn export_traktor_nml_file(
    store: &CrateStore,
    crate_id: Option<i64>,
    playlist_name: &str,
    out_path: &Path,
) -> Result<usize, DjError> {
    let tracks = store.query_tracks(&CrateQuery {
        crate_id,
        ..CrateQuery::default()
    })?;
    let mut cues_by_path: HashMap<String, Vec<(usize, f64, Option<TrackCueMeta>)>> = HashMap::new();
    for t in &tracks {
        let cues = store.get_track_hot_cues(&t.path).unwrap_or([None; 4]);
        let metas = store.get_track_hot_cue_metas(&t.path).unwrap_or_default();
        let mut list = Vec::new();
        for (slot, sec) in cues.iter().enumerate() {
            if let Some(pos_sec) = sec {
                list.push((slot, *pos_sec, Some(metas[slot].clone())));
            }
        }
        if !list.is_empty() {
            cues_by_path.insert(t.path.clone(), list);
        }
    }
    let xml = encode_traktor_nml(&tracks, &cues_by_path, playlist_name);
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(out_path, xml)
        .map_err(|e| DjError::Deck(format!("Failed to write Traktor NML: {e}")))?;
    Ok(tracks.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traktor_nml_roundtrip_locations_camelot_cues_and_playlists() {
        let track = CrateTrack {
            id: 1,
            path: "D:\\Music\\Techno\\Peak Hour.flac".to_string(),
            title: "Peak Hour".to_string(),
            artist: "Vienna Club".to_string(),
            genre: "Techno".to_string(),
            bpm: 132.0,
            musical_key: "8A".to_string(),
            duration_seconds: 342.0,
            beatgrid_offset_sec: 0.0,
            tuning_cents: None,
            drum_hits: None,
            rating: 5,
            analyzed_at: None,
            crate_position: 0,
        };
        let mut cues = HashMap::new();
        cues.insert(
            track.path.clone(),
            vec![(
                0usize,
                32.5_f64,
                Some(TrackCueMeta {
                    label: "Main Drop".to_string(),
                    color: None,
                    behavior: "standard".to_string(),
                }),
            )],
        );
        let nml = encode_traktor_nml(std::slice::from_ref(&track), &cues, "Club Peak");
        let parsed = parse_traktor_nml(&nml);
        assert_eq!(parsed.tracks.len(), 1);
        let t = &parsed.tracks[0];
        assert_eq!(t.path, track.path);
        assert_eq!(t.title, "Peak Hour");
        assert_eq!(t.artist, "Vienna Club");
        assert_eq!(t.musical_key, "8A");
        assert!((t.bpm - 132.0).abs() < 0.01);
        assert_eq!(t.cues.len(), 1);
        assert_eq!(t.cues[0].slot, 0);
        assert!((t.cues[0].start_sec - 32.5).abs() < 0.01);
        assert_eq!(t.cues[0].name, "Main Drop");
        assert_eq!(parsed.playlists.len(), 1);
        assert_eq!(parsed.playlists[0].name, "Club Peak");
        assert_eq!(parsed.playlists[0].primary_keys.len(), 1);
    }
}
