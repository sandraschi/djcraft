//! Real-time 3-slot FX rack dock.

use djcraft_audio::{
    builtin_fx_presets, fx_packs_dir, load_user_fx_presets_from_dir, save_fx_preset_to_dir, FxKind,
    FxPackId, FxPackPreset, FxStemScope, FxTarget,
};
use djcraft_core::Command;

use super::skins::SkinPalette;
use super::theme;

impl super::DjCraftApp {
    pub(super) fn render_fx_rack_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        theme::card_frame(ui, palette, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Real-Time DJ FX Rack (3 Free Built-In Packs: Club Core, Modulation, Battle & Lo-Fi)");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Save FX 1 Pack").clicked() {
                        let u0 = &self.orch.fx_units[0];
                        let preset = FxPackPreset {
                            name: self.fx_save_name.trim().to_string(),
                            pack: FxPackId::ClubCore,
                            beats: u0.beats,
                            wet_dry: u0.wet_dry,
                            stem_scope: u0.stem_scope,
                            slots: u0.slots,
                        };
                        match save_fx_preset_to_dir(&fx_packs_dir(), &preset) {
                            Ok(p) => {
                                self.fx_user_presets = load_user_fx_presets_from_dir(&fx_packs_dir());
                                self.status_msg = format!("Saved FX preset -> {}", p.display());
                            }
                            Err(e) => self.status_msg = format!("FX save error: {e}"),
                        }
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.fx_save_name)
                            .desired_width(120.0)
                            .hint_text("Preset name..."),
                    );
                });
            });
            ui.add_space(4.0);

            // Built-in & User FX Pack Presets list (loaded via compact per-bank dropdown)
            let mut presets = builtin_fx_presets();
            presets.extend(self.fx_user_presets.clone());
            ui.add_space(4.0);

            // Side-by-side FX Bank 1 and FX Bank 2
            let mut pending_cmd: Option<Command> = None;
            ui.columns(2, |cols| {
                for (unit_idx, col) in cols.iter_mut().enumerate().take(2) {
                    let accent = if unit_idx == 0 {
                        egui::Color32::from_rgb(168, 85, 247)
                    } else {
                        egui::Color32::from_rgb(236, 72, 153)
                    };
                    let mut p = self.orch.fx_units[unit_idx].clone();
                    let mut changed = false;

                    theme::card_frame(col, palette, |ui| {
                        ui.push_id(("fx_bank", unit_idx), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let on_btn = egui::Button::new(format!(
                                    "FX {} {}",
                                    unit_idx + 1,
                                    if p.enabled { "ON" } else { "OFF" }
                                ))
                                .small()
                                .fill(if p.enabled { accent } else { palette.card_fill });
                                if ui.add(on_btn).clicked() {
                                    p.enabled = !p.enabled;
                                    changed = true;
                                }

                                egui::ComboBox::from_id_salt(("fx_target", unit_idx))
                                    .selected_text(p.target.label())
                                    .width(72.0)
                                    .show_ui(ui, |ui| {
                                        for tgt in FxTarget::ALL {
                                            if ui
                                                .selectable_label(p.target == tgt, tgt.label())
                                                .clicked()
                                            {
                                                p.target = tgt;
                                                changed = true;
                                            }
                                        }
                                    });

                                let scope_ui = egui::ComboBox::from_id_salt(("fx_scope", unit_idx))
                                    .selected_text(p.stem_scope.label())
                                    .width(84.0)
                                    .show_ui(ui, |ui| {
                                        for sc in FxStemScope::ALL {
                                            if ui
                                                .selectable_label(p.stem_scope == sc, sc.label())
                                                .clicked()
                                            {
                                                p.stem_scope = sc;
                                                changed = true;
                                            }
                                        }
                                    });
                                scope_ui.response.on_hover_text(
                                    "Vocals and Drums apply only when this deck has a .djcraft-stems sidecar. Without one, those scopes stay dry.",
                                );

                                let beat_options = [
                                    ("1/8", 0.125),
                                    ("1/4", 0.25),
                                    ("1/2", 0.5),
                                    ("1 Beat", 1.0),
                                    ("2 Beats", 2.0),
                                    ("4 Beats", 4.0),
                                ];
                                let cur_beat_lbl = beat_options
                                    .iter()
                                    .find(|(_, v)| (p.beats - *v).abs() < 0.01)
                                    .map(|(l, _)| *l)
                                    .unwrap_or("1/2");
                                egui::ComboBox::from_id_salt(("fx_beats", unit_idx))
                                    .selected_text(cur_beat_lbl)
                                    .width(66.0)
                                    .show_ui(ui, |ui| {
                                        for (lbl, val) in beat_options {
                                            if ui
                                                .selectable_label((p.beats - val).abs() < 0.01, lbl)
                                                .clicked()
                                            {
                                                p.beats = val;
                                                changed = true;
                                            }
                                        }
                                    });

                                egui::ComboBox::from_id_salt(("fx_preset_pick", unit_idx))
                                    .selected_text("Preset...")
                                    .width(96.0)
                                    .show_ui(ui, |ui| {
                                        for pr in &presets {
                                            if ui.selectable_label(false, &pr.name).clicked() {
                                                pending_cmd = Some(Command::LoadFxPreset {
                                                    unit: unit_idx,
                                                    preset_name: pr.name.clone(),
                                                });
                                            }
                                        }
                                    });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Wet/Dry");
                                if ui
                                    .add(
                                        egui::Slider::new(&mut p.wet_dry, 0.0..=1.0)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });

                            ui.separator();
                            // 3 Chained Multi-FX Slots
                            for (slot_idx, slot) in p.slots.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut slot.enabled, format!("S{}", slot_idx + 1)).changed() {
                                        changed = true;
                                    }
                                    let cur_lbl =
                                        format!("[{}] {}", slot.kind.pack().label().split(' ').next().unwrap_or("FX"), slot.kind.label());
                                    egui::ComboBox::from_id_salt(("fx_slot_kind", unit_idx, slot_idx))
                                        .selected_text(cur_lbl)
                                        .width(155.0)
                                        .show_ui(ui, |ui| {
                                            for k in FxKind::ALL {
                                                let lbl = format!(
                                                    "[{}] {}",
                                                    k.pack().label().split(' ').next().unwrap_or("FX"),
                                                    k.label()
                                                );
                                                if ui.selectable_label(slot.kind == k, lbl).clicked() {
                                                    slot.kind = k;
                                                    changed = true;
                                                }
                                            }
                                        });
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut slot.depth, 0.0..=1.0)
                                                .show_value(false),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });
                            }
                        });
                    });

                    if changed {
                        self.orch.fx_units[unit_idx] = p.clone();
                        pending_cmd = Some(Command::SetFxUnit {
                            unit: unit_idx,
                            params: p,
                        });
                    }
                }
            });
            if let Some(cmd) = pending_cmd {
                self.dispatch_and_report(cmd);
            }
        });
    }
}
