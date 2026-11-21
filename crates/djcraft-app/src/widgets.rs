#![forbid(unsafe_code)]
//! GPU-painted tactile DJ hardware controls:
//! rotary EQ/gain knobs, vertical club faders, Magnetic battle crossfader,
//! 8 RGB MPC performance pads, stacked parallel beatmatching waveforms,
//! motorized vinyl platters, stereo VU meters, and 12-hour Camelot wheel.

use djcraft_audio::dsp::WaveformSummary;
use djcraft_audio::{AudioEngine, DeckState};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::skins::SkinPalette;

/// Result of interacting with a custom-painted hardware knob or fader.
#[derive(Debug, Clone, Copy)]
pub struct ControlResponse {
    pub changed: bool,
    pub committed: bool,
}

/// Tactile rotary DJ potentiometer with 270-degree LED arc, vertical drag, and double-click reset.
#[allow(clippy::too_many_arguments)]
pub fn rotary_knob(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    min: f64,
    max: f64,
    default_val: f64,
    bipolar: bool,
    accent: Color32,
    palette: &SkinPalette,
) -> ControlResponse {
    let mut changed = false;
    let mut committed = false;

    ui.vertical(|ui| {
        let size = Vec2::new(48.0, 48.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Slider,
                ui.is_enabled(),
                format!("{label}: {:.2}", *value),
            )
        });

        if resp.double_clicked() {
            *value = default_val;
            changed = true;
            committed = true;
        } else if resp.dragged() {
            let delta = -resp.drag_delta().y as f64 * (max - min) / 140.0;
            *value = (*value + delta).clamp(min, max);
            changed = true;
        } else if resp.has_focus() {
            let step = (max - min) / 40.0;
            let (inc, dec, rst) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowRight),
                    i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::Home) || i.key_pressed(egui::Key::Num0),
                )
            });
            if inc {
                *value = (*value + step).clamp(min, max);
                changed = true;
                committed = true;
            } else if dec {
                *value = (*value - step).clamp(min, max);
                changed = true;
                committed = true;
            } else if rst {
                *value = default_val;
                changed = true;
                committed = true;
            }
        }
        if resp.drag_stopped() {
            committed = true;
        }

        let painter = ui.painter_at(rect);
        let center = rect.center();
        let radius = 17.0_f32;
        let norm = ((*value - min) / (max - min).max(1e-9)).clamp(0.0, 1.0) as f32;

        // 270-degree arc: 135 deg (bottom-left) clockwise to 405 deg (bottom-right)
        let start_deg = 135.0_f32;
        let sweep_deg = 270.0_f32;
        let arc_r = radius + 4.5;

        // Background LED track
        draw_arc(
            &painter,
            center,
            arc_r,
            start_deg,
            start_deg + sweep_deg,
            Stroke::new(3.0, palette.knob_track),
        );

        // Active LED arc (bipolar from 12 o'clock, or unipolar from start)
        let val_deg = start_deg + norm * sweep_deg;
        if bipolar {
            let mid_deg = start_deg + sweep_deg * 0.5;
            let (a0, a1) = if val_deg >= mid_deg {
                (mid_deg, val_deg)
            } else {
                (val_deg, mid_deg)
            };
            draw_arc(&painter, center, arc_r, a0, a1, Stroke::new(3.2, accent));
        } else {
            draw_arc(
                &painter,
                center,
                arc_r,
                start_deg,
                val_deg,
                Stroke::new(3.2, accent),
            );
        }

        // Knob cap body + highlight rim
        painter.circle_filled(center, radius, palette.knob_body);
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(
                1.5,
                if resp.hovered() || resp.dragged() {
                    accent
                } else {
                    palette.card_border
                },
            ),
        );

        // Pointer notch
        let rad = val_deg.to_radians();
        let dir = Vec2::new(rad.cos(), rad.sin());
        painter.line_segment(
            [center + dir * 4.0, center + dir * (radius - 2.5)],
            Stroke::new(2.4, palette.text_primary),
        );

        ui.label(
            egui::RichText::new(label)
                .size(11.0)
                .color(palette.text_secondary),
        );
    });

    ControlResponse { changed, committed }
}

