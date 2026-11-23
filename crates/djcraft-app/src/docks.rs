//! Lower docks and modal windows.

use std::sync::Arc;

use djcraft_audio::dsp::{decode_audio_file, WaveformSummary};
use djcraft_audio::{DeckId, TrackMeta};
use djcraft_core::{
    batch_stem_folder, butterchurn_sync_bpm, chat_with_booth_ai, fetch_available_models,
    generate_speech_drop, parse_natural_language_command, plan_harmonic_setlist,
    probe_fleet_health, sfx_search, vfx_apply_preset, AiProvider, Command, ControllerProfileId,
    CrateSortField, MidiControllerManager, MidiTargetAction, MixxxxxVideoControls,
};

use super::skins::SkinPalette;
use super::theme;
use super::video::{VIDEO_H, VIDEO_W};
use super::widgets;
use super::{HelpTab, APP_VERSION};

impl super::DjCraftApp {
    pub(super) fn render_crate_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
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
                                    self.status_msg = format!(
                                        "Imported {} -> '{}'",
                                        s.imported_tracks, s.crate_path
                                    );
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
                                    Ok(n) => {
                                        self.status_msg = format!("Exported {n} tracks to Serato")
                                    }
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
                                    self.status_msg = format!(
                                        "Imported {} -> '{}'",
                                        s.imported_tracks, s.crate_path
                                    );
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
                                    Ok(n) => {
                                        self.status_msg = format!("Exported {n} tracks to VDJ")
                                    }
                                    Err(e) => self.status_msg = format!("Export error: {e}"),
                                }
                            }
                        }
                    }
                }
                if ui
                    .small_button("Batch Stem Folder...")
                    .on_hover_text("Run Demucs into transparent .djcraft-stems/<track>/ subfolders")
                    .clicked()
                {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.status_msg =
                            format!("Batch stemming {} in background...", dir.display());
                        let bg = Arc::clone(&self.bg_status);
                        std::thread::spawn(move || {
                            let msg = match batch_stem_folder(&dir) {
                                Ok(sum) => format!(
                                    "Batch stem done: {} stemmed, {} skipped, {} failed",
                                    sum.stemmed, sum.skipped_existing, sum.failed
                                ),
                                Err(e) => format!("Batch stem error: {e}"),
                            };
                            if let Ok(mut g) = bg.lock() {
                                *g = Some(msg);
                            }
                        });
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
                        self.crate_query.crate_id.is_some()
                            && !self.crate_new_name.trim().is_empty(),
                        egui::Button::new("+ Subcrate").small(),
                    )
                    .clicked()
                {
                    if let Some(ref store) = self.crate_store {
                        if let Ok(cid) =
                            store.create_subcrate(self.crate_query.crate_id, &self.crate_new_name)
                        {
                            self.crate_query.crate_id = Some(cid);
                            self.crate_new_name.clear();
                            self.crate_dirty = true;
                        }
                    }
                }
            });

            ui.horizontal_wrapped(|ui| {
                if ui
                    .selectable_label(self.crate_query.crate_id.is_none(), "All")
                    .clicked()
                {
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
                if ui
                    .checkbox(&mut self.crate_harmonic_deck_a, "Harmonic A")
                    .changed()
                {
                    self.crate_dirty = true;
                }
                if ui
                    .checkbox(&mut self.crate_query.include_subcrates, "Subcrates")
                    .changed()
                {
                    self.crate_dirty = true;
                }
                egui::ComboBox::from_id_salt("crate_sort")
                    .selected_text(self.crate_query.sort_by.label())
                    .show_ui(ui, |ui| {
                        for f in CrateSortField::ALL {
                            if ui
                                .selectable_value(&mut self.crate_query.sort_by, f, f.label())
                                .changed()
                            {
                                self.crate_dirty = true;
                            }
                        }
                    });
                if ui
                    .small_button(if self.crate_query.sort_asc {
                        "ASC"
                    } else {
                        "DESC"
                    })
                    .clicked()
                {
                    self.crate_query.sort_asc = !self.crate_query.sort_asc;
                    self.crate_dirty = true;
                }
            });

            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(190.0)
                .show(ui, |ui| {
                    for t in self.crate_results.clone() {
                        ui.horizontal(|ui| {
                            let key = if t.musical_key.is_empty() {
                                "--"
                            } else {
                                t.musical_key.as_str()
                            };
                            let bpm = if t.bpm > 0.0 {
                                format!("{:.1}", t.bpm)
                            } else {
                                "---".to_string()
                            };
                            ui.label(format!(
                                "{} - {} | {} | {} BPM",
                                t.artist, t.title, key, bpm
                            ));
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
                                            self.status_msg = format!(
                                                "Analyzed '{}': {:.1} BPM",
                                                up.title, up.bpm
                                            );
                                            self.crate_dirty = true;
                                        }
                                    }
                                }
                                if ui.small_button("Dettato").clicked() {
                                    if let Some(ref store) = self.crate_store {
                                        match store.analyze_track_dettato(t.id) {
                                            Ok(up) => {
                                                self.status_msg = format!(
                                                    "Dettato '{}': {:?}",
                                                    up.title, up.tuning_cents
                                                );
                                                self.crate_dirty = true;
                                            }
                                            Err(e) => {
                                                self.status_msg = format!("Dettato error: {e}")
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                });
        });
    }

    pub(super) fn render_video_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
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

    pub(super) fn render_midi_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
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
                if ui.small_button("Import XML Mapper...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("MIDI Mapper XML", &["xml"])
                        .pick_file()
                    {
                        match self.midi.import_xml_mapper_file(&path) {
                            Ok(n) => {
                                self.status_msg = format!(
                                    "Imported {n} MIDI mapping rule(s) from {}",
                                    path.display()
                                )
                            }
                            Err(e) => self.status_msg = format!("XML import error: {e}"),
                        }
                    }
                }
            });

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.strong("MIDI Learn:");
                if let Some(target) = self.midi.learn_target {
                    ui.label(
                        egui::RichText::new(format!(
                            "Move hardware control for '{}'...",
                            target.label()
                        ))
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

            ui.separator();
            ui.horizontal_wrapped(|ui| {
                theme::status_dot(ui, self.dvs_input.is_some() && self.dvs_telemetry.locked);
                ui.label(
                    egui::RichText::new("DVS Timecode Vinyl (Safe Rust Quadrature Decoder)")
                        .strong()
                        .color(palette.text_primary),
                );
                if ui.small_button("Rescan Audio In").clicked() {
                    self.dvs_devices = djcraft_audio::list_input_devices();
                    self.status_msg =
                        format!("Found {} audio input device(s)", self.dvs_devices.len());
                }
            });
            ui.horizontal_wrapped(|ui| {
                let dev_label = self
                    .dvs_selected_device
                    .as_deref()
                    .unwrap_or("Default Input");
                egui::ComboBox::from_id_salt("dvs_input_device")
                    .selected_text(dev_label)
                    .width(175.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.dvs_selected_device, None, "Default Input");
                        for dev in &self.dvs_devices {
                            ui.selectable_value(
                                &mut self.dvs_selected_device,
                                Some(dev.clone()),
                                dev,
                            );
                        }
                    });
                egui::ComboBox::from_id_salt("dvs_carrier_select")
                    .selected_text(self.dvs_carrier.label())
                    .width(155.0)
                    .show_ui(ui, |ui| {
                        for c in djcraft_audio::DvsCarrier::ALL {
                            ui.selectable_value(&mut self.dvs_carrier, c, c.label());
                        }
                    });
                egui::ComboBox::from_id_salt("dvs_deck_select")
                    .selected_text(self.dvs_target_deck.label())
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.dvs_target_deck,
                            djcraft_audio::DeckId::DeckA,
                            "Deck A",
                        );
                        ui.selectable_value(
                            &mut self.dvs_target_deck,
                            djcraft_audio::DeckId::DeckB,
                            "Deck B",
                        );
                    });
                let dvs_active = self.dvs_input.is_some();
                if ui
                    .small_button(if dvs_active { "Stop DVS" } else { "Start DVS" })
                    .clicked()
                {
                    if dvs_active {
                        self.dvs_input = None;
                        self.dvs_telemetry = djcraft_audio::DvsTelemetry::default();
                        self.status_msg = "DVS timecode input stopped".to_string();
                    } else {
                        match djcraft_audio::DvsInputStream::start(
                            self.dvs_selected_device.as_deref(),
                            self.dvs_carrier,
                        ) {
                            Ok(stream) => {
                                self.dvs_input = Some(stream);
                                self.status_msg = format!(
                                    "DVS active on {} ({})",
                                    self.dvs_target_deck.label(),
                                    self.dvs_carrier.label()
                                );
                            }
                            Err(e) => self.status_msg = format!("DVS start error: {e}"),
                        }
                    }
                }
            });
            if self.dvs_input.is_some() {
                ui.monospace(format!(
                    "DVS {}: {} | Speed {:+.3}x | Pitch {:+.2}% | Level {:.1} dBFS",
                    self.dvs_target_deck.label(),
                    if self.dvs_telemetry.locked {
                        "LOCKED"
                    } else {
                        "NO SIGNAL"
                    },
                    self.dvs_telemetry.speed_ratio,
                    self.dvs_telemetry.pitch_pct,
                    self.dvs_telemetry.signal_dbfs,
                ));
            }
        });
    }

    pub(super) fn trigger_ai_model_refresh(&mut self) {
        self.ai_models_fetched_once = true;
        let cfg = self.ai_cfg.clone();
        let models_arc = Arc::clone(&self.ai_models);
        let status_arc = Arc::clone(&self.ai_models_status);
        if let Ok(mut st) = status_arc.lock() {
            *st = "Loading models...".to_string();
        }
        std::thread::spawn(move || match fetch_available_models(&cfg) {
            Ok(list) => {
                let count = list.len();
                if let Ok(mut m) = models_arc.lock() {
                    *m = list;
                }
                if let Ok(mut st) = status_arc.lock() {
                    *st = format!("{count} model(s)");
                }
            }
            Err(e) => {
                if let Ok(mut st) = status_arc.lock() {
                    *st = format!("Offline ({e})");
                }
            }
        });
    }

    pub(super) fn render_ai_provider_controls(
        &mut self,
        ui: &mut egui::Ui,
        palette: &SkinPalette,
        id_prefix: &str,
    ) {
        if !self.ai_models_fetched_once {
            self.trigger_ai_model_refresh();
        }

        // Row 1: Provider selector + Model dropdown + Refresh + custom model input
        ui.horizontal_wrapped(|ui| {
            ui.label("Provider:");
            let prev_provider = self.ai_cfg.provider;
            egui::ComboBox::from_id_salt((id_prefix, "ai_provider_combo"))
                .selected_text(self.ai_cfg.provider.label())
                .width(155.0)
                .show_ui(ui, |ui| {
                    for prov in AiProvider::ALL {
                        ui.selectable_value(&mut self.ai_cfg.provider, prov, prov.label());
                    }
                });
            if self.ai_cfg.provider != prev_provider {
                self.ai_cfg.endpoint = self.ai_cfg.provider.default_endpoint().to_string();
                self.ai_cfg.model = self.ai_cfg.provider.default_model().to_string();
                if let Ok(mut m) = self.ai_models.lock() {
                    m.clear();
                }
                self.trigger_ai_model_refresh();
            }

            // Build candidate model list: live fetched models + provider presets + current value
            let mut choices: Vec<String> =
                self.ai_models.lock().map(|m| m.clone()).unwrap_or_default();
            for preset in self.ai_cfg.provider.preset_models() {
                if !choices.iter().any(|c| c == preset) {
                    choices.push((*preset).to_string());
                }
            }
            if !self.ai_cfg.model.trim().is_empty()
                && !choices.iter().any(|c| c == &self.ai_cfg.model)
            {
                choices.insert(0, self.ai_cfg.model.clone());
            }

            ui.label("Model:");
            let selected_lbl = if self.ai_cfg.model.trim().is_empty() {
                "(select model)"
            } else {
                self.ai_cfg.model.as_str()
            };
            egui::ComboBox::from_id_salt((id_prefix, "ai_model_combo"))
                .selected_text(selected_lbl)
                .width(175.0)
                .show_ui(ui, |ui| {
                    if choices.is_empty() {
                        ui.label("No models found -- click Refresh or type below");
                    } else {
                        for m in &choices {
                            ui.selectable_value(&mut self.ai_cfg.model, m.clone(), m);
                        }
                    }
                });

            if ui
                .small_button("Refresh")
                .on_hover_text("Fetch installed/available models from the selected provider")
                .clicked()
            {
                self.trigger_ai_model_refresh();
            }

            let status_txt = self
                .ai_models_status
                .lock()
                .map(|s| s.clone())
                .unwrap_or_default();
            if !status_txt.is_empty() {
                ui.label(
                    egui::RichText::new(status_txt)
                        .small()
                        .color(palette.text_secondary),
                );
            }
        });

        // Row 2: Custom model override + Endpoint URL + API Key (masked)
        ui.horizontal_wrapped(|ui| {
            ui.label("Custom:");
            ui.add(
                egui::TextEdit::singleline(&mut self.ai_cfg.model)
                    .desired_width(130.0)
                    .hint_text("model tag..."),
            );
            ui.label("URL:");
            ui.add(egui::TextEdit::singleline(&mut self.ai_cfg.endpoint).desired_width(230.0));
            ui.label("API Key:");
            let env_detected =
                self.ai_cfg.api_key.trim().is_empty() && self.ai_cfg.resolved_api_key().is_some();
            let key_hint = if env_detected {
                "Using env var (override...)"
            } else if self.ai_cfg.provider.requires_api_key() {
                "Enter API key..."
            } else {
                "Optional (local)"
            };
            ui.add(
                egui::TextEdit::singleline(&mut self.ai_cfg.api_key)
                    .password(!self.ai_show_api_key)
                    .desired_width(145.0)
                    .hint_text(key_hint),
            );
            if ui
                .small_button(if self.ai_show_api_key { "Hide" } else { "Show" })
                .clicked()
            {
                self.ai_show_api_key = !self.ai_show_api_key;
            }
        });
    }

    pub(super) fn render_ai_chat_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("AI Booth Assistant & Workflows");
                if ui.small_button("Build 4-Track Harmonic Setlist").clicked() {
                    let seed = self
                        .orch
                        .deck_a
                        .loaded_track
                        .as_ref()
                        .map(|t| t.musical_key.as_str())
                        .unwrap_or("8A");
                    if let Some(ref store) = self.crate_store {
                        match plan_harmonic_setlist(store, seed, 4) {
                            Ok(list) => {
                                let lines: Vec<String> = list
                                    .iter()
                                    .enumerate()
                                    .map(|(i, t)| {
                                        format!(
                                            "{}. {} - {} ({} | {:.1} BPM)",
                                            i + 1,
                                            t.artist,
                                            t.title,
                                            t.musical_key,
                                            t.bpm
                                        )
                                    })
                                    .collect();
                                let reply = if lines.is_empty() {
                                    "No harmonic tracks found in crate store.".to_string()
                                } else {
                                    format!("Harmonic Setlist from {seed}:\n{}", lines.join("\n"))
                                };
                                if let Ok(mut log) = self.ai_log.lock() {
                                    log.push(("Workflow".to_string(), reply));
                                }
                            }
                            Err(e) => self.status_msg = format!("Setlist error: {e}"),
                        }
                    }
                }
            });

            self.render_ai_provider_controls(ui, palette, "dock");

            egui::ScrollArea::vertical()
                .max_height(140.0)
                .show(ui, |ui| {
                    if let Ok(log) = self.ai_log.lock() {
                        for (who, msg) in log.iter() {
                            ui.label(format!("[{who}] {msg}"));
                        }
                    }
                });

            ui.horizontal(|ui| {
                let enter = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.ai_input)
                            .desired_width(340.0)
                            .hint_text("Type a deck command ('sync', 'kill bass a') or ask AI..."),
                    )
                    .lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("Send").clicked() || enter) && !self.ai_input.trim().is_empty() {
                    let prompt = self.ai_input.trim().to_string();
                    self.ai_input.clear();
                    if let Ok(mut log) = self.ai_log.lock() {
                        log.push(("You".to_string(), prompt.clone()));
                    }
                    if let Some(cmd) = parse_natural_language_command(&prompt) {
                        self.dispatch_and_report(cmd);
                        if let Ok(mut log) = self.ai_log.lock() {
                            log.push(("DJCraft".to_string(), self.status_msg.clone()));
                        }
                    } else {
                        let cfg = self.ai_cfg.clone();
                        let ctx_str = format!(
                            "Deck A: {:?}, Deck B: {:?}, Crossfader: {:+.2}",
                            self.orch.deck_a.loaded_track.as_ref().map(|t| (
                                &t.title,
                                t.bpm,
                                &t.musical_key
                            )),
                            self.orch.deck_b.loaded_track.as_ref().map(|t| (
                                &t.title,
                                t.bpm,
                                &t.musical_key
                            )),
                            self.orch.crossfader
                        );
                        let log_arc = Arc::clone(&self.ai_log);
                        let bg = Arc::clone(&self.bg_status);
                        std::thread::spawn(move || {
                            let reply = match chat_with_booth_ai(&cfg, &prompt, &ctx_str) {
                                Ok(r) => r,
                                Err(e) => format!("LLM offline: {e}"),
                            };
                            if let Ok(mut log) = log_arc.lock() {
                                log.push(("AI".to_string(), reply));
                            }
                            if let Ok(mut g) = bg.lock() {
                                *g = Some("AI Booth reply received".to_string());
                            }
                        });
                    }
                }
            });
        });
    }

    pub(super) fn render_fleet_hub_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("External Bridges (Optional)");
                let changed = ui
                    .checkbox(
                        &mut self.fleet_endpoints.enabled,
                        "Enable local bridge probing",
                    )
                    .changed();
                if changed && !self.fleet_endpoints.enabled {
                    self.fleet_health = Default::default();
                    self.fleet_last_output = "External local bridges disabled.".to_string();
                }
                if self.fleet_endpoints.enabled {
                    if ui.small_button("Probe Bridges").clicked() {
                        self.fleet_health = probe_fleet_health(&self.fleet_endpoints);
                        self.fleet_last_output = "Probed configured local endpoints.".to_string();
                    }
                    theme::status_dot(ui, self.fleet_health.speechcraft_up);
                    ui.monospace(format!("Speech:{}", self.fleet_endpoints.speechcraft_port));
                    theme::status_dot(ui, self.fleet_health.sfx_up);
                    ui.monospace(format!("SFX:{}", self.fleet_endpoints.sfx_port));
                    theme::status_dot(ui, self.fleet_health.vfx_up);
                    ui.monospace(format!("VFX:{}", self.fleet_endpoints.vfx_port));
                    theme::status_dot(ui, self.fleet_health.butterchurn_up);
                    ui.monospace(format!(
                        "Visualizer:{}",
                        self.fleet_endpoints.butterchurn_port
                    ));
                    theme::status_dot(ui, self.fleet_health.ollama_up);
                    ui.monospace(format!("LLM:{}", self.fleet_endpoints.ollama_port));
                }
            });

            if !self.fleet_endpoints.enabled {
                ui.label(
                    egui::RichText::new(
                        "Disabled by default so standalone runs make zero localhost connections. Enable above if you run optional local Speech/SFX/VFX/Visualizer/LLM bridges.",
                    )
                    .small()
                    .color(palette.text_secondary),
                );
                return;
            }

            ui.separator();
            // SpeechCraft / SAPI DJ Vocal Drop -> Deck C
            ui.horizontal(|ui| {
                ui.strong("DJ Vocal Drop:");
                ui.add(egui::TextEdit::singleline(&mut self.speech_drop_text).desired_width(220.0));
                if ui.small_button("Speak -> Deck C").clicked() {
                    let out_wav = std::env::temp_dir().join("djcraft_vocal_drop.wav");
                    match generate_speech_drop(&self.speech_drop_text, &out_wav) {
                        Ok(path) => {
                            if let Ok(decoded) = decode_audio_file(&path) {
                                let dur = decoded.duration_seconds;
                                self.deck_waveforms[2] =
                                    Some(WaveformSummary::from_decoded(&decoded));
                                self.audio_out.set_deck_audio(2, Some(Arc::new(decoded)));
                                let meta = TrackMeta {
                                    title: self.speech_drop_text.clone(),
                                    artist: "DJ Vocal Drop".to_string(),
                                    bpm: 126.0,
                                    musical_key: "8A".to_string(),
                                    duration_seconds: dur,
                                };
                                self.dispatch_and_report(Command::LoadTrack {
                                    deck: DeckId::DeckC,
                                    track: meta,
                                });
                                self.dispatch_and_report(Command::PlayPause {
                                    deck: DeckId::DeckC,
                                });
                                self.fleet_last_output =
                                    format!("Played vocal drop on Deck C ({dur:.1}s)");
                            }
                        }
                        Err(e) => self.fleet_last_output = format!("Speech drop error: {e}"),
                    }
                }
            });

            // Butterchurn BPM sync + SFX + VFX triggers
            ui.horizontal_wrapped(|ui| {
                if ui
                    .small_button("Sync BPM -> Butterchurn (:10878)")
                    .clicked()
                {
                    let bpm = self
                        .orch
                        .deck_a
                        .loaded_track
                        .as_ref()
                        .map(|t| t.bpm * (1.0 + self.orch.deck_a.pitch_bend_pct / 100.0))
                        .unwrap_or(126.0);
                    self.fleet_last_output = match butterchurn_sync_bpm(
                        self.fleet_endpoints.butterchurn_port,
                        bpm,
                        None,
                    ) {
                        Ok(s) => s,
                        Err(e) => e.to_string(),
                    };
                }
                ui.add(egui::TextEdit::singleline(&mut self.sfx_query).desired_width(110.0));
                if ui.small_button("Search SFX (:11120)").clicked() {
                    self.fleet_last_output =
                        match sfx_search(self.fleet_endpoints.sfx_port, &self.sfx_query) {
                            Ok(s) => s,
                            Err(e) => e.to_string(),
                        };
                }
                ui.add(egui::TextEdit::singleline(&mut self.vfx_preset).desired_width(95.0));
                if ui.small_button("Apply VFX (:11122)").clicked() {
                    let vid = self
                        .orch
                        .deck_a
                        .audio_path
                        .as_deref()
                        .unwrap_or("deck_a.mp4");
                    self.fleet_last_output = match vfx_apply_preset(
                        self.fleet_endpoints.vfx_port,
                        vid,
                        &self.vfx_preset,
                    ) {
                        Ok(s) => s,
                        Err(e) => e.to_string(),
                    };
                }
            });

            ui.monospace(&self.fleet_last_output);
        });
    }

    pub(super) fn render_camelot_mixxxxx_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
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
                widgets::paint_camelot_wheel_mini(ui, key_a, key_b, palette);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.strong("Camelot Harmonic Assistant");
                    if let Ok(plan) = self.orch.plan_transition() {
                        ui.label(format!("{} -> {}", plan.from_track, plan.to_track));
                        ui.monospace(format!(
                            "BPM {:.1} -> {:.1} (Delta {:+.2}) | {} bars",
                            plan.from_bpm,
                            plan.to_bpm,
                            plan.bpm_delta,
                            plan.recommended_transition_bars
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

    pub(super) fn render_modals(&mut self, ctx: &egui::Context) {
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
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
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
                    ui.monospace("Ctrl+,       Open Settings");
                    ui.monospace("Ctrl+L       Toggle Booth AI Chat Drawer");
                    ui.monospace("Arrows/Home  Nudge or reset focused rotary knob");
                    ui.label("Double-click any rotary knob or fader to reset to default.");
                    ui.label("Ctrl + Mouse Wheel : Zoom UI | Ctrl + 0 : Reset 100%");
                });
            self.show_shortcuts_dialog = open;
        }

        if self.show_help_center {
            let mut open = self.show_help_center;
            egui::Window::new("DJCraft Pro Help Center")
                .open(&mut open)
                .collapsible(false)
                .default_width(740.0)
                .default_height(480.0)
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
                            ui.strong("1. Starting Playback on Deck A, B, C, or D");
                            ui.label("- Click PLAY on any deck (or press Space for Deck A, Shift+Space for Deck B).");
                            ui.label("- Load audio via AUDIO/LOAD on a deck, drag files/folders from Explorer, or click A/B/C/D in BOOTH + CRATE or CRATES & GIG PREP.");
                            ui.label("- Verify Battle Mixer: CH A/B faders up, MST > 0%, MUTE off, and Magnetic crossfader centered or toward the active deck.");
                            ui.add_space(6.0);
                            ui.strong("2. Menu-First UI, Workspace Modes & Layouts");
                            ui.label("- All workspace, layout, panel, sync, Auto-DJ, Link, Chat, and Settings actions live in the standard top Menu Bar (File / Edit / View / Insert / Modify / Commands / Window / Help).");
                            ui.label("- Optional Quick Toolbar: Hidden by default for a clean booth. Re-enable anytime via View -> Show Quick Toolbar (or Settings -> Booth Layouts).");
                            ui.label("- View -> Workspace: LIVE CONSOLE, BOOTH + CRATE (--booth-crate), CRATES & GIG PREP (--crates), or SETTINGS (Ctrl+,).");
                            ui.label("- View -> Booth Layouts: 7 Serato/VirtualDJ presets (Classic 2-Deck, Serato Vertical, Extended 4-Deck, Scratch/Battle, Performance, Library/Prep, Video/VJ).");
                            ui.add_space(6.0);
                            ui.strong("3. Skins, Card Decor, Backdrops & Full-Window Settings (Ctrl+,)");
                            ui.label("- 9 Skins (View -> Console Skin or Settings -> Appearance): Battle Console, Club Dark, Midnight Booth, Carbon Deck, Shocking Pink Club, Daylight Booth, Star Trek LCARS, Baroque Velvet & Gold, Synthwave Outrun.");
                            ui.label("- 6 Bold Backdrops (Solid, Brushed Metal, Carbon Weave, Sparkle Club, Starfield Warp, Baroque Gold Damask) + 5 Card Decoration Styles + live opacity/border/color/intensity sliders.");
                            ui.label("- 7 Languages (EN, DE, FR, ES, JA, KO, ZH) + Full-Window 2-Pane Settings (File -> Settings... or Ctrl+,) persisted to %LOCALAPPDATA%\\djcraft\\ui.json.");
                        }
                        HelpTab::Functions => {
                            ui.strong("3-Band RGB & Overview Waveforms, Scratching & DVS");
                            ui.label("- 3-Band RGB Waveforms: Low (<250 Hz) Red, Mid (250-2500 Hz) Green, High (>2500 Hz) Cyan with white-hot transient cores, beatgrid lines, loop bands, and cue flags.");
                            ui.label("- Clickable Overview Strip: Click or drag the full-track overview below each deck header to needle-drop seek.");
                            ui.label("- Vinyl Scratching & DVS: Drag any platter to scratch (-4x..+4x), or enable DVS Timecode Vinyl (1000/2000 Hz) in Settings -> MIDI & DVS.");
                            ui.add_space(6.0);
                            ui.strong("Prelisten (PFL), Mix Recording, Auto-DJ & UDP Link");
                            ui.label("- Dual-Device PFL: Pick a 2nd audio output in Battle Mixer or Settings -> Audio for stereo headphone cue.");
                            ui.label("- Single-Device Split Cue: Enable SPLIT: ON (Left = Cue Mono, Right = Master Mono) for a DJ splitter cable.");
                            ui.label("- Lossless Mix Recorder: WAV (16-bit PCM) or FLAC (via ffmpeg) -> %USERPROFILE%\\Music\\DJCraft_Recordings\\.");
                            ui.label("- Auto-DJ & LUFS Auto-Gain: Enable Auto-DJ in Commands -> Enable Auto-DJ Automix; toggle Auto-Gain in Settings -> Audio.");
                            ui.label("- UDP Multicast Link: Toggle in Commands -> Enable Link Sync (UDP) for clean-room beat/tempo sync on 224.76.78.75:20808.");
                            ui.add_space(6.0);
                            ui.strong("Crates, Smart Hot Cues, Stems, MIDI, AI & iPad Touch Remote");
                            ui.label("- Smart Hot Cues: Slots 1-4 save timestamps + custom names, colors & behaviors (Right-click CUE 1..4) per track in SQLite.");
                            ui.label("- MIDI (midir): 9 built-in profiles (Rane, Pioneer, Hercules, Denon, Numark, Allen & Heath, Reloop Ready) + MIDI Learn + Import XML Mapper.");
                            ui.label("- Multi-Provider AI Chat (Ctrl+L) & iPad Touch Remote: Configure Ollama/LM Studio/OpenAI/Anthropic/Gemini/OpenRouter and start the opt-in LAN/Tailscale iPad Multi-Touch PWA (with optional PIN) in Settings -> AI & External Bridges.");
                        }
                        HelpTab::Usage => {
                            ui.strong("Desktop Controls & Shortcuts");
                            ui.label("- Space: Play/Pause A | Shift+Space: Play/Pause B | Esc: Stop All");
                            ui.label("- Ctrl+K: Command Palette | F1: Shortcuts | Ctrl+,: Settings | Ctrl+L: Chat Drawer");
                            ui.label("- Ctrl+Z / Ctrl+Y: Undo/Redo | Ctrl+Wheel: Zoom | Ctrl+0: Reset 100%");
                            ui.label("- Double-click any knob/fader to reset; Arrow keys / Home / 0 nudge focused rotary knobs.");
                            ui.add_space(6.0);
                            ui.strong("CLI & MCP Automation");
                            ui.monospace("djcraft.exe --smoke | --booth-crate | --crates | --four-deck");
                            ui.monospace("djcraft-cli status | transition | camelot 8A 9A | crate-list");
                            ui.monospace("djcraft-cli crate-import-serato <file> | crate-import-vdj <file>");
                            ui.monospace("djcraft-cli mixxxxx-ping | analyze-dettato <audio_file>");
                            ui.label("MCP stdio server (djcraft-mcp.exe): JSON-RPC 2.0 tools sharing the L1 Command::dispatch bus.");
                        }
                        HelpTab::TipsAndTricks => {
                            ui.label("- Booth + Crate Workflow: Switch to BOOTH + CRATE for compact platters, center vertical RGB waveforms, and your prepared gig crate in the lower half.");
                            ui.label("- Pre-Cue Without a 2nd Sound Card: Click SPLIT: ON in Battle Mixer and toggle PFL B (Left = Cue, Right = Master).");
                            ui.label("- Beatgrid Quantize: Enable Quantize in Settings -> Decks & Booth to snap hot cues cleanly to the beatgrid.");
                            ui.label("- Harmonic Mixing: Check 'Harmonic' in Crates or open the Chat Drawer (Ctrl+L) for 1-click compatible tracks.");
                        }
                        HelpTab::PitfallsAndSafety => {
                            ui.label("- Deck B silent after loading? Verify PLAY is active, CH B fader is up, and crossfader is not slammed full left (-1.0).");
                            ui.label("- FLAC recording and native MP4/MKV video require ffmpeg on PATH (FLAC falls back cleanly to WAV if ffmpeg is missing).");
                            ui.label("- Batch Stemming requires demucs (set DEMUCS_BIN or install on PATH). Run stemming off the live audio thread.");
                            ui.label("- All workspace crates enforce #![forbid(unsafe_code)] and strict L1 parameter clamping.");
                        }
                        HelpTab::PlannedExtensions => {
                            ui.label("- Implemented in v0.2.x: 3-Band RGB & Overview Waveforms, DVS Timecode Vinyl, UDP Multicast Link, Auto-DJ, Quantize, LUFS Auto-Gain, 5-Tab Settings, Side-Drawer Chat, and Per-File Autosaved Hot Cues.");
                            ui.label("- Future roadmap: Out-of-process VST3 plugin host sidecar and Icecast/Shoutcast live broadcast.");
                        }
                        HelpTab::OtherCrafts => {
                            ui.label("- Optional external bridges (disabled by default in Settings -> AI & Bridges):");
                            ui.label("  - Video-extended OSC 1.0 bridge (user-configured UDP host/port).");
                            ui.label("  - Local TTS vocal drops, SFX/VFX/Visualizer HTTP bridges, and offline dettato CLI (via DETTATO_BIN or PATH).");
                        }
                        HelpTab::ProjectDocs => {
                            ui.label("See README.md, HELP.md, CHANGELOG.md, BUILD_LOG.md, and docs/target-app-parity.md in the DJCraft repository.");
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
                    ui.label(
                        "Crates: egui, eframe, cpal, symphonia, midir, rusqlite, rfd, clap, serde.",
                    );
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
                    ui.label(
                        "Rane Twelve MKII / Seventy-Two MKII MIDI | Serato & VirtualDJ Crates.",
                    );
                });
            self.show_about_dialog = open;
        }
    }
}
