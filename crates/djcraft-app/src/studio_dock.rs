#![forbid(unsafe_code)]
//! Studio Beatmaker (16-step BPM-synced drum sequencer) & Auto-Slice Sample Chopper UI.

use std::sync::Arc;

use djcraft_audio::dsp::decode_audio_file;
use djcraft_audio::{auto_slice_audio, DrumGenre, LANE_LABELS, STEP_COUNT, STEP_LANES};

use crate::skins::SkinPalette;
use crate::{theme, DjCraftApp};

impl DjCraftApp {
    pub(crate) fn render_studio_beatmaker_dock(
        &mut self,
        ui: &mut egui::Ui,
        palette: &SkinPalette,
    ) {
        let (seq_on, cur_step, mut steps) = self.audio_out.sequencer_snapshot();

        // 1. 16-Step BPM-Synced Drum Sequencer ("Make Beats")
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Studio Beatmaker (16-Step Drum Sequencer)");
                let run_btn = egui::Button::new(if seq_on { "BEAT: ON" } else { "BEAT: OFF" })
                    .small()
                    .fill(if seq_on {
                        egui::Color32::from_rgb(22, 163, 74)
                    } else {
                        palette.card_fill
                    });
                if ui.add(run_btn).clicked() {
                    let next = !seq_on;
                    self.audio_out.set_sequencer_enabled(next);
                    self.status_msg = format!(
                        "Studio Beatmaker {}",
                        if next {
                            "STARTED (locked to Deck A BPM)"
                        } else {
                            "STOPPED"
                        }
                    );
                }

                egui::ComboBox::from_id_salt("studio_drum_genre")
                    .selected_text(self.studio_genre.label())
                    .width(142.0)
                    .show_ui(ui, |ui| {
                        for g in DrumGenre::ALL {
                            if ui
                                .selectable_label(self.studio_genre == g, g.label())
                                .clicked()
                            {
                                self.studio_genre = g;
                                self.audio_out.set_sequencer_genre(g);
                                self.status_msg = format!("Loaded {} drum pattern", g.label());
                            }
                        }
                    });

                if ui.small_button("Clear").clicked() {
                    self.audio_out.clear_sequencer();
                    self.status_msg = "Cleared 16-step drum grid".to_string();
                }

                ui.label("Vol");
                if ui
                    .add(
                        egui::Slider::new(&mut self.studio_seq_vol, 0.0..=1.0)
                            .show_value(false)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                    )
                    .changed()
                {
                    self.audio_out.set_sequencer_volume(self.studio_seq_vol);
                }
            });
            ui.add_space(4.0);

            for (lane_idx, lane_label) in LANE_LABELS.iter().enumerate().take(STEP_LANES) {
                ui.horizontal(|ui| {
                    ui.add_sized([48.0, 20.0], egui::Label::new(*lane_label));
                    let lane_color = palette.pad_colors[lane_idx % palette.pad_colors.len()];
                    for (step_idx, cell) in steps[lane_idx].iter_mut().enumerate().take(STEP_COUNT)
                    {
                        if step_idx > 0 && step_idx % 4 == 0 {
                            ui.add_space(4.0);
                        }
                        let active = *cell;
                        let is_head = seq_on && cur_step == step_idx;
                        let fill = if active && is_head {
                            egui::Color32::WHITE
                        } else if active {
                            lane_color
                        } else if is_head {
                            egui::Color32::from_rgba_unmultiplied(
                                palette.deck_a.r(),
                                palette.deck_a.g(),
                                palette.deck_a.b(),
                                85,
                            )
                        } else if (step_idx / 4) % 2 == 0 {
                            palette.card_fill
                        } else {
                            palette.chassis
                        };
                        let btn = egui::Button::new("")
                            .min_size(egui::vec2(20.0, 20.0))
                            .fill(fill);
                        if ui.add(btn).clicked() {
                            let next = !active;
                            *cell = next;
                            self.audio_out.set_sequencer_step(lane_idx, step_idx, next);
                        }
                    }
                });
            }
        });

        ui.add_space(6.0);

        // 2. Auto-Slice Sample Chopper ("Find Samples" -> Sampler Bank)
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Auto-Slice Sample Chopper (Find Samples -> 8 Sampler Pads)");
                let bank_labels = ["-> Bank A", "-> Bank B", "-> Bank C", "-> Bank D"];
                egui::ComboBox::from_id_salt("studio_chop_bank")
                    .selected_text(bank_labels[self.studio_chop_bank])
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for (b_idx, lbl) in bank_labels.into_iter().enumerate() {
                            ui.selectable_value(&mut self.studio_chop_bank, b_idx, lbl);
                        }
                    });
                if ui.small_button("Chop Deck A").clicked() {
                    self.chop_deck_into_sampler(0);
                }
                if ui.small_button("Chop Deck B").clicked() {
                    self.chop_deck_into_sampler(1);
                }
                if ui.small_button("Chop Audio File...").clicked() {
                    self.chop_audio_file_dialog();
                }
            });
            ui.add_space(3.0);
            if self.studio_slices.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "Load an audio file on Deck A/B (or click 'Chop Audio File...') to auto-detect 8 transient slices and map them onto the Sampler pads.",
                    )
                    .color(palette.text_secondary),
                );
            } else {
                ui.horizontal_wrapped(|ui| {
                    for (i, sl) in self.studio_slices.iter().enumerate() {
                        let lbl = format!("{} ({:.2}s)", sl.label, sl.start_sec);
                        if ui.small_button(lbl).clicked() {
                            self.audio_out.trigger_sample(self.studio_chop_bank, i);
                        }
                    }
                });
            }
        });
    }

    fn chop_deck_into_sampler(&mut self, deck_idx: usize) {
        let Some(audio) = self
            .deck_decoded
            .get(deck_idx)
            .and_then(|o| o.as_ref())
            .cloned()
        else {
            self.status_msg = format!(
                "Deck {} has no decoded local audio file yet (click AUDIO on that deck first)",
                if deck_idx == 0 { "A" } else { "B" }
            );
            return;
        };
        self.apply_chopped_slices(&audio, if deck_idx == 0 { "Deck A" } else { "Deck B" });
    }

    fn chop_audio_file_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "m4a"])
            .pick_file()
        else {
            return;
        };
        match decode_audio_file(&path) {
            Ok(decoded) => {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Sample");
                self.apply_chopped_slices(&decoded, name);
            }
            Err(e) => self.status_msg = format!("Chop decode failed: {e}"),
        }
    }

    fn apply_chopped_slices(
        &mut self,
        decoded: &djcraft_audio::dsp::DecodedAudio,
        source_name: &str,
    ) {
        let slices = auto_slice_audio(decoded, 8);
        if slices.is_empty() {
            self.status_msg = format!("Could not find slices in '{source_name}' (too short)");
            return;
        }
        let bank = self.studio_chop_bank;
        for (idx, sl) in slices.iter().enumerate().take(8) {
            self.audio_out.set_sample_slot_audio(
                bank,
                idx,
                sl.label.clone(),
                Arc::clone(&sl.audio),
            );
            self.sampler_names[bank][idx] = sl.label.clone();
        }
        let count = slices.len();
        self.studio_slices = slices;
        self.sampler_bank = bank;
        self.status_msg = format!(
            "Auto-chopped '{source_name}' into {count} slices -> Sampler Bank {}",
            ['A', 'B', 'C', 'D'][bank]
        );
    }
}