fn draw_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start_deg: f32,
    end_deg: f32,
    stroke: Stroke,
) {
    let steps = 24usize;
    let span = end_deg - start_deg;
    if span.abs() < 0.5 {
        return;
    }
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let deg = start_deg + span * (i as f32 / steps as f32);
            let r = deg.to_radians();
            center + Vec2::new(r.cos(), r.sin()) * radius
        })
        .collect();
    painter.add(egui::Shape::line(pts, stroke));
}

/// Tactile vertical club fader (channel volume or tempo/pitch) with tick scale and metallic cap.
#[allow(clippy::too_many_arguments)]
pub fn vertical_fader(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    min: f64,
    max: f64,
    default_val: f64,
    height: f32,
    accent: Color32,
    palette: &SkinPalette,
) -> ControlResponse {
    let mut changed = false;
    let mut committed = false;

    ui.vertical(|ui| {
        let size = Vec2::new(38.0, height);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());

        let pad_y = 12.0_f32;
        let track_top = rect.top() + pad_y;
        let track_bot = rect.bottom() - pad_y;
        let track_h = (track_bot - track_top).max(1.0);

        if resp.double_clicked() {
            *value = default_val;
            changed = true;
            committed = true;
        } else if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let norm = ((track_bot - pos.y) / track_h).clamp(0.0, 1.0) as f64;
                *value = min + norm * (max - min);
                changed = true;
            }
        }
        if resp.drag_stopped() {
            committed = true;
        }

        let painter = ui.painter_at(rect);
        let cx = rect.center().x;

        // Tick marks on left & right of fader rail
        for i in 0..=8 {
            let y = track_top + (i as f32 / 8.0) * track_h;
            let len = if i == 4 { 7.0 } else { 4.0 };
            let col = if i == 4 { accent } else { palette.card_border };
            painter.line_segment(
                [Pos2::new(cx - 14.0, y), Pos2::new(cx - 14.0 + len, y)],
                Stroke::new(1.0, col),
            );
            painter.line_segment(
                [Pos2::new(cx + 14.0 - len, y), Pos2::new(cx + 14.0, y)],
                Stroke::new(1.0, col),
            );
        }

        // Recessed fader slot
        let slot = Rect::from_center_size(Pos2::new(cx, rect.center().y), Vec2::new(6.0, track_h));
        painter.rect_filled(slot, CornerRadius::same(3), palette.fader_slot);

        // Active fill bar below cap
        let norm = ((*value - min) / (max - min).max(1e-9)).clamp(0.0, 1.0) as f32;
        let cap_y = track_bot - norm * track_h;
        let fill = Rect::from_min_max(Pos2::new(cx - 2.0, cap_y), Pos2::new(cx + 2.0, track_bot));
        painter.rect_filled(fill, CornerRadius::same(2), accent);

        // Brushed metallic fader cap
        let cap = Rect::from_center_size(Pos2::new(cx, cap_y), Vec2::new(26.0, 14.0));
        painter.rect(
            cap,
            CornerRadius::same(3),
            palette.fader_cap,
            Stroke::new(1.2, palette.canvas),
            StrokeKind::Inside,
        );
        painter.line_segment(
            [
                Pos2::new(cap.left() + 3.0, cap_y),
                Pos2::new(cap.right() - 3.0, cap_y),
            ],
            Stroke::new(2.0, palette.canvas),
        );

        ui.label(
            egui::RichText::new(label)
                .size(10.5)
                .color(palette.text_secondary),
        );
    });

    ControlResponse { changed, committed }
}

