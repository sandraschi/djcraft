#![forbid(unsafe_code)]
//! Shared JSON configuration (`%LOCALAPPDATA%\djcraft\ui.json`) for both `djcraft` GUI and `djcraft-cli`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_skin")]
    pub skin: String,
    #[serde(default = "default_skin")]
    pub last_dark_skin: String,
    #[serde(default = "default_backdrop")]
    pub backdrop: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default)]
    pub four_deck_layout: bool,
    #[serde(default = "default_zoom")]
    pub zoom_factor: f32,
    #[serde(default = "default_true")]
    pub show_deck_a: bool,
    #[serde(default = "default_true")]
    pub show_deck_b: bool,
    #[serde(default = "default_true")]
    pub show_decks_cd: bool,
    #[serde(default = "default_true")]
    pub show_parallel_waveforms: bool,
    #[serde(default)]
    pub show_live_video_strip: bool,
    #[serde(default)]
    pub show_live_sampler_strip: bool,
    #[serde(default)]
    pub vertical_waveforms: bool,
    #[serde(default = "default_booth_strip")]
    pub booth_strip: String,
    #[serde(default = "default_true")]
    pub auto_gain_enabled: bool,
    #[serde(default = "default_pitch_range")]
    pub pitch_range_pct: f64,
    #[serde(default)]
    pub quantize_default: bool,
    #[serde(default = "default_auto_dj_fade")]
    pub auto_dj_fade_sec: f64,

    // Extended CLI-configurable fields
    #[serde(default = "default_workspace")]
    pub workspace: String,
    #[serde(default)]
    pub headphone_device: Option<String>,
    #[serde(default)]
    pub split_cue: bool,
    #[serde(default = "default_rec_format")]
    pub recording_format: String,
    #[serde(default)]
    pub dvs_device: Option<String>,
    #[serde(default = "default_dvs_carrier")]
    pub dvs_carrier: String,
    #[serde(default = "default_ai_endpoint")]
    pub ai_endpoint: String,
    #[serde(default = "default_ai_model")]
    pub ai_model: String,
    #[serde(default)]
    pub bridges_enabled: bool,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_skin() -> String {
    "BattleConsole".to_string()
}
fn default_backdrop() -> String {
    "SparkleClub".to_string()
}
fn default_locale() -> String {
    "En".to_string()
}
fn default_zoom() -> f32 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_booth_strip() -> String {
    "None".to_string()
}
fn default_pitch_range() -> f64 {
    16.0
}
fn default_auto_dj_fade() -> f64 {
    8.0
}
fn default_workspace() -> String {
    "live".to_string()
}
fn default_rec_format() -> String {
    "wav".to_string()
}
fn default_dvs_carrier() -> String {
    "1k".to_string()
}
fn default_ai_endpoint() -> String {
    "http://127.0.0.1:11434/v1/chat/completions".to_string()
}
fn default_ai_model() -> String {
    "qwen2.5:7b".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            skin: default_skin(),
            last_dark_skin: default_skin(),
            backdrop: default_backdrop(),
            locale: default_locale(),
            four_deck_layout: false,
            zoom_factor: 1.0,
            show_deck_a: true,
            show_deck_b: true,
            show_decks_cd: true,
            show_parallel_waveforms: true,
            show_live_video_strip: false,
            show_live_sampler_strip: false,
            vertical_waveforms: false,
            booth_strip: default_booth_strip(),
            auto_gain_enabled: true,
            pitch_range_pct: 16.0,
            quantize_default: false,
            auto_dj_fade_sec: 8.0,
            workspace: default_workspace(),
            headphone_device: None,
            split_cue: false,
            recording_format: default_rec_format(),
            dvs_device: None,
            dvs_carrier: default_dvs_carrier(),
            ai_endpoint: default_ai_endpoint(),
            ai_model: default_ai_model(),
            bridges_enabled: false,
            extra: BTreeMap::new(),
        }
    }
}

