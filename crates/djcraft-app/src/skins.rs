#![forbid(unsafe_code)]
//! Pro DJ Skin Engine for DJCraft.
//!
//! Ships 9 curated studio palettes (Battle Console, Club Dark, Midnight Booth,
//! Carbon Deck, Shocking Pink Club, Daylight Booth, Star Trek LCARS,
//! Baroque Velvet & Gold, Synthwave Outrun) plus bold background patterns
//! and configurable card decorations.

use egui::{Color32, Stroke, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CardDecorStyle {
    Clean,
    #[default]
    DropShadow,
    NeonGlow,
    OrnateDoubleBorder,
    LcarsBracket,
}

impl CardDecorStyle {
    pub const ALL: [Self; 5] = [
        Self::Clean,
        Self::DropShadow,
        Self::NeonGlow,
        Self::OrnateDoubleBorder,
        Self::LcarsBracket,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Clean => "Clean Flat",
            Self::DropShadow => "Bold Drop Shadow",
            Self::NeonGlow => "Neon Outer Glow",
            Self::OrnateDoubleBorder => "Baroque Double Frame",
            Self::LcarsBracket => "LCARS Starfleet Bracket",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CardDecorConfig {
    pub style: CardDecorStyle,
    pub border_width: f32,
    pub card_opacity: f32,
    pub color_boost: f32,
}

impl Default for CardDecorConfig {
    fn default() -> Self {
        Self {
            style: CardDecorStyle::DropShadow,
            border_width: 1.8,
            card_opacity: 0.88,
            color_boost: 0.25,
        }
    }
}

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
    LcarsStarfleet,
    BaroqueGold,
    SynthwaveOutrun,
}

impl SkinId {
    pub const ALL: [Self; 9] = [
        Self::BattleConsole,
        Self::ClubDark,
        Self::MidnightBooth,
        Self::CarbonDeck,
        Self::ShockingPink,
        Self::DaylightBooth,
        Self::LcarsStarfleet,
        Self::BaroqueGold,
        Self::SynthwaveOutrun,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::BattleConsole => "Battle Console",
            Self::ClubDark => "Club Dark",
            Self::MidnightBooth => "Midnight Booth",
            Self::CarbonDeck => "Carbon Deck",
            Self::ShockingPink => "Shocking Pink Club",
            Self::DaylightBooth => "Daylight Booth (Light)",
            Self::LcarsStarfleet => "Star Trek LCARS",
            Self::BaroqueGold => "Baroque Velvet & Gold",
            Self::SynthwaveOutrun => "Synthwave Outrun",
        }
    }

    pub fn recommended_decor(self) -> CardDecorStyle {
        match self {
            Self::LcarsStarfleet => CardDecorStyle::LcarsBracket,
            Self::BaroqueGold => CardDecorStyle::OrnateDoubleBorder,
            Self::ShockingPink | Self::SynthwaveOutrun => CardDecorStyle::NeonGlow,
            _ => CardDecorStyle::DropShadow,
        }
    }

    pub fn recommended_backdrop(self) -> BackdropStyle {
        match self {
            Self::LcarsStarfleet => BackdropStyle::StarfieldWarp,
            Self::BaroqueGold => BackdropStyle::BaroqueDamask,
            Self::ShockingPink | Self::SynthwaveOutrun => BackdropStyle::SparkleClub,
            Self::CarbonDeck => BackdropStyle::CarbonWeave,
            _ => BackdropStyle::SparkleClub,
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
                decor: CardDecorConfig::default(),
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
                decor: CardDecorConfig::default(),
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
                decor: CardDecorConfig::default(),
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
                decor: CardDecorConfig::default(),
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
                decor: CardDecorConfig {
                    style: CardDecorStyle::NeonGlow,
                    border_width: 2.2,
                    card_opacity: 0.86,
                    color_boost: 0.35,
                },
            },
            Self::DaylightBooth => SkinPalette {
                is_light: true,
                canvas: Color32::from_rgb(226, 232, 240),
                chassis: Color32::from_rgb(238, 242, 246),
                card_fill: Color32::from_rgb(255, 255, 255),
                card_border: Color32::from_rgb(100, 116, 139),
                text_primary: Color32::BLACK,
                text_secondary: Color32::from_rgb(15, 18, 24),
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
                decor: CardDecorConfig::default(),
            },
            Self::LcarsStarfleet => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(6, 7, 16),
                chassis: Color32::from_rgb(16, 18, 36),
                card_fill: Color32::from_rgb(22, 24, 48),
                card_border: Color32::from_rgb(255, 153, 0), // LCARS Amber-Gold
                text_primary: Color32::from_rgb(255, 244, 214),
                text_secondary: Color32::from_rgb(204, 153, 255), // LCARS Lavender
                deck_a: Color32::from_rgb(255, 153, 0),           // Ops Gold
                deck_b: Color32::from_rgb(153, 153, 255),         // Science Periwinkle
                deck_c: Color32::from_rgb(85, 204, 255),          // Tricorder Cyan
                deck_d: Color32::from_rgb(255, 102, 102),         // Red Alert
                knob_body: Color32::from_rgb(36, 30, 64),
                knob_track: Color32::from_rgb(92, 66, 132),
                fader_slot: Color32::from_rgb(6, 5, 14),
                fader_cap: Color32::from_rgb(255, 186, 82),
                pad_colors: [
                    Color32::from_rgb(255, 153, 0),
                    Color32::from_rgb(204, 153, 204),
                    Color32::from_rgb(153, 153, 255),
                    Color32::from_rgb(255, 153, 102),
                    Color32::from_rgb(85, 204, 255),
                    Color32::from_rgb(255, 204, 102),
                    Color32::from_rgb(204, 102, 153),
                    Color32::from_rgb(255, 85, 85),
                ],
                decor: CardDecorConfig {
                    style: CardDecorStyle::LcarsBracket,
                    border_width: 2.2,
                    card_opacity: 0.88,
                    color_boost: 0.30,
                },
            },
            Self::BaroqueGold => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(24, 6, 14),
                chassis: Color32::from_rgb(44, 12, 24),
                card_fill: Color32::from_rgb(56, 16, 30),
                card_border: Color32::from_rgb(229, 184, 75), // Antique Burnished Gold
                text_primary: Color32::from_rgb(255, 246, 224),
                text_secondary: Color32::from_rgb(236, 204, 134),
                deck_a: Color32::from_rgb(245, 197, 66), // Royal Gold
                deck_b: Color32::from_rgb(16, 185, 129), // Venetian Emerald
                deck_c: Color32::from_rgb(96, 165, 250), // Baroque Sapphire
                deck_d: Color32::from_rgb(244, 63, 94),  // Imperial Ruby
                knob_body: Color32::from_rgb(72, 22, 38),
                knob_track: Color32::from_rgb(138, 98, 42),
                fader_slot: Color32::from_rgb(16, 4, 8),
                fader_cap: Color32::from_rgb(245, 208, 112),
                pad_colors: [
                    Color32::from_rgb(245, 197, 66),
                    Color32::from_rgb(220, 38, 38),
                    Color32::from_rgb(16, 185, 129),
                    Color32::from_rgb(59, 130, 246),
                    Color32::from_rgb(168, 85, 247),
                    Color32::from_rgb(245, 158, 11),
                    Color32::from_rgb(236, 72, 153),
                    Color32::from_rgb(234, 179, 8),
                ],
                decor: CardDecorConfig {
                    style: CardDecorStyle::OrnateDoubleBorder,
                    border_width: 2.2,
                    card_opacity: 0.90,
                    color_boost: 0.35,
                },
            },
            Self::SynthwaveOutrun => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(13, 6, 36),
                chassis: Color32::from_rgb(26, 12, 64),
                card_fill: Color32::from_rgb(34, 16, 82),
                card_border: Color32::from_rgb(0, 240, 255), // Laser Cyan
                text_primary: Color32::from_rgb(252, 246, 255),
                text_secondary: Color32::from_rgb(244, 148, 255),
                deck_a: Color32::from_rgb(0, 240, 255), // Laser Cyan
                deck_b: Color32::from_rgb(255, 42, 164), // Outrun Magenta
                deck_c: Color32::from_rgb(255, 184, 0), // Sunset Amber
                deck_d: Color32::from_rgb(168, 85, 247), // Ultraviolet
                knob_body: Color32::from_rgb(48, 20, 108),
                knob_track: Color32::from_rgb(120, 42, 186),
                fader_slot: Color32::from_rgb(9, 4, 24),
                fader_cap: Color32::from_rgb(255, 130, 224),
                pad_colors: [
                    Color32::from_rgb(0, 240, 255),
                    Color32::from_rgb(255, 42, 164),
                    Color32::from_rgb(255, 184, 0),
                    Color32::from_rgb(168, 85, 247),
                    Color32::from_rgb(34, 211, 238),
                    Color32::from_rgb(251, 113, 133),
                    Color32::from_rgb(192, 132, 252),
                    Color32::from_rgb(250, 204, 21),
                ],
                decor: CardDecorConfig {
                    style: CardDecorStyle::NeonGlow,
                    border_width: 2.2,
                    card_opacity: 0.86,
                    color_boost: 0.40,
                },
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
    pub decor: CardDecorConfig,
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t).round() as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t).round() as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t).round() as u8,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BackdropStyle {
    Solid,
    BrushedMetal,
    CarbonWeave,
    #[default]
    SparkleClub,
    StarfieldWarp,
    BaroqueDamask,
}

