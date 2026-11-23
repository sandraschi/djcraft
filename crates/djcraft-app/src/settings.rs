#![forbid(unsafe_code)]
//! Full-Window 2-Pane Settings Workspace (`WorkspaceMode::Settings` / `Ctrl+,`)
//! and Toggleable Crate-Aware Booth Chat Drawer (`Ctrl+L`).

use djcraft_audio::output::AudioOutput;
use djcraft_audio::{
    list_input_devices, AudioEngine, DeckId, DvsCarrier, DvsInputStream, RecordingFormat,
};
use djcraft_core::i18n::AppLocale;
use djcraft_core::{
    chat_with_booth_ai, parse_natural_language_command, probe_fleet_health, Command,
    ControllerProfileId,
};

use super::skins::{BackdropStyle, CardDecorStyle, SkinId, SkinPalette};
use super::theme;
use super::widgets::WaveformZoom;
use super::{default_recording_path, BoothLayoutPreset, DjCraftApp, LowerDockTab};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SettingsTab {
    #[default]
    Appearance,
    Layouts,
    Audio,
    DecksAndBeatgrid,
    MidiAndDvs,
    AutoDjAndRecording,
    AiAndBridges,
}

impl SettingsTab {
    pub(crate) const ALL: [Self; 7] = [
        Self::Appearance,
        Self::Layouts,
        Self::Audio,
        Self::DecksAndBeatgrid,
        Self::MidiAndDvs,
        Self::AutoDjAndRecording,
        Self::AiAndBridges,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance & Skins",
            Self::Layouts => "Booth Layouts",
            Self::Audio => "Audio & Headphone Cue",
            Self::DecksAndBeatgrid => "Decks & Beatgrid",
            Self::MidiAndDvs => "MIDI & DVS Timecode",
            Self::AutoDjAndRecording => "Auto-DJ & Recording",
            Self::AiAndBridges => "AI & External Bridges",
        }
    }
}

