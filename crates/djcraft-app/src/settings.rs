#![forbid(unsafe_code)]
//! Unified 5-tab Settings Modal (`Ctrl+,`) and Toggleable Crate-Aware Booth Chat Drawer (`Ctrl+L`).

use djcraft_audio::output::AudioOutput;
use djcraft_audio::{
    list_input_devices, AudioEngine, DeckId, DvsCarrier, DvsInputStream, RecordingFormat,
};
use djcraft_core::{
    chat_with_booth_ai, parse_natural_language_command, probe_fleet_health, Command,
    ControllerProfileId,
};

use super::skins::{BackdropStyle, SkinId, SkinPalette};
use super::{default_recording_path, DjCraftApp, WorkspaceMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SettingsTab {
    #[default]
    Audio,
    DecksAndBooth,
    MidiAndDvs,
    AutoDjAndRecording,
    AiAndBridges,
}

impl SettingsTab {
    pub(crate) const ALL: [Self; 5] = [
        Self::Audio,
        Self::DecksAndBooth,
        Self::MidiAndDvs,
        Self::AutoDjAndRecording,
        Self::AiAndBridges,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::DecksAndBooth => "Decks & Booth",
            Self::MidiAndDvs => "MIDI & DVS",
            Self::AutoDjAndRecording => "Auto-DJ & Rec",
            Self::AiAndBridges => "AI & Bridges",
        }
    }
}