impl BackdropStyle {
    pub const ALL: [Self; 6] = [
        Self::Solid,
        Self::BrushedMetal,
        Self::CarbonWeave,
        Self::SparkleClub,
        Self::StarfieldWarp,
        Self::BaroqueDamask,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::BrushedMetal => "Brushed Metal",
            Self::CarbonWeave => "Carbon Weave",
            Self::SparkleClub => "Sparkle Club",
            Self::StarfieldWarp => "Starfield Warp (LCARS)",
            Self::BaroqueDamask => "Baroque Gold Damask",
        }
    }
}

impl SkinPalette {
    /// Apply user-configured card decoration & color boost onto this palette.
    pub fn with_decor(mut self, decor: CardDecorConfig) -> Self {
        let boost = decor.color_boost.clamp(0.0, 1.0);
        if boost > 0.01 {
            let tint = lerp_color(self.deck_a, self.deck_b, 0.35);
            let fill_amt = if self.is_light {
                boost * 0.14
            } else {
                boost * 0.24
            };
            self.card_fill = lerp_color(self.card_fill, tint, fill_amt);
            self.canvas = lerp_color(self.canvas, self.deck_a, fill_amt * 0.8);
            self.card_border = lerp_color(self.card_border, self.deck_a, boost * 0.55);
        }
        self.decor = decor;
        self
    }
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
                if self.is_light { 96 } else { 88 },
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

fn scale_alpha(base: u8, intensity: f32) -> u8 {
    ((base as f32) * intensity.clamp(0.15, 2.5)).clamp(0.0, 255.0) as u8
}

/// Paint a bold textured or dynamic underlay behind all UI panels.
pub fn paint_backdrop(
    ctx: &egui::Context,
    palette: &SkinPalette,
    style: BackdropStyle,
    time_sec: f32,
    energy: f32,
    intensity: f32,
) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(rect, 0.0, palette.canvas);
    let intensity = intensity.clamp(0.2, 2.0);