/// Battle-style horizontal MAG FOUR club crossfader (-1.0 Deck A .. +1.0 Deck B).
pub fn club_crossfader(
    ui: &mut egui::Ui,
    value: &mut f64,
    palette: &SkinPalette,
) -> ControlResponse {
    let mut changed = false;
    let mut committed = false;

    let desired = Vec2::new(ui.available_width().clamp(180.0, 340.0), 34.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let pad_x = 18.0_f32;
    let left_x = rect.left() + pad_x;
    let right_x = rect.right() - pad_x;
    let span = (right_x - left_x).max(1.0);

    if resp.double_clicked() {
        *value = 0.0;
        changed = true;
        committed = true;
    } else if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let norm = ((pos.x - left_x) / span).clamp(0.0, 1.0) as f64;
            *value = norm * 2.0 - 1.0;
            changed = true;
        }
    }
    if resp.drag_stopped() {
        committed = true;
    }

    let painter = ui.painter_at(rect);
    let cy = rect.center().y;

    // Recessed rail + center zero marker
    let rail = Rect::from_center_size(rect.center(), Vec2::new(span + 8.0, 8.0));
    painter.rect_filled(rail, CornerRadius::same(4), palette.fader_slot);
    painter.line_segment(
        [
            Pos2::new(rect.center().x, rect.top() + 3.0),
            Pos2::new(rect.center().x, rect.bottom() - 3.0),
        ],
        Stroke::new(1.5, palette.card_border),
    );

    // Side labels A / B
    painter.text(
        Pos2::new(rect.left() + 6.0, cy),
        Align2::CENTER_CENTER,
        "A",
        FontId::monospace(11.0),
        palette.deck_a,
    );
    painter.text(
        Pos2::new(rect.right() - 6.0, cy),
        Align2::CENTER_CENTER,
        "B",
        FontId::monospace(11.0),
        palette.deck_b,
    );

    // Crossfader cap
    let norm = ((*value + 1.0) * 0.5).clamp(0.0, 1.0) as f32;
    let cap_x = left_x + norm * span;
    let cap = Rect::from_center_size(Pos2::new(cap_x, cy), Vec2::new(18.0, 26.0));
    painter.rect(
        cap,
        CornerRadius::same(3),
        palette.fader_cap,
        Stroke::new(1.5, palette.canvas),
        StrokeKind::Inside,
    );
    painter.line_segment(
        [
            Pos2::new(cap_x, cap.top() + 3.0),
            Pos2::new(cap_x, cap.bottom() - 3.0),
        ],
        Stroke::new(2.0, palette.canvas),
    );

    ControlResponse { changed, committed }
}

/// 8 RGB backlit MPC-style performance pads per deck.
/// Returns `Some(pad_index 0..8)` when clicked.
pub fn performance_pads_grid(
    ui: &mut egui::Ui,
    deck: &DeckState,
    palette: &SkinPalette,
) -> Option<usize> {
    let labels = [
        "CUE 1", "CUE 2", "CUE 3", "CUE 4", "LOOP 4", "LOOP 8", "KILL B", "KILL V",
    ];
    let active = [
        deck.hot_cue_seconds[0].is_some(),
        deck.hot_cue_seconds[1].is_some(),
        deck.hot_cue_seconds[2].is_some(),
        deck.hot_cue_seconds[3].is_some(),
        deck.loop_bars == Some(4),
        deck.loop_bars == Some(8),
        deck.eq_low <= 0.01,
        deck.stem_vocals_gain <= 0.01,
    ];

    let mut clicked = None;
    for row in 0..2 {
        ui.horizontal(|ui| {
            for col in 0..4 {
                let idx = row * 4 + col;
                let pad_col = palette.pad_colors[idx];
                let is_on = active[idx];
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(62.0, 28.0), Sense::click());
                let painter = ui.painter_at(rect);
                let fill = if is_on {
                    Color32::from_rgba_unmultiplied(pad_col.r(), pad_col.g(), pad_col.b(), 190)
                } else {
                    Color32::from_rgba_unmultiplied(pad_col.r(), pad_col.g(), pad_col.b(), 36)
                };
                painter.rect(
                    rect,
                    CornerRadius::same(5),
                    fill,
                    Stroke::new(if is_on || resp.hovered() { 2.0 } else { 1.2 }, pad_col),
                    StrokeKind::Inside,
                );
                let txt_col = if is_on {
                    Color32::BLACK
                } else {
                    palette.text_primary
                };
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    labels[idx],
                    FontId::monospace(10.5),
                    txt_col,
                );
                if resp.clicked() {
                    clicked = Some(idx);
                }
            }
        });
    }
    clicked
}

