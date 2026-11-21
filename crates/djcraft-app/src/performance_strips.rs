//! Collapsible live sampler and three-monitor video strips.

use std::sync::Arc;

use djcraft_audio::dsp::decode_audio_file;
use djcraft_audio::{DeckId, SAMPLER_SLOTS};

use super::skins::SkinPalette;
use super::theme;
use super::video::{DeckVideoFx, VideoTransitionFx};

impl super::DjCraftApp {
    pub(super) fn render_collapsible_sampler_strip(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        let bank = self.sampler_bank;
        let active_mask = self.audio_out.sampler_active_mask(bank);
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Sampler");
                let bank_labels = ["Bank A", "Bank B", "Bank C", "Bank D"];
                egui::ComboBox::from_id_salt("sampler_bank_combo")
                    .selected_text(bank_labels[self.sampler_bank])
                    .width(78.0)
                    .show_ui(ui, |ui| {
                        for (b_idx, lbl) in bank_labels.into_iter().enumerate() {
                            ui.selectable_value(&mut self.sampler_bank, b_idx, lbl);
                        }
                    });
                ui.separator();
                ui.label("Sampler Vol");
                if ui
                    .add(
                        egui::Slider::new(&mut self.sampler_master_vol, 0.0..=1.0)
                            .show_value(false)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                    )
                    .changed()
                {
                    self.audio_out
                        .set_sampler_master_volume(self.sampler_master_vol);
                }
                let stop_btn = egui::Button::new("STOP ALL")
                    .small()
                    .fill(egui::Color32::from_rgb(185, 28, 28));
                if ui.add(stop_btn).clicked() {
                    self.audio_out.stop_all_samples();
                    self.status_msg = "Sampler: stopped all active slots".to_string();
                }
            });
            ui.add_space(4.0);

            ui.columns(SAMPLER_SLOTS, |cols| {
                for (slot_idx, col) in cols.iter_mut().enumerate().take(SAMPLER_SLOTS) {
                    let is_playing = active_mask[slot_idx];
                    let pad_col = palette.pad_colors[slot_idx % palette.pad_colors.len()];
                    let slot_name = self.sampler_names[bank][slot_idx].clone();
                    let mode = self.sampler_modes[bank][slot_idx];

                    col.vertical_centered(|ui| {
                        let fill = if is_playing {
                            pad_col
                        } else {
                            egui::Color32::from_rgba_unmultiplied(
                                pad_col.r(),
                                pad_col.g(),
                                pad_col.b(),
                                if palette.is_light { 55 } else { 70 },
                            )
                        };
                        let fg = if is_playing {
                            egui::Color32::BLACK
                        } else {
                            palette.text_primary
                        };
                        let btn = egui::Button::new(
                            egui::RichText::new(format!("{}. {}", slot_idx + 1, slot_name))
                                .strong()
                                .size(11.5)
                                .color(fg),
                        )
                        .min_size(egui::vec2(ui.available_width(), 28.0))
                        .fill(fill);
                        if ui.add(btn).clicked() {
                            self.audio_out.trigger_sample(bank, slot_idx);
                            self.status_msg = format!(
                                "Sampler Bank {} Slot {}: {}",
                                ['A', 'B', 'C', 'D'][bank],
                                slot_idx + 1,
                                slot_name
                            );
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .small_button(mode.label())
                                .on_hover_text("Toggle Trigger (one-shot) vs Loop")
                                .clicked()
                            {
                                let next = mode.toggle();
                                self.sampler_modes[bank][slot_idx] = next;
                                self.audio_out.set_sample_slot_config(
                                    bank,
                                    slot_idx,
                                    next,
                                    self.sampler_vols[bank][slot_idx],
                                );
                            }
                            if ui
                                .small_button("Load")
                                .on_hover_text("Load WAV/MP3/FLAC/OGG into this sampler pad")
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Audio Samples", &["wav", "mp3", "flac", "ogg", "m4a"])
                                    .pick_file()
                                {
                                    match decode_audio_file(&path) {
                                        Ok(decoded) => {
                                            let name = path
                                                .file_stem()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or("Sample")
                                                .chars()
                                                .take(14)
                                                .collect::<String>();
                                            self.audio_out.set_sample_slot_audio(
                                                bank,
                                                slot_idx,
                                                name.clone(),
                                                Arc::new(decoded),
                                            );
                                            self.sampler_names[bank][slot_idx] = name.clone();
                                            self.status_msg = format!(
                                                "Loaded sample '{}' into Bank {} Slot {}",
                                                name,
                                                ['A', 'B', 'C', 'D'][bank],
                                                slot_idx + 1
                                            );
                                        }
                                        Err(e) => {
                                            self.status_msg = format!("Sample decode error: {e}");
                                        }
                                    }
                                }
                            }
                        });
                    });
                }
            });
        });
    }

    pub(super) fn render_collapsible_live_video_strip(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.columns(3, |cols| {
                // LEFT: Deck A Video Box + Individual FX Dropdown
                cols[0].vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(egui::RichText::new("Deck A Video").color(palette.deck_a));
                        if ui.small_button("Load A").clicked() {
                            self.load_video_file_dialog(DeckId::DeckA);
                        }
                        egui::ComboBox::from_id_salt("live_vfx_a")
                            .selected_text(self.video_fx_a.label())
                            .width(96.0)
                            .show_ui(ui, |ui| {
                                for fx in DeckVideoFx::ALL {
                                    ui.selectable_value(&mut self.video_fx_a, fx, fx.label());
                                }
                            });
                        if self.video_fx_a != DeckVideoFx::None {
                            ui.add(
                                egui::Slider::new(&mut self.video_fx_a_amt, 0.0..=1.0)
                                    .show_value(false),
                            );
                        }
                    });
                    if let Some(tex) = &self.video_tex_a {
                        ui.add(
                            egui::Image::new((tex.id(), egui::vec2(192.0, 108.0)))
                                .corner_radius(4.0),
                        );
                    }
                });

                // CENTER: Mixed Master Video Output + Transition FX Dropdown + Link XF
                cols[1].vertical_centered(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong("Mixed Output");
                        egui::ComboBox::from_id_salt("live_vfx_transition")
                            .selected_text(self.video_transition.label())
                            .width(108.0)
                            .show_ui(ui, |ui| {
                                for tr in VideoTransitionFx::ALL {
                                    ui.selectable_value(&mut self.video_transition, tr, tr.label());
                                }
                            });
                        ui.checkbox(&mut self.video_link_xfader, "Link XF")
                            .on_hover_text("Link video crossfader to audio crossfader");
                        let proj_lbl = if self.video_projector_open {
                            "Close Proj"
                        } else {
                            "Projector"
                        };
                        if ui.small_button(proj_lbl).clicked() {
                            self.video_projector_open = !self.video_projector_open;
                        }
                    });
                    if let Some(tex) = &self.video_texture {
                        ui.add(
                            egui::Image::new((tex.id(), egui::vec2(224.0, 126.0)))
                                .corner_radius(4.0),
                        );
                    }
                    if !self.video_link_xfader {
                        ui.add(
                            egui::Slider::new(&mut self.video_crossfader, -1.0..=1.0)
                                .text("Video XF"),
                        );
                    }
                });

                // RIGHT: Deck B Video Box + Individual FX Dropdown
                cols[2].vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(egui::RichText::new("Deck B Video").color(palette.deck_b));
                        if ui.small_button("Load B").clicked() {
                            self.load_video_file_dialog(DeckId::DeckB);
                        }
                        egui::ComboBox::from_id_salt("live_vfx_b")
                            .selected_text(self.video_fx_b.label())
                            .width(96.0)
                            .show_ui(ui, |ui| {
                                for fx in DeckVideoFx::ALL {
                                    ui.selectable_value(&mut self.video_fx_b, fx, fx.label());
                                }
                            });
                        if self.video_fx_b != DeckVideoFx::None {
                            ui.add(
                                egui::Slider::new(&mut self.video_fx_b_amt, 0.0..=1.0)
                                    .show_value(false),
                            );
                        }
                    });
                    if let Some(tex) = &self.video_tex_b {
                        ui.add(
                            egui::Image::new((tex.id(), egui::vec2(192.0, 108.0)))
                                .corner_radius(4.0),
                        );
                    }
                });
            });
        });
    }
}
