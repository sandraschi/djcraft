#![forbid(unsafe_code)]
//! Pro DJ Skin Engine for DJCraft.
//!
//! Ships 6 curated studio palettes (Battle Console, Club Dark, Midnight Booth,
//! Carbon Deck, Shocking Pink Club, Daylight Booth) and supports user-editable JSON skins in
//! `%LOCALAPPDATA%\djcraft\skins\`.

use egui::{Color32, Stroke, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SkinId {
    #[default]
    #[serde(alias = "RaneBattle")]
    BattleConsole,
    #[serde(alias = "PioneerClub")]
    ClubDark,
    #[serde(alias = "SeratoMidnight")]
    MidnightBooth,
    #[serde(alias = "VirtualDjCarbon")]
    CarbonDeck,
    ShockingPink,
    DaylightBooth,
}

impl SkinId {
    pub const ALL: [Self; 6] = [
        Self::BattleConsole,
        Self::ClubDark,
        Self::MidnightBooth,
        Self::CarbonDeck,
        Self::ShockingPink,
        Self::DaylightBooth,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::BattleConsole => "Battle Console",
            Self::ClubDark => "Club Dark",
            Self::MidnightBooth => "Midnight Booth",
            Self::CarbonDeck => "Carbon Deck",
            Self::ShockingPink => "Shocking Pink Club",
            Self::DaylightBooth => "Daylight Booth (Light)",
        }
    }

    pub fn palette(self) -> SkinPalette {
        match self {
            Self::BattleConsole => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(9, 10, 14),
                chassis: Color32::from_rgb(18, 20, 27),
                card_fill: Color32::from_rgb(24, 27, 36),
                card_border: Color32::from_rgb(54, 60, 78),
                text_primary: Color32::from_rgb(244, 246, 250),
                text_secondary: Color32::from_rgb(168, 176, 192),
                deck_a: Color32::from_rgb(56, 189, 248), // Electric Cyan
                deck_b: Color32::from_rgb(239, 68, 68),  // Battle Crimson
                deck_c: Color32::from_rgb(52, 211, 153), // Emerald
                deck_d: Color32::from_rgb(250, 204, 21), // Gold
                knob_body: Color32::from_rgb(28, 32, 42),
                knob_track: Color32::from_rgb(48, 54, 70),
                fader_slot: Color32::from_rgb(8, 9, 12),
                fader_cap: Color32::from_rgb(196, 202, 214),
                pad_colors: [
                    Color32::from_rgb(239, 68, 68),
                    Color32::from_rgb(249, 115, 22),
                    Color32::from_rgb(250, 204, 21),
                    Color32::from_rgb(34, 197, 94),
                    Color32::from_rgb(56, 189, 248),
                    Color32::from_rgb(99, 102, 241),
                    Color32::from_rgb(192, 132, 252),
                    Color32::from_rgb(236, 72, 153),
                ],
            },
            Self::ClubDark => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(8, 12, 18),
                chassis: Color32::from_rgb(15, 23, 38),
                card_fill: Color32::from_rgb(19, 30, 50),
                card_border: Color32::from_rgb(42, 60, 92),
                text_primary: Color32::from_rgb(245, 248, 252),
                text_secondary: Color32::from_rgb(176, 190, 210),
                deck_a: Color32::from_rgb(56, 189, 248),
                deck_b: Color32::from_rgb(245, 158, 11),
                deck_c: Color32::from_rgb(34, 197, 94),
                deck_d: Color32::from_rgb(168, 85, 247),
                knob_body: Color32::from_rgb(24, 34, 54),
                knob_track: Color32::from_rgb(44, 62, 94),
                fader_slot: Color32::from_rgb(6, 9, 15),
                fader_cap: Color32::from_rgb(210, 218, 230),
                pad_colors: [
                    Color32::from_rgb(56, 189, 248),
                    Color32::from_rgb(59, 130, 246),
                    Color32::from_rgb(34, 197, 94),
                    Color32::from_rgb(234, 179, 8),
                    Color32::from_rgb(249, 115, 22),
                    Color32::from_rgb(239, 68, 68),
                    Color32::from_rgb(236, 72, 153),
                    Color32::from_rgb(168, 85, 247),
                ],
            },
            Self::MidnightBooth => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(6, 7, 10),
                chassis: Color32::from_rgb(14, 16, 22),
                card_fill: Color32::from_rgb(20, 22, 30),
                card_border: Color32::from_rgb(46, 52, 68),
                text_primary: Color32::from_rgb(242, 245, 250),
                text_secondary: Color32::from_rgb(156, 164, 180),
                deck_a: Color32::from_rgb(34, 211, 238),
                deck_b: Color32::from_rgb(244, 114, 182),
                deck_c: Color32::from_rgb(163, 230, 53),
                deck_d: Color32::from_rgb(251, 146, 60),
                knob_body: Color32::from_rgb(22, 25, 34),
                knob_track: Color32::from_rgb(50, 56, 74),
                fader_slot: Color32::from_rgb(4, 5, 8),
                fader_cap: Color32::from_rgb(220, 224, 232),
                pad_colors: [
                    Color32::from_rgb(34, 211, 238),
                    Color32::from_rgb(163, 230, 53),
                    Color32::from_rgb(250, 204, 21),
                    Color32::from_rgb(251, 146, 60),
                    Color32::from_rgb(244, 114, 182),
                    Color32::from_rgb(192, 132, 252),
                    Color32::from_rgb(96, 165, 250),
                    Color32::from_rgb(248, 113, 113),
                ],
            },
            Self::CarbonDeck => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(14, 16, 20),
                chassis: Color32::from_rgb(24, 28, 34),
                card_fill: Color32::from_rgb(30, 35, 43),
                card_border: Color32::from_rgb(64, 72, 86),
                text_primary: Color32::from_rgb(245, 247, 250),
                text_secondary: Color32::from_rgb(176, 184, 196),
                deck_a: Color32::from_rgb(20, 184, 166),
                deck_b: Color32::from_rgb(249, 115, 22),
                deck_c: Color32::from_rgb(96, 165, 250),
                deck_d: Color32::from_rgb(244, 63, 94),
                knob_body: Color32::from_rgb(36, 42, 52),
                knob_track: Color32::from_rgb(68, 76, 92),
                fader_slot: Color32::from_rgb(10, 12, 16),
                fader_cap: Color32::from_rgb(214, 220, 228),
                pad_colors: [
                    Color32::from_rgb(20, 184, 166),
                    Color32::from_rgb(249, 115, 22),
                    Color32::from_rgb(56, 189, 248),
                    Color32::from_rgb(250, 204, 21),
                    Color32::from_rgb(34, 197, 94),
                    Color32::from_rgb(244, 63, 94),
                    Color32::from_rgb(168, 85, 247),
                    Color32::from_rgb(236, 72, 153),
                ],
            },
            Self::ShockingPink => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(20, 4, 18),
                chassis: Color32::from_rgb(34, 8, 30),
                card_fill: Color32::from_rgb(44, 11, 38),
                card_border: Color32::from_rgb(255, 46, 151),
                text_primary: Color32::from_rgb(255, 245, 252),
                text_secondary: Color32::from_rgb(255, 182, 224),
                deck_a: Color32::from_rgb(255, 20, 147), // Shocking Pink
                deck_b: Color32::from_rgb(0, 245, 255),  // Electric Cyan
                deck_c: Color32::from_rgb(163, 255, 18), // Acid Lime
                deck_d: Color32::from_rgb(255, 215, 0),  // Hot Gold
                knob_body: Color32::from_rgb(56, 14, 48),
                knob_track: Color32::from_rgb(122, 28, 88),
                fader_slot: Color32::from_rgb(14, 2, 12),
                fader_cap: Color32::from_rgb(255, 165, 220),
                pad_colors: [
                    Color32::from_rgb(255, 20, 147),
                    Color32::from_rgb(255, 105, 180),
                    Color32::from_rgb(0, 245, 255),
                    Color32::from_rgb(163, 255, 18),
                    Color32::from_rgb(255, 215, 0),
                    Color32::from_rgb(255, 94, 0),
                    Color32::from_rgb(217, 70, 239),
                    Color32::from_rgb(244, 63, 94),
                ],
            },
            Self::DaylightBooth => SkinPalette {
                is_light: true,
                canvas: Color32::from_rgb(226, 232, 240),
                chassis: Color32::from_rgb(238, 242, 246),
                card_fill: Color32::from_rgb(255, 255, 255),
                card_border: Color32::from_rgb(100, 116, 139),
                text_primary: Color32::from_rgb(2, 6, 23),
                text_secondary: Color32::from_rgb(30, 41, 59),
                deck_a: Color32::from_rgb(3, 105, 161),
                deck_b: Color32::from_rgb(194, 65, 12),
                deck_c: Color32::from_rgb(4, 120, 87),
                deck_d: Color32::from_rgb(109, 40, 217),
                knob_body: Color32::from_rgb(203, 213, 225),
                knob_track: Color32::from_rgb(100, 116, 139),
                fader_slot: Color32::from_rgb(148, 163, 184),
                fader_cap: Color32::from_rgb(15, 23, 42),
                pad_colors: [
                    Color32::from_rgb(220, 38, 38),
                    Color32::from_rgb(234, 88, 12),
                    Color32::from_rgb(202, 138, 4),
                    Color32::from_rgb(22, 163, 74),
                    Color32::from_rgb(2, 132, 199),
                    Color32::from_rgb(79, 70, 229),
                    Color32::from_rgb(147, 51, 234),
                    Color32::from_rgb(219, 39, 119),
                ],
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SkinPalette {
    pub is_light: bool,
    pub canvas: Color32,
    pub chassis: Color32,
    pub card_fill: Color32,
    pub card_border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub deck_a: Color32,
    pub deck_b: Color32,
    pub deck_c: Color32,
    pub deck_d: Color32,
    pub knob_body: Color32,
    pub knob_track: Color32,
    pub fader_slot: Color32,
    pub fader_cap: Color32,
    pub pad_colors: [Color32; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BackdropStyle {
    Solid,
    BrushedMetal,
    CarbonWeave,
    #[default]
    SparkleClub,
}

impl BackdropStyle {
    pub const ALL: [Self; 4] = [
        Self::Solid,
        Self::BrushedMetal,
        Self::CarbonWeave,
        Self::SparkleClub,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::BrushedMetal => "Brushed Metal",
            Self::CarbonWeave => "Carbon Weave",
            Self::SparkleClub => "Sparkle Club",
        }
    }
}

impl SkinPalette {
    pub fn to_visuals(self, transparent_canvas: bool) -> Visuals {
        let mut visuals = if self.is_light {
            Visuals::light()
        } else {
            Visuals::dark()
        };
        visuals.panel_fill = if transparent_canvas {
            Color32::from_rgba_unmultiplied(
                self.canvas.r(),
                self.canvas.g(),
                self.canvas.b(),
                if self.is_light { 165 } else { 155 },
            )
        } else {
            self.canvas
        };
        visuals.window_fill = self.card_fill;
        visuals.extreme_bg_color = if self.is_light {
            Color32::from_rgb(241, 245, 249)
        } else {
            self.fader_slot
        };
        visuals.faint_bg_color = self.chassis;
        visuals.override_text_color = Some(self.text_primary);
        visuals.weak_text_color = Some(self.text_secondary);
        visuals.hyperlink_color = self.deck_a;
        visuals.window_stroke = Stroke::new(1.0, self.card_border);

        visuals.widgets.noninteractive.bg_fill = self.card_fill;
        visuals.widgets.noninteractive.weak_bg_fill = self.chassis;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.card_border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_primary);

        let inactive_bg = if self.is_light {
            Color32::from_rgb(203, 213, 225)
        } else {
            self.knob_body
        };
        let hovered_bg = if self.is_light {
            Color32::from_rgb(186, 200, 218)
        } else {
            self.knob_track
        };

        visuals.widgets.inactive.bg_fill = inactive_bg;
        visuals.widgets.inactive.weak_bg_fill = inactive_bg;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, self.card_border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_primary);

        visuals.widgets.hovered.bg_fill = hovered_bg;
        visuals.widgets.hovered.weak_bg_fill = hovered_bg;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, self.deck_a);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, self.text_primary);

        visuals.widgets.open.bg_fill = hovered_bg;
        visuals.widgets.open.weak_bg_fill = hovered_bg;
        visuals.widgets.open.bg_stroke = Stroke::new(1.5, self.deck_a);
        visuals.widgets.open.fg_stroke = Stroke::new(1.5, self.text_primary);

        let active_fg = if self.is_light {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        visuals.widgets.active.bg_fill = self.deck_a;
        visuals.widgets.active.weak_bg_fill = self.deck_a;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5, self.text_primary);
        visuals.widgets.active.fg_stroke = Stroke::new(1.5, active_fg);

        visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(
            self.deck_a.r(),
            self.deck_a.g(),
            self.deck_a.b(),
            if self.is_light { 70 } else { 95 },
        );
        visuals.selection.stroke = Stroke::new(1.2, self.text_primary);
        visuals
    }
}

