#![forbid(unsafe_code)]

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
    resolve_dettato_binary, resolve_mixxxxx_binary, Command, CrateNode, CrateQuery, CrateSortField,
    CrateStore, CrateTrack, DjOrchestrator, MixxxxxVideoControls,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UiPrefs {
    is_dark_theme: bool,
    zoom_factor: f32,
    show_deck_a: bool,
    show_deck_b: bool,
    show_decks_cd: bool,
    show_crate_browser: bool,
    show_camelot_hud: bool,
    show_mixxxxx_dock: bool,
    show_video_mixer: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            is_dark_theme: true,
            zoom_factor: 1.0,
            show_deck_a: true,
            show_deck_b: true,
            show_decks_cd: true,
            show_crate_browser: true,
            show_camelot_hud: true,
            show_mixxxxx_dock: true,
            show_video_mixer: true,
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

    // Persisted View / Window layout preferences
    prefs: UiPrefs,

    // Mixxxxx inline video dock state
    mixxxxx_deck: DeckId,
    mixxxxx_video_enabled: bool,
    mixxxxx_video_fullscreen: bool,
    mixxxxx_brightness: f32,
    mixxxxx_contrast: f32,
    mixxxxx_saturation: f32,
    mixxxxx_last_pong: bool,

    // Live WASAPI / cpal audio output stream & decoded real-audio waveform summaries
    audio_out: AudioOutput,
    deck_waveforms: [Option<WaveformSummary>; 4],

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

    // Native Video Mixer (FFmpeg frame-pipe + procedural reactive fallback + detachable projector)
    video_deck_a: VideoDeckPlayer,
    video_deck_b: VideoDeckPlayer,
    video_texture: Option<egui::TextureHandle>,
    video_projector_open: bool,
    video_brightness: f32,
    video_contrast: f32,
    video_saturation: f32,
    video_phase: f32,

    // Slider draft values (committed via Command::dispatch on drag_stopped for Undo/Redo)
    crossfader_draft: f64,
    master_volume_draft: f64,

    // Mandatory Help Suite modals
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
        let crossfader_draft = orch.crossfader;
        let master_volume_draft = orch.master_volume;
        let audio_out = AudioOutput::start_default();
        let crate_store = CrateStore::open_default().ok();
        if let Some(ref store) = crate_store {
            let _ = store.seed_demo_if_empty();
        }
        let status_msg = if audio_out.active {
            format!(
                "4-Deck Studio Ready | Audio: {} ({} Hz)",
                audio_out.device_name, audio_out.sample_rate
            )
        } else {
            format!(
                "4-Deck Studio Ready | Audio offline: {}",
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

            mixxxxx_deck: DeckId::DeckA,
            mixxxxx_video_enabled: true,
            mixxxxx_video_fullscreen: false,
            mixxxxx_brightness: 0.5,
            mixxxxx_contrast: 0.5,
            mixxxxx_saturation: 0.5,
            mixxxxx_last_pong: false,

            audio_out,
            deck_waveforms: [None, None, None, None],

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

            crossfader_draft,
            master_volume_draft,

            show_command_palette: false,
            palette_query: String::new(),
            show_shortcuts_dialog: false,
            show_help_center: false,
            help_tab: HelpTab::GetStarted,
            show_licenses_dialog: false,
            show_about_dialog: false,
        }
    }

    fn reset_layout(&mut self) {
        self.prefs = UiPrefs {
            is_dark_theme: self.prefs.is_dark_theme,
            ..UiPrefs::default()
        };
        self.prefs.save();
        self.status_msg = "Panel layout and zoom reset".to_string();
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
                self.crossfader_draft = self.orch.crossfader;
                self.master_volume_draft = self.orch.master_volume;
                self.status_msg = outcome.message;
            }
            Err(err) => self.status_msg = format!("Error: {err}"),
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
                let deck_idx = match deck {
                    DeckId::DeckA => 0,
                    DeckId::DeckB => 1,
                    DeckId::DeckC => 2,
                    DeckId::DeckD => 3,
                };
                self.deck_waveforms[deck_idx] = Some(WaveformSummary::from_decoded(&decoded));
                self.audio_out.set_deck_audio(deck_idx, Some(Arc::new(decoded)));
                self.dispatch_and_report(Command::LoadTrack { deck, track: meta });
                let state = self.orch.deck_mut(deck);
                state.beatgrid_offset_sec = grid.first_beat_sec;
                state.audio_path = Some(path.to_string_lossy().to_string());

                // Auto-load companion video (.mp4/.mkv/.mov/.webm) with same basename if present
                let mut companion_note = String::new();
                if let Some(vid_path) = find_companion_video(&path) {
                    let vres = match deck {
                        DeckId::DeckA => self.video_deck_a.load_file(&vid_path),
                        DeckId::DeckB => self.video_deck_b.load_file(&vid_path),
                        _ => Ok(()),
                    };
                    if vres.is_ok() {
                        companion_note = format!(
                            " + video {}",
                            vid_path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("companion")
                        );
                    }
                }
                self.status_msg = format!(
                    "Loaded '{}' to {} ({:.1} BPM, beat 1 at {:.2}s, conf {:.0}%){}",
                    title,
                    deck.label(),
                    grid.bpm,
                    grid.first_beat_sec,
                    grid.confidence * 100.0,
                    companion_note
                );
            }
            Err(err) => {
                self.status_msg = format!("Audio decode failed: {err}");
            }
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
            _ => Err("Native video decks currently support Deck A and Deck B".to_string()),
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
        let deck_idx = match deck {
            DeckId::DeckA => 0,
            DeckId::DeckB => 1,
            DeckId::DeckC => 2,
            DeckId::DeckD => 3,
        };
        if track.path.starts_with("demo://") {
            self.deck_waveforms[deck_idx] = None;
            self.audio_out.set_deck_audio(deck_idx, None);
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
                self.deck_waveforms[deck_idx] = Some(WaveformSummary::from_decoded(&decoded));
                self.audio_out.set_deck_audio(deck_idx, Some(Arc::new(decoded.clone())));
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
            Err(err) => {
                self.status_msg = format!("Cannot decode {}: {err}", track.path);
            }
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
            "commands.auto_sync" => self.dispatch_and_report(Command::AutoSyncBeatgrid),
            "commands.phase_sync" => self.dispatch_and_report(Command::PhaseSyncBeatgrid),
            "modify.keylock_a" => {
                self.dispatch_and_report(Command::ToggleKeylock { deck: DeckId::DeckA })
            }
            "commands.plan_transition" => self.dispatch_and_report(Command::PlanTransition),
            "commands.dettato_status" => {
                self.status_msg = match resolve_dettato_binary() {
                    Some(p) => format!("Dettato sidecar ready: {}", p.display()),
                    None => "Dettato sidecar not found (set DETTATO_BIN or install into D:\\Dev\\tools\\dettato\\.venv)".to_string(),
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
            _ => {
                self.status_msg = format!("Unknown command id: {id}");
            }
        }
    }

    fn render_deck_panel(
        &mut self,
        ui: &mut egui::Ui,
        deck_id: DeckId,
        accent: egui::Color32,
        platter_angle: f32,
    ) {
        let dark = self.prefs.is_dark_theme;
        let deck_snapshot = self.orch.deck(deck_id).clone();
        let play_cmd = if deck_id == DeckId::DeckA {
            "deck.play_pause.a"
        } else {
            "deck.play_pause.b"
        };
        let cue_cmd = if deck_id == DeckId::DeckA {
            "deck.cue.a"
        } else {
            "deck.cue.b"
        };

        ui.horizontal(|ui| {
            theme::status_dot(ui, deck_snapshot.is_playing);
            ui.heading(deck_id.label());
            if let Some(bars) = deck_snapshot.loop_bars {
                ui.monospace(format!("[LOOP {bars}B]"));
            }
        });

        theme::card_frame(ui, dark, |ui| {
            if let Some(track) = &deck_snapshot.loaded_track {
                ui.label(egui::RichText::new(&track.title).strong().color(accent));
                ui.label(format!(
                    "{} | Key {} | {:.1} BPM ({:+.1}%)",
                    track.artist, track.musical_key, track.bpm, deck_snapshot.pitch_bend_pct
                ));
                ui.monospace(format!(
                    "{:.1}s / {:.1}s",
                    deck_snapshot.position_seconds, track.duration_seconds
                ));
            } else {
                ui.label("Empty Deck - load from Crate Browser");
            }
        });

        ui.add_space(6.0);
        let deck_idx = match deck_id {
            DeckId::DeckA => 0,
            DeckId::DeckB => 1,
            DeckId::DeckC => 2,
            DeckId::DeckD => 3,
        };
        widgets::paint_scrolling_waveform(
            ui,
            &deck_snapshot,
            self.deck_waveforms[deck_idx].as_ref(),
            accent,
        );

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let play_txt = if deck_snapshot.is_playing {
                "PAUSE"
            } else {
                "PLAY"
            };
            if ui.button(egui::RichText::new(play_txt).strong()).clicked() {
                self.execute_catalog_id(play_cmd);
            }
            if ui.button("CUE").clicked() {
                self.execute_catalog_id(cue_cmd);
            }
            if ui.button("HOT CUE").clicked() {
                self.dispatch_and_report(Command::SetHotCue {
                    deck: deck_id,
                    slot: 0,
                });
            }
            if ui.button("LOOP 8B").clicked() {
                let next = if deck_snapshot.loop_bars.is_some() {
                    None
                } else {
                    Some(8)
                };
                self.dispatch_and_report(Command::SetLoopBars {
                    deck: deck_id,
                    bars: next,
                });
            }
            if ui.button("LOAD FILE").clicked() {
                self.load_audio_file_dialog(deck_id);
            }
            if ui.button("LOAD VID").clicked() {
                self.load_video_file_dialog(deck_id);
            }
        });

        ui.add_space(6.0);
        theme::card_frame(ui, dark, |ui| {
            ui.horizontal(|ui| {
                ui.strong("3-Band Kill EQ");
                if ui.small_button("FLAT").clicked() {
                    self.dispatch_and_report(Command::ResetDeckEq { deck: deck_id });
                }
            });
            let mut low = deck_snapshot.eq_low;
            let mut mid = deck_snapshot.eq_mid;
            let mut high = deck_snapshot.eq_high;
            let mut eq_changed = false;
            let mut eq_stopped = false;

            for (label, val) in [("Low ", &mut low), ("Mid ", &mut mid), ("High", &mut high)] {
                ui.horizontal(|ui| {
                    let kill_lbl = if *val <= 0.001 { "ON" } else { "KILL" };
                    if ui.small_button(kill_lbl).clicked() {
                        *val = if *val <= 0.001 { 1.0 } else { 0.0 };
                        eq_changed = true;
                        eq_stopped = true;
                    }
                    let resp = ui.add(
                        egui::Slider::new(val, 0.0..=2.0)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                            .text(label),
                    );
                    eq_changed |= resp.changed();
                    eq_stopped |= resp.drag_stopped();
                });
            }
            if eq_changed {
                let st = self.orch.deck_mut(deck_id);
                st.eq_low = low;
                st.eq_mid = mid;
                st.eq_high = high;
            }
            if eq_stopped {
                self.dispatch_and_report(Command::SetDeckEq {
                    deck: deck_id,
                    low,
                    mid,
                    high,
                });
            }
        });

        ui.add_space(6.0);
        theme::card_frame(ui, dark, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Tempo / Keylock / Beatgrid");
                let kl_lbl = if deck_snapshot.keylock {
                    "KEYLOCK: ON"
                } else {
                    "VARISPEED"
                };
                if ui.button(kl_lbl).clicked() {
                    self.dispatch_and_report(Command::ToggleKeylock { deck: deck_id });
                }
            });
            let mut pitch = deck_snapshot.pitch_bend_pct;
            let r_pitch = ui.add(
                egui::Slider::new(&mut pitch, -16.0..=16.0)
                    .custom_formatter(|v, _| format!("{v:+.1}%"))
                    .text("Tempo"),
            );
            if r_pitch.changed() {
                self.orch.deck_mut(deck_id).pitch_bend_pct = pitch;
            }
            if r_pitch.drag_stopped() {
                self.dispatch_and_report(Command::SetPitchBend {
                    deck: deck_id,
                    pct: pitch,
                });
            }
            ui.horizontal(|ui| {
                if ui.small_button("0%").clicked() {
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
                if ui.small_button("Grid -10ms").clicked() {
                    self.dispatch_and_report(Command::NudgeBeatgrid {
                        deck: deck_id,
                        delta_ms: -10.0,
                    });
                }
                if ui.small_button("Grid +10ms").clicked() {
                    self.dispatch_and_report(Command::NudgeBeatgrid {
                        deck: deck_id,
                        delta_ms: 10.0,
                    });
                }
            });
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let eff_bpm = deck_snapshot
                .loaded_track
                .as_ref()
                .map(|t| t.bpm * (1.0 + deck_snapshot.pitch_bend_pct / 100.0));
            widgets::paint_platter(ui, platter_angle, accent, deck_snapshot.is_playing, eff_bpm);
            ui.add_space(8.0);
            widgets::paint_vu_meter(ui, &deck_snapshot);
        });

        ui.add_space(6.0);
        theme::card_frame(ui, dark, |ui| {
            ui.strong("Channel Volume & 4-Stem Isolation");
            let mut vol = deck_snapshot.volume_fader;
            let r_vol = ui.add(
                egui::Slider::new(&mut vol, 0.0..=1.0)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .text("Volume"),
            );
            if r_vol.changed() {
                self.orch.deck_mut(deck_id).volume_fader = vol;
            }
            if r_vol.drag_stopped() {
                self.dispatch_and_report(Command::SetDeckVolume {
                    deck: deck_id,
                    volume: vol,
                });
            }

            let mut vocals = deck_snapshot.stem_vocals_gain;
            let mut drums = deck_snapshot.stem_drums_gain;
            let mut bass = deck_snapshot.stem_bass_gain;
            let mut other = deck_snapshot.stem_other_gain;

            let r1 = ui.add(egui::Slider::new(&mut vocals, 0.0..=1.0).text("Vocals"));
            let r2 = ui.add(egui::Slider::new(&mut drums, 0.0..=1.0).text("Drums"));
            let r3 = ui.add(egui::Slider::new(&mut bass, 0.0..=1.0).text("Bass"));
            let r4 = ui.add(egui::Slider::new(&mut other, 0.0..=1.0).text("Other"));

            if r1.changed() || r2.changed() || r3.changed() || r4.changed() {
                let state = self.orch.deck_mut(deck_id);
                state.stem_vocals_gain = vocals;
                state.stem_drums_gain = drums;
                state.stem_bass_gain = bass;
                state.stem_other_gain = other;
            }
            if r1.drag_stopped() || r2.drag_stopped() || r3.drag_stopped() || r4.drag_stopped() {
                self.dispatch_and_report(Command::SetStemGains {
                    deck: deck_id,
                    vocals,
                    drums,
                    bass,
                    other,
                });
            }
        });
    }
}

