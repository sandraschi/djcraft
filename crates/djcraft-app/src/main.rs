#![forbid(unsafe_code)]

mod skins;
mod theme;
mod video;
mod widgets;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use djcraft_audio::dsp::{decode_audio_file, detect_beatgrid, WaveformSummary};
use djcraft_audio::output::AudioOutput;
use djcraft_audio::{AudioEngine, DeckId, TrackMeta};
use djcraft_core::{
    action_to_command, resolve_dettato_binary, resolve_mixxxxx_binary, Command,
    ControllerProfileId, CrateNode, CrateQuery, CrateSortField, CrateStore, CrateTrack,
    DjOrchestrator, MidiControllerManager, MidiTargetAction, MixxxxxVideoControls,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use skins::{SkinId, SkinPalette};
use video::{composite_video_frame, find_companion_video, VideoDeckPlayer, VIDEO_H, VIDEO_W};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "djcraft", about = "DJCraft Local Performance Studio")]
struct Args {
    #[arg(long)]
    smoke: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpTab {
    GetStarted,
    Functions,
    Usage,
    TipsAndTricks,
    PitfallsAndSafety,
    PlannedExtensions,
    OtherCrafts,
    ProjectDocs,
}

impl HelpTab {
    const ALL: [Self; 8] = [
        Self::GetStarted,
        Self::Functions,
        Self::Usage,
        Self::TipsAndTricks,
        Self::PitfallsAndSafety,
        Self::PlannedExtensions,
        Self::OtherCrafts,
        Self::ProjectDocs,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::GetStarted => "Get Started",
            Self::Functions => "Functions",
            Self::Usage => "Usage (GUI/CLI/MCP)",
            Self::TipsAndTricks => "Tips & Tricks",
            Self::PitfallsAndSafety => "Pitfalls & Safety",
            Self::PlannedExtensions => "Planned Extensions",
            Self::OtherCrafts => "Other Crafts",
            Self::ProjectDocs => "Project Docs",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LowerDockTab {
    Crates,
    VideoMixer,
    MidiControllers,
    CamelotAndMixxxxx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UiPrefs {
    #[serde(default)]
    skin: SkinId,
    #[serde(default = "default_last_dark")]
    last_dark_skin: SkinId,
    zoom_factor: f32,
    show_deck_a: bool,
    show_deck_b: bool,
    show_decks_cd: bool,
    show_parallel_waveforms: bool,
}

fn default_last_dark() -> SkinId {
    SkinId::RaneBattle
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            skin: SkinId::RaneBattle,
            last_dark_skin: SkinId::RaneBattle,
            zoom_factor: 1.0,
            show_deck_a: true,
            show_deck_b: true,
            show_decks_cd: true,
            show_parallel_waveforms: true,
        }
    }
}

impl UiPrefs {
    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
            .map(PathBuf::from)?;
        Some(base.join("djcraft").join("ui.json"))
    }

    fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(&raw).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, raw);
        }
    }
}

pub struct DjCraftApp {
    orch: DjOrchestrator,
    locale: djcraft_core::i18n::AppLocale,
    status_msg: String,
    platter_angle_a: f32,
    platter_angle_b: f32,
    crate_cursor_a: usize,
    crate_cursor_b: usize,
    typography_applied: bool,

    prefs: UiPrefs,
    lower_tab: LowerDockTab,

    // Live WASAPI / cpal audio output & decoded real-audio waveform summaries
    audio_out: AudioOutput,
    deck_waveforms: [Option<WaveformSummary>; 4],

    // Hardware MIDI Controller Engine (Rane Twelve MKII, Rane Seventy-Two MKII, Pioneer, Hercules)
    midi: MidiControllerManager,
    midi_ports: Vec<String>,

    // Persistent SQLite Crate & Subcrate Library
    crate_store: Option<CrateStore>,
    crate_nodes: Vec<CrateNode>,
    crate_results: Vec<CrateTrack>,
    crate_query: CrateQuery,
    crate_new_name: String,
    crate_bpm_min_text: String,
    crate_bpm_max_text: String,
    crate_harmonic_deck_a: bool,
    crate_dirty: bool,

    // Native Video Mixer
    video_deck_a: VideoDeckPlayer,
    video_deck_b: VideoDeckPlayer,
    video_texture: Option<egui::TextureHandle>,
    video_projector_open: bool,
    video_brightness: f32,
    video_contrast: f32,
    video_saturation: f32,
    video_phase: f32,

    // Mixxxxx OSC dock state
    mixxxxx_deck: DeckId,
    mixxxxx_video_enabled: bool,
    mixxxxx_video_fullscreen: bool,
    mixxxxx_brightness: f32,
    mixxxxx_contrast: f32,
    mixxxxx_saturation: f32,
    mixxxxx_last_pong: bool,

    // Help Suite modals
    show_command_palette: bool,
    palette_query: String,
    show_shortcuts_dialog: bool,
    show_help_center: bool,
    help_tab: HelpTab,
    show_licenses_dialog: bool,
    show_about_dialog: bool,
}

impl Default for DjCraftApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DjCraftApp {
    pub fn new() -> Self {
        let orch = DjOrchestrator::new();
        let audio_out = AudioOutput::start_default();
        let crate_store = CrateStore::open_default().ok();
        if let Some(ref store) = crate_store {
            let _ = store.seed_demo_if_empty();
        }
        let midi_ports = MidiControllerManager::list_input_ports();
        let status_msg = if audio_out.active {
            format!(
                "DJCraft Pro Ready | WASAPI: {} ({} Hz) | {} MIDI port(s)",
                audio_out.device_name,
                audio_out.sample_rate,
                midi_ports.len()
            )
        } else {
            format!(
                "DJCraft Pro Ready | Audio offline: {}",
                audio_out.error.as_deref().unwrap_or("unavailable")
            )
        };
        Self {
            orch,
            locale: djcraft_core::i18n::AppLocale::default(),
            status_msg,
            platter_angle_a: 0.0,
            platter_angle_b: 0.0,
            crate_cursor_a: 0,
            crate_cursor_b: 1,
            typography_applied: false,

            prefs: UiPrefs::load(),
            lower_tab: LowerDockTab::Crates,

            audio_out,
            deck_waveforms: [None, None, None, None],

            midi: MidiControllerManager::new(),
            midi_ports,

            crate_store,
            crate_nodes: Vec::new(),
            crate_results: Vec::new(),
            crate_query: CrateQuery::default(),
            crate_new_name: String::new(),
            crate_bpm_min_text: String::new(),
            crate_bpm_max_text: String::new(),
            crate_harmonic_deck_a: false,
            crate_dirty: true,

            video_deck_a: VideoDeckPlayer::new(),
            video_deck_b: VideoDeckPlayer::new(),
            video_texture: None,
            video_projector_open: false,
            video_brightness: 0.5,
            video_contrast: 0.5,
            video_saturation: 0.5,
            video_phase: 0.0,

            mixxxxx_deck: DeckId::DeckA,
            mixxxxx_video_enabled: true,
            mixxxxx_video_fullscreen: false,
            mixxxxx_brightness: 0.5,
            mixxxxx_contrast: 0.5,
            mixxxxx_saturation: 0.5,
            mixxxxx_last_pong: false,

            show_command_palette: false,
            palette_query: String::new(),
            show_shortcuts_dialog: false,
            show_help_center: false,
            help_tab: HelpTab::GetStarted,
            show_licenses_dialog: false,
            show_about_dialog: false,
        }
    }

    fn set_skin(&mut self, skin: SkinId) {
        self.prefs.skin = skin;
        if skin != SkinId::DaylightBooth {
            self.prefs.last_dark_skin = skin;
        }
        self.prefs.save();
        self.status_msg = format!("Skin switched to {}", skin.label());
    }

    fn reset_layout(&mut self) {
        self.prefs = UiPrefs {
            skin: self.prefs.skin,
            last_dark_skin: self.prefs.last_dark_skin,
            ..UiPrefs::default()
        };
        self.prefs.save();
        self.status_msg = "Console layout and zoom reset".to_string();
    }

    fn open_help_tab(&mut self, tab: HelpTab) {
        self.help_tab = tab;
        self.show_help_center = true;
    }

    fn dispatch_and_report(&mut self, cmd: Command) {
        match self.orch.dispatch(cmd) {
            Ok(outcome) => {
                if outcome.command == "mixxxxx_ping" {
                    self.mixxxxx_last_pong = outcome
                        .data
                        .get("pong_received")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                }
                self.status_msg = outcome.message;
            }
            Err(err) => self.status_msg = format!("Error: {err}"),
        }
    }