/// Serato/VDJ signature stacked parallel waveforms banner across the top for visual beatmatching.
pub fn paint_parallel_waveforms(
    ui: &mut egui::Ui,
    deck_a: &DeckState,
    wf_a: Option<&WaveformSummary>,
    deck_b: &DeckState,
    wf_b: Option<&WaveformSummary>,
    palette: &SkinPalette,
) {
    let w = ui.available_width().max(320.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 92.0), Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect(
        rect,
        CornerRadius::same(6),
        palette.fader_slot,
        Stroke::new(1.0, palette.card_border),
        StrokeKind::Inside,
    );

    let half_h = rect.height() * 0.5;
    let rect_a = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + half_h));
    let rect_b = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + half_h), rect.max);

    paint_waveform_lane(&painter, rect_a, deck_a, wf_a, palette.deck_a, "A");
    paint_waveform_lane(&painter, rect_b, deck_b, wf_b, palette.deck_b, "B");

    // Divider between Deck A and Deck B lanes
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.min.y + half_h),
            Pos2::new(rect.right(), rect.min.y + half_h),
        ],
        Stroke::new(1.0, palette.card_border),
    );

    // Shared center beatmatch playhead cursor
    painter.line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(2.0, Color32::from_rgb(239, 68, 68)),
    );
}

/// Vertical layout: Deck A and Deck B waveforms scroll top-to-bottom side-by-side between the decks,
/// with a shared horizontal playhead in the middle for visual beatmatching.
pub fn paint_vertical_waveforms(
    ui: &mut egui::Ui,
    deck_a: &DeckState,
    wf_a: Option<&WaveformSummary>,
    deck_b: &DeckState,
    wf_b: Option<&WaveformSummary>,
    palette: &SkinPalette,
    height: f32,
) {
    let w = ui.available_width().clamp(120.0, 260.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, height.max(160.0)), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(6),
        palette.fader_slot,
        Stroke::new(1.0, palette.card_border),
        StrokeKind::Inside,
    );
    let half_w = rect.width() * 0.5;
    let rect_a = Rect::from_min_max(rect.min, Pos2::new(rect.min.x + half_w, rect.max.y));
    let rect_b = Rect::from_min_max(Pos2::new(rect.min.x + half_w, rect.min.y), rect.max);
    paint_vertical_lane(&painter, rect_a, deck_a, wf_a, palette.deck_a, "A");
    paint_vertical_lane(&painter, rect_b, deck_b, wf_b, palette.deck_b, "B");
    painter.line_segment(
        [
            Pos2::new(rect.min.x + half_w, rect.top()),
            Pos2::new(rect.min.x + half_w, rect.bottom()),
        ],
        Stroke::new(1.0, palette.card_border),
    );
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.center().y),
            Pos2::new(rect.right(), rect.center().y),
        ],
        Stroke::new(2.0, Color32::from_rgb(239, 68, 68)),
    );
}

