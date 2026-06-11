//! Ballistic VU meter reading the tape level (what's hitting the record
//! head), cream face, red zone past 0 VU.

use super::theme;
use egui::{Align2, FontId, Rect, Stroke, StrokeKind, Ui, pos2, vec2};

/// `level` is the engine's tape-level envelope (linear). 0 VU is calibrated
/// to a healthy tape drive level (~0.5 linear).
pub fn draw(ui: &Ui, rect: Rect, level: f32) {
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, theme::VU_FACE);
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(2.0, theme::PANEL_EDGE),
        StrokeKind::Outside,
    );

    let pivot = pos2(rect.center().x, rect.bottom() - 6.0);
    let needle_len = rect.height() * 0.78;

    // Map level to needle: -20 dB .. +3 dB relative to 0 VU = 0.5 linear.
    let db = 20.0 * (level.max(1e-5) / 0.5).log10();
    let norm = ((db + 20.0) / 23.0).clamp(0.0, 1.0);

    // Scale arc: -50 deg .. +50 deg from vertical.
    let arc_start = -50.0f32;
    let arc_span = 100.0f32;

    // Tick marks and the red zone (top ~22% of the arc).
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let a = (arc_start + t * arc_span - 90.0).to_radians();
        let dir = vec2(a.cos(), a.sin());
        let red = t > 0.78;
        let color = if red { theme::VU_RED } else { theme::VU_NEEDLE };
        painter.line_segment(
            [
                pivot + dir * (needle_len * 0.88),
                pivot + dir * (needle_len * (if i % 5 == 0 { 0.74 } else { 0.80 })),
            ],
            Stroke::new(if i % 5 == 0 { 1.6 } else { 1.0 }, color),
        );
    }

    let a = (arc_start + norm * arc_span - 90.0).to_radians();
    let dir = vec2(a.cos(), a.sin());
    painter.line_segment(
        [pivot, pivot + dir * needle_len],
        Stroke::new(1.8, theme::VU_NEEDLE),
    );
    painter.circle_filled(pivot, 3.0, theme::VU_NEEDLE);

    painter.text(
        pos2(rect.center().x, rect.bottom() - 16.0),
        Align2::CENTER_CENTER,
        "VU",
        FontId::monospace(10.0),
        theme::VU_NEEDLE,
    );
}