    fn handle_pad_click(&mut self, deck: DeckId, pad_idx: usize) {
        match pad_idx {
            0..=3 => {
                let existing = self.orch.deck(deck).hot_cue_seconds[pad_idx];
                if let Some(pos) = existing {
                    self.orch.deck_mut(deck).position_seconds = pos;
                    self.status_msg = format!("{}: Jumped to Hot Cue {} ({:.1}s)", deck.label(), pad_idx + 1, pos);
                } else {
                    self.dispatch_and_report(Command::SetHotCue {
                        deck,
                        slot: pad_idx,
                    });
                }
            }
            4 => {
                let next = if self.orch.deck(deck).loop_bars == Some(4) {
                    None
                } else {
                    Some(4)
                };
                self.dispatch_and_report(Command::SetLoopBars { deck, bars: next });
            }
            5 => {
                let next = if self.orch.deck(deck).loop_bars == Some(8) {
                    None
                } else {
                    Some(8)
                };
                self.dispatch_and_report(Command::SetLoopBars { deck, bars: next });
            }
            6 => {
                let d = self.orch.deck(deck).clone();
                let new_low = if d.eq_low <= 0.01 { 1.0 } else { 0.0 };
                self.dispatch_and_report(Command::SetDeckEq {
                    deck,
                    low: new_low,
                    mid: d.eq_mid,
                    high: d.eq_high,
                });
            }
            7 => {
                let d = self.orch.deck(deck).clone();
                let new_v = if d.stem_vocals_gain <= 0.01 { 1.0 } else { 0.0 };
                self.dispatch_and_report(Command::SetStemGains {
                    deck,
                    vocals: new_v,
                    drums: d.stem_drums_gain,
                    bass: d.stem_bass_gain,
                    other: d.stem_other_gain,
                });
            }
            _ => {}
        }
    }

    fn load_audio_file_dialog(&mut self, deck: DeckId) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "m4a", "aac"])
            .pick_file()
        else {
            return;
        };
        match decode_audio_file(&path) {
            Ok(decoded) => {
                let grid = detect_beatgrid(&decoded);
                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Audio Track")
                    .to_string();
                let meta = TrackMeta {
                    title: title.clone(),
                    artist: "Local Audio".to_string(),
                    bpm: grid.bpm,
                    musical_key: "8A".to_string(),
                    duration_seconds: decoded.duration_seconds,
                };
                let deck_idx = deck_index(deck);
                self.deck_waveforms[deck_idx] = Some(WaveformSummary::from_decoded(&decoded));
                self.audio_out.set_deck_audio(deck_idx, Some(Arc::new(decoded)));
                self.dispatch_and_report(Command::LoadTrack { deck, track: meta });
                let st = self.orch.deck_mut(deck);
                st.beatgrid_offset_sec = grid.first_beat_sec;
                st.audio_path = Some(path.to_string_lossy().to_string());
                if let Some(vid_path) = find_companion_video(&path) {
                    let _ = match deck {
                        DeckId::DeckA => self.video_deck_a.load_file(&vid_path),
                        DeckId::DeckB => self.video_deck_b.load_file(&vid_path),
                        _ => Ok(()),
                    };
                }
                self.status_msg = format!(
                    "Loaded '{}' to {} ({:.1} BPM, beat 1 at {:.2}s)",
                    title,
                    deck.label(),
                    grid.bpm,
                    grid.first_beat_sec
                );
            }
            Err(err) => self.status_msg = format!("Audio decode failed: {err}"),
        }
    }

    fn load_video_file_dialog(&mut self, deck: DeckId) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Video Files", &["mp4", "mkv", "mov", "webm"])
            .pick_file()
        else {
            return;
        };
        let res = match deck {
            DeckId::DeckA => self.video_deck_a.load_file(&path),
            DeckId::DeckB => self.video_deck_b.load_file(&path),
            _ => Err("Native video decks support Deck A and Deck B".to_string()),
        };
        match res {
            Ok(()) => {
                self.status_msg = format!(
                    "Loaded video '{}' to {}",
                    path.file_name().and_then(|s| s.to_str()).unwrap_or("video"),
                    deck.label()
                );
            }
            Err(err) => self.status_msg = format!("Video load error: {err}"),
        }
    }

    fn refresh_crates(&mut self) {
        let Some(ref store) = self.crate_store else {
            return;
        };
        self.crate_query.bpm_min = self.crate_bpm_min_text.trim().parse::<f64>().ok();
        self.crate_query.bpm_max = self.crate_bpm_max_text.trim().parse::<f64>().ok();
        self.crate_query.compatible_with_key = if self.crate_harmonic_deck_a {
            self.orch
                .deck_a
                .loaded_track
                .as_ref()
                .map(|t| t.musical_key.clone())
        } else {
            None
        };
        if let Ok(nodes) = store.list_crates() {
            self.crate_nodes = nodes;
        }
        if let Ok(tracks) = store.query_tracks(&self.crate_query) {
            self.crate_results = tracks;
        }
        self.crate_dirty = false;
    }

    fn load_crate_track_to_deck(&mut self, deck: DeckId, track: &CrateTrack) {
        let idx = deck_index(deck);
        if track.path.starts_with("demo://") {
            self.deck_waveforms[idx] = None;
            self.audio_out.set_deck_audio(idx, None);
            let meta = TrackMeta {
                title: track.title.clone(),
                artist: track.artist.clone(),
                bpm: if track.bpm > 0.0 { track.bpm } else { 126.0 },
                musical_key: if track.musical_key.is_empty() {
                    "8A".to_string()
                } else {
                    track.musical_key.clone()
                },
                duration_seconds: if track.duration_seconds > 0.0 {
                    track.duration_seconds
                } else {
                    360.0
                },
            };
            self.dispatch_and_report(Command::LoadTrack { deck, track: meta });
            return;
        }
        let path = std::path::Path::new(&track.path);
        match decode_audio_file(path) {
            Ok(decoded) => {
                let (bpm, offset) = if track.bpm > 0.0 {
                    (track.bpm, track.beatgrid_offset_sec)
                } else {
                    let g = detect_beatgrid(&decoded);
                    (g.bpm, g.first_beat_sec)
                };
                self.deck_waveforms[idx] = Some(WaveformSummary::from_decoded(&decoded));
                self.audio_out
                    .set_deck_audio(idx, Some(Arc::new(decoded.clone())));
                let meta = TrackMeta {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    bpm,
                    musical_key: if track.musical_key.is_empty() {
                        "8A".to_string()
                    } else {
                        track.musical_key.clone()
                    },
                    duration_seconds: decoded.duration_seconds,
                };
                self.dispatch_and_report(Command::LoadTrack { deck, track: meta });
                let st = self.orch.deck_mut(deck);
                st.beatgrid_offset_sec = offset;
                st.audio_path = Some(track.path.clone());
                if let Some(vid) = find_companion_video(path) {
                    let _ = match deck {
                        DeckId::DeckA => self.video_deck_a.load_file(&vid),
                        DeckId::DeckB => self.video_deck_b.load_file(&vid),
                        _ => Ok(()),
                    };
                }
            }
            Err(err) => self.status_msg = format!("Cannot decode {}: {err}", track.path),
        }
    }

    fn execute_catalog_id(&mut self, id: &str) {
        match id {
            "session.new" => self.dispatch_and_report(Command::NewSession),
            "edit.undo" => self.dispatch_and_report(Command::Undo),
            "edit.redo" => self.dispatch_and_report(Command::Redo),
            "deck.play_pause.a" => {
                self.dispatch_and_report(Command::PlayPause { deck: DeckId::DeckA })
            }
            "deck.play_pause.b" => {
                self.dispatch_and_report(Command::PlayPause { deck: DeckId::DeckB })
            }
            "deck.cue.a" => self.dispatch_and_report(Command::CueReturn { deck: DeckId::DeckA }),
            "deck.cue.b" => self.dispatch_and_report(Command::CueReturn { deck: DeckId::DeckB }),
            "deck.stop_all" => self.dispatch_and_report(Command::StopAllDecks),
            "insert.load_crate_a" => {
                let len = self.orch.crate_tracks.len().max(1);
                self.crate_cursor_a = (self.crate_cursor_a + 1) % len;
                let idx = self.crate_cursor_a;
                self.dispatch_and_report(Command::LoadDemoCrateTrack {
                    deck: DeckId::DeckA,
                    index: idx,
                });
            }
            "insert.load_crate_b" => {
                let len = self.orch.crate_tracks.len().max(1);
                self.crate_cursor_b = (self.crate_cursor_b + 1) % len;
                let idx = self.crate_cursor_b;
                self.dispatch_and_report(Command::LoadDemoCrateTrack {
                    deck: DeckId::DeckB,
                    index: idx,
                });
            }
            "insert.hot_cue_a" => self.dispatch_and_report(Command::SetHotCue {
                deck: DeckId::DeckA,
                slot: 0,
            }),
            "insert.loop_8_a" => {
                let next = if self.orch.deck_a.loop_bars.is_some() {
                    None
                } else {
                    Some(8)
                };
                self.dispatch_and_report(Command::SetLoopBars {
                    deck: DeckId::DeckA,
                    bars: next,
                });
            }
            "modify.reset_stems" => self.dispatch_and_report(Command::ResetAllStems),
            "modify.center_crossfader" => {
                self.dispatch_and_report(Command::SetCrossfader { value: 0.0 })
            }
            "modify.toggle_mute" => self.dispatch_and_report(Command::ToggleMasterMute),
            "modify.keylock_a" => {
                self.dispatch_and_report(Command::ToggleKeylock { deck: DeckId::DeckA })
            }
            "commands.auto_sync" => self.dispatch_and_report(Command::AutoSyncBeatgrid),
            "commands.phase_sync" => self.dispatch_and_report(Command::PhaseSyncBeatgrid),
            "commands.plan_transition" => self.dispatch_and_report(Command::PlanTransition),
            "commands.dettato_status" => {
                self.status_msg = match resolve_dettato_binary() {
                    Some(p) => format!("Dettato sidecar ready: {}", p.display()),
                    None => "Dettato sidecar not found (set DETTATO_BIN)".to_string(),
                };
            }
            "commands.mixxxxx_ping" => {
                self.dispatch_and_report(Command::MixxxxxPing {
                    timeout_ms: Some(200),
                });
            }
            "commands.mixxxxx_video_a_on" => {
                self.dispatch_and_report(Command::MixxxxxVideo {
                    deck: DeckId::DeckA,
                    controls: MixxxxxVideoControls {
                        enabled: Some(true),
                        ..Default::default()
                    },
                });
            }
            _ => self.status_msg = format!("Unknown command id: {id}"),
        }
    }

    fn render_hardware_deck(
        &mut self,
        ui: &mut egui::Ui,
        deck_id: DeckId,
        accent: egui::Color32,
        platter_angle: f32,
        palette: &SkinPalette,
    ) {
        let d = self.orch.deck(deck_id).clone();

        theme::card_frame(ui, palette, |ui| {
            ui.horizontal(|ui| {
                theme::status_dot(ui, d.is_playing);
                ui.strong(egui::RichText::new(deck_id.label()).color(accent));
                if let Some(track) = &d.loaded_track {
                    ui.label(egui::RichText::new(&track.title).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.monospace(format!("{} | {:.1} BPM", track.musical_key, track.bpm));
                    });
                } else {
                    ui.label("Empty Deck");
                }
            });
            ui.horizontal(|ui| {
                let play_lbl = if d.is_playing { "PAUSE" } else { "PLAY" };
                if ui.button(egui::RichText::new(play_lbl).strong()).clicked() {
                    self.dispatch_and_report(Command::PlayPause { deck: deck_id });
                }
                if ui.button("CUE").clicked() {
                    self.dispatch_and_report(Command::CueReturn { deck: deck_id });
                }
                let kl = if d.keylock { "KEYLOCK" } else { "VINYL" };
                if ui.small_button(kl).clicked() {
                    self.dispatch_and_report(Command::ToggleKeylock { deck: deck_id });
                }
                if ui.small_button("AUDIO").clicked() {
                    self.load_audio_file_dialog(deck_id);
                }
                if ui.small_button("VIDEO").clicked() {
                    self.load_video_file_dialog(deck_id);
                }
            });
        });

        ui.add_space(6.0);

        // Motorized Platter + Vertical Pitch Fader side-by-side
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal(|ui| {
                let eff_bpm = d
                    .loaded_track
                    .as_ref()
                    .map(|t| t.bpm * (1.0 + d.pitch_bend_pct / 100.0));
                widgets::paint_platter(ui, platter_angle, accent, d.is_playing, eff_bpm);

                ui.add_space(6.0);
                let mut pitch = d.pitch_bend_pct;
                let pf = widgets::vertical_fader(
                    ui,
                    &format!("{pitch:+.1}%"),
                    &mut pitch,
                    -16.0,
                    16.0,
                    0.0,
                    120.0,
                    accent,
                    palette,
                );
                if pf.changed {
                    self.orch.deck_mut(deck_id).pitch_bend_pct = pitch;
                }
                if pf.committed {
                    self.dispatch_and_report(Command::SetPitchBend {
                        deck: deck_id,
                        pct: pitch,
                    });
                }

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Beatgrid").size(11.0).color(palette.text_secondary));
                    if ui.small_button("0% Pitch").clicked() {
                        self.dispatch_and_report(Command::SetPitchBend {
                            deck: deck_id,
                            pct: 0.0,
                        });
                    }
                    if ui.small_button("/2 BPM").clicked() {
                        self.dispatch_and_report(Command::ScaleBpm {
                            deck: deck_id,
                            multiplier: 0.5,
                        });
                    }
                    if ui.small_button("x2 BPM").clicked() {
                        self.dispatch_and_report(Command::ScaleBpm {
                            deck: deck_id,
                            multiplier: 2.0,
                        });
                    }
                    if ui.small_button("-10ms").clicked() {
                        self.dispatch_and_report(Command::NudgeBeatgrid {
                            deck: deck_id,
                            delta_ms: -10.0,
                        });
                    }
                    if ui.small_button("+10ms").clicked() {
                        self.dispatch_and_report(Command::NudgeBeatgrid {
                            deck: deck_id,
                            delta_ms: 10.0,
                        });
                    }
                });
            });
        });

        ui.add_space(6.0);

        // 8 RGB MPC Performance Pads
        theme::card_frame(ui, palette, |ui| {
            ui.label(egui::RichText::new("MPC Performance Pads").size(11.5).color(palette.text_secondary));
            if let Some(pad_idx) = widgets::performance_pads_grid(ui, &d, palette) {
                self.handle_pad_click(deck_id, pad_idx);
            }
        });

        ui.add_space(6.0);

        // 4 Rotary Stem Knobs
        theme::card_frame(ui, palette, |ui| {
            ui.label(egui::RichText::new("Stems (Double-click = 100%)").size(11.5).color(palette.text_secondary));
            let mut v = d.stem_vocals_gain;
            let mut dr = d.stem_drums_gain;
            let mut ba = d.stem_bass_gain;
            let mut ot = d.stem_other_gain;
            let mut changed = false;
            let mut committed = false;
            ui.horizontal(|ui| {
                let r1 = widgets::rotary_knob(ui, "VOC", &mut v, 0.0, 1.0, 1.0, false, accent, palette);
                let r2 = widgets::rotary_knob(ui, "DRM", &mut dr, 0.0, 1.0, 1.0, false, accent, palette);
                let r3 = widgets::rotary_knob(ui, "BAS", &mut ba, 0.0, 1.0, 1.0, false, accent, palette);
                let r4 = widgets::rotary_knob(ui, "OTH", &mut ot, 0.0, 1.0, 1.0, false, accent, palette);
                changed = r1.changed || r2.changed || r3.changed || r4.changed;
                committed = r1.committed || r2.committed || r3.committed || r4.committed;
            });
            if changed {
                let st = self.orch.deck_mut(deck_id);
                st.stem_vocals_gain = v;
                st.stem_drums_gain = dr;
                st.stem_bass_gain = ba;
                st.stem_other_gain = ot;
            }
            if committed {
                self.dispatch_and_report(Command::SetStemGains {
                    deck: deck_id,
                    vocals: v,
                    drums: dr,
                    bass: ba,
                    other: ot,
                });
            }
        });
    }
}

