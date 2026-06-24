//! The cassette window: a shell behind smoked acrylic, running at the actual
//! motor speed from the DSP — glides, MTR drag, anomaly hiccups and all.
//!
//! The spools are real: the engine reports signed tape footage, and one pack
//! winds onto the other over a C60 side (30 minutes at nominal speed), with
//! pack radii following tape-area conservation and each reel's angular speed
//! tracking its *current* radius — the supply reel visibly spins faster as it
//! runs low, exactly like the real thing. RW runs it all backwards. At the
//! end of the side it ping-pongs, like an auto-reverse deck. Eject puts in a
//! fresh, fully-wound cassette.
//!
//! All geometry scales with the window rect; everything is painter-drawn
//! vectors (no textures, no meaningful CPU cost).

use super::theme;
use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Ui};

/// One side of the virtual cassette, in seconds of tape at nominal speed.
/// C60s — two 30-minute sides — were the everyday cassette; the spool
/// travels its full journey in one side.
pub const SIDE_SECONDS: f32 = 30.0 * 60.0;

/// Per-reel rotation state, integrated each frame from the live motor speed
/// and the current pack radii.
#[derive(Debug, Clone, Copy)]
pub struct ReelAnim {
    pub angle_l: f32,
    pub angle_r: f32,
    pub angle_cap: f32,
}

impl Default for ReelAnim {
    fn default() -> Self {
        Self {
            angle_l: 0.0,
            angle_r: 1.7,
            angle_cap: 0.0,
        }
    }
}