fn paint_vertical_lane(
    painter: &egui::Painter,
    rect: Rect,
    deck: &DeckState,
    waveform: Option<&WaveformSummary>,
    accent: Color32,
    tag: &str,
) {
    painter.text(
        Pos2::new(rect.center().x, rect.top() + 9.0),
        Align2::CENTER_CENTER,
        tag,
        FontId::monospace(11.0),
        accent,
    );
    let Some(track) = deck.loaded_track.as_ref() else {
        return;
    };
    let bpm = (track.bpm * (1.0 + deck.pitch_bend_pct / 100.0)).max(20.0);
    let beat_sec = 60.0 / bpm;
    let window_sec = 8.0_f64;
    // Time flows downward: future audio at the top, past at the bottom.
    let start_t = deck.position_seconds + window_sec * 0.5;
    let rows = ((rect.height() / 3.0) as usize).clamp(48, 400);
    let row_h = rect.height() / rows as f32;
    let mid_x = rect.center().x;
    let max_half_w = rect.width() * 0.44;

    for row in 0..rows {
        let frac = row as f64 / rows as f64;
        let t = start_t - frac * window_sec;
        if t < 0.0 || t > track.duration_seconds {
            continue;
        }
        let (bass_amp, high_amp) = if let Some(wf) = waveform {
            let (lo, hi) = wf.sample_at(t);
            (
                (lo as f64 * deck.eq_low).clamp(0.0, 1.0),
                (hi as f64 * deck.eq_high.max(deck.eq_mid)).clamp(0.0, 1.0),
            )
        } else {
            let beat_phase = ((t - deck.beatgrid_offset_sec).rem_euclid(beat_sec)) / beat_sec;
            let kick = if beat_phase < 0.22 {
                1.0 - beat_phase / 0.22
            } else {
                0.18
            };
            let mel = (t * 3.7).sin().abs() * 0.5 + (t * 7.3).cos().abs() * 0.5;
            (
                (kick * 0.7 + 0.3).clamp(0.0, 1.0),
                (mel * 0.8 + 0.2).clamp(0.0, 1.0),
            )
        };
        let y = rect.top() + row as f32 * row_h + row_h * 0.5;
        let w_bass = bass_amp as f32 * max_half_w * deck.volume_fader as f32;
        let w_high = high_amp as f32 * max_half_w * 0.65 * deck.volume_fader as f32;
        painter.line_segment(
            [Pos2::new(mid_x - w_bass, y), Pos2::new(mid_x + w_bass, y)],
            Stroke::new(
                2.0,
                Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 165),
            ),
        );
        painter.line_segment(
            [Pos2::new(mid_x - w_high, y), Pos2::new(mid_x + w_high, y)],
            Stroke::new(1.3, Color32::from_rgb(236, 242, 248)),
        );
    }

    // Beatgrid ticks (horizontal), bars brighter
    let last_beat = ((start_t - deck.beatgrid_offset_sec) / beat_sec).floor() as i64;
    let first_beat = ((start_t - window_sec - deck.beatgrid_offset_sec) / beat_sec).ceil() as i64;
    for b in first_beat..=last_beat {
        if b < 0 {
            continue;
        }
        let t = deck.beatgrid_offset_sec + b as f64 * beat_sec;
        let y = rect.top() + ((start_t - t) / window_sec) as f32 * rect.height();
        let is_bar = b % 4 == 0;
        let col = if is_bar {
            Color32::from_rgb(248, 250, 252)
        } else {
            Color32::from_rgb(100, 116, 139)
        };
        let len = if is_bar { 10.0 } else { 5.0 };
        painter.line_segment(
            [
                Pos2::new(rect.left() + 1.0, y),
                Pos2::new(rect.left() + len, y),
            ],
            Stroke::new(if is_bar { 1.6 } else { 1.0 }, col),
        );
    }
}