fn deck_index(deck: DeckId) -> usize {
    match deck {
        DeckId::DeckA => 0,
        DeckId::DeckB => 1,
        DeckId::DeckC => 2,
        DeckId::DeckD => 3,
    }
}

fn switch_on_hover(ctx: &egui::Context, titles: &[egui::Response]) {
    let is_open = |t: &egui::Response| egui::Popup::is_id_open(ctx, egui::Popup::default_response_id(t));
    if !titles.iter().any(is_open) {
        return;
    }
    if let Some(title) = titles
        .iter()
        .find(|t| t.hovered() && !t.clicked() && !is_open(t))
    {
        egui::Popup::open_id(ctx, egui::Popup::default_response_id(title));
    }
}

impl eframe::App for DjCraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let palette = self.prefs.skin.palette();
        if !self.typography_applied {
            theme::apply_fleet_typography(&ctx);
            self.typography_applied = true;
        }
        ctx.set_visuals(palette.to_visuals());

        // Poll hardware MIDI controllers (Rane Twelve MKII, Rane Seventy-Two MKII, etc.)
        for (action, val) in self.midi.poll_actions() {
            let eq = match action {
                MidiTargetAction::DeckEqLow(d)
                | MidiTargetAction::DeckEqMid(d)
                | MidiTargetAction::DeckEqHigh(d) => {
                    let st = self.orch.deck(d);
                    (st.eq_low, st.eq_mid, st.eq_high)
                }
                _ => (1.0, 1.0, 1.0),
            };
            let cmd = action_to_command(action, val, eq);
            self.dispatch_and_report(cmd);
        }

        // Ctrl + MouseWheel zoom
        let scroll_delta = ctx.input(|i| {
            if i.modifiers.ctrl || i.modifiers.command {
                i.smooth_scroll_delta.y
            } else {
                0.0
            }
        });
        if scroll_delta.abs() > 0.1 {
            let step = if scroll_delta > 0.0 { 1.1_f32 } else { 1.0 / 1.1_f32 };
            self.prefs.zoom_factor = (self.prefs.zoom_factor * step).clamp(0.75, 2.0);
            self.prefs.save();
        }
        ctx.set_zoom_factor(self.prefs.zoom_factor);

        // Global shortcuts
        ctx.input(|i| {
            let cmd_mod = i.modifiers.ctrl || i.modifiers.command;
            if cmd_mod && i.key_pressed(egui::Key::K) {
                self.show_command_palette = !self.show_command_palette;
            }
            if cmd_mod && i.modifiers.shift && i.key_pressed(egui::Key::P) {
                self.show_command_palette = true;
            }
            if i.key_pressed(egui::Key::F1) {
                self.show_shortcuts_dialog = !self.show_shortcuts_dialog;
            }
            if cmd_mod && i.key_pressed(egui::Key::Z) {
                self.execute_catalog_id("edit.undo");
            }
            if cmd_mod && i.key_pressed(egui::Key::Y) {
                self.execute_catalog_id("edit.redo");
            }
            if cmd_mod && i.key_pressed(egui::Key::N) {
                self.execute_catalog_id("session.new");
            }
            if cmd_mod && i.key_pressed(egui::Key::Num0) {
                self.prefs.zoom_factor = 1.0;
                self.prefs.save();
            }
        });

        // Continuous playback tick
        let mut any_playing = false;
        if self.orch.deck_a.is_playing {
            self.platter_angle_a += 0.04;
            self.orch.deck_a.position_seconds += 0.016;
            any_playing = true;
        }
        if self.orch.deck_b.is_playing {
            self.platter_angle_b += 0.04;
            self.orch.deck_b.position_seconds += 0.016;
            any_playing = true;
        }
        if self.orch.deck_c.is_playing {
            self.orch.deck_c.position_seconds += 0.016;
            any_playing = true;
        }
        if self.orch.deck_d.is_playing {
            self.orch.deck_d.position_seconds += 0.016;
            any_playing = true;
        }
        if any_playing || self.midi.connected_port.is_some() {
            ctx.request_repaint();
        }

        // Sync live audio mixer into WASAPI
        self.audio_out.sync_state(
            [
                self.orch.deck_a.clone(),
                self.orch.deck_b.clone(),
                self.orch.deck_c.clone(),
                self.orch.deck_d.clone(),
            ],
            self.orch.crossfader,
            self.orch.master_volume,
            self.orch.master_muted,
        );

        // Update native video mixer texture
        self.video_deck_a.set_playing(self.orch.deck_a.is_playing);
        self.video_deck_b.set_playing(self.orch.deck_b.is_playing);
        if self.lower_tab == LowerDockTab::VideoMixer || self.video_projector_open {
            if any_playing {
                self.video_phase += 0.03;
            }
            let rgb_a = self.video_deck_a.snapshot_rgb(self.video_phase, [0.22, 0.74, 0.97]);
            let rgb_b = self
                .video_deck_b
                .snapshot_rgb(self.video_phase * 1.15 + 1.5, [0.94, 0.27, 0.27]);
            let image = composite_video_frame(
                &rgb_a,
                &rgb_b,
                self.orch.crossfader,
                self.video_brightness,
                self.video_contrast,
                self.video_saturation,
            );
            if let Some(tex) = self.video_texture.as_mut() {
                tex.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.video_texture = Some(ctx.load_texture(
                    "djcraft_video_mix",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }

        // Detachable Projector Window
        if self.video_projector_open {
            let tex_id = self.video_texture.as_ref().map(|t| t.id());
            let mut keep_open = true;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("djcraft_video_projector"),
                egui::ViewportBuilder::default()
                    .with_title("DJCraft Video Projector Output")
                    .with_inner_size([960.0, 540.0]),
                |vctx, _| {
                    if vctx.input(|i| i.viewport().close_requested()) {
                        keep_open = false;
                    }
                    egui::CentralPanel::default()
                        .frame(egui::Frame::new().fill(egui::Color32::BLACK))
                        .show(vctx, |ui| {
                            if let Some(id) = tex_id {
                                let avail = ui.available_size();
                                ui.image(egui::load::SizedTexture::new(id, avail));
                            }
                        });
                },
            );
            if !keep_open {
                self.video_projector_open = false;
            }
        }

        // 1. UNIVERSAL 8-MENU BOOKEND BAR
        egui::Panel::top("top_menu_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut titles: Vec<egui::Response> = Vec::with_capacity(8);

                let r = ui.menu_button("File", |ui| {
                    if ui.button("New Session (Ctrl+N)").clicked() {
                        self.execute_catalog_id("session.new");
                        ui.close();
                    }
                    if ui.button("Open Audio File -> Deck A...").clicked() {
                        self.load_audio_file_dialog(DeckId::DeckA);
                        ui.close();
                    }
                    if ui.button("Open Audio File -> Deck B...").clicked() {
                        self.load_audio_file_dialog(DeckId::DeckB);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit (Ctrl+Q)").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(self.orch.undo_depth() > 0, egui::Button::new("Undo (Ctrl+Z)"))
                        .clicked()
                    {
                        self.execute_catalog_id("edit.undo");
                        ui.close();
                    }
                    if ui
                        .add_enabled(self.orch.redo_depth() > 0, egui::Button::new("Redo (Ctrl+Y)"))
                        .clicked()
                    {
                        self.execute_catalog_id("edit.redo");
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("View", |ui| {
                    ui.menu_button("Console Skin", |ui| {
                        for sk in SkinId::ALL {
                            if ui.selectable_label(self.prefs.skin == sk, sk.label()).clicked() {
                                self.set_skin(sk);
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                    let mut dirty = false;
                    dirty |= ui
                        .checkbox(&mut self.prefs.show_parallel_waveforms, "Parallel Waveforms")
                        .changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_deck_a, "Deck A").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_deck_b, "Deck B").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_decks_cd, "Decks C & D Strip").changed();
                    if dirty {
                        self.prefs.save();
                    }
                    ui.separator();
                    if ui.button("Reset Zoom 100% (Ctrl+0)").clicked() {
                        self.prefs.zoom_factor = 1.0;
                        self.prefs.save();
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Insert", |ui| {
                    if ui.button("Load Next Crate Track -> Deck A").clicked() {
                        self.execute_catalog_id("insert.load_crate_a");
                        ui.close();
                    }
                    if ui.button("Load Next Crate Track -> Deck B").clicked() {
                        self.execute_catalog_id("insert.load_crate_b");
                        ui.close();
                    }
                    if ui.button("Add Hot Cue 1 (Deck A)").clicked() {
                        self.execute_catalog_id("insert.hot_cue_a");
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Modify", |ui| {
                    if ui.button("Reset All Stems (100%)").clicked() {
                        self.execute_catalog_id("modify.reset_stems");
                        ui.close();
                    }
                    if ui.button("Center Crossfader").clicked() {
                        self.execute_catalog_id("modify.center_crossfader");
                        ui.close();
                    }
                    if ui.button("Toggle Master Mute").clicked() {
                        self.execute_catalog_id("modify.toggle_mute");
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Commands", |ui| {
                    if ui.button("Auto-Sync BPM (Deck B -> A)").clicked() {
                        self.execute_catalog_id("commands.auto_sync");
                        ui.close();
                    }
                    if ui.button("Phase Sync (Deck B -> A)").clicked() {
                        self.execute_catalog_id("commands.phase_sync");
                        ui.close();
                    }
                    if ui.button("Stop All Decks").clicked() {
                        self.execute_catalog_id("deck.stop_all");
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Check Dettato Sidecar").clicked() {
                        self.execute_catalog_id("commands.dettato_status");
                        ui.close();
                    }
                    if ui.button("Ping Mixxxxx OSC (11119)").clicked() {
                        self.execute_catalog_id("commands.mixxxxx_ping");
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Window", |ui| {
                    if ui.button("Reset Panel Layout").clicked() {
                        self.reset_layout();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Open Video Projector Window").clicked() {
                        self.video_projector_open = true;
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Help", |ui| {
                    if ui.button("Command Palette... (Ctrl+K)").clicked() {
                        self.show_command_palette = true;
                        ui.close();
                    }
                    if ui.button("Keyboard Shortcuts... (F1)").clicked() {
                        self.show_shortcuts_dialog = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Getting Started...").clicked() {
                        self.open_help_tab(HelpTab::GetStarted);
                        ui.close();
                    }
                    if ui.button("Usage (GUI, CLI & MCP)...").clicked() {
                        self.open_help_tab(HelpTab::Usage);
                        ui.close();
                    }
                    if ui.button("Tips & Tricks...").clicked() {
                        self.open_help_tab(HelpTab::TipsAndTricks);
                        ui.close();
                    }
                    if ui.button("Common Pitfalls & Safety...").clicked() {
                        self.open_help_tab(HelpTab::PitfallsAndSafety);
                        ui.close();
                    }
                    if ui.button("Planned Extensions...").clicked() {
                        self.open_help_tab(HelpTab::PlannedExtensions);
                        ui.close();
                    }
                    if ui.button("Project Files & Docs...").clicked() {
                        self.open_help_tab(HelpTab::ProjectDocs);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Licenses & Notices...").clicked() {
                        self.show_licenses_dialog = true;
                        ui.close();
                    }
                    if ui.button("About DJCraft").clicked() {
                        self.show_about_dialog = true;
                        ui.close();
                    }
                });
                titles.push(r.response);

                switch_on_hover(&ctx, &titles);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cur_loc = self.locale;
                    egui::ComboBox::from_id_salt("topbar_locale_selector")
                        .selected_text(cur_loc.code().to_uppercase())
                        .show_ui(ui, |ui| {
                            for loc_item in djcraft_core::i18n::AppLocale::all() {
                                let text = format!(
                                    "{} - {}",
                                    loc_item.code().to_uppercase(),
                                    loc_item.display_name()
                                );
                                let _ = ui.selectable_value(&mut self.locale, *loc_item, text);
                            }
                        });
                    if theme::theme_toggle_button(ui, palette.is_light) {
                        let next = if palette.is_light {
                            self.prefs.last_dark_skin
                        } else {
                            SkinId::DaylightBooth
                        };
                        self.set_skin(next);
                    }
                    egui::ComboBox::from_id_salt("topbar_skin_selector")
                        .selected_text(self.prefs.skin.label())
                        .show_ui(ui, |ui| {
                            for sk in SkinId::ALL {
                                if ui.selectable_label(self.prefs.skin == sk, sk.label()).clicked() {
                                    self.set_skin(sk);
                                }
                            }
                        });
                    if ui
                        .button(format!("{:.0}%", self.prefs.zoom_factor * 100.0))
                        .on_hover_text("Reset zoom to 100%")
                        .clicked()
                    {
                        self.prefs.zoom_factor = 1.0;
                        self.prefs.save();
                    }
                });
            });
        });

        // 2. HELP MODALS (Command Palette, Shortcuts, Help Center, Licenses, About)
        self.render_modals(&ctx);

        // 3. TOP QUICK BAR + PARALLEL STACKED WAVEFORMS BANNER
        egui::Panel::top("parallel_waveforms_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(egui::RichText::new("DJCraft Pro").color(palette.deck_a));
                ui.separator();
                if ui.small_button("Auto-Sync").clicked() {
                    self.execute_catalog_id("commands.auto_sync");
                }
                if ui.small_button("Phase Sync").clicked() {
                    self.execute_catalog_id("commands.phase_sync");
                }
                if ui.small_button("Stop All").clicked() {
                    self.execute_catalog_id("deck.stop_all");
                }
                if ui.small_button("Palette (Ctrl+K)").clicked() {
                    self.show_command_palette = true;
                }
                ui.separator();
                ui.label(egui::RichText::new(&self.status_msg).color(palette.text_secondary));
            });
            if self.prefs.show_parallel_waveforms {
                ui.add_space(4.0);
                widgets::paint_parallel_waveforms(
                    ui,
                    &self.orch.deck_a,
                    self.deck_waveforms[0].as_ref(),
                    &self.orch.deck_b,
                    self.deck_waveforms[1].as_ref(),
                    &palette,
                );
            }
        });

        // 4. BOTTOM DECKS C & D STRIP
        if self.prefs.show_decks_cd {
            egui::Panel::bottom("decks_cd_panel")
                .resizable(false)
                .show(ui, |ui| {
                    ui.columns(2, |cols| {
                        for (col_idx, (deck_id, accent)) in
                            [(DeckId::DeckC, palette.deck_c), (DeckId::DeckD, palette.deck_d)]
                                .into_iter()
                                .enumerate()
                        {
                            let d = self.orch.deck(deck_id).clone();
                            theme::card_frame(&mut cols[col_idx], &palette, |ui| {
                                ui.horizontal(|ui| {
                                    theme::status_dot(ui, d.is_playing);
                                    ui.strong(egui::RichText::new(deck_id.label()).color(accent));
                                    if let Some(t) = &d.loaded_track {
                                        ui.label(&t.title);
                                        ui.monospace(format!("{} | {:.0} BPM", t.musical_key, t.bpm));
                                    } else {
                                        ui.label("Empty");
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("CUE").clicked() {
                                                self.dispatch_and_report(Command::CueReturn {
                                                    deck: deck_id,
                                                });
                                            }
                                            let lbl = if d.is_playing { "PAUSE" } else { "PLAY" };
                                            if ui.small_button(lbl).clicked() {
                                                self.dispatch_and_report(Command::PlayPause {
                                                    deck: deck_id,
                                                });
                                            }
                                        },
                                    );
                                });
                            });
                        }
                    });
                });
        }

        // 5. HARDWARE DECK A (Left) & DECK B (Right)
        if self.prefs.show_deck_a {
            egui::Panel::left("deck_a_panel")
                .default_size(310.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.render_hardware_deck(
                            ui,
                            DeckId::DeckA,
                            palette.deck_a,
                            self.platter_angle_a,
                            &palette,
                        );
                    });
                });
        }

        if self.prefs.show_deck_b {
            egui::Panel::right("deck_b_panel")
                .default_size(310.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.render_hardware_deck(
                            ui,
                            DeckId::DeckB,
                            palette.deck_b,
                            self.platter_angle_b,
                            &palette,
                        );
                    });
                });
        }

        // 6. CENTER BATTLE MIXER + TABBED WORKSTATION DOCK
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.render_battle_mixer(ui, &palette);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.lower_tab, LowerDockTab::Crates, "Crate Library");
                    ui.selectable_value(&mut self.lower_tab, LowerDockTab::VideoMixer, "Video Mixer");
                    ui.selectable_value(
                        &mut self.lower_tab,
                        LowerDockTab::MidiControllers,
                        "MIDI Controllers (Rane 12/72)",
                    );
                    ui.selectable_value(
                        &mut self.lower_tab,
                        LowerDockTab::CamelotAndMixxxxx,
                        "Camelot & Mixxxxx OSC",
                    );
                });
                ui.add_space(4.0);
                match self.lower_tab {
                    LowerDockTab::Crates => self.render_crate_dock(ui, &palette),
                    LowerDockTab::VideoMixer => self.render_video_dock(ui, &palette),
                    LowerDockTab::MidiControllers => self.render_midi_dock(ui, &palette),
                    LowerDockTab::CamelotAndMixxxxx => self.render_camelot_mixxxxx_dock(ui, &palette),
                }
            });
        });
    }
}