/// Paint a textured or dynamic sparkly club underlay behind all UI panels.
pub fn paint_backdrop(
    ctx: &egui::Context,
    palette: &SkinPalette,
    style: BackdropStyle,
    time_sec: f32,
    energy: f32,
) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(rect, 0.0, palette.canvas);

    match style {
        BackdropStyle::Solid => {}
        BackdropStyle::BrushedMetal => {
            let step = 6.0_f32;
            let mut y = rect.top();
            let mut row = 0u32;
            while y < rect.bottom() {
                let alpha = if row.is_multiple_of(2) { 16 } else { 7 };
                let col = if palette.is_light {
                    Color32::from_rgba_unmultiplied(15, 23, 42, alpha)
                } else {
                    Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
                };
                painter.line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    Stroke::new(1.0, col),
                );
                y += step;
                row += 1;
            }
        }
        BackdropStyle::CarbonWeave => {
            let step = 18.0_f32;
            let col_a = Color32::from_rgba_unmultiplied(
                palette.deck_a.r(),
                palette.deck_a.g(),
                palette.deck_a.b(),
                if palette.is_light { 24 } else { 22 },
            );
            let col_b = Color32::from_rgba_unmultiplied(
                palette.deck_b.r(),
                palette.deck_b.g(),
                palette.deck_b.b(),
                18,
            );
            let mut x = rect.left() - rect.height();
            let mut idx = 0u32;
            while x < rect.right() + rect.height() {
                let col = if idx.is_multiple_of(2) { col_a } else { col_b };
                painter.line_segment(
                    [
                        egui::pos2(x, rect.top()),
                        egui::pos2(x + rect.height(), rect.bottom()),
                    ],
                    Stroke::new(1.2, col),
                );
                x += step;
                idx += 1;
            }
        }
        BackdropStyle::SparkleClub => {
            // Soft ambient glow orbs
            for i in 0..6u32 {
                let fi = i as f32;
                let px = rect.left()
                    + rect.width()
                        * (0.12 + 0.76 * ((fi * 0.37 + time_sec * 0.03).sin() * 0.5 + 0.5));
                let py = rect.top()
                    + rect.height()
                        * (0.15 + 0.70 * ((fi * 0.53 + time_sec * 0.025).cos() * 0.5 + 0.5));
                let base = if i % 2 == 0 {
                    palette.deck_a
                } else {
                    palette.deck_b
                };
                let r = 90.0 + 35.0 * energy;
                painter.circle_filled(
                    egui::pos2(px, py),
                    r,
                    Color32::from_rgba_unmultiplied(
                        base.r(),
                        base.g(),
                        base.b(),
                        if palette.is_light { 22 } else { 26 },
                    ),
                );
            }
            // Animated diamond sparkles
            for i in 0..68u32 {
                let seed_x = ((i.wrapping_mul(73856093) ^ 0x5A17) % 1000) as f32 / 1000.0;
                let seed_y = ((i.wrapping_mul(19349663) ^ 0x2F91) % 1000) as f32 / 1000.0;
                let phase = (i as f32) * 1.13 + time_sec * (1.8 + (i % 5) as f32 * 0.45);
                let twinkle = (phase.sin() * 0.5 + 0.5).powf(2.4);
                let x = rect.left() + seed_x * rect.width();
                let y = rect.top() + seed_y * rect.height();
                let col = match i % 3 {
                    0 => palette.deck_a,
                    1 => palette.deck_b,
                    _ => palette.pad_colors[(i as usize) % 8],
                };
                let alpha = ((40.0 + 185.0 * twinkle * (0.65 + 0.35 * energy)) as u8).min(230);
                let c = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha);
                let sz = 1.6 + 3.4 * twinkle;
                painter.circle_filled(egui::pos2(x, y), sz * 0.45, c);
                if twinkle > 0.55 {
                    painter.line_segment(
                        [egui::pos2(x - sz * 1.6, y), egui::pos2(x + sz * 1.6, y)],
                        Stroke::new(1.0, c),
                    );
                    painter.line_segment(
                        [egui::pos2(x, y - sz * 1.6), egui::pos2(x, y + sz * 1.6)],
                        Stroke::new(1.0, c),
                    );
                }
            }
        }
    }
}
