#![forbid(unsafe_code)]
//! GPU-painted DJ workstation widgets (platters, scrolling waveforms, VU meters, Camelot wheel).

use djcraft_audio::{AudioEngine, DeckState};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};

/// Paint a CDJ-style jog platter with vinyl grooves, strobe ring, and rotating playhead.
pub fn paint_platter(
    ui: &mut egui::Ui,
    angle: f32,
    accent: Color32,
    is_playing: bool,
    bpm: Option<f64>,
) {
    let (rect, _resp) = ui.allocate_exact_size(Vec2::splat(138.0), egui::Sense::drag());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 64.0;

    // Outer chassis ring
    painter.circle_filled(center, radius, Color32::from_rgb(16, 20, 28));
    let ring_color = if is_playing {
        accent
    } else {
        Color32::from_rgb(72, 84, 108)
    };
    painter.circle_stroke(center, radius, Stroke::new(2.5, ring_color));

    // Strobe dots around rim
    for i in 0..24 {
        let a = angle * 0.5 + (i as f32) * std::f32::consts::TAU / 24.0;
        let p = center + Vec2::new(a.cos(), a.sin()) * (radius - 6.0);
        painter.circle_filled(p, 1.4, Color32::from_rgb(120, 134, 160));
    }

    // Subtle vinyl groove rings
    for r in [48.0, 39.0, 31.0] {
        painter.circle_stroke(center, r, Stroke::new(0.8, Color32::from_rgb(34, 42, 58)));
    }

    // Center hub + BPM readout
    painter.circle_filled(center, 22.0, Color32::from_rgb(12, 16, 24));
    painter.circle_stroke(center, 22.0, Stroke::new(1.5, accent));
    let bpm_txt = bpm
        .map(|b| format!("{b:.1}"))
        .unwrap_or_else(|| "---".to_string());
    painter.text(
        center,
        Align2::CENTER_CENTER,
        bpm_txt,
        FontId::monospace(11.5),
        Color32::from_rgb(240, 244, 250),
    );

    // Rotating marker needle
    let dir = Vec2::new(angle.cos(), angle.sin());
    painter.line_segment(
        [center + dir * 23.0, center + dir * (radius - 9.0)],
        Stroke::new(3.0, accent),
    );
}

/// Paint a scrolling dual-frequency waveform with beatgrid ticks, loop band, and hot-cue pins.
pub fn paint_scrolling_waveform(ui: &mut egui::Ui, deck: &DeckState, accent: Color32) {
    let desired = Vec2::new(ui.available_width().max(180.0), 64.0);
    let (rect, _resp) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect(
        rect,
        CornerRadius::same(6),
        Color32::from_rgb(8, 12, 20),
        Stroke::new(1.0, Color32::from_rgb(38, 52, 82)),
        StrokeKind::Inside,
    );

    let mid_y = rect.center().y;
    painter.line_segment(
        [Pos2::new(rect.left(), mid_y), Pos2::new(rect.right(), mid_y)],
        Stroke::new(1.0, Color32::from_rgb(30, 41, 59)),
    );

    let Some(track) = deck.loaded_track.as_ref() else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No track loaded",
            FontId::proportional(12.0),
            Color32::from_rgb(148, 163, 184),
        );
        return;
    };

    let bpm = (track.bpm * (1.0 + deck.pitch_bend_pct / 100.0)).max(20.0);
    let beat_sec = 60.0 / bpm;
    let window_sec = 8.0_f64; // 4s behind, 4s ahead
    let start_t = deck.position_seconds - window_sec * 0.5;

    // Loop highlight band when active
    if let Some(bars) = deck.loop_bars {
        let loop_w = ((bars as f64 * 4.0 * beat_sec) / window_sec).min(0.45) as f32 * rect.width();
        let loop_rect = Rect::from_min_max(
            Pos2::new(rect.center().x, rect.top() + 2.0),
            Pos2::new((rect.center().x + loop_w).min(rect.right() - 2.0), rect.bottom() - 2.0),
        );
        painter.rect_filled(loop_rect, CornerRadius::same(2), Color32::from_rgba_unmultiplied(34, 197, 94, 34));
    }

    // Dual-frequency waveform bars (bass in deep blue/amber, highs in bright accent)
    let columns = ((rect.width() / 3.0) as usize).clamp(32, 180);
    let col_w = rect.width() / columns as f32;
    let max_half_h = rect.height() * 0.42;

    for col in 0..columns {
        let frac = col as f64 / columns as f64;
        let t = start_t + frac * window_sec;
        if t < 0.0 || t > track.duration_seconds {
            continue;
        }
        let beat_phase = (t % beat_sec) / beat_sec;
        let kick_env = if beat_phase < 0.22 {
            1.0 - beat_phase / 0.22
        } else {
            0.18
        };
        let melodic = ((t * 3.7).sin().abs() * 0.5 + (t * 7.3).cos().abs() * 0.5) * deck.stem_vocals_gain;
        let bass_amp = (kick_env * deck.stem_drums_gain * 0.7 + 0.3 * deck.stem_bass_gain).clamp(0.0, 1.0);
        let high_amp = (melodic * 0.8 + 0.2 * deck.stem_other_gain).clamp(0.0, 1.0);

        let x = rect.left() + col as f32 * col_w + col_w * 0.5;
        let h_bass = (bass_amp as f32) * max_half_h * deck.volume_fader as f32;
        let h_high = (high_amp as f32) * max_half_h * 0.65 * deck.volume_fader as f32;

        painter.line_segment(
            [Pos2::new(x, mid_y - h_bass), Pos2::new(x, mid_y + h_bass)],
            Stroke::new(2.0, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 140)),
        );
        painter.line_segment(
            [Pos2::new(x, mid_y - h_high), Pos2::new(x, mid_y + h_high)],
            Stroke::new(1.4, Color32::from_rgb(226, 232, 240)),
        );
    }

    // Beatgrid tick marks
    let first_beat = (start_t / beat_sec).ceil() as i64;
    let last_beat = ((start_t + window_sec) / beat_sec).floor() as i64;
    for b in first_beat..=last_beat {
        if b < 0 {
            continue;
        }
        let t = b as f64 * beat_sec;
        let x = rect.left() + ((t - start_t) / window_sec) as f32 * rect.width();
        let is_bar = b % 4 == 0;
        let tick_color = if is_bar {
            Color32::from_rgb(248, 250, 252)
        } else {
            Color32::from_rgb(100, 116, 139)
        };
        painter.line_segment(
            [Pos2::new(x, rect.top() + 2.0), Pos2::new(x, rect.top() + if is_bar { 14.0 } else { 7.0 })],
            Stroke::new(if is_bar { 1.6 } else { 1.0 }, tick_color),
        );
    }

    // Hot cue markers
    for (idx, cue_opt) in deck.hot_cue_seconds.iter().enumerate() {
        if let Some(cue_t) = cue_opt {
            if *cue_t >= start_t && *cue_t <= start_t + window_sec {
                let x = rect.left() + ((*cue_t - start_t) / window_sec) as f32 * rect.width();
                painter.line_segment(
                    [Pos2::new(x, rect.top() + 2.0), Pos2::new(x, rect.bottom() - 2.0)],
                    Stroke::new(1.5, Color32::from_rgb(250, 204, 21)),
                );
                painter.text(
                    Pos2::new(x + 3.0, rect.bottom() - 10.0),
                    Align2::LEFT_CENTER,
                    format!("C{}", idx + 1),
                    FontId::monospace(10.0),
                    Color32::from_rgb(250, 204, 21),
                );
            }
        }
    }

    // Center playhead cursor
    painter.line_segment(
        [Pos2::new(rect.center().x, rect.top()), Pos2::new(rect.center().x, rect.bottom())],
        Stroke::new(2.0, Color32::from_rgb(239, 68, 68)),
    );
}