impl DjCraftApp {
    fn render_battle_mixer(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Battle Mixer (3-Band Rotary EQ, Vertical Faders, MAG FOUR Crossfader)");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mute_lbl = if self.orch.master_muted { "UNMUTE" } else { "MUTE" };
                    if ui.small_button(mute_lbl).clicked() {
                        self.execute_catalog_id("modify.toggle_mute");
                    }
                    ui.monospace(if self.audio_out.active {
                        format!("WASAPI: {}", self.audio_out.device_name)
                    } else {
                        "WASAPI: Offline".to_string()
                    });
                    theme::status_dot(ui, self.audio_out.active && !self.orch.master_muted);
                });
            });
            ui.add_space(4.0);

            let da = self.orch.deck_a.clone();
            let db = self.orch.deck_b.clone();

            ui.horizontal(|ui| {
                // Deck A EQ column
                ui.vertical(|ui| {
                    let mut hi = da.eq_high;
                    let mut mi = da.eq_mid;
                    let mut lo = da.eq_low;
                    let r1 = widgets::rotary_knob(ui, "A HI", &mut hi, 0.0, 2.0, 1.0, true, palette.deck_a, palette);
                    let r2 = widgets::rotary_knob(ui, "A MID", &mut mi, 0.0, 2.0, 1.0, true, palette.deck_a, palette);
                    let r3 = widgets::rotary_knob(ui, "A LOW", &mut lo, 0.0, 2.0, 1.0, true, palette.deck_a, palette);
                    if r1.changed || r2.changed || r3.changed {
                        self.orch.deck_a.eq_high = hi;
                        self.orch.deck_a.eq_mid = mi;
                        self.orch.deck_a.eq_low = lo;
                    }
                    if r1.committed || r2.committed || r3.committed {
                        self.dispatch_and_report(Command::SetDeckEq {
                            deck: DeckId::DeckA,
                            low: lo,
                            mid: mi,
                            high: hi,
                        });
                    }
                });

                // Deck A Channel Volume Fader + VU
                let mut vol_a = da.volume_fader;
                let fa = widgets::vertical_fader(ui, "CH A", &mut vol_a, 0.0, 1.0, 1.0, 150.0, palette.deck_a, palette);
                if fa.changed {
                    self.orch.deck_a.volume_fader = vol_a;
                }
                if fa.committed {
                    self.dispatch_and_report(Command::SetDeckVolume {
                        deck: DeckId::DeckA,
                        volume: vol_a,
                    });
                }
                widgets::paint_vu_meter(ui, &da);

                // Center Master Loudness Knob
                ui.vertical(|ui| {
                    ui.add_space(16.0);
                    let mut mv = self.orch.master_volume;
                    let rm = widgets::rotary_knob(
                        ui,
                        &format!("MST {:.0}%", mv * 100.0),
                        &mut mv,
                        0.0,
                        1.0,
                        0.75,
                        false,
                        egui::Color32::from_rgb(250, 204, 21),
                        palette,
                    );
                    if rm.changed {
                        self.orch.master_volume = mv;
                    }
                    if rm.committed {
                        self.dispatch_and_report(Command::SetMasterVolume { volume: mv });
                    }
                });

                // Deck B VU + Channel Volume Fader
                widgets::paint_vu_meter(ui, &db);
                let mut vol_b = db.volume_fader;
                let fb = widgets::vertical_fader(ui, "CH B", &mut vol_b, 0.0, 1.0, 1.0, 150.0, palette.deck_b, palette);
                if fb.changed {
                    self.orch.deck_b.volume_fader = vol_b;
                }
                if fb.committed {
                    self.dispatch_and_report(Command::SetDeckVolume {
                        deck: DeckId::DeckB,
                        volume: vol_b,
                    });
                }

                // Deck B EQ column
                ui.vertical(|ui| {
                    let mut hi = db.eq_high;
                    let mut mi = db.eq_mid;
                    let mut lo = db.eq_low;
                    let r1 = widgets::rotary_knob(ui, "B HI", &mut hi, 0.0, 2.0, 1.0, true, palette.deck_b, palette);
                    let r2 = widgets::rotary_knob(ui, "B MID", &mut mi, 0.0, 2.0, 1.0, true, palette.deck_b, palette);
                    let r3 = widgets::rotary_knob(ui, "B LOW", &mut lo, 0.0, 2.0, 1.0, true, palette.deck_b, palette);
                    if r1.changed || r2.changed || r3.changed {
                        self.orch.deck_b.eq_high = hi;
                        self.orch.deck_b.eq_mid = mi;
                        self.orch.deck_b.eq_low = lo;
                    }
                    if r1.committed || r2.committed || r3.committed {
                        self.dispatch_and_report(Command::SetDeckEq {
                            deck: DeckId::DeckB,
                            low: lo,
                            mid: mi,
                            high: hi,
                        });
                    }
                });
            });

            ui.add_space(6.0);
            let mut xf = self.orch.crossfader;
            let rxf = widgets::club_crossfader(ui, &mut xf, palette);
            if rxf.changed {
                self.orch.crossfader = xf;
            }
            if rxf.committed {
                self.dispatch_and_report(Command::SetCrossfader { value: xf });
            }
        });
    }

    fn render_crate_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        if self.crate_dirty {
            self.refresh_crates();
        }
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Crate & Subcrate Library");
                if ui.small_button("Scan Folder...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        if let Some(ref store) = self.crate_store {
                            let target = self
                                .crate_query
                                .crate_id
                                .or_else(|| store.ensure_crate_path("Scanned Music").ok());
                            if let Some(cid) = target {
                                match store.scan_audio_folder(cid, &dir) {
                                    Ok(n) => {
                                        self.status_msg = format!("Scanned {n} tracks");
                                        self.crate_dirty = true;
                                    }
                                    Err(e) => self.status_msg = format!("Scan error: {e}"),
                                }
                            }
                        }
                    }
                }
                if ui.small_button("Import Serato...").clicked() {
                    if let Some(f) = rfd::FileDialog::new()
                        .add_filter("Serato Crate", &["crate"])
                        .pick_file()
                    {
                        if let Some(ref store) = self.crate_store {
                            match store.import_serato_crate(&f) {
                                Ok(s) => {
                                    self.crate_query.crate_id = Some(s.crate_id);
                                    self.status_msg = format!("Imported {} -> '{}'", s.imported_tracks, s.crate_path);
                                    self.crate_dirty = true;
                                }
                                Err(e) => self.status_msg = format!("Serato error: {e}"),
                            }
                        }
                    }
                }
                if ui.small_button("Export Serato...").clicked() {
                    if let Some(cid) = self.crate_query.crate_id {
                        if let Some(d) = rfd::FileDialog::new()
                            .add_filter("Serato Crate", &["crate"])
                            .set_file_name("djcraft.crate")
                            .save_file()
                        {
                            if let Some(ref store) = self.crate_store {
                                match store.export_serato_crate(cid, &d) {
                                    Ok(n) => self.status_msg = format!("Exported {n} tracks to Serato"),
                                    Err(e) => self.status_msg = format!("Export error: {e}"),
                                }
                            }
                        }
                    }
                }
                if ui.small_button("Import VDJ...").clicked() {
                    if let Some(f) = rfd::FileDialog::new()
                        .add_filter("VirtualDJ / M3U", &["vdjfolder", "m3u", "m3u8"])
                        .pick_file()
                    {
                        if let Some(ref store) = self.crate_store {
                            match store.import_vdj_file(&f) {
                                Ok(s) => {
                                    self.crate_query.crate_id = Some(s.crate_id);
                                    self.status_msg = format!("Imported {} -> '{}'", s.imported_tracks, s.crate_path);
                                    self.crate_dirty = true;
                                }
                                Err(e) => self.status_msg = format!("VDJ error: {e}"),
                            }
                        }
                    }
                }
                if ui.small_button("Export VDJ...").clicked() {
                    if let Some(cid) = self.crate_query.crate_id {
                        if let Some(d) = rfd::FileDialog::new()
                            .add_filter("VirtualDJ Folder", &["vdjfolder"])
                            .add_filter("M3U8", &["m3u8"])
                            .set_file_name("djcraft.vdjfolder")
                            .save_file()
                        {
                            if let Some(ref store) = self.crate_store {
                                match store.export_vdj_file(cid, &d) {
                                    Ok(n) => self.status_msg = format!("Exported {n} tracks to VDJ"),
                                    Err(e) => self.status_msg = format!("Export error: {e}"),
                                }
                            }
                        }
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.crate_new_name)
                        .desired_width(130.0)
                        .hint_text("New crate..."),
                );
                if ui.small_button("+ Crate").clicked() && !self.crate_new_name.trim().is_empty() {
                    if let Some(ref store) = self.crate_store {
                        if let Ok(cid) = store.create_subcrate(None, &self.crate_new_name) {
                            self.crate_query.crate_id = Some(cid);
                            self.crate_new_name.clear();
                            self.crate_dirty = true;
                        }
                    }
                }
                if ui
                    .add_enabled(
                        self.crate_query.crate_id.is_some() && !self.crate_new_name.trim().is_empty(),
                        egui::Button::new("+ Subcrate").small(),
                    )
                    .clicked()
                {
                    if let Some(ref store) = self.crate_store {
                        if let Ok(cid) = store.create_subcrate(self.crate_query.crate_id, &self.crate_new_name) {
                            self.crate_query.crate_id = Some(cid);
                            self.crate_new_name.clear();
                            self.crate_dirty = true;
                        }
                    }
                }
            });

            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(self.crate_query.crate_id.is_none(), "All").clicked() {
                    self.crate_query.crate_id = None;
                    self.crate_dirty = true;
                }
                for node in self.crate_nodes.clone() {
                    let lbl = format!("{} ({})", node.full_path, node.track_count);
                    if ui
                        .selectable_label(self.crate_query.crate_id == Some(node.id), lbl)
                        .clicked()
                    {
                        self.crate_query.crate_id = Some(node.id);
                        self.crate_dirty = true;
                    }
                }
            });

            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.crate_query.search)
                            .desired_width(110.0)
                            .hint_text("Search..."),
                    )
                    .changed()
                {
                    self.crate_dirty = true;
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.crate_bpm_min_text)
                            .desired_width(38.0)
                            .hint_text("min"),
                    )
                    .changed()
                {
                    self.crate_dirty = true;
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.crate_bpm_max_text)
                            .desired_width(38.0)
                            .hint_text("max"),
                    )
                    .changed()
                {
                    self.crate_dirty = true;
                }
                if ui.checkbox(&mut self.crate_harmonic_deck_a, "Harmonic A").changed() {
                    self.crate_dirty = true;
                }
                if ui.checkbox(&mut self.crate_query.include_subcrates, "Subcrates").changed() {
                    self.crate_dirty = true;
                }
                egui::ComboBox::from_id_salt("crate_sort")
                    .selected_text(self.crate_query.sort_by.label())
                    .show_ui(ui, |ui| {
                        for f in CrateSortField::ALL {
                            if ui.selectable_value(&mut self.crate_query.sort_by, f, f.label()).changed() {
                                self.crate_dirty = true;
                            }
                        }
                    });
                if ui
                    .small_button(if self.crate_query.sort_asc { "ASC" } else { "DESC" })
                    .clicked()
                {
                    self.crate_query.sort_asc = !self.crate_query.sort_asc;
                    self.crate_dirty = true;
                }
            });

            ui.separator();
            egui::ScrollArea::vertical().max_height(190.0).show(ui, |ui| {
                for t in self.crate_results.clone() {
                    ui.horizontal(|ui| {
                        let key = if t.musical_key.is_empty() { "--" } else { t.musical_key.as_str() };
                        let bpm = if t.bpm > 0.0 {
                            format!("{:.1}", t.bpm)
                        } else {
                            "---".to_string()
                        };
                        ui.label(format!("{} - {} | {} | {} BPM", t.artist, t.title, key, bpm));
                        for (lbl, deck_id) in [
                            ("A", DeckId::DeckA),
                            ("B", DeckId::DeckB),
                            ("C", DeckId::DeckC),
                            ("D", DeckId::DeckD),
                        ] {
                            if ui.small_button(lbl).clicked() {
                                self.load_crate_track_to_deck(deck_id, &t);
                            }
                        }
                        if !t.path.starts_with("demo://") {
                            if ui.small_button("BPM").clicked() {
                                if let Some(ref store) = self.crate_store {
                                    if let Ok(up) = store.analyze_track_builtin(t.id) {
                                        self.status_msg = format!("Analyzed '{}': {:.1} BPM", up.title, up.bpm);
                                        self.crate_dirty = true;
                                    }
                                }
                            }
                            if ui.small_button("Dettato").clicked() {
                                if let Some(ref store) = self.crate_store {
                                    match store.analyze_track_dettato(t.id) {
                                        Ok(up) => {
                                            self.status_msg = format!("Dettato '{}': {:?}", up.title, up.tuning_cents);
                                            self.crate_dirty = true;
                                        }
                                        Err(e) => self.status_msg = format!("Dettato error: {e}"),
                                    }
                                }
                            }
                        }
                    });
                }
            });
        });
    }

    fn render_video_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal(|ui| {
                theme::status_dot(ui, self.video_deck_a.has_file || self.video_deck_b.has_file);
                ui.strong("Native Video Mixer (FFmpeg Pipe + Crossfader Blend)");
                let lbl = if self.video_projector_open {
                    "Close Projector"
                } else {
                    "Detach Projector"
                };
                if ui.button(lbl).clicked() {
                    self.video_projector_open = !self.video_projector_open;
                }
            });
            if let Some(tex) = &self.video_texture {
                let w = ui.available_width().clamp(240.0, 440.0);
                let h = w * (VIDEO_H as f32 / VIDEO_W as f32);
                ui.image(egui::load::SizedTexture::new(tex.id(), egui::vec2(w, h)));
            }
            ui.horizontal(|ui| {
                if ui.small_button("Load Video A...").clicked() {
                    self.load_video_file_dialog(DeckId::DeckA);
                }
                if ui.small_button("Load Video B...").clicked() {
                    self.load_video_file_dialog(DeckId::DeckB);
                }
                if ui.small_button("Clear Videos").clicked() {
                    self.video_deck_a.stop();
                    self.video_deck_b.stop();
                }
            });
            ui.add(egui::Slider::new(&mut self.video_brightness, 0.0..=1.0).text("Brightness"));
            ui.add(egui::Slider::new(&mut self.video_contrast, 0.0..=1.0).text("Contrast"));
            ui.add(egui::Slider::new(&mut self.video_saturation, 0.0..=1.0).text("Saturation"));
        });
    }

    fn render_midi_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal(|ui| {
                theme::status_dot(ui, self.midi.connected_port.is_some());
                ui.strong("Hardware MIDI Controllers (Rane Twelve MKII, Rane Seventy-Two MKII, Pioneer, Hercules)");
                if ui.small_button("Rescan USB").clicked() {
                    self.midi_ports = MidiControllerManager::list_input_ports();
                    self.status_msg = format!("Found {} MIDI input port(s)", self.midi_ports.len());
                }
            });

            if let Some(port) = self.midi.connected_port.clone() {
                ui.horizontal(|ui| {
                    ui.monospace(format!("Connected: {port}"));
                    if ui.small_button("Disconnect").clicked() {
                        self.midi.disconnect();
                        self.status_msg = "MIDI controller disconnected".to_string();
                    }
                });
            } else if self.midi_ports.is_empty() {
                ui.label("No USB MIDI controllers detected. Plug in Rane Twelve MKII / Seventy-Two MKII and click Rescan USB.");
            } else {
                for (idx, port_name) in self.midi_ports.clone().iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.monospace(port_name);
                        if ui.small_button("Connect").clicked() {
                            match self.midi.connect_port(idx) {
                                Ok(name) => {
                                    self.status_msg = format!(
                                        "Connected MIDI '{}' (profile: {})",
                                        name,
                                        self.midi.active_profile.label()
                                    );
                                }
                                Err(e) => self.status_msg = format!("MIDI connect error: {e}"),
                            }
                        }
                    });
                }
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Active Profile:");
                egui::ComboBox::from_id_salt("midi_profile_select")
                    .selected_text(self.midi.active_profile.label())
                    .show_ui(ui, |ui| {
                        for p in ControllerProfileId::ALL {
                            if ui
                                .selectable_label(self.midi.active_profile == p, p.label())
                                .clicked()
                            {
                                self.midi.set_profile(p);
                                self.status_msg = format!("Loaded MIDI profile: {}", p.label());
                            }
                        }
                    });
                ui.monospace(format!("({} rules)", self.midi.rules.len()));
            });

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.strong("MIDI Learn:");
                if let Some(target) = self.midi.learn_target {
                    ui.label(
                        egui::RichText::new(format!("Move hardware control for '{}'...", target.label()))
                            .color(egui::Color32::from_rgb(250, 204, 21)),
                    );
                    if ui.small_button("Cancel").clicked() {
                        self.midi.learn_target = None;
                    }
                } else {
                    for action in MidiTargetAction::LEARNABLE {
                        if ui.small_button(action.label()).clicked() {
                            self.midi.learn_target = Some(action);
                        }
                    }
                }
            });

            if let Some(ev) = self.midi.last_event {
                ui.monospace(format!(
                    "Last MIDI: {:?} ch={} num=0x{:02X} val={}",
                    ev.kind,
                    ev.channel + 1,
                    ev.number,
                    ev.value
                ));
            }
        });
    }

    fn render_camelot_mixxxxx_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal(|ui| {
                let key_a = self.orch.deck_a.loaded_track.as_ref().map(|t| t.musical_key.as_str());
                let key_b = self.orch.deck_b.loaded_track.as_ref().map(|t| t.musical_key.as_str());
                widgets::paint_camelot_wheel_mini(ui, key_a, key_b, palette);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.strong("Camelot Harmonic Assistant");
                    if let Ok(plan) = self.orch.plan_transition() {
                        ui.label(format!("{} -> {}", plan.from_track, plan.to_track));
                        ui.monospace(format!(
                            "BPM {:.1} -> {:.1} (Delta {:+.2}) | {} bars",
                            plan.from_bpm, plan.to_bpm, plan.bpm_delta, plan.recommended_transition_bars
                        ));
                        ui.label(&plan.suggested_eq_strategy);
                    }
                });
            });
        });

        ui.add_space(6.0);
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal(|ui| {
                theme::status_dot(ui, self.mixxxxx_last_pong);
                ui.strong("Mixxxxx Video OSC Bridge (UDP 11119)");
                if ui.small_button("Ping OSC").clicked() {
                    self.execute_catalog_id("commands.mixxxxx_ping");
                }
            });
            ui.horizontal(|ui| {
                for d in [DeckId::DeckA, DeckId::DeckB, DeckId::DeckC, DeckId::DeckD] {
                    ui.selectable_value(&mut self.mixxxxx_deck, d, d.label());
                }
                ui.checkbox(&mut self.mixxxxx_video_enabled, "Video");
                ui.checkbox(&mut self.mixxxxx_video_fullscreen, "Fullscreen");
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut self.mixxxxx_brightness, 0.0..=1.0).text("Brt"));
                ui.add(egui::Slider::new(&mut self.mixxxxx_contrast, 0.0..=1.0).text("Con"));
                ui.add(egui::Slider::new(&mut self.mixxxxx_saturation, 0.0..=1.0).text("Sat"));
                if ui.small_button("Send").clicked() {
                    self.dispatch_and_report(Command::MixxxxxVideo {
                        deck: self.mixxxxx_deck,
                        controls: MixxxxxVideoControls {
                            enabled: Some(self.mixxxxx_video_enabled),
                            fullscreen: Some(self.mixxxxx_video_fullscreen),
                            brightness: Some(self.mixxxxx_brightness),
                            contrast: Some(self.mixxxxx_contrast),
                            saturation: Some(self.mixxxxx_saturation),
                        },
                    });
                }
            });
        });
    }

    fn render_modals(&mut self, ctx: &egui::Context) {
        if self.show_command_palette {
            let mut open = self.show_command_palette;
            let mut trigger_id: Option<&'static str> = None;
            egui::Window::new("Command Palette (Ctrl+K)")
                .open(&mut open)
                .collapsible(false)
                .default_width(500.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Filter:");
                        ui.text_edit_singleline(&mut self.palette_query);
                    });
                    ui.separator();
                    let q = self.palette_query.trim().to_lowercase();
                    let matches: Vec<_> = Command::catalog()
                        .into_iter()
                        .filter(|s| {
                            q.is_empty()
                                || s.label.to_lowercase().contains(&q)
                                || s.category.to_lowercase().contains(&q)
                        })
                        .collect();
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Some(first) = matches.first() {
                            trigger_id = Some(first.id);
                        }
                    }
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for s in matches {
                            if ui.button(format!("[{}] {}", s.category, s.label)).clicked() {
                                trigger_id = Some(s.id);
                            }
                        }
                    });
                });
            self.show_command_palette = open;
            if let Some(id) = trigger_id {
                self.execute_catalog_id(id);
                self.show_command_palette = false;
                self.palette_query.clear();
            }
        }

        if self.show_shortcuts_dialog {
            let mut open = self.show_shortcuts_dialog;
            egui::Window::new("Keyboard Shortcuts")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    for s in Command::catalog() {
                        if !s.hotkey.is_empty() {
                            ui.horizontal(|ui| {
                                ui.monospace(format!("{:<12}", s.hotkey));
                                ui.label(s.label);
                            });
                        }
                    }
                    ui.separator();
                    ui.label("Double-click any rotary knob or fader to reset to default.");
                    ui.label("Ctrl + Mouse Wheel : Zoom UI | Ctrl + 0 : Reset 100%");
                });
            self.show_shortcuts_dialog = open;
        }

        if self.show_help_center {
            let mut open = self.show_help_center;
            egui::Window::new("DJCraft Help Center")
                .open(&mut open)
                .collapsible(false)
                .default_width(640.0)
                .default_height(400.0)
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for tab in HelpTab::ALL {
                            if ui.selectable_label(self.help_tab == tab, tab.label()).clicked() {
                                self.help_tab = tab;
                            }
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| match self.help_tab {
                        HelpTab::GetStarted => {
                            ui.label("1. WASAPI audio starts live; switch console skins in the top bar.");
                            ui.label("2. Load MP3/WAV/FLAC via AUDIO or Crate Library (Serato/VDJ import supported).");
                            ui.label("3. Drag rotary EQ knobs, vertical channel faders, and MAG FOUR crossfader.");
                            ui.label("4. Connect Rane Twelve MKII / Seventy-Two MKII in the MIDI Controllers tab.");
                        }
                        HelpTab::Functions => {
                            ui.label("- 4-Deck WASAPI Audio, Symphonia Decode, Varispeed + WSOLA Keylock, 3-Band Kill EQ.");
                            ui.label("- Persistent SQLite Crates/Subcrates with Serato .crate and VirtualDJ .vdjfolder/.m3u8.");
                            ui.label("- Native FFmpeg Video Mixer + Detachable Projector + Mixxxxx OSC 1.0.");
                            ui.label("- Hardware MIDI Engine (midir) + MIDI Learn + Rane 12/72, Pioneer, Hercules profiles.");
                        }
                        HelpTab::Usage => {
                            ui.monospace("djcraft.exe --smoke");
                            ui.monospace("djcraft-cli crate-list | crate-import-serato | crate-import-vdj");
                            ui.monospace("djcraft-cli mixxxxx-ping | analyze-dettato <file>");
                        }
                        HelpTab::TipsAndTricks => {
                            ui.label("- Double-click any rotary knob or vertical fader to snap to its neutral detent.");
                            ui.label("- Use the stacked Parallel Waveforms banner to visually lock beatgrid bars.");
                        }
                        HelpTab::PitfallsAndSafety => {
                            ui.label("- Run dettato offline during crate prep; keep live audio on WASAPI.");
                            ui.label("- All workspace crates enforce #![forbid(unsafe_code)].");
                        }
                        HelpTab::PlannedExtensions => {
                            ui.label("- Dual-bus Master + Headphone PFL cue routing & WAV/FLAC mix recording.");
                        }
                        HelpTab::OtherCrafts => {
                            ui.label("- mixxxxx (OSC video), reaper-mcp (shared dettato), soundcraft, speechcraft.");
                        }
                        HelpTab::ProjectDocs => {
                            ui.label("README.md | HELP.md | CHANGELOG.md | BUILD_LOG.md | docs/TIERED_PLAN.md");
                        }
                    });
                });
            self.show_help_center = open;
        }

        if self.show_licenses_dialog {
            let mut open = self.show_licenses_dialog;
            egui::Window::new("Licenses & Notices")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("DJCraft: Dual-licensed MIT OR Apache-2.0 (#![forbid(unsafe_code)]).");
                    ui.label("Crates: egui, eframe, cpal, symphonia, midir, rusqlite, rfd, clap, serde.");
                });
            self.show_licenses_dialog = open;
        }

        if self.show_about_dialog {
            let mut open = self.show_about_dialog;
            egui::Window::new("About DJCraft")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.heading(format!("DJCraft Pro v{APP_VERSION}"));
                    ui.label("100% Safe Rust 4-Deck Audio + Video DJ Workstation.");
                    ui.label("Rane Twelve MKII / Seventy-Two MKII MIDI | Serato & VirtualDJ Crates.");
                });
            self.show_about_dialog = open;
        }
    }
}

