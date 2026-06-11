//! The cassette window: a shell behind smoked acrylic, running at the actual
//! motor speed from the DSP — glides, MTR drag, anomaly hiccups and all.
//!
//! The cassette in the well is a Space Case TE-2 tape: the tribute lives on
//! the label. Reels turn at physically-correct rates (angular speed inversely
//! proportional to pack radius), the capstan spins fast, and the tape runs
//! the real path: left pack, guide, across the head block, capstan/pinch
//! roller, right pack.

use super::theme;
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Ui, pos2, vec2};

/// Pack radii relative to the hub. Slightly uneven for looks; the rotation
/// rates follow from these (constant linear tape speed).
const LEFT_PACK: f32 = 30.0;
const RIGHT_PACK: f32 = 22.0;
const HUB: f32 = 13.0;

pub fn draw(ui: &Ui, rect: Rect, reel_angle: f32, speed: f32) {
    let painter = ui.painter();

    // Window bezel + smoked glass.
    painter.rect_filled(rect, 6.0, Color32::from_rgb(0x0A, 0x0A, 0x0B));
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(2.0, theme::PANEL_EDGE),
        StrokeKind::Outside,
    );

    // Cassette shell.
    let shell = rect.shrink2(vec2(12.0, 14.0));
    painter.rect_filled(shell, 4.0, theme::CASSETTE_SHELL);
    painter.rect_stroke(
        shell,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(0x3A, 0x34, 0x30)),
        StrokeKind::Inside,
    );

    // Label strip — the tribute easter egg.
    let strip = Rect::from_min_max(
        pos2(shell.left() + 9.0, shell.top() + 5.0),
        pos2(shell.right() - 9.0, shell.top() + 20.0),
    );
    painter.rect_filled(strip, 2.0, Color32::from_rgb(0xD8, 0xCF, 0xB8));
    painter.text(
        strip.center(),
        Align2::CENTER_CENTER,
        "SPACE CASE TE-2  ·  POSITION · HIGH",
        FontId::monospace(7.0),
        Color32::from_rgb(0x33, 0x2E, 0x28),
    );

    // Reel geometry.
    let cy = shell.center().y + 6.0;
    let lx = shell.left() + shell.width() * 0.30;
    let rx = shell.left() + shell.width() * 0.70;
    let left_center = pos2(lx, cy);
    let right_center = pos2(rx, cy);

    // Constant linear tape speed: angular speed scales as 1/pack radius.
    let angle_l = reel_angle * (24.0 / LEFT_PACK);
    let angle_r = reel_angle * (24.0 / RIGHT_PACK);

    // Tape transport hardware along the bottom edge.
    let deck_y = shell.bottom() - 9.0;
    let head_w = 26.0;
    let head_cx = shell.center().x;

    // Capstan (thin, spins fast: radius ~2.5 -> big multiplier) + pinch roller.
    let capstan = pos2(rx - 4.0, deck_y - 2.0);
    let pinch = pos2(capstan.x + 9.0, capstan.y);
    let capstan_angle = reel_angle * (24.0 / 2.5);

    // Guide rollers at the path corners.
    let guide_l = pos2(lx - 14.0, deck_y - 2.0);
    let guide_r = pos2(head_cx + head_w * 0.5 + 10.0, deck_y - 2.0);

    // Tape path: left pack tangent -> guide -> head block -> guide -> capstan
    // -> up to the right pack tangent.
    let tape_stroke = Stroke::new(2.0, theme::TAPE_PACK);
    let l_tangent = pos2(lx - LEFT_PACK, cy + 4.0);
    let r_tangent = pos2(rx + RIGHT_PACK - 4.0, cy + 5.0);
    let path = [
        l_tangent,
        pos2(guide_l.x - 3.0, guide_l.y - 3.0),
        pos2(head_cx - head_w * 0.5 - 6.0, deck_y - 5.0),
        pos2(head_cx + head_w * 0.5 + 6.0, deck_y - 5.0),
        pos2(guide_r.x + 2.0, guide_r.y - 3.0),
        pos2(capstan.x - 2.0, capstan.y - 4.0),
        r_tangent,
    ];
    for pair in path.windows(2) {
        painter.line_segment([pair[0], pair[1]], tape_stroke);
    }

    // Head block: erase head (small) + record/play head (wide), pressed up
    // against the tape from below.
    let erase = Rect::from_center_size(pos2(head_cx - head_w * 0.5 + 2.0, deck_y + 1.0), vec2(8.0, 8.0));
    let rp_head = Rect::from_center_size(pos2(head_cx + 4.0, deck_y + 1.5), vec2(16.0, 9.0));
    for (r, c) in [
        (erase, Color32::from_rgb(0x52, 0x50, 0x4C)),
        (rp_head, Color32::from_rgb(0x6A, 0x68, 0x62)),
    ] {
        painter.rect_filled(r, 1.5, c);
        painter.rect_stroke(r, 1.5, Stroke::new(0.8, Color32::from_rgb(0x2A, 0x28, 0x26)), StrokeKind::Inside);
    }

    // Guides, capstan, pinch roller.
    for g in [guide_l, guide_r] {
        painter.circle_filled(g, 3.2, Color32::from_rgb(0x44, 0x42, 0x3E));
        painter.circle_filled(g, 1.2, Color32::from_rgb(0x18, 0x16, 0x15));
    }
    painter.circle_filled(capstan, 2.4, Color32::from_rgb(0xB8, 0xB4, 0xAC));
    // Rotation marker on the capstan so its speed is visible.
    let cdir = vec2(capstan_angle.cos(), capstan_angle.sin());
    painter.line_segment(
        [capstan, capstan + cdir * 2.2],
        Stroke::new(1.0, Color32::from_rgb(0x33, 0x30, 0x2C)),
    );
    painter.circle_filled(pinch, 4.6, Color32::from_rgb(0x2E, 0x2C, 0x2A));
    painter.circle_stroke(pinch, 4.6, Stroke::new(0.8, Color32::from_rgb(0x48, 0x44, 0x40)));

    // Reels (drawn after the path so packs cover the tangent joins).
    draw_reel(ui, left_center, LEFT_PACK, angle_l);
    draw_reel(ui, right_center, RIGHT_PACK, angle_r);

    // A faint motion streak on the packs when the tape is really moving.
    if speed.abs() > 3.0 {
        for (c, r) in [(left_center, LEFT_PACK), (right_center, RIGHT_PACK)] {
            painter.circle_stroke(
                c,
                r - 2.0,
                Stroke::new(1.0, Color32::from_white_alpha(10)),
            );
        }
    }

    // Window screws.
    for corner in [
        rect.left_top() + vec2(8.0, 8.0),
        rect.right_top() + vec2(-8.0, 8.0),
        rect.left_bottom() + vec2(8.0, -8.0),
        rect.right_bottom() + vec2(-8.0, -8.0),
    ] {
        painter.circle_filled(corner, 2.5, Color32::from_rgb(0x4A, 0x4A, 0x4E));
    }
}

fn draw_reel(ui: &Ui, center: Pos2, pack_radius: f32, angle: f32) {
    let painter = ui.painter();

    // Tape pack with a couple of sheen rings.
    painter.circle_filled(center, pack_radius, theme::TAPE_PACK);
    for ring in [0.55f32, 0.8] {
        painter.circle_stroke(
            center,
            pack_radius * ring,
            Stroke::new(0.7, Color32::from_black_alpha(60)),
        );
    }

    // Hub: dark center, bright rim, six teeth.
    painter.circle_filled(center, HUB, Color32::from_rgb(0x14, 0x12, 0x11));
    painter.circle_stroke(center, HUB, Stroke::new(1.5, theme::HUB));
    for i in 0..6 {
        let a = angle + i as f32 * std::f32::consts::TAU / 6.0;
        let dir = vec2(a.cos(), a.sin());
        painter.line_segment(
            [center + dir * (HUB * 0.35), center + dir * (HUB * 0.85)],
            Stroke::new(2.2, theme::HUB),
        );
    }
}