    match style {
        BackdropStyle::Solid => {}
        BackdropStyle::BrushedMetal => {
            let step = 4.0_f32;
            let mut y = rect.top();
            let mut row = 0u32;
            while y < rect.bottom() {
                let base_a = if row.is_multiple_of(3) {
                    115
                } else if row.is_multiple_of(2) {
                    68
                } else {
                    36
                };
                let alpha = scale_alpha(base_a, intensity);
                let col = if palette.is_light {
                    Color32::from_rgba_unmultiplied(15, 23, 42, alpha)
                } else {
                    Color32::from_rgba_unmultiplied(226, 232, 240, alpha)
                };
                painter.line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    Stroke::new(1.6, col),
                );
                y += step;
                row += 1;
            }
            for i in 0..4 {
                let x0 = rect.left() + rect.width() * (0.08 + i as f32 * 0.27);
                painter.line_segment(
                    [
                        egui::pos2(x0, rect.top()),
                        egui::pos2(x0 + rect.height() * 0.35, rect.bottom()),
                    ],
                    Stroke::new(
                        56.0,
                        Color32::from_rgba_unmultiplied(
                            palette.deck_a.r(),
                            palette.deck_a.g(),
                            palette.deck_a.b(),
                            scale_alpha(if palette.is_light { 42 } else { 32 }, intensity),
                        ),
                    ),
                );
            }
        }
        BackdropStyle::CarbonWeave => {
            let step = 14.0_f32;
            let col_a = Color32::from_rgba_unmultiplied(
                palette.deck_a.r(),
                palette.deck_a.g(),
                palette.deck_a.b(),
                scale_alpha(if palette.is_light { 110 } else { 120 }, intensity),
            );
            let col_b = Color32::from_rgba_unmultiplied(
                palette.deck_b.r(),
                palette.deck_b.g(),
                palette.deck_b.b(),
                scale_alpha(if palette.is_light { 78 } else { 88 }, intensity),
            );
            let h = rect.height();
            let mut x = rect.left() - h;
            let mut idx = 0u32;
            while x < rect.right() + h {
                let col = if idx.is_multiple_of(2) { col_a } else { col_b };
                painter.line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x + h, rect.bottom())],
                    Stroke::new(2.8, col),
                );
                painter.line_segment(
                    [egui::pos2(x + h, rect.top()), egui::pos2(x, rect.bottom())],
                    Stroke::new(2.4, col_b),
                );
                x += step;
                idx += 1;
            }
        }
        BackdropStyle::SparkleClub => {
            // Bold ambient club glow orbs
            for i in 0..8u32 {
                let fi = i as f32;
                let px = rect.left()
                    + rect.width()
                        * (0.08 + 0.84 * ((fi * 0.41 + time_sec * 0.04).sin() * 0.5 + 0.5));
                let py = rect.top()
                    + rect.height()
                        * (0.10 + 0.80 * ((fi * 0.59 + time_sec * 0.03).cos() * 0.5 + 0.5));
                let base = match i % 3 {
                    0 => palette.deck_a,
                    1 => palette.deck_b,
                    _ => palette.deck_c,
                };
                let r = (120.0 + 55.0 * energy) * (0.8 + 0.25 * intensity);
                painter.circle_filled(
                    egui::pos2(px, py),
                    r,
                    Color32::from_rgba_unmultiplied(
                        base.r(),
                        base.g(),
                        base.b(),
                        scale_alpha(if palette.is_light { 62 } else { 78 }, intensity),
                    ),
                );
            }
            // Vivid animated diamond sparkles
            for i in 0..96u32 {
                let seed_x = ((i.wrapping_mul(73856093) ^ 0x5A17) % 1000) as f32 / 1000.0;
                let seed_y = ((i.wrapping_mul(19349663) ^ 0x2F91) % 1000) as f32 / 1000.0;
                let phase = (i as f32) * 1.13 + time_sec * (2.0 + (i % 5) as f32 * 0.45);
                let twinkle = (phase.sin() * 0.5 + 0.5).powf(1.8);
                let x = rect.left() + seed_x * rect.width();
                let y = rect.top() + seed_y * rect.height();
                let col = match i % 3 {
                    0 => palette.deck_a,
                    1 => palette.deck_b,
                    _ => palette.pad_colors[(i as usize) % 8],
                };
                let alpha = scale_alpha(
                    (70.0 + 185.0 * twinkle * (0.65 + 0.35 * energy)) as u8,
                    intensity,
                );
                let c = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha);
                let sz = (2.2 + 4.4 * twinkle) * (0.75 + 0.3 * intensity);
                painter.circle_filled(egui::pos2(x, y), sz * 0.55, c);
                if twinkle > 0.40 {
                    painter.line_segment(
                        [egui::pos2(x - sz * 2.0, y), egui::pos2(x + sz * 2.0, y)],
                        Stroke::new(1.5, c),
                    );
                    painter.line_segment(
                        [egui::pos2(x, y - sz * 2.0), egui::pos2(x, y + sz * 2.0)],
                        Stroke::new(1.5, c),
                    );
                }
            }
        }
        BackdropStyle::StarfieldWarp => {
            // LCARS tactical subspace grid + streaking warp stars
            let grid_step = 48.0_f32;
            let grid_col = Color32::from_rgba_unmultiplied(
                palette.deck_a.r(),
                palette.deck_a.g(),
                palette.deck_a.b(),
                scale_alpha(48, intensity),
            );
            let mut gx = rect.left();
            while gx < rect.right() {
                painter.line_segment(
                    [egui::pos2(gx, rect.top()), egui::pos2(gx, rect.bottom())],
                    Stroke::new(1.0, grid_col),
                );
                gx += grid_step;
            }
            let mut gy = rect.top();
            while gy < rect.bottom() {
                painter.line_segment(
                    [egui::pos2(rect.left(), gy), egui::pos2(rect.right(), gy)],
                    Stroke::new(1.0, grid_col),
                );
                gy += grid_step;
            }
            // Radial warp star streaks from viewport center
            let center = rect.center();
            let max_r = rect.width().hypot(rect.height()) * 0.55;
            for i in 0..110u32 {
                let angle =
                    ((i.wrapping_mul(2654435761) % 3600) as f32 / 3600.0) * std::f32::consts::TAU;
                let speed = 0.14 + (i % 7) as f32 * 0.04 + energy * 0.12;
                let depth = ((i as f32 * 0.173) + time_sec * speed).fract();
                let r0 = depth * depth * max_r;
                let streak = (10.0 + 48.0 * depth) * (0.7 + 0.35 * intensity);
                let dir = egui::vec2(angle.cos(), angle.sin());
                let p0 = center + dir * r0;
                let p1 = center + dir * (r0 + streak);
                let col = match i % 4 {
                    0 => palette.deck_a,
                    1 => palette.deck_b,
                    2 => palette.deck_c,
                    _ => Color32::WHITE,
                };
                let alpha = scale_alpha((60.0 + 190.0 * depth) as u8, intensity);
                painter.line_segment(
                    [p0, p1],
                    Stroke::new(
                        1.0 + 1.8 * depth,
                        Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha),
                    ),
                );
            }
        }
        BackdropStyle::BaroqueDamask => {
            // Bold gilded diamond lattice & ornate medallion filigree
            let tile = 54.0_f32;
            let gold = palette.card_border;
            let line_col = Color32::from_rgba_unmultiplied(
                gold.r(),
                gold.g(),
                gold.b(),
                scale_alpha(115, intensity),
            );
            let accent_col = Color32::from_rgba_unmultiplied(
                palette.deck_a.r(),
                palette.deck_a.g(),
                palette.deck_a.b(),
                scale_alpha(72, intensity),
            );
            let mut y = rect.top() - tile;
            let mut row = 0u32;
            while y < rect.bottom() + tile {
                let mut x = rect.left() - tile
                    + if row.is_multiple_of(2) {
                        0.0
                    } else {
                        tile * 0.5
                    };
                while x < rect.right() + tile {
                    let cx = x + tile * 0.5;
                    let cy = y + tile * 0.5;
                    let r = tile * 0.44;
                    let top = egui::pos2(cx, cy - r);
                    let right = egui::pos2(cx + r, cy);
                    let bot = egui::pos2(cx, cy + r);
                    let left = egui::pos2(cx - r, cy);
                    // Outer diamond trellis
                    painter.line_segment([top, right], Stroke::new(1.6, line_col));
                    painter.line_segment([right, bot], Stroke::new(1.6, line_col));
                    painter.line_segment([bot, left], Stroke::new(1.6, line_col));
                    painter.line_segment([left, top], Stroke::new(1.6, line_col));
                    // Inner gilded rosette medallion
                    painter.circle_stroke(egui::pos2(cx, cy), r * 0.42, Stroke::new(1.5, line_col));
                    painter.circle_filled(egui::pos2(cx, cy), r * 0.16, accent_col);
                    x += tile;
                }
                y += tile * 0.85;
                row += 1;
            }
        }
    }
}