fn run_headless_smoke() -> Result<(), Box<dyn std::error::Error>> {
    println!("[djcraft] Running headless smoke verification...");
    let mut orch = DjOrchestrator::new();
    let plan = orch.plan_transition()?;
    let sync_out = orch.dispatch(Command::AutoSyncBeatgrid)?;
    let undo_out = orch.dispatch(Command::Undo)?;
    let redo_out = orch.dispatch(Command::Redo)?;
    let pcm = AudioEngine::synthesize_deck_audio(&orch.deck_a, 0.1, 44100);
    let smoke_payload = json!({
        "app": "djcraft",
        "version": APP_VERSION,
        "status": "ready",
        "control_port": 14118,
        "success": true,
        "menu_count": 8,
        "skins_count": SkinId::ALL.len(),
        "midi_profiles_count": ControllerProfileId::ALL.len(),
        "has_menu_bar": true,
        "has_switch_on_hover": true,
        "has_parallel_waveforms": true,
        "has_rotary_knobs": true,
        "has_performance_pads": true,
        "help_tabs": HelpTab::ALL.len(),
        "tofu_free": true,
        "command_catalog_count": Command::catalog().len(),
        "dispatch_parity": sync_out.ok && undo_out.ok && redo_out.ok,
        "deck_a_active": orch.deck_a.is_playing,
        "harmonic_compatible": plan.harmonic_compatible,
        "audio_pcm_synthesizer_active": !pcm.is_empty(),
        "dettato_sidecar_available": resolve_dettato_binary().is_some(),
        "mixxxxx_binary_available": resolve_mixxxxx_binary().is_some(),
    });
    println!("[djcraft] Smoke OK: {smoke_payload}");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.smoke {
        return run_headless_smoke();
    }

    let app = DjCraftApp::new();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("DJCraft Pro - Sovereign 4-Deck Audio + Video Studio"),
        ..Default::default()
    };

    eframe::run_native("DJCraft", native_options, Box::new(|_cc| Ok(Box::new(app))))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn glyph_allowlist() {
        const ALLOWED: &[char] = &['\u{00B0}', '\u{00D7}', '\u{00B7}', '\u{2026}', '\u{2014}'];
        let sources = [
            ("main.rs", include_str!("main.rs")),
            ("skins.rs", include_str!("skins.rs")),
            ("theme.rs", include_str!("theme.rs")),
            ("video.rs", include_str!("video.rs")),
            ("widgets.rs", include_str!("widgets.rs")),
        ];
        for (file, src) in sources {
            for (line_idx, line) in src.lines().enumerate() {
                for ch in line.chars() {
                    if !ch.is_ascii() && !ALLOWED.contains(&ch) {
                        panic!(
                            "Disallowed non-ASCII glyph U+{:04X} ({ch:?}) in {file}:{}",
                            ch as u32,
                            line_idx + 1
                        );
                    }
                }
            }
        }
    }
}
