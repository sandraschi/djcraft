#![forbid(unsafe_code)]
//! Fleet SOTA egui typography, skin-aware card frames, and vector-painted controls.

use egui::{Color32, CornerRadius, FontId, Frame, Margin, Stroke, StrokeKind, TextStyle};

use crate::skins::SkinPalette;

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
    Frame::new()
        .fill(palette.card_fill)
        .stroke(Stroke::new(1.0, palette.card_border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(9))
        .show(ui, add_contents)
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
