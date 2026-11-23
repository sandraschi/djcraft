#![forbid(unsafe_code)]
//! Fleet SOTA egui typography, skin-aware card frames, and vector-painted controls.

use egui::{Color32, CornerRadius, FontId, Frame, Margin, Shadow, Stroke, StrokeKind, TextStyle};

use crate::skins::{CardDecorStyle, SkinPalette};

pub fn apply_fleet_typography(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(19.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.5));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.5));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(11.5));
    style.spacing.item_spacing = egui::vec2(7.0, 5.0);
    style.spacing.button_padding = egui::vec2(7.0, 3.5);
    ctx.set_global_style(style);
}

pub fn card_frame<R>(
    ui: &mut egui::Ui,
    palette: &SkinPalette,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let decor = palette.decor;
    let alpha = (decor.card_opacity.clamp(0.45, 1.0) * 255.0).round() as u8;
    let fill = Color32::from_rgba_unmultiplied(
        palette.card_fill.r(),
        palette.card_fill.g(),
        palette.card_fill.b(),
        alpha,
    );
    let bw = decor.border_width.clamp(1.0, 4.0);
    let corner = match decor.style {
        CardDecorStyle::LcarsBracket => CornerRadius::same(14),
        CardDecorStyle::OrnateDoubleBorder => CornerRadius::same(4),
        _ => CornerRadius::same(8),
    };

    let shadow = match decor.style {
        CardDecorStyle::Clean => Shadow::NONE,
        CardDecorStyle::DropShadow => Shadow {
            offset: [4, 5],
            blur: 14,
            spread: 1,
            color: Color32::from_rgba_unmultiplied(
                0,
                0,
                0,
                if palette.is_light { 75 } else { 155 },
            ),
        },
        CardDecorStyle::NeonGlow => Shadow {
            offset: [0, 0],
            blur: 18,
            spread: 2,
            color: Color32::from_rgba_unmultiplied(
                palette.deck_a.r(),
                palette.deck_a.g(),
                palette.deck_a.b(),
                if palette.is_light { 90 } else { 145 },
            ),
        },
        CardDecorStyle::OrnateDoubleBorder => Shadow {
            offset: [3, 4],
            blur: 12,
            spread: 1,
            color: Color32::from_rgba_unmultiplied(
                palette.card_border.r(),
                palette.card_border.g(),
                palette.card_border.b(),
                72,
            ),
        },
        CardDecorStyle::LcarsBracket => Shadow {
            offset: [0, 0],
            blur: 12,
            spread: 1,
            color: Color32::from_rgba_unmultiplied(
                palette.deck_a.r(),
                palette.deck_a.g(),
                palette.deck_a.b(),
                90,
            ),
        },
    };

    let border_col = match decor.style {
        CardDecorStyle::NeonGlow => palette.deck_a,
        _ => palette.card_border,
    };

    let inner_resp = Frame::new()
        .fill(fill)
        .stroke(Stroke::new(bw, border_col))
        .shadow(shadow)
        .corner_radius(corner)
        .inner_margin(Margin::same(9))
        .show(ui, add_contents);

    let rect = inner_resp.response.rect;
    let painter = ui.painter();
    match decor.style {
        CardDecorStyle::OrnateDoubleBorder => {
            // Inner gilded hairline frame + corner filigree diamonds
            let inset = rect.shrink(4.0);
            let gold = palette.card_border;
            painter.rect_stroke(
                inset,
                CornerRadius::same(2),
                Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(gold.r(), gold.g(), gold.b(), 190),
                ),
                StrokeKind::Inside,
            );
            for pt in [
                rect.left_top(),
                rect.right_top(),
                rect.left_bottom(),
                rect.right_bottom(),
            ] {
                painter.circle_filled(pt, 3.2, palette.deck_a);
            }
        }
        CardDecorStyle::LcarsBracket => {
            // Iconic Starfleet LCARS left-edge vertical color bar & top-left elbow cap
            let bar_w = 6.0_f32;
            let left_bar = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 1.0, rect.top() + 8.0),
                egui::pos2(rect.left() + bar_w, rect.bottom() - 8.0),
            );
            painter.rect_filled(left_bar, CornerRadius::same(3), palette.deck_a);
            let top_pill = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 10.0, rect.top() + 1.0),
                egui::pos2(
                    (rect.left() + 72.0).min(rect.right() - 12.0),
                    rect.top() + 5.0,
                ),
            );
            painter.rect_filled(top_pill, CornerRadius::same(2), palette.deck_b);
        }
        CardDecorStyle::NeonGlow => {
            // Subtle inner neon rim accent
            painter.rect_stroke(
                rect.shrink(2.0),
                CornerRadius::same(7),
                Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(
                        palette.deck_b.r(),
                        palette.deck_b.g(),
                        palette.deck_b.b(),
                        120,
                    ),
                ),
                StrokeKind::Inside,
            );
        }
        _ => {}
    }

    inner_resp
}

/// Vector-painted 26x26 Sun / Crescent Moon toggle (switches between Daylight Booth and active dark skin).
pub fn theme_toggle_button(ui: &mut egui::Ui, is_light: bool) -> bool {
    let size = egui::vec2(26.0, 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&resp);
        ui.painter().rect(
            rect,
            CornerRadius::same(6),
            visuals.bg_fill,
            visuals.bg_stroke,
            StrokeKind::Inside,
        );
        let center = rect.center();
        let painter = ui.painter();
        if !is_light {
            let sun_color = Color32::from_rgb(251, 191, 36);
            painter.circle_filled(center, 4.2, sun_color);
            for i in 0..8 {
                let angle = (i as f32) * std::f32::consts::TAU / 8.0;
                let dir = egui::vec2(angle.cos(), angle.sin());
                painter.line_segment(
                    [center + dir * 6.0, center + dir * 8.6],
                    Stroke::new(1.5, sun_color),
                );
            }
        } else {
            let moon_color = Color32::from_rgb(51, 65, 85);
            painter.circle_filled(center, 6.0, moon_color);
            painter.circle_filled(center + egui::vec2(2.8, -2.0), 5.0, visuals.bg_fill);
        }
    }
    resp.on_hover_text("Toggle Daylight Booth / Dark Club Skin")
        .clicked()
}

/// Vector-painted status indicator dot (green = active/ok, amber = standby).
pub fn status_dot(ui: &mut egui::Ui, active: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    let color = if active {
        Color32::from_rgb(34, 197, 94)
    } else {
        Color32::from_rgb(245, 158, 11)
    };
    ui.painter().circle_filled(rect.center(), 4.2, color);
}