/// Paint a vertical stereo segmented VU level bar.
pub fn paint_vu_meter(ui: &mut egui::Ui, deck: &DeckState) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 92.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(4),
        Color32::from_rgb(8, 12, 20),
        Stroke::new(1.0, Color32::from_rgb(38, 52, 82)),
        StrokeKind::Inside,
    );

    let stem_avg = (deck.stem_vocals_gain
        + deck.stem_drums_gain
        + deck.stem_bass_gain
        + deck.stem_other_gain)
        * 0.25;
    let pulse = if deck.is_playing && deck.loaded_track.is_some() {
        let p = (deck.position_seconds * 8.0).sin().abs() * 0.25 + 0.72;
        (p * stem_avg * deck.volume_fader).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let segments = 12usize;
    let seg_h = (rect.height() - 8.0) / segments as f32;
    let lit = (pulse * segments as f64).round() as usize;

    for i in 0..segments {
        let y_bot = rect.bottom() - 4.0 - i as f32 * seg_h;
        let y_top = y_bot - seg_h + 2.0;
        let active = i < lit;
        let color = if !active {
            Color32::from_rgb(22, 30, 46)
        } else if i >= 10 {
            Color32::from_rgb(239, 68, 68)
        } else if i >= 7 {
            Color32::from_rgb(245, 158, 11)
        } else {
            Color32::from_rgb(34, 197, 94)
        };
        let seg_rect = Rect::from_min_max(
            Pos2::new(rect.left() + 4.0, y_top),
            Pos2::new(rect.right() - 4.0, y_bot),
        );
        painter.rect_filled(seg_rect, CornerRadius::same(1), color);
    }
}

/// Paint a compact 12-hour Camelot harmonic wheel highlighting Deck A, Deck B, and compatible neighbors.
pub fn paint_camelot_wheel_mini(ui: &mut egui::Ui, key_a: Option<&str>, key_b: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(134.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let parsed_a = key_a.and_then(AudioEngine::parse_camelot);
    let parsed_b = key_b.and_then(AudioEngine::parse_camelot);
    let compat_a: Vec<String> = key_a
        .map(AudioEngine::compatible_camelot_keys)
        .unwrap_or_default();

    painter.circle_filled(center, 62.0, Color32::from_rgb(10, 14, 24));
    painter.circle_stroke(center, 62.0, Stroke::new(1.2, Color32::from_rgb(51, 65, 85)));
    painter.circle_stroke(center, 38.0, Stroke::new(1.0, Color32::from_rgb(38, 52, 82)));

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
                Color32::from_rgb(56, 189, 248)
            } else if is_b {
                Color32::from_rgb(245, 158, 11)
            } else if is_compat {
                Color32::from_rgb(30, 64, 100)
            } else {
                Color32::from_rgb(20, 28, 44)
            };
            painter.circle_filled(pos, 9.5, bg);
            let txt_color = if is_a || is_b {
                Color32::BLACK
            } else {
                Color32::from_rgb(220, 228, 240)
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
