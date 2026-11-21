#![forbid(unsafe_code)]
//! AI Booth Chat, Natural-Language Workflows, and Fleet Service Hub
//! (Ollama/LM Studio, SpeechCraft, sfx-mcp, vfx-mcp, butterchurn-mcp).

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use djcraft_audio::{AudioEngine, DeckId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{Command, CrateQuery, CrateStore, CrateTrack, DjError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatConfig {
    pub endpoint: String,
    pub model: String,
}

impl Default for AiChatConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:11434/v1/chat/completions".to_string(),
            model: "qwen2.5:7b".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetEndpoints {
    #[serde(default)]
    pub enabled: bool,
    pub speechcraft_port: u16,
    pub sfx_port: u16,
    pub vfx_port: u16,
    pub butterchurn_port: u16,
    pub ollama_port: u16,
}

impl Default for FleetEndpoints {
    fn default() -> Self {
        Self {
            enabled: false,
            speechcraft_port: 14132,
            sfx_port: 11120,
            vfx_port: 11122,
            butterchurn_port: 10878,
            ollama_port: 11434,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetHealthReport {
    pub speechcraft_up: bool,
    pub sfx_up: bool,
    pub vfx_up: bool,
    pub butterchurn_up: bool,
    pub ollama_up: bool,
}

fn tcp_alive(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

pub fn probe_fleet_health(ep: &FleetEndpoints) -> FleetHealthReport {
    if !ep.enabled {
        return FleetHealthReport::default();
    }
    FleetHealthReport {
        speechcraft_up: tcp_alive(ep.speechcraft_port),
        sfx_up: tcp_alive(ep.sfx_port),
        vfx_up: tcp_alive(ep.vfx_port),
        butterchurn_up: tcp_alive(ep.butterchurn_port),
        ollama_up: tcp_alive(ep.ollama_port),
    }
}

/// Instant local natural-language parser for common DJ booth voice/chat commands.
pub fn parse_natural_language_command(input: &str) -> Option<Command> {
    let s = input.trim().to_lowercase();
    if s.contains("phase sync") {
        return Some(Command::PhaseSyncBeatgrid);
    }
    if s == "sync" || s.contains("auto sync") || s.contains("sync deck") {
        return Some(Command::AutoSyncBeatgrid);
    }
    if s.contains("stop all") || s == "stop" {
        return Some(Command::StopAllDecks);
    }
    if s == "mute" || s == "unmute" || s.contains("toggle mute") {
        return Some(Command::ToggleMasterMute);
    }
    if s.contains("reset stem") {
        return Some(Command::ResetAllStems);
    }
    if s.contains("center crossfader") || s.contains("crossfader center") {
        return Some(Command::SetCrossfader { value: 0.0 });
    }
    if s.contains("kill bass a") || s.contains("kill low a") {
        return Some(Command::SetDeckEq {
            deck: DeckId::DeckA,
            low: 0.0,
            mid: 1.0,
            high: 1.0,
        });
    }
    if s.contains("kill bass b") || s.contains("kill low b") {
        return Some(Command::SetDeckEq {
            deck: DeckId::DeckB,
            low: 0.0,
            mid: 1.0,
            high: 1.0,
        });
    }
    if s.contains("play a") || s.contains("pause a") {
        return Some(Command::PlayPause {
            deck: DeckId::DeckA,
        });
    }
    if s.contains("play b") || s.contains("pause b") {
        return Some(Command::PlayPause {
            deck: DeckId::DeckB,
        });
    }
    None
}

/// Build a Camelot-harmonic setlist starting from `seed_key` across the user's SQLite crate store.
pub fn plan_harmonic_setlist(
    store: &CrateStore,
    seed_key: &str,
    max_tracks: usize,
) -> Result<Vec<CrateTrack>, DjError> {
    let all = store.query_tracks(&CrateQuery::default())?;
    let mut chosen: Vec<CrateTrack> = Vec::new();
    let mut current_key = seed_key.trim().to_uppercase();

    for _ in 0..max_tracks {
        // Prefer staying on a key that still has unpicked harmonic neighbors (avoids greedy dead ends)
        let mut candidates: Vec<&CrateTrack> = all
            .iter()
            .filter(|t| {
                !chosen.iter().any(|c| c.id == t.id)
                    && AudioEngine::is_camelot_compatible(&current_key, &t.musical_key)
            })
            .collect();
        if candidates.is_empty() {
            break;
        }
        // Warnsdorff-style heuristic: avoid dead ends (deg == 0) while preferring
        // low-degree leaves first so central harmonic hubs stay available as bridges.
        candidates.sort_by_key(|cand| {
            let deg = all
                .iter()
                .filter(|other| {
                    other.id != cand.id
                        && !chosen.iter().any(|c| c.id == other.id)
                        && AudioEngine::is_camelot_compatible(&cand.musical_key, &other.musical_key)
                })
                .count();
            (deg == 0, deg)
        });
        let track = candidates[0];
        if !track.musical_key.trim().is_empty() {
            current_key = track.musical_key.trim().to_uppercase();
        }
        chosen.push(track.clone());
    }
    Ok(chosen)
}

/// Send a booth-context-aware chat turn to a local OpenAI-compatible LLM (`/v1/chat/completions`).
pub fn chat_with_booth_ai(
    cfg: &AiChatConfig,
    user_prompt: &str,
    booth_context: &str,
) -> Result<String, DjError> {
    let payload = json!({
        "model": cfg.model,
        "messages": [
            {
                "role": "system",
                "content": format!(
                    "You are DJCraft's concise technical DJ booth assistant. Current booth state: {booth_context}"
                )
            },
            {
                "role": "user",
                "content": user_prompt
            }
        ],
        "temperature": 0.4
    });

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .build();
    let resp = agent
        .post(&cfg.endpoint)
        .set("Content-Type", "application/json")
        .send_json(payload)
        .map_err(|e| DjError::Sidecar(format!("LLM request failed ({}): {e}", cfg.endpoint)))?;

    let val: Value = resp
        .into_json()
        .map_err(|e| DjError::Sidecar(format!("Invalid LLM JSON response: {e}")))?;

    let content = val
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .unwrap_or("(empty LLM reply)")
        .trim()
        .to_string();
    Ok(content)
}

/// Push live master BPM & optional preset hint to `butterchurn-mcp` (:10878).
pub fn butterchurn_sync_bpm(
    port: u16,
    bpm: f64,
    preset_hint: Option<&str>,
) -> Result<String, DjError> {
    let url = format!("http://127.0.0.1:{port}/api/bpm");
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(3))
        .build();
    let payload = json!({
        "bpm": bpm,
        "preset": preset_hint,
        "source": "djcraft"
    });
    match agent.post(&url).send_json(payload) {
        Ok(resp) => Ok(format!(
            "Butterchurn synced at {:.1} BPM (HTTP {})",
            bpm,
            resp.status()
        )),
        Err(e) => Err(DjError::Sidecar(format!(
            "butterchurn-mcp unreachable on :{port} ({e})"
        ))),
    }
}

/// Query `sfx-mcp` (:11120) for one-shot DJ risers/impacts/sweeps.
pub fn sfx_search(port: u16, query: &str) -> Result<String, DjError> {
    let url = format!(
        "http://127.0.0.1:{port}/api/search?q={}",
        query.replace(' ', "%20")
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(4))
        .build();
    match agent.get(&url).call() {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.into_string().unwrap_or_default();
            let preview: String = body.chars().take(180).collect();
            Ok(format!("sfx-mcp ({status}): {preview}"))
        }
        Err(e) => Err(DjError::Sidecar(format!(
            "sfx-mcp unreachable on :{port} ({e})"
        ))),
    }
}

/// Trigger a video effect job on `vfx-mcp` (:11122).
pub fn vfx_apply_preset(port: u16, video_path: &str, preset: &str) -> Result<String, DjError> {
    let url = format!("http://127.0.0.1:{port}/api/vfx");
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    let payload = json!({
        "input_path": video_path,
        "preset": preset,
        "source": "djcraft"
    });
    match agent.post(&url).send_json(payload) {
        Ok(resp) => Ok(format!(
            "vfx-mcp queued '{preset}' (HTTP {})",
            resp.status()
        )),
        Err(e) => Err(DjError::Sidecar(format!(
            "vfx-mcp unreachable on :{port} ({e})"
        ))),
    }
}

/// Synthesize a DJ vocal drop WAV using `speechcraft-cli` (or Windows System.Speech SAPI fallback).
pub fn generate_speech_drop(text: &str, out_wav: &Path) -> Result<PathBuf, DjError> {
    let clean = text.trim();
    if clean.is_empty() {
        return Err(DjError::Sidecar(
            "Speech drop text cannot be empty".to_string(),
        ));
    }
    if let Some(parent) = out_wav.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // 1. Try optional SPEECHCRAFT_BIN or speechcraft-cli on PATH if installed
    if let Some(bin) = std::env::var_os("SPEECHCRAFT_BIN")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| Some(PathBuf::from("speechcraft-cli")))
    {
        if let Ok(out) = std::process::Command::new(&bin)
            .args(["speak", "--text", clean, "--output"])
            .arg(out_wav)
            .output()
        {
            if out.status.success() && out_wav.is_file() {
                return Ok(out_wav.to_path_buf());
            }
        }
    }

    // 2. Fallback: native Windows System.Speech SAPI WAV synthesis (zero extra installs)
    let escaped_text = clean.replace('\'', "''");
    let escaped_out = out_wav.to_string_lossy().replace('\'', "''");
    let ps = format!(
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.SetOutputToWaveFile('{escaped_out}'); $s.Speak('{escaped_text}'); $s.Dispose()"
    );
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &ps])
        .output()
        .map_err(|e| DjError::Sidecar(format!("Speech synthesis launch failed: {e}")))?;
    if output.status.success() && out_wav.is_file() {
        Ok(out_wav.to_path_buf())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(DjError::Sidecar(format!(
            "Speech drop failed: {}",
            stderr.trim()
        )))
    }
}
