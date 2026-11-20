#![forbid(unsafe_code)]
//! Pro DJ Skin Engine for DJCraft.
//!
//! Ships 5 curated studio palettes (Rane Battle, Pioneer Club, Serato Midnight,
//! VirtualDJ Carbon, Daylight Booth) and supports user-editable JSON skins in
//! `%LOCALAPPDATA%\djcraft\skins\`.

use egui::{Color32, Stroke, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SkinId {
    #[default]
    RaneBattle,
    PioneerClub,
    SeratoMidnight,
    VirtualDjCarbon,
    DaylightBooth,
}

impl SkinId {
    pub const ALL: [Self; 5] = [
        Self::RaneBattle,
        Self::PioneerClub,
        Self::SeratoMidnight,
        Self::VirtualDjCarbon,
        Self::DaylightBooth,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::RaneBattle => "Rane Seventy-Two Battle",
            Self::PioneerClub => "Pioneer Club Dark",
            Self::SeratoMidnight => "Serato Midnight",
            Self::VirtualDjCarbon => "VirtualDJ Carbon",
            Self::DaylightBooth => "Daylight Booth (Light)",
        }
    }

    pub fn palette(self) -> SkinPalette {
        match self {
            Self::RaneBattle => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(9, 10, 14),
                chassis: Color32::from_rgb(18, 20, 27),
                card_fill: Color32::from_rgb(24, 27, 36),
                card_border: Color32::from_rgb(54, 60, 78),
                text_primary: Color32::from_rgb(244, 246, 250),
                text_secondary: Color32::from_rgb(168, 176, 192),
                deck_a: Color32::from_rgb(56, 189, 248),  // Electric Cyan
                deck_b: Color32::from_rgb(239, 68, 68),   // Rane Crimson
                deck_c: Color32::from_rgb(52, 211, 153),  // Emerald
                deck_d: Color32::from_rgb(250, 204, 21),  // Gold
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
            Self::PioneerClub => SkinPalette {
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
            Self::SeratoMidnight => SkinPalette {
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
            Self::VirtualDjCarbon => SkinPalette {
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
            Self::DaylightBooth => SkinPalette {
                is_light: true,
                canvas: Color32::from_rgb(241, 245, 249),
                chassis: Color32::from_rgb(248, 250, 252),
                card_fill: Color32::WHITE,
                card_border: Color32::from_rgb(186, 198, 214),
                text_primary: Color32::from_rgb(15, 23, 42),
                text_secondary: Color32::from_rgb(51, 65, 85),
                deck_a: Color32::from_rgb(2, 132, 199),
                deck_b: Color32::from_rgb(217, 119, 6),
                deck_c: Color32::from_rgb(5, 150, 105),
                deck_d: Color32::from_rgb(124, 58, 237),
                knob_body: Color32::from_rgb(226, 232, 240),
                knob_track: Color32::from_rgb(148, 163, 184),
                fader_slot: Color32::from_rgb(203, 213, 225),
                fader_cap: Color32::from_rgb(30, 41, 59),
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

impl SkinPalette {
    pub fn to_visuals(self) -> Visuals {
        let mut visuals = if self.is_light {
            Visuals::light()
        } else {
            Visuals::dark()
        };
        visuals.panel_fill = self.canvas;
        visuals.window_fill = self.card_fill;
        visuals.extreme_bg_color = self.fader_slot;
        visuals.faint_bg_color = self.chassis;
        visuals.override_text_color = Some(self.text_primary);
        visuals.weak_text_color = Some(self.text_secondary);
        visuals.window_stroke = Stroke::new(1.0, self.card_border);

        visuals.widgets.noninteractive.bg_fill = self.card_fill;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.card_border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_primary);

        let inactive_bg = if self.is_light {
            Color32::from_rgb(226, 232, 240)
        } else {
            self.knob_body
        };
        let hovered_bg = if self.is_light {
            Color32::from_rgb(203, 213, 225)
        } else {
            self.knob_track
        };

        visuals.widgets.inactive.bg_fill = inactive_bg;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, self.card_border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_primary);

        visuals.widgets.hovered.bg_fill = hovered_bg;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, self.deck_a);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, self.text_primary);

        visuals.widgets.active.bg_fill = self.deck_a;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5, self.text_primary);
        visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::BLACK);
        visuals
    }
}
