//! AI Lighting Assistant Dock (Art-Net 4 & sACN E1.31 DMX512).

use djcraft_core::{FixtureKind, IpcResponse, LightingProtocol, LightingSceneMode, PatchedFixture};

use super::skins::SkinPalette;
use super::theme;

impl super::DjCraftApp {
    pub(super) fn tick_lighting_assistant(&mut self) {
        let bpm = if self.orch.deck_a.is_playing {
            self.orch
                .deck_a
                .loaded_track
                .as_ref()
                .map_or(self.link_engine.bpm, |t| t.bpm)
                * (1.0 + self.orch.deck_a.pitch_bend_pct / 100.0)
        } else if self.orch.deck_b.is_playing {
            self.orch
                .deck_b
                .loaded_track
                .as_ref()
                .map_or(self.link_engine.bpm, |t| t.bpm)
                * (1.0 + self.orch.deck_b.pitch_bend_pct / 100.0)
        } else {
            self.link_engine.bpm
        };
        let active_decks = [
            self.orch.deck_a.is_playing,
            self.orch.deck_b.is_playing,
            self.orch.deck_c.is_playing,
            self.orch.deck_d.is_playing,
        ]
        .into_iter()
        .filter(|&p| p)
        .count() as f32;
        let energy = if active_decks > 0.0 {
            (0.45 + active_decks * 0.2).clamp(0.0, 1.0)
        } else {
            0.2
        };
        self.lighting
            .tick_and_send(bpm, self.link_engine.beat_phase, energy);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_lighting_ipc(
        &mut self,
        action: &str,
        enabled: Option<bool>,
        protocol: Option<&str>,
        target_ip: Option<&str>,
        universe: Option<u16>,
        mode: Option<&str>,
        dimmer: Option<f32>,
        strobe: Option<bool>,
    ) -> IpcResponse {
        if let Some(en) = enabled {
            self.lighting.enabled = en;
        }
        if let Some(proto) = protocol {
            self.lighting.protocol = match proto.trim().to_lowercase().as_str() {
                "sacn" | "e131" | "e1.31" => LightingProtocol::SacnE131,
                _ => LightingProtocol::ArtNet,
            };
        }
        if let Some(ip) = target_ip.filter(|s| !s.trim().is_empty()) {
            self.lighting.target_ip = ip.trim().to_string();
        }
        if let Some(u) = universe {
            self.lighting.universe = u;
        }
        if let Some(m) = mode {
            match LightingSceneMode::parse(m) {
                Ok(parsed) => self.lighting.mode = parsed,
                Err(e) => return IpcResponse::err(e),
            }
        }
        if let Some(d) = dimmer {
            self.lighting.master_dimmer = d.clamp(0.0, 1.0);
        }
        if let Some(s) = strobe {
            self.lighting.strobe_enable = s;
        }

        match action.trim().to_lowercase().as_str() {
            "status" | "configure" => {}
            "on" | "start" => self.lighting.enabled = true,
            "off" | "stop" => self.lighting.enabled = false,
            "blackout" => {
                self.lighting.mode = LightingSceneMode::Blackout;
                self.lighting.compute_frame(120.0, 0.0, 0.0);
                let _ = self.lighting.send_now();
            }
            other => {
                return IpcResponse::err(format!(
                    "Unknown lighting action '{other}' (expected status, configure, on, off, blackout)"
                ));
            }
        }

        IpcResponse::ok(serde_json::json!({
            "enabled": self.lighting.enabled,
            "protocol": self.lighting.protocol.label(),
            "target_ip": self.lighting.target_ip,
            "universe": self.lighting.universe,
            "mode": self.lighting.mode.label(),
            "master_dimmer": self.lighting.master_dimmer,
            "strobe_enable": self.lighting.strobe_enable,
            "packets_sent": self.lighting.packets_sent,
            "fixtures_count": self.lighting.fixtures.len(),
            "last_error": self.lighting.last_error
        }))
    }

    pub(super) fn render_lighting_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        ui.columns(2, |cols| {
            theme::card_frame(&mut cols[0], palette, |ui| {
                ui.strong("AI Lighting Assistant (Hands-Free Art-Net 4 / sACN DMX)");
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.lighting.enabled, "Transmit Live UDP");
                    if ui.button("BLACKOUT").clicked() {
                        self.lighting.mode = LightingSceneMode::Blackout;
                        self.lighting.compute_frame(120.0, 0.0, 0.0);
                        let _ = self.lighting.send_now();
                    }
                    ui.small(format!("Packets: {}", self.lighting.packets_sent));
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label("Protocol:");
                    for (lbl, p) in [
                        ("Art-Net :6454", LightingProtocol::ArtNet),
                        ("sACN :5568", LightingProtocol::SacnE131),
                    ] {
                        if ui
                            .selectable_label(self.lighting.protocol == p, lbl)
                            .clicked()
                        {
                            self.lighting.protocol = p;
                        }
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label("Target IP:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.lighting.target_ip)
                            .desired_width(120.0),
                    );
                    ui.label("Univ:");
                    ui.add(egui::DragValue::new(&mut self.lighting.universe).range(0..=255));
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label("AI Mode:");
                    for m in LightingSceneMode::ALL {
                        if ui
                            .selectable_label(self.lighting.mode == m, m.label())
                            .clicked()
                        {
                            self.lighting.mode = m;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Dimmer:");
                    ui.add(
                        egui::Slider::new(&mut self.lighting.master_dimmer, 0.0..=1.0)
                            .show_value(false),
                    );
                    ui.checkbox(&mut self.lighting.strobe_enable, "Strobe");
                });

                if let Some(ref err) = self.lighting.last_error {
                    ui.colored_label(egui::Color32::from_rgb(235, 80, 80), err);
                }
            });

            theme::card_frame(&mut cols[1], palette, |ui| {
                ui.strong("Patched Fixtures & Live DMX Output (Ch 1-32)");
                ui.add_space(4.0);
                // Live 32-channel DMX bar preview
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 32.0),
                    egui::Sense::hover(),
                );
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 3.0, palette.canvas);
                let col_w = (rect.width() / 32.0).max(2.0);
                for ch in 0..32 {
                    let v = self.lighting.dmx[ch] as f32 / 255.0;
                    let h = rect.height() * v;
                    let bar = egui::Rect::from_min_max(
                        egui::pos2(rect.left() + ch as f32 * col_w + 1.0, rect.bottom() - h),
                        egui::pos2(rect.left() + (ch + 1) as f32 * col_w - 1.0, rect.bottom()),
                    );
                    let color = match ch % 4 {
                        0 => egui::Color32::from_rgb(240, 80, 90),
                        1 => egui::Color32::from_rgb(80, 220, 120),
                        2 => egui::Color32::from_rgb(70, 170, 255),
                        _ => egui::Color32::from_rgb(245, 220, 110),
                    };
                    painter.rect_filled(bar, 1.0, color);
                }

                ui.add_space(4.0);
                for fx in &self.lighting.fixtures {
                    ui.small(format!(
                        "Ch {:>2}-{:<2} | {} ({})",
                        fx.start_channel,
                        fx.start_channel as usize + fx.kind.channels() - 1,
                        fx.name,
                        fx.kind.label()
                    ));
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("+ RGBW Par").clicked() {
                        let next_ch = self
                            .lighting
                            .fixtures
                            .iter()
                            .map(|f| f.start_channel + f.kind.channels() as u16)
                            .max()
                            .unwrap_or(1);
                        if next_ch + 3 <= 512 {
                            self.lighting.fixtures.push(PatchedFixture {
                                name: format!("Par {}", self.lighting.fixtures.len() + 1),
                                kind: FixtureKind::RgbwPar,
                                start_channel: next_ch,
                            });
                        }
                    }
                    if ui.small_button("+ Moving Head").clicked() {
                        let next_ch = self
                            .lighting
                            .fixtures
                            .iter()
                            .map(|f| f.start_channel + f.kind.channels() as u16)
                            .max()
                            .unwrap_or(1);
                        if next_ch + 7 <= 512 {
                            self.lighting.fixtures.push(PatchedFixture {
                                name: format!("Mover {}", self.lighting.fixtures.len() + 1),
                                kind: FixtureKind::MovingHead,
                                start_channel: next_ch,
                            });
                        }
                    }
                });
            });
        });
    }
}