fn paint_waveform_lane(
    painter: &egui::Painter,
    rect: Rect,
    deck: &DeckState,
    waveform: Option<&WaveformSummary>,
    accent: Color32,
    tag: &str,
) {
    let mid_y = rect.center().y;
    painter.text(
        Pos2::new(rect.left() + 10.0, rect.top() + 10.0),
        Align2::LEFT_CENTER,
        tag,
        FontId::monospace(11.0),
        accent,
    );
    let Some(track) = deck.loaded_track.as_ref() else {
        return;
    };

    let bpm = (track.bpm * (1.0 + deck.pitch_bend_pct / 100.0)).max(20.0);
    let beat_sec = 60.0 / bpm;
    let window_sec = 8.0_f64;
    let start_t = deck.position_seconds - window_sec * 0.5;

    let columns = ((rect.width() / 3.0) as usize).clamp(48, 320);
    let col_w = rect.width() / columns as f32;
    let max_half_h = rect.height() * 0.42;

    for col in 0..columns {
        let frac = col as f64 / columns as f64;
        let t = start_t + frac * window_sec;
        if t < 0.0 || t > track.duration_seconds {
            continue;
        }
        let (bass_amp, high_amp) = if let Some(wf) = waveform {
            let (lo, hi) = wf.sample_at(t);
            (
                (lo as f64 * deck.eq_low).clamp(0.0, 1.0),
                (hi as f64 * deck.eq_high.max(deck.eq_mid)).clamp(0.0, 1.0),
            )
        } else {
            let beat_phase = ((t - deck.beatgrid_offset_sec).rem_euclid(beat_sec)) / beat_sec;
            let kick_env = if beat_phase < 0.22 {
                1.0 - beat_phase / 0.22
            } else {
                0.18
            };
            let melodic =
                ((t * 3.7).sin().abs() * 0.5 + (t * 7.3).cos().abs() * 0.5) * deck.stem_vocals_gain;
            (
                (kick_env * deck.stem_drums_gain * 0.7 + 0.3 * deck.stem_bass_gain).clamp(0.0, 1.0),
                (melodic * 0.8 + 0.2 * deck.stem_other_gain).clamp(0.0, 1.0),
            )
        };

        let x = rect.left() + col as f32 * col_w + col_w * 0.5;
        let h_bass = (bass_amp as f32) * max_half_h * deck.volume_fader as f32;
        let h_high = (high_amp as f32) * max_half_h * 0.65 * deck.volume_fader as f32;

        painter.line_segment(
            [Pos2::new(x, mid_y - h_bass), Pos2::new(x, mid_y + h_bass)],
            Stroke::new(
                2.0,
                Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 165),
            ),
        );
        painter.line_segment(
            [Pos2::new(x, mid_y - h_high), Pos2::new(x, mid_y + h_high)],
            Stroke::new(1.3, Color32::from_rgb(236, 242, 248)),
        );
    }

    // Beatgrid tick marks
    let first_beat = ((start_t - deck.beatgrid_offset_sec) / beat_sec).ceil() as i64;
    let last_beat = ((start_t + window_sec - deck.beatgrid_offset_sec) / beat_sec).floor() as i64;
    for b in first_beat..=last_beat {
        if b < 0 {
            continue;
        }
        let t = deck.beatgrid_offset_sec + b as f64 * beat_sec;
        let x = rect.left() + ((t - start_t) / window_sec) as f32 * rect.width();
        let is_bar = b % 4 == 0;
        let tick_color = if is_bar {
            Color32::from_rgb(248, 250, 252)
        } else {
            Color32::from_rgb(100, 116, 139)
        };
        painter.line_segment(
            [
                Pos2::new(x, rect.top() + 1.0),
                Pos2::new(x, rect.top() + if is_bar { 10.0 } else { 5.0 }),
            ],
            Stroke::new(if is_bar { 1.6 } else { 1.0 }, tick_color),
        );
    }
}

/// Live scratch interaction output from the motorized vinyl platter.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlatterResponse {
    pub scratch_velocity: Option<f64>,
    pub angle_delta: f32,
}

/// Paint an interactive Rane Twelve MKII / CDJ motorized vinyl jog platter with live scratching.
pub fn paint_platter(
    ui: &mut egui::Ui,
    angle: f32,
    accent: Color32,
    is_playing: bool,
    bpm: Option<f64>,
) -> PlatterResponse {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(134.0), Sense::click_and_drag());
    let mut out = PlatterResponse::default();
    let scratching = resp.dragged() || resp.is_pointer_button_down_on();
    if scratching {
        let d = resp.drag_delta();
        // Horizontal drag (plus slight vertical component) drives forward/reverse vinyl scratch
        let vel = ((d.x - d.y * 0.25) as f64 * 0.16).clamp(-4.0, 4.0);
        out.scratch_velocity = Some(vel);
        out.angle_delta = (vel as f32) * 0.14;
    }

    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 62.0;

    painter.circle_filled(center, radius, Color32::from_rgb(14, 16, 22));
    let ring_color = if scratching {
        Color32::from_rgb(255, 255, 255)
    } else if is_playing {
        accent
    } else {
        Color32::from_rgb(72, 84, 108)
    };
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(if scratching { 3.6 } else { 2.8 }, ring_color),
    );

    for i in 0..24 {
        let a = angle * 0.5 + (i as f32) * std::f32::consts::TAU / 24.0;
        let p = center + Vec2::new(a.cos(), a.sin()) * (radius - 5.5);
        painter.circle_filled(p, 1.4, Color32::from_rgb(140, 152, 176));
    }

    for r in [47.0, 38.0, 30.0] {
        painter.circle_stroke(center, r, Stroke::new(0.8, Color32::from_rgb(36, 42, 56)));
    }

    painter.circle_filled(center, 22.0, Color32::from_rgb(10, 12, 18));
    painter.circle_stroke(center, 22.0, Stroke::new(1.6, accent));
    let bpm_txt = if scratching {
        "SCRATCH".to_string()
    } else {
        bpm.map(|b| format!("{b:.1}"))
            .unwrap_or_else(|| "---".to_string())
    };
    painter.text(
        center,
        Align2::CENTER_CENTER,
        bpm_txt,
        FontId::monospace(if scratching { 9.5 } else { 11.5 }),
        Color32::from_rgb(240, 244, 250),
    );

    let dir = Vec2::new(angle.cos(), angle.sin());
    painter.line_segment(
        [center + dir * 23.0, center + dir * (radius - 8.0)],
        Stroke::new(3.2, accent),
    );

    let _ = resp.on_hover_text("Drag platter left/right to vinyl-scratch (forward & reverse)");
    out
}