impl AppConfig {
    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
            .map(PathBuf::from)?;
        Some(base.join("djcraft").join("ui.json"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<PathBuf, String> {
        let path = Self::path().ok_or_else(|| "Cannot resolve config directory".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialize config failed: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("Write config failed: {e}"))?;
        Ok(path)
    }

    #[allow(dead_code)]
    pub fn set_key(&mut self, key: &str, value: &str) -> Result<String, String> {
        let parse_bool = |v: &str| -> Result<bool, String> {
            match v.trim().to_lowercase().as_str() {
                "true" | "1" | "on" | "yes" => Ok(true),
                "false" | "0" | "off" | "no" => Ok(false),
                _ => Err(format!(
                    "Invalid boolean '{v}' (expected true/false/on/off)"
                )),
            }
        };

        match key.trim().to_lowercase().as_str() {
            "skin" => {
                self.skin = match value.trim().to_lowercase().as_str() {
                    "battle" | "battleconsole" => "BattleConsole",
                    "club" | "clubdark" => "ClubDark",
                    "midnight" | "midnightbooth" => "MidnightBooth",
                    "carbon" | "carbondeck" => "CarbonDeck",
                    "pink" | "shockingpink" => "ShockingPink",
                    "daylight" | "daylightbooth" | "light" => "DaylightBooth",
                    _ => {
                        return Err(format!(
                            "Unknown skin '{value}' (expected: battle, club, midnight, carbon, pink, daylight)"
                        ))
                    }
                }
                .to_string();
                if self.skin != "DaylightBooth" {
                    self.last_dark_skin = self.skin.clone();
                }
            }
            "backdrop" => {
                self.backdrop = match value.trim().to_lowercase().as_str() {
                    "solid" => "Solid",
                    "metal" | "brushedmetal" => "BrushedMetal",
                    "weave" | "carbonweave" => "CarbonWeave",
                    "sparkle" | "sparkleclub" => "SparkleClub",
                    _ => {
                        return Err(format!(
                            "Unknown backdrop '{value}' (expected: solid, metal, weave, sparkle)"
                        ))
                    }
                }
                .to_string();
            }
            "locale" => {
                self.locale = match value.trim().to_lowercase().as_str() {
                    "en" => "En",
                    "de" => "De",
                    "fr" => "Fr",
                    "es" => "Es",
                    "ja" => "Ja",
                    "ko" => "Ko",
                    "zh" => "Zh",
                    _ => {
                        return Err(format!(
                            "Unknown locale '{value}' (expected: en, de, fr, es, ja, ko, zh)"
                        ))
                    }
                }
                .to_string();
            }
            "workspace" => {
                self.workspace = match value.trim().to_lowercase().as_str() {
                    "live" | "liveconsole" => "live",
                    "booth-crate" | "booth" | "boothandcrate" => "booth-crate",
                    "crates" | "cratesgigprep" => "crates",
                    _ => {
                        return Err(format!(
                            "Unknown workspace '{value}' (expected: live, booth-crate, crates)"
                        ))
                    }
                }
                .to_string();
            }
            "four-deck" | "four_deck_layout" => self.four_deck_layout = parse_bool(value)?,
            "zoom" | "zoom_factor" => {
                let z: f32 = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("Invalid zoom '{value}'"))?;
                self.zoom_factor = z.clamp(0.75, 2.0);
            }
            "pitch-range" | "pitch_range_pct" => {
                let p: f64 = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("Invalid pitch range '{value}' (expected 8, 16, 50)"))?;
                if ![8.0, 16.0, 50.0].contains(&p) {
                    return Err("pitch-range must be 8, 16, or 50".to_string());
                }
                self.pitch_range_pct = p;
            }
            "quantize" | "quantize_default" => self.quantize_default = parse_bool(value)?,
            "auto-gain" | "auto_gain_enabled" => self.auto_gain_enabled = parse_bool(value)?,
            "auto-dj-fade" | "auto_dj_fade_sec" => {
                let s: f64 = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("Invalid fade seconds '{value}'"))?;
                self.auto_dj_fade_sec = s.clamp(2.0, 20.0);
            }
            "headphone-device" | "headphone_device" => {
                let v = value.trim();
                self.headphone_device = if v.is_empty() || v.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(v.to_string())
                };
            }
            "split-cue" | "split_cue" => self.split_cue = parse_bool(value)?,
            "recording-format" | "recording_format" => {
                self.recording_format = match value.trim().to_lowercase().as_str() {
                    "wav" => "wav",
                    "flac" => "flac",
                    _ => return Err("recording-format must be 'wav' or 'flac'".to_string()),
                }
                .to_string();
            }
            "dvs-device" | "dvs_device" => {
                let v = value.trim();
                self.dvs_device = if v.is_empty() || v.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(v.to_string())
                };
            }
            "dvs-carrier" | "dvs_carrier" => {
                self.dvs_carrier = match value.trim().to_lowercase().as_str() {
                    "1k" | "1000" => "1k",
                    "2k" | "2000" => "2k",
                    _ => return Err("dvs-carrier must be '1k' or '2k'".to_string()),
                }
                .to_string();
            }
            "ai-endpoint" | "ai_endpoint" => self.ai_endpoint = value.trim().to_string(),
            "ai-model" | "ai_model" => self.ai_model = value.trim().to_string(),
            "bridges-enabled" | "bridges_enabled" => self.bridges_enabled = parse_bool(value)?,
            other => return Err(format!("Unknown config key '{other}'")),
        }
        Ok(format!("{key} = {value}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_set_key_validation() {
        let mut cfg = AppConfig::default();
        assert!(cfg.set_key("skin", "pink").is_ok());
        assert_eq!(cfg.skin, "ShockingPink");
        assert!(cfg.set_key("backdrop", "weave").is_ok());
        assert_eq!(cfg.backdrop, "CarbonWeave");
        assert!(cfg.set_key("pitch-range", "50").is_ok());
        assert_eq!(cfg.pitch_range_pct, 50.0);
        assert!(cfg.set_key("pitch-range", "99").is_err());
        assert!(cfg
            .set_key("headphone-device", "Speakers (Reloop Ready)")
            .is_ok());
        assert_eq!(
            cfg.headphone_device.as_deref(),
            Some("Speakers (Reloop Ready)")
        );
        assert!(cfg.set_key("headphone-device", "none").is_ok());
        assert!(cfg.headphone_device.is_none());
    }
}