impl DjCraftApp {
    pub(crate) fn render_settings_modal(&mut self, ctx: &egui::Context, palette: &SkinPalette) {
        if !self.show_settings {
            return;
        }
        let mut open = self.show_settings;
        egui::Window::new("DJCraft Settings (Ctrl+,)")
            .open(&mut open)
            .resizable(true)
            .default_width(580.0)
            .default_height(420.0)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for tab in SettingsTab::ALL {
                        if ui
                            .selectable_label(self.settings_tab == tab, tab.label())
                            .clicked()
                        {
                            self.settings_tab = tab;
                        }
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| match self.settings_tab {
                    SettingsTab::Audio => {
                        ui.strong(
                            egui::RichText::new("Audio Output & Headphone Pre-Cue")
                                .color(palette.text_primary),
                        );
                        ui.monospace(if self.audio_out.active {
                            format!("Main Output: {}", self.audio_out.device_name)
                        } else {
                            "Main Output: Offline".to_string()
                        });
                        ui.horizontal(|ui| {
                            ui.label("Headphone PFL Device:");
                            let hp_lbl = self
                                .selected_headphone_device
                                .as_deref()
                                .unwrap_or("Off / Split Cue");
                            egui::ComboBox::from_id_salt("settings_hp_dev")
                                .selected_text(hp_lbl)
                                .width(240.0)
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            self.selected_headphone_device.is_none(),
                                            "Off / Split Cue",
                                        )
                                        .clicked()
                                    {
                                        self.selected_headphone_device = None;
                                        let _ = self.audio_out.set_headphone_device(None);
                                    }
                                    for dev in self.audio_output_devices.clone() {
                                        let sel = self.selected_headphone_device.as_deref()
                                            == Some(dev.as_str());
                                        if ui.selectable_label(sel, &dev).clicked() {
                                            match self.audio_out.set_headphone_device(Some(&dev)) {
                                                Ok(name) => {
                                                    self.selected_headphone_device = Some(name);
                                                }
                                                Err(e) => {
                                                    self.status_msg = format!("Headphone error: {e}");
                                                }
                                            }
                                        }
                                    }
                                });
                            if ui.small_button("Rescan").clicked() {
                                self.audio_output_devices = AudioOutput::list_output_devices();
                            }
                        });
                        let mut changed = false;
                        changed |= ui
                            .checkbox(
                                &mut self.prefs.auto_gain_enabled,
                                "Auto-Gain (LUFS-style level normalization on track load)",
                            )
                            .changed();
                        if ui
                            .checkbox(
                                &mut self.orch.headphone_split_cue,
                                "Split Mono Cue (Left = PFL Cue, Right = Master)",
                            )
                            .changed()
                        {
                            self.status_msg = format!(
                                "Split Cue {}",
                                if self.orch.headphone_split_cue {
                                    "ON"
                                } else {
                                    "OFF"
                                }
                            );
                        }
                        if changed {
                            self.prefs.save();
                        }
                    }
                    SettingsTab::DecksAndBooth => {
                        ui.strong(
                            egui::RichText::new("Decks, Pitch Range & Appearance")
                                .color(palette.text_primary),
                        );
                        ui.horizontal(|ui| {
                            ui.label("Workspace Mode:");
                            for (mode, lbl) in [
                                (WorkspaceMode::LiveConsole, "Live Console"),
                                (WorkspaceMode::BoothAndCrate, "Booth + Crate"),
                                (WorkspaceMode::CratesGigPrep, "Crates & Gig Prep"),
                            ] {
                                if ui
                                    .selectable_label(self.workspace_mode == mode, lbl)
                                    .clicked()
                                {
                                    self.workspace_mode = mode;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Pitch Fader Range:");
                            for pct in [8.0_f64, 16.0, 50.0] {
                                if ui
                                    .selectable_label(
                                        (self.prefs.pitch_range_pct - pct).abs() < 0.1,
                                        format!("+-{pct:.0}%"),
                                    )
                                    .clicked()
                                {
                                    self.prefs.pitch_range_pct = pct;
                                    self.orch.deck_a.pitch_range_pct = pct;
                                    self.orch.deck_b.pitch_range_pct = pct;
                                    self.orch.deck_c.pitch_range_pct = pct;
                                    self.orch.deck_d.pitch_range_pct = pct;
                                    self.prefs.save();
                                }
                            }
                        });
                        let mut changed = false;
                        if ui
                            .checkbox(
                                &mut self.prefs.quantize_default,
                                "Quantize hot cues to beatgrid",
                            )
                            .changed()
                        {
                            self.orch.deck_a.quantize = self.prefs.quantize_default;
                            self.orch.deck_b.quantize = self.prefs.quantize_default;
                            changed = true;
                        }
                        ui.horizontal(|ui| {
                            ui.label("Skin:");
                            egui::ComboBox::from_id_salt("settings_skin")
                                .selected_text(self.prefs.skin.label())
                                .show_ui(ui, |ui| {
                                    for sk in SkinId::ALL {
                                        if ui
                                            .selectable_label(self.prefs.skin == sk, sk.label())
                                            .clicked()
                                        {
                                            self.set_skin(sk);
                                        }
                                    }
                                });
                            ui.label("Backdrop:");
                            egui::ComboBox::from_id_salt("settings_backdrop")
                                .selected_text(self.prefs.backdrop.label())
                                .show_ui(ui, |ui| {
                                    for bg in BackdropStyle::ALL {
                                        if ui
                                            .selectable_label(self.prefs.backdrop == bg, bg.label())
                                            .clicked()
                                        {
                                            self.prefs.backdrop = bg;
                                            changed = true;
                                        }
                                    }
                                });
                        });
                        if changed {
                            self.prefs.save();
                        }
                    }
                    SettingsTab::MidiAndDvs => {
                        ui.strong(
                            egui::RichText::new("MIDI Controller & DVS Timecode Vinyl")
                                .color(palette.text_primary),
                        );
                        ui.horizontal(|ui| {
                            ui.label("MIDI Profile:");
                            egui::ComboBox::from_id_salt("settings_midi_profile")
                                .selected_text(self.midi.active_profile.label())
                                .width(260.0)
                                .show_ui(ui, |ui| {
                                    for p in ControllerProfileId::ALL {
                                        if ui
                                            .selectable_label(
                                                self.midi.active_profile == p,
                                                p.label(),
                                            )
                                            .clicked()
                                        {
                                            self.midi.set_profile(p);
                                        }
                                    }
                                });
                        });
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.label("DVS Input:");
                            let dev_lbl = self
                                .dvs_selected_device
                                .as_deref()
                                .unwrap_or("Default Input");
                            egui::ComboBox::from_id_salt("settings_dvs_dev")
                                .selected_text(dev_lbl)
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.dvs_selected_device,
                                        None,
                                        "Default Input",
                                    );
                                    for dev in &self.dvs_devices {
                                        ui.selectable_value(
                                            &mut self.dvs_selected_device,
                                            Some(dev.clone()),
                                            dev,
                                        );
                                    }
                                });
                            if ui.small_button("Rescan").clicked() {
                                self.dvs_devices = list_input_devices();
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Carrier:");
                            for c in DvsCarrier::ALL {
                                ui.selectable_value(&mut self.dvs_carrier, c, c.label());
                            }
                            let dvs_on = self.dvs_input.is_some();
                            if ui
                                .button(if dvs_on { "Stop DVS" } else { "Start DVS" })
                                .clicked()
                            {
                                if dvs_on {
                                    self.dvs_input = None;
                                } else if let Ok(s) = DvsInputStream::start(
                                    self.dvs_selected_device.as_deref(),
                                    self.dvs_carrier,
                                ) {
                                    self.dvs_input = Some(s);
                                }
                            }
                        });
                    }
                    SettingsTab::AutoDjAndRecording => {
                        ui.strong(
                            egui::RichText::new("Auto-DJ Automix & Mix Recording")
                                .color(palette.text_primary),
                        );
                        ui.checkbox(
                            &mut self.auto_dj_enabled,
                            "Enable Auto-DJ (auto-load & crossfade next track from active crate)",
                        );
                        ui.horizontal(|ui| {
                            ui.label("Auto-DJ Crossfade Duration (s):");
                            if ui
                                .add(
                                    egui::Slider::new(&mut self.prefs.auto_dj_fade_sec, 2.0..=20.0)
                                        .suffix("s"),
                                )
                                .changed()
                            {
                                self.prefs.save();
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Recording Format:");
                            ui.selectable_value(
                                &mut self.recording_format,
                                RecordingFormat::Wav,
                                "WAV (16-bit PCM)",
                            );
                            ui.selectable_value(
                                &mut self.recording_format,
                                RecordingFormat::Flac,
                                "FLAC (via ffmpeg)",
                            );
                        });
                        ui.monospace(format!(
                            "Default Recording Folder: {}",
                            default_recording_path(self.recording_format)
                                .parent()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default()
                        ));
                    }
                    SettingsTab::AiAndBridges => {
                        ui.strong(
                            egui::RichText::new("AI Booth Chat & Optional External Bridges")
                                .color(palette.text_primary),
                        );
                        ui.add_space(4.0);
                        self.render_ai_provider_controls(ui, palette, "settings");
                        ui.separator();
                        if ui
                            .checkbox(
                                &mut self.fleet_endpoints.enabled,
                                "Enable optional local external bridges (Speech/SFX/VFX/Visualizer)",
                            )
                            .changed()
                            && self.fleet_endpoints.enabled
                        {
                            self.fleet_health = probe_fleet_health(&self.fleet_endpoints);
                        }
                    }
                });
            });
        self.show_settings = open;
    }

    pub(crate) fn render_chat_drawer(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        if !self.show_chat_drawer {
            return;
        }
        egui::Panel::right("booth_chat_drawer")
            .resizable(true)
            .default_size(310.0)
            .min_size(250.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(
                        egui::RichText::new("Booth AI & Crate Assistant")
                            .color(palette.text_primary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").clicked() {
                            self.show_chat_drawer = false;
                        }
                    });
                });
                ui.separator();
                ui.collapsing(
                    format!(
                        "Provider: {} | {}",
                        self.ai_cfg.provider.label(),
                        self.ai_cfg.model
                    ),
                    |ui| {
                        self.render_ai_provider_controls(ui, palette, "drawer");
                    },
                );
                ui.separator();
                let deck_a_key = self
                    .orch
                    .deck_a
                    .loaded_track
                    .as_ref()
                    .map(|t| (t.title.clone(), t.bpm, t.musical_key.clone()));
                if let Some((title, bpm, key)) = &deck_a_key {
                    ui.monospace(format!("Deck A: {title} ({key} | {bpm:.1} BPM)"));
                    let compat: Vec<_> = self
                        .crate_results
                        .iter()
                        .filter(|t| {
                            AudioEngine::is_camelot_compatible(key, &t.musical_key)
                                && t.title != *title
                        })
                        .take(3)
                        .cloned()
                        .collect();
                    if !compat.is_empty() {
                        ui.label(
                            egui::RichText::new("Harmonic Matches in Crate:")
                                .small()
                                .color(palette.text_secondary),
                        );
                        for tr in compat {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!(
                                            "{} ({} | {:.0})",
                                            tr.title, tr.musical_key, tr.bpm
                                        ))
                                        .small(),
                                    )
                                    .truncate(),
                                );
                                if ui.small_button("-> B").clicked() {
                                    self.load_crate_track_to_deck(DeckId::DeckB, &tr);
                                }
                            });
                        }
                        ui.separator();
                    }
                }

                // Chat History
                let avail_h = (ui.available_height() - 62.0).max(120.0);
                egui::ScrollArea::vertical()
                    .max_height(avail_h)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if let Ok(log) = self.ai_log.lock() {
                            for (who, msg) in log.iter() {
                                let col = if who == "DJ" {
                                    palette.deck_a
                                } else {
                                    palette.deck_c
                                };
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong(egui::RichText::new(format!("{who}:")).color(col));
                                    ui.label(msg);
                                });
                                ui.add_space(3.0);
                            }
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.ai_input)
                            .desired_width(ui.available_width() - 56.0)
                            .hint_text("Ask or command (sync, auto dj, load compatible)..."),
                    );
                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.small_button("Send").clicked() || enter)
                        && !self.ai_input.trim().is_empty()
                    {
                        let prompt = self.ai_input.trim().to_string();
                        self.ai_input.clear();
                        self.handle_booth_chat_prompt(prompt);
                    }
                });
            });
    }

    fn handle_booth_chat_prompt(&mut self, prompt: String) {
        if let Ok(mut log) = self.ai_log.lock() {
            log.push(("DJ".to_string(), prompt.clone()));
        }
        let lc = prompt.to_lowercase();
        if lc.contains("auto dj on") || lc.contains("enable auto dj") {
            self.auto_dj_enabled = true;
            if let Ok(mut log) = self.ai_log.lock() {
                log.push(("AI".to_string(), "Auto-DJ enabled.".to_string()));
            }
            return;
        }
        if lc.contains("auto dj off") || lc.contains("disable auto dj") {
            self.auto_dj_enabled = false;
            if let Ok(mut log) = self.ai_log.lock() {
                log.push(("AI".to_string(), "Auto-DJ disabled.".to_string()));
            }
            return;
        }
        if lc.contains("load compatible") || lc.contains("next harmonic") {
            if let Some(t) = &self.orch.deck_a.loaded_track {
                let key = t.musical_key.clone();
                let title = t.title.clone();
                if let Some(match_tr) = self
                    .crate_results
                    .iter()
                    .find(|c| {
                        AudioEngine::is_camelot_compatible(&key, &c.musical_key) && c.title != title
                    })
                    .cloned()
                {
                    let msg = format!(
                        "Loaded '{}' ({} | {:.1} BPM) to Deck B.",
                        match_tr.title, match_tr.musical_key, match_tr.bpm
                    );
                    self.load_crate_track_to_deck(DeckId::DeckB, &match_tr);
                    if let Ok(mut log) = self.ai_log.lock() {
                        log.push(("AI".to_string(), msg));
                    }
                    return;
                }
            }
        }
        if let Some(cmd) = parse_natural_language_command(&prompt) {
            self.dispatch_and_report(cmd);
            if let Ok(mut log) = self.ai_log.lock() {
                log.push(("AI".to_string(), self.status_msg.clone()));
            }
            return;
        }
        // Background LLM call with live booth & crate context
        let cfg = self.ai_cfg.clone();
        let a_summary = self
            .orch
            .deck_a
            .loaded_track
            .as_ref()
            .map(|t| format!("{} ({} {:.1} BPM)", t.title, t.musical_key, t.bpm))
            .unwrap_or_else(|| "Empty".to_string());
        let b_summary = self
            .orch
            .deck_b
            .loaded_track
            .as_ref()
            .map(|t| format!("{} ({} {:.1} BPM)", t.title, t.musical_key, t.bpm))
            .unwrap_or_else(|| "Empty".to_string());
        let crate_ctx: Vec<String> = self
            .crate_results
            .iter()
            .take(8)
            .map(|t| format!("{} ({} {:.0} BPM)", t.title, t.musical_key, t.bpm))
            .collect();
        let booth_ctx = format!(
            "Deck A: {a_summary}. Deck B: {b_summary}. Crate tracks: [{}].",
            crate_ctx.join(", ")
        );
        let log_arc = std::sync::Arc::clone(&self.ai_log);
        let _ = Command::Status;
        std::thread::spawn(move || {
            let reply = match chat_with_booth_ai(&cfg, &prompt, &booth_ctx) {
                Ok(r) => r,
                Err(e) => format!("Offline / LLM not configured ({e}). Try 'sync', 'load compatible', or configure LLM in Settings -> AI & Bridges."),
            };
            if let Ok(mut log) = log_arc.lock() {
                log.push(("AI".to_string(), reply));
            }
        });
    }
}