/// Paint a vertical stereo segmented VU level bar.
pub fn paint_vu_meter(ui: &mut egui::Ui, deck: &DeckState) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(20.0, 120.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(4),
        Color32::from_rgb(8, 10, 16),
        Stroke::new(1.0, Color32::from_rgb(44, 52, 72)),
        StrokeKind::Inside,
    );

    let stem_avg =
        (deck.stem_vocals_gain + deck.stem_drums_gain + deck.stem_bass_gain + deck.stem_other_gain)
            * 0.25;
    let pulse = if deck.is_playing && deck.loaded_track.is_some() {
        let p = (deck.position_seconds * 8.0).sin().abs() * 0.25 + 0.72;
        (p * stem_avg * deck.volume_fader).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let segments = 14usize;
    let seg_h = (rect.height() - 8.0) / segments as f32;
    let lit = (pulse * segments as f64).round() as usize;

    for i in 0..segments {
        let y_bot = rect.bottom() - 4.0 - i as f32 * seg_h;
        let y_top = y_bot - seg_h + 2.0;
        let active = i < lit;
        let color = if !active {
            Color32::from_rgb(22, 26, 38)
        } else if i >= 12 {
            Color32::from_rgb(239, 68, 68)
        } else if i >= 9 {
            Color32::from_rgb(245, 158, 11)
        } else {
            Color32::from_rgb(34, 197, 94)
        };
        let seg_rect = Rect::from_min_max(
            Pos2::new(rect.left() + 3.0, y_top),
            Pos2::new(rect.right() - 3.0, y_bot),
        );
        painter.rect_filled(seg_rect, CornerRadius::same(1), color);
    }
}

/// Paint a compact 12-hour Camelot harmonic wheel highlighting Deck A, Deck B, and compatible neighbors.
pub fn paint_camelot_wheel_mini(
    ui: &mut egui::Ui,
    key_a: Option<&str>,
    key_b: Option<&str>,
    palette: &SkinPalette,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(134.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let parsed_a = key_a.and_then(AudioEngine::parse_camelot);
    let parsed_b = key_b.and_then(AudioEngine::parse_camelot);
    let compat_a: Vec<String> = key_a
        .map(AudioEngine::compatible_camelot_keys)
        .unwrap_or_default();

    painter.circle_filled(center, 62.0, palette.fader_slot);
    painter.circle_stroke(center, 62.0, Stroke::new(1.2, palette.card_border));
    painter.circle_stroke(center, 38.0, Stroke::new(1.0, palette.knob_track));

    for hour in 1u32..=12 {
        let angle = ((hour as f32 - 3.0) / 12.0) * std::f32::consts::TAU;
        let dir = Vec2::new(angle.cos(), angle.sin());

        for (mode, radius) in [('B', 50.0_f32), ('A', 27.0_f32)] {
            let label = format!("{hour}{mode}");
            let pos = center + dir * radius;
            let is_a = parsed_a == Some((hour, mode));
            let is_b = parsed_b == Some((hour, mode));
            let is_compat = compat_a.iter().any(|k| k == &label);

            let bg = if is_a {
                palette.deck_a
            } else if is_b {
                palette.deck_b
            } else if is_compat {
                Color32::from_rgb(30, 64, 100)
            } else {
                palette.knob_body
            };
            painter.circle_filled(pos, 9.5, bg);
            let txt_color = if is_a || is_b {
                Color32::BLACK
            } else {
                palette.text_primary
            };
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                label,
                FontId::monospace(8.5),
                txt_color,
            );
        }
    }
}
