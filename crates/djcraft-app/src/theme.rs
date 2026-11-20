#![forbid(unsafe_code)]
//! Fleet SOTA egui visuals, typography, card containers, and vector-painted controls.

use egui::{Color32, CornerRadius, FontId, Frame, Margin, Stroke, StrokeKind, TextStyle, Visuals};

pub fn apply_fleet_typography(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(20.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.5));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(13.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    ctx.set_global_style(style);
}

/// High-contrast slate-950 dark theme (EGUI_DESKTOP_UI_AND_PARITY.md).
pub fn harsh_visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    let canvas = Color32::from_rgb(10, 13, 18); // #0a0d12 slate-950
    let card = Color32::from_rgb(16, 26, 46); // #101a2e
    let border = Color32::from_rgb(38, 52, 82); // #263452
    let ink = Color32::from_rgb(245, 247, 250); // #f5f7fa
    let weak = Color32::from_rgb(186, 194, 206); // #bac2ce

    visuals.panel_fill = canvas;
    visuals.window_fill = card;
    visuals.extreme_bg_color = Color32::from_rgb(7, 10, 15);
    visuals.faint_bg_color = Color32::from_rgb(20, 30, 50);
    visuals.override_text_color = Some(ink);
    visuals.weak_text_color = Some(weak);
    visuals.window_stroke = Stroke::new(1.0, border);
    visuals.widgets.noninteractive.bg_fill = card;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, ink);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(24, 36, 60);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, ink);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(36, 54, 88);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::from_rgb(96, 165, 250));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    visuals.widgets.active.bg_fill = Color32::from_rgb(48, 72, 118);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, Color32::WHITE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    visuals
}

/// High-contrast crisp light theme companion (slate-50 canvas, pure white cards).
pub fn crisp_light_visuals() -> Visuals {
    let mut visuals = Visuals::light();
    let canvas = Color32::from_rgb(248, 250, 252); // #f8fafc slate-50
    let card = Color32::WHITE;
    let border = Color32::from_rgb(203, 213, 225); // #cbd5e1 slate-300
    let ink = Color32::from_rgb(15, 23, 42); // #0f172a slate-900
    let weak = Color32::from_rgb(51, 65, 85); // #334155

    visuals.panel_fill = canvas;
    visuals.window_fill = card;
    visuals.extreme_bg_color = Color32::from_rgb(241, 245, 249);
    visuals.faint_bg_color = Color32::from_rgb(241, 245, 249);
    visuals.override_text_color = Some(ink);
    visuals.weak_text_color = Some(weak);
    visuals.window_stroke = Stroke::new(1.0, border);
    visuals.widgets.noninteractive.bg_fill = card;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, ink);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(226, 232, 240);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, ink);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(203, 213, 225);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, ink);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, ink);
    visuals.widgets.active.bg_fill = Color32::from_rgb(148, 163, 184);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, ink);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, ink);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, ink);
    visuals
}

pub fn card_frame<R>(
    ui: &mut egui::Ui,
    dark_mode: bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let fill = if dark_mode {
        Color32::from_rgb(16, 26, 46)
    } else {
        Color32::WHITE
    };
    let stroke = if dark_mode {
        Stroke::new(1.0, Color32::from_rgb(38, 52, 82))
    } else {
        Stroke::new(1.0, Color32::from_rgb(203, 213, 225))
    };
    Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .show(ui, add_contents)
}

/// Vector-painted 26x26 Sun / Crescent Moon toggle (zero Unicode tofu risk).
pub fn theme_toggle_button(ui: &mut egui::Ui, dark_mode: &mut bool) -> bool {
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
        if *dark_mode {
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
    let clicked = resp.on_hover_text("Toggle Light / Dark theme").clicked();
    if clicked {
        *dark_mode = !*dark_mode;
    }
    clicked
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