/// `footage_s` is the engine's signed tape footage; `stock_label` is printed
/// on the shell; `wear` 0..1 yellows the label as the tape ages.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &Ui,
    rect: Rect,
    anim: &mut ReelAnim,
    dt: f32,
    speed: f32,
    footage_s: f32,
    stock_label: &str,
    wear: f32,
) {
    let painter = ui.painter();
    let h = rect.height();

    // --- Spool state from real footage: ping-pong over one side. ---
    let x = (footage_s / SIDE_SECONDS).rem_euclid(2.0);
    let frac = if x > 1.0 { 2.0 - x } else { x }; // 0 = all tape on the left
    let hub = h * 0.079;
    let r_full = h * 0.185;
    let r_min = hub + 2.5;
    // Tape cross-section area is conserved: radius grows with sqrt(fraction).
    let pack = |f: f32| (r_min * r_min + (r_full * r_full - r_min * r_min) * f).sqrt();
    let left_pack = pack(1.0 - frac);
    let right_pack = pack(frac);

    // Constant linear tape speed: angular speed scales as 1/current radius.
    // (144 = legacy 6 rad/s spin feel x the 24 px reference radius.)
    anim.angle_l += speed * dt * 144.0 / left_pack.max(1.0);
    anim.angle_r += speed * dt * 144.0 / right_pack.max(1.0);
    anim.angle_cap += speed * dt * 57.6;

    // --- Window bezel + smoked glass. ---
    painter.rect_filled(rect, 6.0, Color32::from_rgb(0x0A, 0x0A, 0x0B));
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(2.0, theme::PANEL_EDGE),
        StrokeKind::Outside,
    );

    // --- Cassette shell. ---
    let shell = rect.shrink2(vec2(12.0, 14.0));
    painter.rect_filled(shell, 5.0, theme::CASSETTE_SHELL);
    painter.rect_stroke(
        shell,
        5.0,
        Stroke::new(1.0, Color32::from_rgb(0x3A, 0x34, 0x30)),
        StrokeKind::Inside,
    );

    // Label strip, printed with the stock in the well. The paper yellows as
    // the tape wears.
    let strip_h = (h * 0.11).clamp(14.0, 26.0);
    let strip = Rect::from_min_max(
        pos2(shell.left() + 10.0, shell.top() + 6.0),
        pos2(shell.right() - 10.0, shell.top() + 6.0 + strip_h),
    );
    let w = wear.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * w) as u8;
    let paper = Color32::from_rgb(lerp(0xD8, 0xAE), lerp(0xCF, 0x9C), lerp(0xB8, 0x78));
    painter.rect_filled(strip, 2.0, paper);
    // Ruled writing lines, like a real index label.
    for i in 1..3 {
        let ly = strip.top() + strip.height() * (i as f32 / 3.0) + 1.0;
        painter.line_segment(
            [pos2(strip.left() + 6.0, ly), pos2(strip.right() - 6.0, ly)],
            Stroke::new(0.6, Color32::from_black_alpha(28)),
        );
    }
    painter.text(
        pos2(strip.left() + 8.0, strip.center().y),
        Align2::LEFT_CENTER,
        "A",
        FontId::monospace((h * 0.05).clamp(8.0, 12.0)),
        Color32::from_rgb(0x33, 0x2E, 0x28),
    );
    painter.text(
        strip.center(),
        Align2::CENTER_CENTER,
        stock_label,
        FontId::monospace((h * 0.042).clamp(7.0, 10.0)),
        Color32::from_rgb(0x33, 0x2E, 0x28),
    );
    painter.text(
        pos2(strip.right() - 8.0, strip.center().y),
        Align2::RIGHT_CENTER,
        "60",
        FontId::monospace((h * 0.042).clamp(7.0, 10.0)),
        Color32::from_rgb(0x33, 0x2E, 0x28),
    );

    // Reel geometry.
    let cy = shell.center().y + h * 0.045;
    let lx = shell.left() + shell.width() * 0.30;
    let rx = shell.left() + shell.width() * 0.70;
    let left_center = pos2(lx, cy);
    let right_center = pos2(rx, cy);

    // The clear window across the hubs (the see-through part of the shell).
    let window = Rect::from_min_max(
        pos2(lx - r_full - 6.0, cy - r_full - 4.0),
        pos2(rx + r_full + 6.0, cy + r_full + 4.0),
    );
    painter.rect_filled(window, r_full * 0.6, Color32::from_rgb(0x0E, 0x0D, 0x0C));
    painter.rect_stroke(
        window,
        r_full * 0.6,
        Stroke::new(1.2, Color32::from_rgb(0x42, 0x3A, 0x34)),
        StrokeKind::Inside,
    );

    // Tape transport hardware along the bottom edge.
    let deck_y = shell.bottom() - h * 0.055;
    let head_w = h * 0.16;
    let head_cx = shell.center().x;

    // Capstan (thin, spins fast) + pinch roller.
    let capstan = pos2(rx - 4.0, deck_y - 2.0);
    let pinch = pos2(capstan.x + h * 0.055, capstan.y);
    let capstan_r = h * 0.015;

    // Guide rollers at the path corners.
    let guide_l = pos2(lx - h * 0.085, deck_y - 2.0);
    let guide_r = pos2(head_cx + head_w * 0.5 + h * 0.06, deck_y - 2.0);

    // Tape path: left pack tangent -> guide -> head block -> guide -> capstan
    // -> up to the right pack tangent. The tangents ride the live pack radii.
    let tape_stroke = Stroke::new(2.0, theme::TAPE_PACK);
    let l_tangent = pos2(lx - left_pack, cy + 4.0);
    let r_tangent = pos2(rx + right_pack - 4.0, cy + 5.0);
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
    let erase = Rect::from_center_size(
        pos2(head_cx - head_w * 0.5 + 2.0, deck_y + 1.0),
        vec2(h * 0.05, h * 0.05),
    );
    let rp_head = Rect::from_center_size(
        pos2(head_cx + head_w * 0.15, deck_y + 1.5),
        vec2(h * 0.10, h * 0.055),
    );
    for (r, c) in [
        (erase, Color32::from_rgb(0x52, 0x50, 0x4C)),
        (rp_head, Color32::from_rgb(0x6A, 0x68, 0x62)),
    ] {
        painter.rect_filled(r, 1.5, c);
        painter.rect_stroke(
            r,
            1.5,
            Stroke::new(0.8, Color32::from_rgb(0x2A, 0x28, 0x26)),
            StrokeKind::Inside,
        );
    }

    // Guides, capstan, pinch roller.
    for g in [guide_l, guide_r] {
        painter.circle_filled(g, h * 0.02, Color32::from_rgb(0x44, 0x42, 0x3E));
        painter.circle_filled(g, h * 0.0075, Color32::from_rgb(0x18, 0x16, 0x15));
    }
    painter.circle_filled(capstan, capstan_r, Color32::from_rgb(0xB8, 0xB4, 0xAC));
    // Rotation marker on the capstan so its speed is visible.
    let cdir = vec2(anim.angle_cap.cos(), anim.angle_cap.sin());
    painter.line_segment(
        [capstan, capstan + cdir * capstan_r * 0.9],
        Stroke::new(1.0, Color32::from_rgb(0x33, 0x30, 0x2C)),
    );
    let pinch_r = h * 0.028;
    painter.circle_filled(pinch, pinch_r, Color32::from_rgb(0x2E, 0x2C, 0x2A));
    painter.circle_stroke(
        pinch,
        pinch_r,
        Stroke::new(0.8, Color32::from_rgb(0x48, 0x44, 0x40)),
    );

    // Reels (drawn after the path so packs cover the tangent joins).
    draw_reel(ui, left_center, left_pack, hub, anim.angle_l);
    draw_reel(ui, right_center, right_pack, hub, anim.angle_r);

    // A faint motion streak on the packs when the tape is really moving.
    if speed.abs() > 3.0 {
        for (c, r) in [(left_center, left_pack), (right_center, right_pack)] {
            painter.circle_stroke(c, r - 2.0, Stroke::new(1.0, Color32::from_white_alpha(10)));
        }
    }

    // Shell screws (center bottom + label corners, like the real molding).
    for screw in [
        pos2(shell.center().x, cy),
        pos2(shell.left() + 7.0, shell.bottom() - 7.0),
        pos2(shell.right() - 7.0, shell.bottom() - 7.0),
    ] {
        painter.circle_filled(screw, 2.0, Color32::from_rgb(0x16, 0x14, 0x12));
        painter.circle_stroke(
            screw,
            2.0,
            Stroke::new(0.6, Color32::from_rgb(0x4A, 0x44, 0x3E)),
        );
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

fn draw_reel(ui: &Ui, center: Pos2, pack_radius: f32, hub: f32, angle: f32) {
    let painter = ui.painter();

    // Tape pack with a couple of sheen rings.
    painter.circle_filled(center, pack_radius, theme::TAPE_PACK);
    for ring in [0.55f32, 0.8] {
        let r = pack_radius * ring;
        if r > hub + 1.5 {
            painter.circle_stroke(center, r, Stroke::new(0.7, Color32::from_black_alpha(60)));
        }
    }
    // Outer edge highlight: wound tape has a slightly glossy rim.
    painter.circle_stroke(
        center,
        pack_radius,
        Stroke::new(1.0, Color32::from_rgb(0x5E, 0x42, 0x34)),
    );

    // Hub: dark center, bright rim, six teeth.
    painter.circle_filled(center, hub, Color32::from_rgb(0x14, 0x12, 0x11));
    painter.circle_stroke(center, hub, Stroke::new(1.5, theme::HUB));
    for i in 0..6 {
        let a = angle + i as f32 * std::f32::consts::TAU / 6.0;
        let dir = vec2(a.cos(), a.sin());
        painter.line_segment(
            [center + dir * (hub * 0.35), center + dir * (hub * 0.85)],
            Stroke::new(2.2, theme::HUB),
        );
    }
}