impl DjCraftApp {
    pub(crate) fn render_settings_workspace(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        egui::Panel::left("settings_nav_sidebar")
            .resizable(true)
            .default_size(220.0)
            .min_size(180.0)
            .max_size(300.0)
            .show(ui, |ui| {
                theme::card_frame(ui, palette, |ui| {
                    ui.heading(egui::RichText::new("DJCraft Settings").color(palette.text_primary));
                    ui.label(
                        egui::RichText::new("Changes apply live & persist to ui.json")
                            .small()
                            .color(palette.text_secondary),
                    );
                    ui.add_space(6.0);
                    if ui
                        .add_sized(
                            [ui.available_width(), 28.0],
                            egui::Button::new("<- Back to Booth (Esc)"),
                        )
                        .clicked()
                    {
                        self.workspace_mode = self.prev_workspace_mode;
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    for tab in SettingsTab::ALL {
                        let selected = self.settings_tab == tab;
                        let btn =
                            egui::Button::new(egui::RichText::new(tab.label()).strong().color(
                                if selected {
                                    egui::Color32::BLACK
                                } else {
                                    palette.text_primary
                                },
                            ))
                            .min_size(egui::vec2(ui.available_width(), 30.0))
                            .fill(if selected {
                                palette.deck_a
                            } else {
                                palette.chassis
                            });
                        if ui.add(btn).clicked() {
                            self.settings_tab = tab;
                        }
                        ui.add_space(3.0);
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(6.0);
                    if ui
                        .add_sized(
                            [ui.available_width(), 26.0],
                            egui::Button::new("Reset Visual Defaults"),
                        )
                        .clicked()
                    {
                        self.prefs.card_decor = self.prefs.skin.recommended_decor();
                        self.prefs.card_border_width = 1.8;
                        self.prefs.card_opacity = 0.88;
                        self.prefs.color_boost = 0.25;
                        self.prefs.backdrop_intensity = 1.25;
                        self.prefs.zoom_factor = 1.0;
                        self.prefs.save();
                        self.status_msg = "Visual defaults restored".to_string();
                    }
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut prefs_changed = false;

                match self.settings_tab {
                    SettingsTab::Appearance => {
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Skins & Theme Presets");
                            ui.label(
                                egui::RichText::new("Pick a skin to apply its palette, recommended card frame, and matching background pattern.")
                                    .small()
                                    .color(palette.text_secondary),
                            );
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                for sk in SkinId::ALL {
                                    let p = sk.palette();
                                    let sel = self.prefs.skin == sk;
                                    let btn = egui::Button::new(
                                        egui::RichText::new(sk.label())
                                            .strong()
                                            .color(p.text_primary),
                                    )
                                    .min_size(egui::vec2(175.0, 34.0))
                                    .fill(p.card_fill)
                                    .stroke(egui::Stroke::new(
                                        if sel { 3.0 } else { 1.4 },
                                        if sel { p.deck_a } else { p.card_border },
                                    ));
                                    if ui.add(btn).clicked() {
                                        self.set_skin(sk);
                                        self.prefs.card_decor = sk.recommended_decor();
                                        self.prefs.backdrop = sk.recommended_backdrop();
                                        prefs_changed = true;
                                    }
                                }
                            });
                        });

                        ui.add_space(8.0);
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Card Decorations, Borders & Color Punch");
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Card Decoration:");
                                for style in CardDecorStyle::ALL {
                                    if ui
                                        .selectable_label(self.prefs.card_decor == style, style.label())
                                        .clicked()
                                    {
                                        self.prefs.card_decor = style;
                                        prefs_changed = true;
                                    }
                                }
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Border Thickness:");
                                prefs_changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.prefs.card_border_width, 1.0..=3.8)
                                            .suffix(" px"),
                                    )
                                    .changed();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Card Opacity (lower = more background shows through):");
                                prefs_changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.prefs.card_opacity, 0.52..=1.0)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    )
                                    .changed();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Color Richness / Accent Boost:");
                                prefs_changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.prefs.color_boost, 0.0..=1.0)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    )
                                    .changed();
                            });
                        });

                        ui.add_space(8.0);
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Background Pattern & Visibility");
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Pattern:");
                                for bg in BackdropStyle::ALL {
                                    if ui
                                        .selectable_label(self.prefs.backdrop == bg, bg.label())
                                        .clicked()
                                    {
                                        self.prefs.backdrop = bg;
                                        prefs_changed = true;
                                    }
                                }
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Pattern Intensity (boldness):");
                                prefs_changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.prefs.backdrop_intensity, 0.2..=2.0)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    )
                                    .changed();
                            });
                        });

                        ui.add_space(8.0);
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Scale & Language");
                            ui.horizontal(|ui| {
                                ui.label("UI Zoom:");
                                prefs_changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.prefs.zoom_factor, 0.75..=1.50)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    )
                                    .changed();
                            });
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Language:");
                                for loc in AppLocale::all() {
                                    if ui
                                        .selectable_label(self.locale == *loc, loc.short_label())
                                        .clicked()
                                    {
                                        self.locale = *loc;
                                        self.prefs.locale = *loc;
                                        prefs_changed = true;
                                    }
                                }
                            });
                        });
                    }

                    SettingsTab::Layouts => {
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Serato / VirtualDJ Favorite Booth Layouts");
                            ui.label(
                                egui::RichText::new("One-click layout presets configure decks, waveform orientation, performance strips, and active dock.")
                                    .small()
                                    .color(palette.text_secondary),
                            );
                            ui.add_space(6.0);
                            for preset in BoothLayoutPreset::ALL {
                                ui.horizontal(|ui| {
                                    let btn = egui::Button::new(
                                        egui::RichText::new(preset.label()).strong(),
                                    )
                                    .min_size(egui::vec2(210.0, 30.0));
                                    if ui.add(btn).clicked() {
                                        self.apply_layout_preset(preset);
                                        self.workspace_mode = self.prev_workspace_mode;
                                    }
                                    ui.label(
                                        egui::RichText::new(preset.description())
                                            .color(palette.text_secondary),
                                    );
                                });
                                ui.add_space(3.0);
                            }
                        });

                        ui.add_space(8.0);
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Fine-Grained Panel Visibility");
                            prefs_changed |= ui
                                .checkbox(&mut self.prefs.four_deck_layout, "4-Deck Full Layout (A+C Left, B+D Right)")
                                .changed();
                            prefs_changed |= ui
                                .checkbox(&mut self.prefs.vertical_waveforms, "Vertical Waveforms (between Deck A & B)")
                                .changed();
                            prefs_changed |= ui
                                .checkbox(&mut self.prefs.show_parallel_waveforms, "Show Parallel Waveforms Banner")
                                .changed();
                            prefs_changed |= ui
                                .checkbox(&mut self.prefs.show_decks_cd, "Show Decks C & D Quick Bar")
                                .changed();
                            prefs_changed |= ui
                                .checkbox(&mut self.prefs.show_live_sampler_strip, "Show Collapsible Live Sampler Strip")
                                .changed();
                            prefs_changed |= ui
                                .checkbox(&mut self.prefs.show_live_video_strip, "Show Collapsible 3-Monitor Live Video Strip")
                                .changed();
                            prefs_changed |= ui
                                .checkbox(&mut self.prefs.show_deck_a, "Show Left Deck Panel (A/C)")
                                .changed();
                            prefs_changed |= ui
                                .checkbox(&mut self.prefs.show_deck_b, "Show Right Deck Panel (B/D)")
                                .changed();
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Active Lower Dock:");
                                egui::ComboBox::from_id_salt("settings_lower_dock")
                                    .selected_text(self.lower_tab.label())
                                    .width(240.0)
                                    .show_ui(ui, |ui| {
                                        for tab in LowerDockTab::ALL {
                                            ui.selectable_value(&mut self.lower_tab, tab, tab.label());
                                        }
                                    });
                            });
                        });
                    }

                    SettingsTab::Audio => {
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Audio Output & Headphone Pre-Cue");
                            ui.monospace(if self.audio_out.active {
                                format!("Main Output: {}", self.audio_out.device_name)
                            } else {
                                "Main Output: Offline".to_string()
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Headphone PFL Device:");
                                let hp_lbl = self
                                    .selected_headphone_device
                                    .as_deref()
                                    .unwrap_or("Off / Split Cue");
                                egui::ComboBox::from_id_salt("settings_hp_dev")
                                    .selected_text(hp_lbl)
                                    .width(260.0)
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
                            prefs_changed |= ui
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
                        });
                    }

                    SettingsTab::DecksAndBeatgrid => {
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Decks, Pitch Range & Waveform Magnification");
                            ui.horizontal(|ui| {
                                ui.label("Pitch Fader Range:");
                                for pct in [6.0_f64, 8.0, 16.0, 50.0] {
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
                                        prefs_changed = true;
                                    }
                                }
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Default Waveform Zoom:");
                                egui::ComboBox::from_id_salt("settings_wf_zoom")
                                    .selected_text(self.prefs.waveform_zoom.label())
                                    .width(150.0)
                                    .show_ui(ui, |ui| {
                                        for z in WaveformZoom::ALL {
                                            if ui
                                                .selectable_label(self.prefs.waveform_zoom == z, z.label())
                                                .clicked()
                                            {
                                                self.prefs.waveform_zoom = z;
                                                prefs_changed = true;
                                            }
                                        }
                                    });
                            });
                            ui.add_space(4.0);
                            if ui
                                .checkbox(
                                    &mut self.prefs.quantize_default,
                                    "Quantize hot cues & loops to beatgrid",
                                )
                                .changed()
                            {
                                self.orch.deck_a.quantize = self.prefs.quantize_default;
                                self.orch.deck_b.quantize = self.prefs.quantize_default;
                                prefs_changed = true;
                            }
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Tip: Right-click any CUE 1..4 pad on a deck to rename it (Intro/Drop/Break), pick its flag color, or assign Gate / Auto-Loop / Cue & Pause behavior.")
                                    .small()
                                    .color(palette.text_secondary),
                            );
                        });
                    }

                    SettingsTab::MidiAndDvs => {
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("MIDI Controller & DVS Timecode Vinyl");
                            ui.horizontal(|ui| {
                                ui.label("MIDI Profile:");
                                egui::ComboBox::from_id_salt("settings_midi_profile")
                                    .selected_text(self.midi.active_profile.label())
                                    .width(280.0)
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
                                ui.label("DVS Audio Input:");
                                let dev_lbl = self
                                    .dvs_selected_device
                                    .as_deref()
                                    .unwrap_or("Default Input");
                                egui::ComboBox::from_id_salt("settings_dvs_dev")
                                    .selected_text(dev_lbl)
                                    .width(240.0)
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
                        });
                    }

                    SettingsTab::AutoDjAndRecording => {
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Auto-DJ Automix & Mix Recording");
                            ui.checkbox(
                                &mut self.auto_dj_enabled,
                                "Enable Auto-DJ (auto-load & crossfade next track from active crate)",
                            );
                            ui.horizontal(|ui| {
                                ui.label("Auto-DJ Crossfade Duration (s):");
                                prefs_changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.prefs.auto_dj_fade_sec, 2.0..=20.0)
                                            .suffix("s"),
                                    )
                                    .changed();
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
                        });
                    }

                    SettingsTab::AiAndBridges => {
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("iPad / Tablet Multi-Touch Remote (LAN PWA)");
                            ui.label(
                                egui::RichText::new("Control decks, multi-touch faders, crossfader, Hot Cues, Sampler pads, XY FX pad, and Crates from iPad Safari over local Wi-Fi.")
                                    .small()
                                    .color(palette.text_secondary),
                            );
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                let running = self.touch_remote.is_some();
                                ui.add_enabled_ui(!running, |ui| {
                                    ui.label("Port:");
                                    ui.add(
                                        egui::DragValue::new(&mut self.touch_remote_port)
                                            .range(1024..=65535),
                                    );
                                });
                                let btn = egui::Button::new(if running {
                                    "Stop Touch Remote"
                                } else {
                                    "Start iPad Touch Remote"
                                })
                                .fill(if running {
                                    egui::Color32::from_rgb(220, 38, 38)
                                } else {
                                    egui::Color32::from_rgb(22, 163, 74)
                                });
                                if ui.add(btn).clicked() {
                                    if running {
                                        self.touch_remote = None;
                                        self.status_msg = "iPad Touch Remote stopped".to_string();
                                    } else {
                                        match super::touch_remote::TouchRemoteServer::start(
                                            self.touch_remote_port,
                                        ) {
                                            Ok(srv) => {
                                                self.status_msg = format!(
                                                    "iPad Touch Remote LIVE at {}",
                                                    srv.lan_url
                                                );
                                                self.touch_remote = Some(srv);
                                            }
                                            Err(e) => {
                                                self.status_msg = e;
                                            }
                                        }
                                    }
                                }
                                if let Some(ref srv) = self.touch_remote {
                                    ui.strong(
                                        egui::RichText::new(&srv.lan_url)
                                            .monospace()
                                            .color(palette.deck_a),
                                    );
                                    if ui.small_button("Copy URL").clicked() {
                                        ui.ctx().copy_text(srv.lan_url.clone());
                                        self.status_msg =
                                            format!("Copied {} to clipboard", srv.lan_url);
                                    }
                                }
                            });
                            ui.label(
                                egui::RichText::new("On iPad: open the URL in Safari -> Share -> Add to Home Screen for fullscreen multi-touch DJ mode. (Allow port in Windows Firewall if prompted.)")
                                    .small()
                                    .color(palette.text_secondary),
                            );
                        });
                        ui.add_space(8.0);
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("AI Booth Assistant (Local & Cloud Providers)");
                            ui.add_space(4.0);
                            self.render_ai_provider_controls(ui, palette, "settings");
                        });
                        ui.add_space(8.0);
                        theme::card_frame(ui, palette, |ui| {
                            ui.heading("Optional External Bridges");
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
                        });
                    }
                }

                if prefs_changed {
                    self.prefs.save();
                }
            });
        });
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
