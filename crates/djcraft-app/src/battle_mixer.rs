//! Battle mixer, headphone pre-cue strip, and mix recorder.

use djcraft_audio::output::AudioOutput;
use djcraft_audio::{DeckId, DeckState, RecordingFormat};
use djcraft_core::i18n::{t, I18nKey};
use djcraft_core::Command;

use super::default_recording_path;
use super::skins::SkinPalette;
use super::theme;
use super::widgets;

impl super::DjCraftApp {
    pub(super) fn render_battle_mixer(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal(|ui| {
                let title_w = (ui.available_width() - 280.0).max(90.0);
                ui.add_sized(
                    [title_w, 18.0],
                    egui::Label::new(
                        egui::RichText::new(t(I18nKey::BattleMixer, self.locale))
                            .strong()
                            .color(palette.text_primary),
                    )
                    .truncate(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mute_lbl = if self.orch.master_muted {
                        "UNMUTE"
                    } else {
                        "MUTE"
                    };
                    if ui.small_button(mute_lbl).clicked() {
                        self.execute_catalog_id("modify.toggle_mute");
                    }
                    let dev_text = if self.audio_out.active {
                        format!("WASAPI: {}", self.audio_out.device_name)
                    } else {
                        "WASAPI: Offline".to_string()
                    };
                    ui.add(
                        egui::Label::new(egui::RichText::new(dev_text).monospace().small())
                            .truncate(),
                    );
                    theme::status_dot(ui, self.audio_out.active && !self.orch.master_muted);
                });
            });
            ui.add_space(4.0);

            let da = self.orch.deck_a.clone();
            let db = self.orch.deck_b.clone();
            let avail_w = ui.available_width();

            if avail_w >= 640.0 {
                // Wide layout: Hardware Mixer on left, Headphone PFL + Mix Recorder on right
                // (eliminates dead empty space on the right of the center column).
                ui.horizontal_top(|ui| {
                    let mixer_col_w = 356.0_f32;
                    ui.allocate_ui_with_layout(
                        egui::vec2(mixer_col_w, 0.0),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            self.render_mixer_controls(ui, palette, &da, &db);
                        },
                    );
                    ui.separator();
                    let right_w = (avail_w - mixer_col_w - 20.0).max(240.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(right_w, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            self.render_cue_and_recorder(ui, palette, &da, &db);
                        },
                    );
                });
            } else {
                // Compact / narrow layout: centered Hardware Mixer, then Cue & Recorder below
                ui.vertical_centered(|ui| {
                    self.render_mixer_controls(ui, palette, &da, &db);
                });
                ui.add_space(6.0);
                ui.separator();
                self.render_cue_and_recorder(ui, palette, &da, &db);
            }
        });
    }

    fn render_mixer_controls(
        &mut self,
        ui: &mut egui::Ui,
        palette: &SkinPalette,
        da: &DeckState,
        db: &DeckState,
    ) {
        ui.horizontal(|ui| {
            // Deck A EQ column
            ui.vertical(|ui| {
                let mut hi = da.eq_high;
                let mut mi = da.eq_mid;
                let mut lo = da.eq_low;
                let mut flt = da.color_filter;
                let r1 = widgets::rotary_knob(
                    ui,
                    "A HI",
                    &mut hi,
                    0.0,
                    2.0,
                    1.0,
                    true,
                    palette.deck_a,
                    palette,
                );
                let r2 = widgets::rotary_knob(
                    ui,
                    "A MID",
                    &mut mi,
                    0.0,
                    2.0,
                    1.0,
                    true,
                    palette.deck_a,
                    palette,
                );
                let r3 = widgets::rotary_knob(
                    ui,
                    "A LOW",
                    &mut lo,
                    0.0,
                    2.0,
                    1.0,
                    true,
                    palette.deck_a,
                    palette,
                );
                let r4 = widgets::rotary_knob(
                    ui,
                    "A FLT",
                    &mut flt,
                    -1.0,
                    1.0,
                    0.0,
                    true,
                    egui::Color32::from_rgb(168, 85, 247),
                    palette,
                );
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
                if r4.changed {
                    self.orch.deck_a.color_filter = flt;
                }
                if r4.committed {
                    self.dispatch_and_report(Command::SetDeckFilter {
                        deck: DeckId::DeckA,
                        value: flt,
                    });
                }
            });

            // Deck A Channel Volume Fader + VU
            let mut vol_a = da.volume_fader;
            let fa = widgets::vertical_fader(
                ui,
                "CH A",
                &mut vol_a,
                0.0,
                1.0,
                1.0,
                185.0,
                palette.deck_a,
                palette,
            );
            if fa.changed {
                self.orch.deck_a.volume_fader = vol_a;
            }
            if fa.committed {
                self.dispatch_and_report(Command::SetDeckVolume {
                    deck: DeckId::DeckA,
                    volume: vol_a,
                });
            }
            widgets::paint_vu_meter(ui, da);

            // Center Master Loudness Knob
            ui.vertical(|ui| {
                ui.add_space(28.0);
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
                ui.add_space(8.0);
                let fx1_on = self.orch.fx_units[0].enabled;
                let fx1_btn = egui::Button::new(if fx1_on { "FX 1: ON" } else { "FX 1" })
                    .small()
                    .fill(if fx1_on {
                        egui::Color32::from_rgb(168, 85, 247)
                    } else {
                        palette.card_fill
                    });
                if ui.add(fx1_btn).on_hover_text("Toggle FX Bank 1").clicked() {
                    self.execute_catalog_id("modify.toggle_fx1");
                }
                let fx2_on = self.orch.fx_units[1].enabled;
                let fx2_btn = egui::Button::new(if fx2_on { "FX 2: ON" } else { "FX 2" })
                    .small()
                    .fill(if fx2_on {
                        egui::Color32::from_rgb(236, 72, 153)
                    } else {
                        palette.card_fill
                    });
                if ui.add(fx2_btn).on_hover_text("Toggle FX Bank 2").clicked() {
                    self.execute_catalog_id("modify.toggle_fx2");
                }
            });

            // Deck B VU + Channel Volume Fader
            widgets::paint_vu_meter(ui, db);
            let mut vol_b = db.volume_fader;
            let fb = widgets::vertical_fader(
                ui,
                "CH B",
                &mut vol_b,
                0.0,
                1.0,
                1.0,
                185.0,
                palette.deck_b,
                palette,
            );
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
                let mut flt = db.color_filter;
                let r1 = widgets::rotary_knob(
                    ui,
                    "B HI",
                    &mut hi,
                    0.0,
                    2.0,
                    1.0,
                    true,
                    palette.deck_b,
                    palette,
                );
                let r2 = widgets::rotary_knob(
                    ui,
                    "B MID",
                    &mut mi,
                    0.0,
                    2.0,
                    1.0,
                    true,
                    palette.deck_b,
                    palette,
                );
                let r3 = widgets::rotary_knob(
                    ui,
                    "B LOW",
                    &mut lo,
                    0.0,
                    2.0,
                    1.0,
                    true,
                    palette.deck_b,
                    palette,
                );
                let r4 = widgets::rotary_knob(
                    ui,
                    "B FLT",
                    &mut flt,
                    -1.0,
                    1.0,
                    0.0,
                    true,
                    egui::Color32::from_rgb(168, 85, 247),
                    palette,
                );
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
                if r4.changed {
                    self.orch.deck_b.color_filter = flt;
                }
                if r4.committed {
                    self.dispatch_and_report(Command::SetDeckFilter {
                        deck: DeckId::DeckB,
                        value: flt,
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
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            let mut curve_dirty = false;
            for c in djcraft_audio::CrossfaderCurve::ALL {
                if ui
                    .selectable_label(self.crossfader_curve == c, c.short_label())
                    .on_hover_text(c.label())
                    .clicked()
                {
                    self.crossfader_curve = c;
                    curve_dirty = true;
                }
            }
            if ui
                .selectable_label(self.crossfader_reverse, "REV")
                .on_hover_text("Hamster crossfader reverse (swaps A/B edges)")
                .clicked()
            {
                self.crossfader_reverse = !self.crossfader_reverse;
                curve_dirty = true;
            }
            if curve_dirty {
                self.audio_out
                    .set_crossfader_curve(self.crossfader_curve, self.crossfader_reverse);
            }
            let gr_db = self.audio_out.master_limiter_gr_db();
            let gr_col = if gr_db > 1.5 {
                egui::Color32::from_rgb(239, 68, 68)
            } else if gr_db > 0.2 {
                egui::Color32::from_rgb(245, 158, 11)
            } else {
                palette.text_secondary
            };
            ui.label(
                egui::RichText::new(format!("LIM -{gr_db:.1}dB"))
                    .monospace()
                    .small()
                    .color(gr_col),
            )
            .on_hover_text("Master soft-knee club limiter gain reduction");
            let ret_on = self.synthcraft_aux_return > 0.01;
            if ui
                .selectable_label(ret_on, if ret_on { "RET ON" } else { "RET" })
                .on_hover_text("SynthCraft stereo Aux Return (UDP 127.0.0.1:20811)")
                .clicked()
            {
                self.synthcraft_aux_return = if ret_on { 0.0 } else { 0.85 };
                self.audio_out
                    .set_synthcraft_aux_return(self.synthcraft_aux_return);
                self.status_msg = if self.synthcraft_aux_return > 0.01 {
                    "SynthCraft Aux Return LIVE on 127.0.0.1:20811".to_string()
                } else {
                    "SynthCraft Aux Return OFF".to_string()
                };
            }
        });
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("XF STEM:")
                    .monospace()
                    .small()
                    .color(palette.text_secondary),
            );
            for mode in djcraft_audio::StemCrossfaderMode::ALL {
                if ui
                    .selectable_label(self.stem_xf_mode == mode, mode.short_label())
                    .on_hover_text(mode.label())
                    .clicked()
                {
                    self.stem_xf_mode = mode;
                    self.audio_out.set_stem_crossfader_mode(mode);
                    self.status_msg = format!("Stem Crossfader: {}", mode.label());
                }
            }
        });
    }

    fn render_cue_and_recorder(
        &mut self,
        ui: &mut egui::Ui,
        palette: &SkinPalette,
        da: &DeckState,
        db: &DeckState,
    ) {
        // Dual-Bus Headphone PFL Pre-Cue Strip
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(t(I18nKey::HeadphonePfl, self.locale))
                    .strong()
                    .color(palette.text_primary),
            );
            let pfl_a = egui::Button::new("PFL A").small().fill(if da.pfl_cue {
                palette.deck_a
            } else {
                palette.card_fill
            });
            if ui
                .add(pfl_a)
                .on_hover_text("Pre-Fader Listen Deck A")
                .clicked()
            {
                self.dispatch_and_report(Command::TogglePflCue {
                    deck: DeckId::DeckA,
                });
            }
            let pfl_b = egui::Button::new("PFL B").small().fill(if db.pfl_cue {
                palette.deck_b
            } else {
                palette.card_fill
            });
            if ui
                .add(pfl_b)
                .on_hover_text("Pre-Fader Listen Deck B")
                .clicked()
            {
                self.dispatch_and_report(Command::TogglePflCue {
                    deck: DeckId::DeckB,
                });
            }
            let split_btn = egui::Button::new(if self.orch.headphone_split_cue {
                "SPLIT: ON"
            } else {
                "SPLIT: OFF"
            })
            .small()
            .fill(if self.orch.headphone_split_cue {
                egui::Color32::from_rgb(245, 158, 11)
            } else {
                palette.card_fill
            });
            if ui
                .add(split_btn)
                .on_hover_text("Single-device Split Mono Cue (Left=Cue, Right=Master)")
                .clicked()
            {
                let next_split = !self.orch.headphone_split_cue;
                self.dispatch_and_report(Command::SetHeadphoneMix {
                    cue_volume: self.orch.headphone_volume,
                    cue_mix: self.orch.headphone_cue_mix,
                    split_cue: next_split,
                });
            }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let mut hp_vol = self.orch.headphone_volume;
            let mut hp_mix = self.orch.headphone_cue_mix;
            ui.label("Vol");
            let r_vol = ui.add(
                egui::Slider::new(&mut hp_vol, 0.0..=1.0)
                    .show_value(false)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
            );
            ui.label("Cue/PGM");
            let r_mix = ui.add(
                egui::Slider::new(&mut hp_mix, 0.0..=1.0)
                    .show_value(false)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
            );
            if r_vol.changed() || r_mix.changed() {
                self.orch.headphone_volume = hp_vol;
                self.orch.headphone_cue_mix = hp_mix;
            }
            if r_vol.drag_stopped() || r_mix.drag_stopped() {
                self.dispatch_and_report(Command::SetHeadphoneMix {
                    cue_volume: hp_vol,
                    cue_mix: hp_mix,
                    split_cue: self.orch.headphone_split_cue,
                });
            }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let selected_label = self
                .selected_headphone_device
                .as_deref()
                .unwrap_or("Off / Split Cue");
            let mut next_dev: Option<Option<String>> = None;
            egui::ComboBox::from_id_salt("hp_device_combo")
                .selected_text(selected_label)
                .width(170.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            self.selected_headphone_device.is_none(),
                            "Off / Split Cue",
                        )
                        .clicked()
                    {
                        next_dev = Some(None);
                    }
                    for dev in &self.audio_output_devices {
                        let is_sel =
                            self.selected_headphone_device.as_deref() == Some(dev.as_str());
                        if ui.selectable_label(is_sel, dev).clicked() {
                            next_dev = Some(Some(dev.clone()));
                        }
                    }
                });
            if let Some(choice) = next_dev {
                match self.audio_out.set_headphone_device(choice.as_deref()) {
                    Ok(msg) => {
                        self.selected_headphone_device = choice;
                        self.status_msg = msg;
                    }
                    Err(e) => {
                        self.status_msg = format!("Headphone bus error: {e}");
                    }
                }
            }
            if ui
                .small_button("Scan")
                .on_hover_text("Refresh WASAPI devices")
                .clicked()
            {
                self.audio_output_devices = AudioOutput::list_output_devices();
                self.status_msg = format!(
                    "Found {} WASAPI output device(s)",
                    self.audio_output_devices.len()
                );
            }
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // Live Lossless WAV / FLAC Mix Recorder Strip
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(t(I18nKey::MixRecorder, self.locale))
                    .strong()
                    .color(palette.text_primary),
            );
            let is_rec = self.audio_out.is_recording();
            ui.add_enabled_ui(!is_rec, |ui| {
                egui::ComboBox::from_id_salt("rec_format_combo")
                    .selected_text(self.recording_format.extension().to_uppercase())
                    .width(64.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.recording_format,
                            RecordingFormat::Wav,
                            "WAV (16-bit PCM)",
                        );
                        ui.selectable_value(
                            &mut self.recording_format,
                            RecordingFormat::Flac,
                            "FLAC (Lossless)",
                        );
                    });
            });
            if is_rec {
                let secs = self.audio_out.recording_seconds();
                let mins = (secs / 60.0) as u64;
                let rem = (secs % 60.0) as u64;
                let stop_btn = egui::Button::new(format!("STOP REC ({mins:02}:{rem:02})"))
                    .small()
                    .fill(egui::Color32::from_rgb(220, 38, 38));
                if ui.add(stop_btn).clicked() {
                    match self.audio_out.stop_recording() {
                        Ok(report) => {
                            self.status_msg = format!(
                                "Saved {:.1}s {} -> {} ({})",
                                report.duration_seconds,
                                report.format.extension().to_uppercase(),
                                report.output_path.display(),
                                report.note
                            );
                        }
                        Err(e) => self.status_msg = format!("Recording stop error: {e}"),
                    }
                }
                if let Some(p) = self.audio_out.recording_path() {
                    let name = p
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("recording");
                    ui.monospace(name);
                }
            } else {
                let rec_btn = egui::Button::new("REC MIX")
                    .small()
                    .fill(egui::Color32::from_rgb(153, 27, 27));
                if ui.add(rec_btn).clicked() {
                    let out_path = default_recording_path(self.recording_format);
                    match self
                        .audio_out
                        .start_recording(out_path.clone(), self.recording_format)
                    {
                        Ok(p) => {
                            self.status_msg = format!(
                                "Recording {} -> {}",
                                self.recording_format.label(),
                                p.display()
                            );
                        }
                        Err(e) => self.status_msg = format!("Recording start error: {e}"),
                    }
                }
                if ui.small_button("Rec As...").clicked() {
                    let ext = self.recording_format.extension();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Audio Mix", &[ext])
                        .set_file_name(format!("djcraft_mix.{ext}"))
                        .save_file()
                    {
                        match self.audio_out.start_recording(path, self.recording_format) {
                            Ok(p) => {
                                self.status_msg = format!("Recording -> {}", p.display());
                            }
                            Err(e) => self.status_msg = format!("Recording start error: {e}"),
                        }
                    }
                }
            }
        });
    }
}