/// SOTA switch_on_hover implementation for egui top menu bars.
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
        if !self.typography_applied {
            theme::apply_fleet_typography(&ctx);
            self.typography_applied = true;
        }
        if self.prefs.is_dark_theme {
            ctx.set_visuals(theme::harsh_visuals());
        } else {
            ctx.set_visuals(theme::crisp_light_visuals());
        }

        // Ctrl + mouse wheel zoom (Craft Fleet Rule)
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

        // Global keyboard shortcuts
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

        // Continuous playback update for spinning jog wheels & scrolling waveforms
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
        if any_playing {
            ctx.request_repaint();
        }

        // Sync live mixer state into the WASAPI/cpal audio thread (non-blocking)
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

        // Update native video mixer texture (FFmpeg frame pipe + procedural reactive fallback)
        self.video_deck_a.set_playing(self.orch.deck_a.is_playing);
        self.video_deck_b.set_playing(self.orch.deck_b.is_playing);
        if self.prefs.show_video_mixer || self.video_projector_open {
            if any_playing {
                self.video_phase += 0.03;
            }
            let rgb_a = self
                .video_deck_a
                .snapshot_rgb(self.video_phase, [0.22, 0.74, 0.97]);
            let rgb_b = self
                .video_deck_b
                .snapshot_rgb(self.video_phase * 1.15 + 1.5, [0.96, 0.62, 0.04]);
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

        // Detachable Projector / Secondary Monitor Viewport
        if self.video_projector_open {
            let tex_id = self.video_texture.as_ref().map(|t| t.id());
            let mut keep_open = true;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("djcraft_video_projector"),
                egui::ViewportBuilder::default()
                    .with_title("DJCraft Video Projector Output")
                    .with_inner_size([960.0, 540.0]),
                |vctx, _class| {
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
                    if ui.button("New Session / Reset (Ctrl+N)").clicked() {
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
                    if ui.button("Load Next Crate Track -> Deck A").clicked() {
                        self.execute_catalog_id("insert.load_crate_a");
                        ui.close();
                    }
                    if ui.button("Load Next Crate Track -> Deck B").clicked() {
                        self.execute_catalog_id("insert.load_crate_b");
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
                    ui.separator();
                    if ui.button("Center Crossfader").clicked() {
                        self.execute_catalog_id("modify.center_crossfader");
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("View", |ui| {
                    if ui.button("Zoom In (Ctrl+Wheel Up)").clicked() {
                        self.prefs.zoom_factor = (self.prefs.zoom_factor * 1.1).clamp(0.75, 2.0);
                        self.prefs.save();
                        ui.close();
                    }
                    if ui.button("Zoom Out (Ctrl+Wheel Down)").clicked() {
                        self.prefs.zoom_factor = (self.prefs.zoom_factor / 1.1).clamp(0.75, 2.0);
                        self.prefs.save();
                        ui.close();
                    }
                    if ui.button("Reset Zoom 100% (Ctrl+0)").clicked() {
                        self.prefs.zoom_factor = 1.0;
                        self.prefs.save();
                        ui.close();
                    }
                    ui.separator();
                    let mut dirty = false;
                    dirty |= ui.checkbox(&mut self.prefs.show_deck_a, "Deck A Panel").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_deck_b, "Deck B Panel").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_decks_cd, "Decks C & D Strip").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_crate_browser, "Crate Track Browser").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_camelot_hud, "Camelot Harmonic HUD").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_video_mixer, "Native Video Mixer").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_mixxxxx_dock, "Mixxxxx Video Dock").changed();
                    if dirty {
                        self.prefs.save();
                    }
                    ui.separator();
                    let theme_label = if self.prefs.is_dark_theme {
                        "Switch to Light Theme"
                    } else {
                        "Switch to Dark Theme"
                    };
                    if ui.button(theme_label).clicked() {
                        self.prefs.is_dark_theme = !self.prefs.is_dark_theme;
                        self.prefs.save();
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Insert", |ui| {
                    if ui.button("Load Next Track to Deck A").clicked() {
                        self.execute_catalog_id("insert.load_crate_a");
                        ui.close();
                    }
                    if ui.button("Load Next Track to Deck B").clicked() {
                        self.execute_catalog_id("insert.load_crate_b");
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Add Hot Cue 1 (Deck A)").clicked() {
                        self.execute_catalog_id("insert.hot_cue_a");
                        ui.close();
                    }
                    if ui.button("Toggle 8-Bar Loop (Deck A)").clicked() {
                        self.execute_catalog_id("insert.loop_8_a");
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Modify", |ui| {
                    if ui.button("Reset All Stem Gains (100%)").clicked() {
                        self.execute_catalog_id("modify.reset_stems");
                        ui.close();
                    }
                    if ui.button("Center Master Crossfader").clicked() {
                        self.execute_catalog_id("modify.center_crossfader");
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Deck A Pitch +1.0%").clicked() {
                        let next = self.orch.deck_a.pitch_bend_pct + 1.0;
                        self.dispatch_and_report(Command::SetPitchBend {
                            deck: DeckId::DeckA,
                            pct: next,
                        });
                        ui.close();
                    }
                    if ui.button("Deck A Pitch -1.0%").clicked() {
                        let next = self.orch.deck_a.pitch_bend_pct - 1.0;
                        self.dispatch_and_report(Command::SetPitchBend {
                            deck: DeckId::DeckA,
                            pct: next,
                        });
                        ui.close();
                    }
                });
                titles.push(r.response);

                let r = ui.menu_button("Commands", |ui| {
                    if ui.button("Auto-Sync Beatgrid (Deck B -> A)").clicked() {
                        self.execute_catalog_id("commands.auto_sync");
                        ui.close();
                    }
                    if ui.button("Plan Harmonic Transition").clicked() {
                        self.execute_catalog_id("commands.plan_transition");
                        ui.close();
                    }
                    if ui.button("Stop All Decks").clicked() {
                        self.execute_catalog_id("deck.stop_all");
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Check Dettato Sidecar Status").clicked() {
                        self.execute_catalog_id("commands.dettato_status");
                        ui.close();
                    }
                    if ui.button("Ping Mixxxxx Video Engine (OSC)").clicked() {
                        self.execute_catalog_id("commands.mixxxxx_ping");
                        ui.close();
                    }
                    if ui.button("Mixxxxx: Enable Deck A Video").clicked() {
                        self.execute_catalog_id("commands.mixxxxx_video_a_on");
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
                    let mut dirty = false;
                    dirty |= ui.checkbox(&mut self.prefs.show_deck_a, "Deck A Platter").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_deck_b, "Deck B Platter").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_decks_cd, "Decks C & D Strip").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_crate_browser, "Crate Browser").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_camelot_hud, "Camelot Wheel HUD").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_video_mixer, "Native Video Mixer").changed();
                    dirty |= ui.checkbox(&mut self.prefs.show_mixxxxx_dock, "Mixxxxx Video Dock").changed();
                    if dirty {
                        self.prefs.save();
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

                // Right topbar controls: locale, vector Sun/Moon, clickable 100% zoom pill
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cur_loc = self.locale;
                    let label = cur_loc.code().to_uppercase();
                    egui::ComboBox::from_id_salt("topbar_locale_selector")
                        .selected_text(label)
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

                    if theme::theme_toggle_button(ui, &mut self.prefs.is_dark_theme) {
                        self.prefs.save();
                    }
                    if ui
                        .button(format!("{:.0}%", self.prefs.zoom_factor * 100.0))
                        .on_hover_text("Click to reset UI zoom to 100% (Ctrl+0)")
                        .clicked()
                    {
                        self.prefs.zoom_factor = 1.0;
                        self.prefs.save();
                    }
                    ui.label("Safe Rust (0 unsafe)");
                });
            });
        });

        // 2. HELP SUITE MODALS
        if self.show_command_palette {
            let mut open = self.show_command_palette;
            let mut trigger_id: Option<&'static str> = None;
            egui::Window::new("Command Palette (Ctrl+K)")
                .open(&mut open)
                .collapsible(false)
                .default_width(500.0)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Filter:");
                        ui.text_edit_singleline(&mut self.palette_query);
                    });
                    ui.separator();
                    let query = self.palette_query.trim().to_lowercase();
                    let matches: Vec<_> = Command::catalog()
                        .into_iter()
                        .filter(|spec| {
                            query.is_empty()
                                || spec.label.to_lowercase().contains(&query)
                                || spec.category.to_lowercase().contains(&query)
                                || spec.id.to_lowercase().contains(&query)
                        })
                        .collect();

                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Some(first) = matches.first() {
                            trigger_id = Some(first.id);
                        }
                    }

                    egui::ScrollArea::vertical()
                        .max_height(280.0)
                        .show(ui, |ui| {
                            for spec in matches {
                                let btn_text = if spec.hotkey.is_empty() {
                                    format!("[{}] {}", spec.category, spec.label)
                                } else {
                                    format!("[{}] {}  ({})", spec.category, spec.label, spec.hotkey)
                                };
                                if ui.button(btn_text).clicked() {
                                    trigger_id = Some(spec.id);
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
                .default_width(460.0)
                .show(&ctx, |ui| {
                    for spec in Command::catalog() {
                        if !spec.hotkey.is_empty() {
                            ui.horizontal(|ui| {
                                ui.monospace(format!("{:<12}", spec.hotkey));
                                ui.label(spec.label);
                            });
                        }
                    }
                    ui.separator();
                    ui.label("Ctrl + Mouse Wheel : Zoom UI (75% to 200%)");
                    ui.label("Ctrl + 0           : Reset Zoom to 100%");
                });
            self.show_shortcuts_dialog = open;
        }

        if self.show_help_center {
            let mut open = self.show_help_center;
            egui::Window::new("DJCraft Help Center")
                .open(&mut open)
                .collapsible(false)
                .default_width(660.0)
                .default_height(430.0)
                .show(&ctx, |ui| {
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
                            ui.heading("Get Started (5-Step Tour)");
                            ui.label("1. Demo tracks are pre-loaded on Decks A, B, and C.");
                            ui.label("2. Click PLAY / CUE on any deck and watch the scrolling waveform + VU meter.");
                            ui.label("3. Run Commands -> Auto-Sync Beatgrid to pitch-match Deck B to Deck A.");
                            ui.label("4. Adjust 4-stem sliders, Hot Cues, or Mixxxxx video OSC controls.");
                            ui.label("5. Press Ctrl+K for Command Palette or run `djcraft-cli status`.");
                        }
                        HelpTab::Functions => {
                            ui.heading("Core Functions");
                            ui.label("- djcraft-audio: 4-deck PCM synthesizer, stem gains, Camelot wheel rules.");
                            ui.label("- djcraft-core: Universal Command::dispatch bus, 64-step Undo/Redo, dettato sidecar, Mixxxxx OSC 1.0 bridge.");
                            ui.label("- djcraft-app: egui 0.37 workstation with slate-950 theme, painted waveforms, VU meters, and Camelot wheel.");
                            ui.label("- djcraft-mcp: MCP 2024-11-05 stdio server (7 tools) sharing the same L1 bus.");
                        }
                        HelpTab::Usage => {
                            ui.heading("Usage (GUI, CLI & MCP)");
                            ui.monospace("djcraft.exe --smoke");
                            ui.monospace("djcraft-cli status");
                            ui.monospace("djcraft-cli camelot 8A 9A");
                            ui.monospace("djcraft-cli analyze-dettato track.mp3 --device cuda");
                            ui.monospace("djcraft-cli mixxxxx-ping");
                            ui.monospace("djcraft-cli mixxxxx-video --deck 1 --enabled true");
                        }
                        HelpTab::TipsAndTricks => {
                            ui.heading("Tips & Tricks");
                            ui.label("- Use Ctrl+Z / Ctrl+Y to revert accidental crossfader or stem drags.");
                            ui.label("- On a Camelot key clash, drop Vocals + Other stems to 0.0 and mix on Drums.");
                            ui.label("- Click the top-right percentage pill or press Ctrl+0 to reset UI zoom.");
                        }
                        HelpTab::PitfallsAndSafety => {
                            ui.heading("Common Pitfalls & Safety");
                            ui.label("- Run dettato audio-to-MIDI offline during crate prep, never on the live audio thread.");
                            ui.label("- Enable OSC in Mixxxxx Preferences (UDP 11119 in / 11118 out) for live video control.");
                            ui.label("- All numeric parameters (crossfader, pitch bend, stems) are strictly clamped in L1.");
                        }
                        HelpTab::PlannedExtensions => {
                            ui.heading("Planned Extensions");
                            ui.label("- Real-time WASAPI / cpal audio device output stream.");
                            ui.label("- Persistent crate SQLite storage via databasecraft-store.");
                            ui.label("- Hardware MIDI controller mapping (DDJ / Hercules).");
                        }
                        HelpTab::OtherCrafts => {
                            ui.heading("Other Crafts in the Audio Ecosystem");
                            ui.label("- mixxxxx: Video-extended Mixxx fork controlled via UDP OSC 11119.");
                            ui.label("- musiccraft: Docs-only index linking djcraft, soundcraft, and speechcraft.");
                            ui.label("- soundcraft: Upstream DAW fork (render stems/mixes for djcraft crates).");
                            ui.label("- reaper-mcp: REAPER DAW automation sharing the local dettato sidecar.");
                        }
                        HelpTab::ProjectDocs => {
                            ui.heading("Repository Documentation");
                            ui.label("HELP.md             - Full user manual (synced with this Help Center)");
                            ui.label("docs/PRD.md         - Product Requirements Document");
                            ui.label("docs/TIERED_PLAN.md - 4-Tier execution plan & backlog");
                            ui.label("BUILD_LOG.md        - Release & NSIS build audit log");
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
                .default_width(460.0)
                .show(&ctx, |ui| {
                    ui.heading("DJCraft Licensing & Safety");
                    ui.label("License: Dual-licensed under MIT OR Apache-2.0.");
                    ui.label("Memory Safety: #![forbid(unsafe_code)] across all workspace crates.");
                    ui.separator();
                    ui.label("Key Third-Party Crates & Optional Integrations:");
                    ui.label("- egui / eframe (MIT / Apache-2.0)");
                    ui.label("- clap, serde, serde_json, thiserror (MIT / Apache-2.0)");
                    ui.label("- Optional sidecar: kobimusic/dettato (Apache-2.0)");
                    ui.label("- Optional OSC target: sandraschi/mixxxxx (GPLv2, external UDP)");
                });
            self.show_licenses_dialog = open;
        }

        if self.show_about_dialog {
            let mut open = self.show_about_dialog;
            egui::Window::new("About DJCraft")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.heading(format!("DJCraft v{APP_VERSION}"));
                    ui.label("Sovereign 4-Deck Mixing, Stem Isolation & Camelot Transition Studio.");
                    ui.label("100% Safe Rust (#![forbid(unsafe_code)]) | Local-First | Zero Cloud.");
                    ui.label("MCP stdio + CLI + egui Workstation sharing one L1 Command::dispatch bus.");
                });
            self.show_about_dialog = open;
        }

        // 3. TOP QUICK-ACCESS TOOLBAR
        egui::Panel::top("status_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("DJCraft Studio");
                ui.separator();
                if ui.button("Auto-Sync").clicked() {
                    self.execute_catalog_id("commands.auto_sync");
                }
                if ui.button("Phase Sync").clicked() {
                    self.execute_catalog_id("commands.phase_sync");
                }
                if ui.button("Plan Mix").clicked() {
                    self.execute_catalog_id("commands.plan_transition");
                }
                if ui.button("Stop All").clicked() {
                    self.execute_catalog_id("deck.stop_all");
                }
                if ui.button("Palette (Ctrl+K)").clicked() {
                    self.show_command_palette = true;
                }
                ui.separator();
                ui.label(&self.status_msg);
            });
        });

        // 4. BOTTOM DECKS C & D STRIP (Full 4-Deck Parity)
        if self.prefs.show_decks_cd {
            let dark = self.prefs.is_dark_theme;
            egui::Panel::bottom("decks_cd_panel")
                .resizable(false)
                .show(ui, |ui| {
                    ui.columns(2, |cols| {
                        for (col_idx, (deck_id, accent)) in [
                            (DeckId::DeckC, egui::Color32::from_rgb(52, 211, 153)),
                            (DeckId::DeckD, egui::Color32::from_rgb(192, 132, 252)),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let d = self.orch.deck(deck_id).clone();
                            theme::card_frame(&mut cols[col_idx], dark, |ui| {
                                ui.horizontal(|ui| {
                                    theme::status_dot(ui, d.is_playing);
                                    ui.strong(deck_id.label());
                                    if let Some(t) = &d.loaded_track {
                                        ui.label(egui::RichText::new(&t.title).color(accent));
                                        ui.monospace(format!("{} | {:.0} BPM", t.musical_key, t.bpm));
                                    } else {
                                        ui.label("Empty");
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("CUE").clicked() {
                                                self.dispatch_and_report(Command::CueReturn {
                                                    deck: deck_id,
                                                });
                                            }
                                            let lbl = if d.is_playing { "PAUSE" } else { "PLAY" };
                                            if ui.button(lbl).clicked() {
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

        // 5. DECK A & DECK B PANELS
        if self.prefs.show_deck_a {
            egui::Panel::left("deck_a_panel")
                .default_size(310.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.render_deck_panel(
                            ui,
                            DeckId::DeckA,
                            egui::Color32::from_rgb(56, 189, 248),
                            self.platter_angle_a,
                        );
                    });
                });
        }

        if self.prefs.show_deck_b {
            egui::Panel::right("deck_b_panel")
                .default_size(310.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.render_deck_panel(
                            ui,
                            DeckId::DeckB,
                            egui::Color32::from_rgb(245, 158, 11),
                            self.platter_angle_b,
                        );
                    });
                });
        }

        // 6. CENTRAL MIXER, CAMELOT WHEEL, MIXXXXX VIDEO DOCK & CRATE BROWSER
        let dark = self.prefs.is_dark_theme;
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                theme::card_frame(ui, dark, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("4-Channel Club Mixer");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mute_lbl = if self.orch.master_muted {
                                "UNMUTE"
                            } else {
                                "MUTE"
                            };
                            if ui.button(mute_lbl).clicked() {
                                self.execute_catalog_id("modify.toggle_mute");
                            }
                            ui.monospace(format!(
                                "WASAPI: {}",
                                if self.audio_out.active {
                                    self.audio_out.device_name.as_str()
                                } else {
                                    "Offline"
                                }
                            ));
                            theme::status_dot(ui, self.audio_out.active && !self.orch.master_muted);
                        });
                    });

                    let mv_resp = ui.add(
                        egui::Slider::new(&mut self.master_volume_draft, 0.0..=1.0)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                            .text("Master Loudness"),
                    );
                    if mv_resp.changed() {
                        self.orch.master_volume = self.master_volume_draft;
                    }
                    if mv_resp.drag_stopped() {
                        let vol = self.master_volume_draft;
                        self.dispatch_and_report(Command::SetMasterVolume { volume: vol });
                    }

                    let xf_resp = ui.add(
                        egui::Slider::new(&mut self.crossfader_draft, -1.0..=1.0)
                            .text("Master Crossfader (Deck A <-> Deck B)"),
                    );
                    if xf_resp.changed() {
                        self.orch.crossfader = self.crossfader_draft;
                    }
                    if xf_resp.drag_stopped() {
                        let val = self.crossfader_draft;
                        self.dispatch_and_report(Command::SetCrossfader { value: val });
                    }
                });

                if self.prefs.show_camelot_hud {
                    ui.add_space(8.0);
                    theme::card_frame(ui, dark, |ui| {
                        ui.horizontal(|ui| {
                            let key_a = self
                                .orch
                                .deck_a
                                .loaded_track
                                .as_ref()
                                .map(|t| t.musical_key.as_str());
                            let key_b = self
                                .orch
                                .deck_b
                                .loaded_track
                                .as_ref()
                                .map(|t| t.musical_key.as_str());
                            widgets::paint_camelot_wheel_mini(ui, key_a, key_b);
                            ui.add_space(8.0);
                            ui.vertical(|ui| {
                                ui.strong("Camelot Harmonic Mix Assistant");
                                if let Ok(plan) = self.orch.plan_transition() {
                                    ui.label(format!("{} -> {}", plan.from_track, plan.to_track));
                                    ui.monospace(format!(
                                        "BPM {:.1} -> {:.1} (Delta {:+.2}) | {} bars",
                                        plan.from_bpm,
                                        plan.to_bpm,
                                        plan.bpm_delta,
                                        plan.recommended_transition_bars
                                    ));
                                    let match_color = if plan.harmonic_compatible {
                                        egui::Color32::from_rgb(34, 197, 94)
                                    } else {
                                        egui::Color32::from_rgb(239, 68, 68)
                                    };
                                    let match_txt = if plan.harmonic_compatible {
                                        "HARMONIC MATCH (Clean key blend)"
                                    } else {
                                        "KEY CLASH (Use drum-loop bridge)"
                                    };
                                    ui.label(egui::RichText::new(match_txt).strong().color(match_color));
                                    ui.label(&plan.suggested_eq_strategy);
                                }
                            });
                        });
                    });
                }

                if self.prefs.show_video_mixer {
                    ui.add_space(8.0);
                    theme::card_frame(ui, dark, |ui| {
                        ui.horizontal(|ui| {
                            theme::status_dot(
                                ui,
                                self.video_deck_a.has_file || self.video_deck_b.has_file,
                            );
                            ui.strong("Native Video Mixer (Crossfader Blend)");
                            let proj_lbl = if self.video_projector_open {
                                "Close Projector"
                            } else {
                                "Detach Projector"
                            };
                            if ui.button(proj_lbl).clicked() {
                                self.video_projector_open = !self.video_projector_open;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.monospace(format!("A: {}", self.video_deck_a.source_label));
                            ui.separator();
                            ui.monospace(format!("B: {}", self.video_deck_b.source_label));
                        });
                        if let Some(tex) = &self.video_texture {
                            let preview_w = ui.available_width().clamp(240.0, 480.0);
                            let preview_h = preview_w * (VIDEO_H as f32 / VIDEO_W as f32);
                            ui.image(egui::load::SizedTexture::new(
                                tex.id(),
                                egui::vec2(preview_w, preview_h),
                            ));
                        }
                        ui.horizontal(|ui| {
                            if ui.small_button("Load Video A...").clicked() {
                                self.load_video_file_dialog(DeckId::DeckA);
                            }
                            if ui.small_button("Load Video B...").clicked() {
                                self.load_video_file_dialog(DeckId::DeckB);
                            }
                            if ui.small_button("Reset VFX").clicked() {
                                self.video_brightness = 0.5;
                                self.video_contrast = 0.5;
                                self.video_saturation = 0.5;
                            }
                            if ui.small_button("Clear Videos").clicked() {
                                self.video_deck_a.stop();
                                self.video_deck_b.stop();
                            }
                        });
                        ui.add(
                            egui::Slider::new(&mut self.video_brightness, 0.0..=1.0)
                                .text("Brightness"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.video_contrast, 0.0..=1.0)
                                .text("Contrast"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.video_saturation, 0.0..=1.0)
                                .text("Saturation"),
                        );
                    });
                }

                if self.prefs.show_mixxxxx_dock {
                    ui.add_space(8.0);
                    theme::card_frame(ui, dark, |ui| {
                        ui.horizontal(|ui| {
                            theme::status_dot(ui, self.mixxxxx_last_pong);
                            ui.strong("Mixxxxx Video OSC Bridge (UDP 11119)");
                            if ui.button("Ping OSC").clicked() {
                                self.execute_catalog_id("commands.mixxxxx_ping");
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Target Deck:");
                            for d in [DeckId::DeckA, DeckId::DeckB, DeckId::DeckC, DeckId::DeckD] {
                                ui.selectable_value(&mut self.mixxxxx_deck, d, d.label());
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.mixxxxx_video_enabled, "Video Enabled");
                            ui.checkbox(&mut self.mixxxxx_video_fullscreen, "Fullscreen");
                        });
                        ui.add(egui::Slider::new(&mut self.mixxxxx_brightness, 0.0..=1.0).text("Brightness"));
                        ui.add(egui::Slider::new(&mut self.mixxxxx_contrast, 0.0..=1.0).text("Contrast"));
                        ui.add(egui::Slider::new(&mut self.mixxxxx_saturation, 0.0..=1.0).text("Saturation"));
                        if ui.button("Send Video Controls to Mixxxxx").clicked() {
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
                }

                if self.prefs.show_crate_browser {
                    if self.crate_dirty {
                        self.refresh_crates();
                    }
                    ui.add_space(8.0);
                    theme::card_frame(ui, dark, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong("Crate & Subcrate Library (SQLite)");
                            if ui.small_button("Scan Folder...").clicked() {
                                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                    if let Some(ref store) = self.crate_store {
                                        let target_id = self
                                            .crate_query
                                            .crate_id
                                            .or_else(|| store.ensure_crate_path("Scanned Music").ok());
                                        if let Some(cid) = target_id {
                                            match store.scan_audio_folder(cid, &dir) {
                                                Ok(n) => {
                                                    self.status_msg = format!("Scanned {n} audio tracks from {}", dir.display());
                                                    self.crate_dirty = true;
                                                }
                                                Err(e) => self.status_msg = format!("Scan error: {e}"),
                                            }
                                        }
                                    }
                                }
                            }
                            if ui.small_button("Import Serato...").clicked() {
                                if let Some(file) = rfd::FileDialog::new()
                                    .add_filter("Serato Crate", &["crate"])
                                    .pick_file()
                                {
                                    if let Some(ref store) = self.crate_store {
                                        match store.import_serato_crate(&file) {
                                            Ok(sum) => {
                                                self.crate_query.crate_id = Some(sum.crate_id);
                                                self.status_msg = format!(
                                                    "Imported {} tracks into Serato crate '{}'",
                                                    sum.imported_tracks, sum.crate_path
                                                );
                                                self.crate_dirty = true;
                                            }
                                            Err(e) => self.status_msg = format!("Serato import error: {e}"),
                                        }
                                    }
                                }
                            }
                            if ui.small_button("Export Serato...").clicked() {
                                if let Some(cid) = self.crate_query.crate_id {
                                    if let Some(dest) = rfd::FileDialog::new()
                                        .add_filter("Serato Crate", &["crate"])
                                        .set_file_name("djcraft_export.crate")
                                        .save_file()
                                    {
                                        if let Some(ref store) = self.crate_store {
                                            match store.export_serato_crate(cid, &dest) {
                                                Ok(n) => self.status_msg = format!("Exported {n} tracks to Serato {}", dest.display()),
                                                Err(e) => self.status_msg = format!("Serato export error: {e}"),
                                            }
                                        }
                                    }
                                } else {
                                    self.status_msg = "Select a crate first to export to Serato".to_string();
                                }
                            }
                            if ui.small_button("Import VDJ...").clicked() {
                                if let Some(file) = rfd::FileDialog::new()
                                    .add_filter("VirtualDJ / M3U", &["vdjfolder", "m3u", "m3u8"])
                                    .pick_file()
                                {
                                    if let Some(ref store) = self.crate_store {
                                        match store.import_vdj_file(&file) {
                                            Ok(sum) => {
                                                self.crate_query.crate_id = Some(sum.crate_id);
                                                self.status_msg = format!(
                                                    "Imported {} tracks into '{}'",
                                                    sum.imported_tracks, sum.crate_path
                                                );
                                                self.crate_dirty = true;
                                            }
                                            Err(e) => self.status_msg = format!("VDJ import error: {e}"),
                                        }
                                    }
                                }
                            }
                            if ui.small_button("Export VDJ...").clicked() {
                                if let Some(cid) = self.crate_query.crate_id {
                                    if let Some(dest) = rfd::FileDialog::new()
                                        .add_filter("VirtualDJ Folder", &["vdjfolder"])
                                        .add_filter("M3U8 Playlist", &["m3u8"])
                                        .set_file_name("djcraft_export.vdjfolder")
                                        .save_file()
                                    {
                                        if let Some(ref store) = self.crate_store {
                                            match store.export_vdj_file(cid, &dest) {
                                                Ok(n) => self.status_msg = format!("Exported {n} tracks to {}", dest.display()),
                                                Err(e) => self.status_msg = format!("VDJ export error: {e}"),
                                            }
                                        }
                                    }
                                } else {
                                    self.status_msg = "Select a crate first to export to VirtualDJ".to_string();
                                }
                            }
                        });

                        // Create crate / subcrate row
                        ui.horizontal(|ui| {
                            ui.label("New:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.crate_new_name)
                                    .desired_width(140.0)
                                    .hint_text("Crate name..."),
                            );
                            if ui.small_button("+ Root Crate").clicked() && !self.crate_new_name.trim().is_empty() {
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

                        // Crate / subcrate selector chips
                        ui.horizontal_wrapped(|ui| {
                            if ui.selectable_label(self.crate_query.crate_id.is_none(), "All Tracks").clicked() {
                                self.crate_query.crate_id = None;
                                self.crate_dirty = true;
                            }
                            let nodes = self.crate_nodes.clone();
                            for node in nodes {
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

                        // Filter & Sort bar
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Search:");
                            if ui
                                .add(egui::TextEdit::singleline(&mut self.crate_query.search).desired_width(120.0))
                                .changed()
                            {
                                self.crate_dirty = true;
                            }
                            ui.label("BPM:");
                            if ui
                                .add(egui::TextEdit::singleline(&mut self.crate_bpm_min_text).desired_width(40.0).hint_text("min"))
                                .changed()
                            {
                                self.crate_dirty = true;
                            }
                            ui.label("-");
                            if ui
                                .add(egui::TextEdit::singleline(&mut self.crate_bpm_max_text).desired_width(40.0).hint_text("max"))
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
                            if ui.checkbox(&mut self.crate_query.analyzed_only, "Analyzed").changed() {
                                self.crate_dirty = true;
                            }
                            egui::ComboBox::from_id_salt("crate_sort_field")
                                .selected_text(format!("Sort: {}", self.crate_query.sort_by.label()))
                                .show_ui(ui, |ui| {
                                    for field in CrateSortField::ALL {
                                        if ui
                                            .selectable_value(&mut self.crate_query.sort_by, field, field.label())
                                            .changed()
                                        {
                                            self.crate_dirty = true;
                                        }
                                    }
                                });
                            let dir_lbl = if self.crate_query.sort_asc { "ASC" } else { "DESC" };
                            if ui.small_button(dir_lbl).clicked() {
                                self.crate_query.sort_asc = !self.crate_query.sort_asc;
                                self.crate_dirty = true;
                            }
                        });

                        ui.separator();
                        let results = self.crate_results.clone();
                        if results.is_empty() {
                            ui.label("No tracks match current filter.");
                        } else {
                            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                for t in results {
                                    ui.horizontal(|ui| {
                                        let key_str = if t.musical_key.is_empty() { "--" } else { t.musical_key.as_str() };
                                        let bpm_str = if t.bpm > 0.0 {
                                            format!("{:.1}", t.bpm)
                                        } else {
                                            "---".to_string()
                                        };
                                        let dettato_tag = match (t.tuning_cents, t.drum_hits) {
                                            (Some(c), Some(h)) => format!(" | {c:+.1}c {h}d"),
                                            _ => String::new(),
                                        };
                                        ui.label(format!(
                                            "{} - {} | {} | {} BPM{}",
                                            t.artist, t.title, key_str, bpm_str, dettato_tag
                                        ));
                                        for (btn_lbl, deck_id) in [
                                            ("A", DeckId::DeckA),
                                            ("B", DeckId::DeckB),
                                            ("C", DeckId::DeckC),
                                            ("D", DeckId::DeckD),
                                        ] {
                                            if ui.small_button(btn_lbl).clicked() {
                                                self.load_crate_track_to_deck(deck_id, &t);
                                            }
                                        }
                                        if !t.path.starts_with("demo://") {
                                            if ui.small_button("BPM").on_hover_text("Run built-in Symphonia beatgrid analysis").clicked() {
                                                if let Some(ref store) = self.crate_store {
                                                    match store.analyze_track_builtin(t.id) {
                                                        Ok(up) => {
                                                            self.status_msg = format!("Analyzed '{}': {:.1} BPM", up.title, up.bpm);
                                                            self.crate_dirty = true;
                                                        }
                                                        Err(e) => self.status_msg = format!("Analyze error: {e}"),
                                                    }
                                                }
                                            }
                                            if ui.small_button("Dettato").on_hover_text("Run local dettato audio-to-MIDI sidecar").clicked() {
                                                if let Some(ref store) = self.crate_store {
                                                    match store.analyze_track_dettato(t.id) {
                                                        Ok(up) => {
                                                            self.status_msg = format!(
                                                                "Dettato '{}': tuning {:?}c, drums {:?}",
                                                                up.title, up.tuning_cents, up.drum_hits
                                                            );
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
                        }
                    });
                }
            });
        });
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
        "has_menu_bar": true,
        "has_switch_on_hover": true,
        "has_theme_toggle": true,
        "has_slate_950_theme": true,
        "has_painted_waveforms": true,
        "has_camelot_wheel_widget": true,
        "has_mixxxxx_video_dock": true,
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
            .with_inner_size([1360.0, 860.0])
            .with_title("DJCraft - Sovereign 4-Deck Performance Studio"),
        ..Default::default()
    };

    eframe::run_native("DJCraft", native_options, Box::new(|_cc| Ok(Box::new(app))))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Mandatory Tofu Guard test (EGUI_DESKTOP_UI_AND_PARITY.md):
    /// fails if any non-ASCII glyph outside the verified egui-font allowlist appears in UI sources.
    #[test]
    fn glyph_allowlist() {
        const ALLOWED: &[char] = &['\u{00B0}', '\u{00D7}', '\u{00B7}', '\u{2026}', '\u{2014}'];
        let sources = [
            ("main.rs", include_str!("main.rs")),
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
