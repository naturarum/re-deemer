//! The RE-DEEMER panel: a vector tribute to the Space Case TE-2 faceplate.
//! Dark walnut frame, black panel, cassette window with a live transport,
//! VU, the 21-fader position matrix, and every control bound to its
//! parameter — with hover help on all of them.

mod cassette;
mod theme;
mod vu;
mod widgets;

use crate::UiShared;
use crate::params::Te2Params;
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use nice_plug::prelude::Editor;
use nice_plug_egui::{EguiSettings, create_egui_editor};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// The fixed logical canvas everything is drawn in. UI scaling resizes the
/// window and scales the layer; coordinates in this file never change.
const CANVAS: egui::Vec2 = egui::Vec2::new(1080.0, 560.0);

/// The scale steps offered in SETUP.
pub const UI_SCALES: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

struct EditorState {
    /// Reel/capstan rotation, integrated from the live motor speed and the
    /// current pack radii (the engine supplies footage; the radii follow).
    anim: cassette::ReelAnim,
    /// The TAPE & MACHINE overlay (opened from the RE-2 logo / SETUP button).
    settings_open: bool,
    /// Scale we last asked the host to size the window for.
    applied_scale: f32,
}

pub fn create(params: Arc<Te2Params>, shared: Arc<UiShared>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        EditorState {
            anim: cassette::ReelAnim::default(),
            settings_open: false,
            applied_scale: 0.0,
        },
        EguiSettings::default(),
        |_ctx, _queue, _state| {},
        move |ui, setter, _queue, state| {
            // The reels never stop turning.
            ui.ctx().request_repaint();
            draw_panel(ui, setter, &params, &shared, state);
        },
    )
}

/// Render one frame of the panel for offline snapshots (dev tooling).
pub fn draw_for_snapshot(
    ui: &mut egui::Ui,
    setter: &nice_plug::prelude::ParamSetter,
    params: &Te2Params,
    shared: &UiShared,
    reel_angle: f32,
    settings_open: bool,
) {
    let mut state = EditorState {
        anim: cassette::ReelAnim {
            angle_l: reel_angle,
            angle_r: reel_angle * 1.3 + 1.7,
            angle_cap: reel_angle * 9.6,
        },
        settings_open,
        applied_scale: 0.0,
    };
    draw_panel(ui, setter, params, shared, &mut state);
}

/// Scale plumbing: the panel is drawn into its own Area whose layer carries
/// a scale transform, and the window is resized to CANVAS x scale. Every
/// coordinate below stays in the fixed 1080x560 canvas space.
fn draw_panel(
    ui: &mut egui::Ui,
    setter: &nice_plug::prelude::ParamSetter,
    params: &Te2Params,
    shared: &UiShared,
    state: &mut EditorState,
) {
    let scale = params.ui_scale.load(Ordering::Relaxed).clamp(0.5, 2.0);
    if (state.applied_scale - scale).abs() > 1e-3 {
        let want = (
            (CANVAS.x * scale).round() as u32,
            (CANVAS.y * scale).round() as u32,
        );
        if params.editor_state.size() != want {
            params.editor_state.set_requested_size(want);
        }
        state.applied_scale = scale;
    }

    let ctx = ui.ctx().clone();
    let panel_id = egui::Id::new("te2-panel");
    ctx.set_transform_layer(
        egui::LayerId::new(egui::Order::Middle, panel_id),
        egui::emath::TSTransform::from_scaling(scale),
    );
    egui::Area::new(panel_id)
        .order(egui::Order::Middle)
        .fixed_pos(pos2(0.0, 0.0))
        .show(&ctx, |ui| {
            draw_panel_inner(ui, setter, params, shared, state, scale);
        });
}

fn draw_panel_inner(
    ui: &mut egui::Ui,
    setter: &nice_plug::prelude::ParamSetter,
    params: &Te2Params,
    shared: &UiShared,
    state: &mut EditorState,
    scale: f32,
) {
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    let speed = shared.speed.load(Ordering::Relaxed);
    let footage = shared.footage.load(Ordering::Relaxed);

    let full = Rect::from_min_size(pos2(0.0, 0.0), CANVAS);
    ui.allocate_rect(full, Sense::hover());
    let painter = ui.painter().clone();

    // Wood frame + faceplate. The grain is a handful of deterministic
    // streaks on the visible border strips — texture without textures.
    painter.rect_filled(full, 0.0, theme::WOOD);
    let fract = |x: f32| x - x.floor();
    for i in 0..34u32 {
        let t = fract(i as f32 * 0.6180340);
        let (color, alpha) = if i % 3 == 0 {
            (theme::WOOD_GRAIN_LIGHT, 90)
        } else {
            (theme::WOOD_GRAIN_DARK, 110)
        };
        let c = color.gamma_multiply(alpha as f32 / 255.0);
        let w = 0.7 + 1.1 * fract(t * 7.13);
        // Horizontal grain across the top and bottom rails…
        let y_top = full.top() + 1.5 + t * 13.0;
        let y_bot = full.bottom() - 1.5 - fract(t * 3.7) * 13.0;
        let x0 = full.left() + fract(t * 11.3) * 140.0;
        let x1 = full.right() - fract(t * 5.9) * 140.0;
        painter.line_segment([pos2(x0, y_top), pos2(x1, y_top)], Stroke::new(w, c));
        painter.line_segment([pos2(x0, y_bot), pos2(x1, y_bot)], Stroke::new(w, c));
        // …and vertical grain down the side cheeks.
        if i < 14 {
            let x_l = full.left() + 1.5 + t * 13.0;
            let x_r = full.right() - 1.5 - fract(t * 3.7) * 13.0;
            let y0 = full.top() + fract(t * 9.1) * 90.0;
            let y1 = full.bottom() - fract(t * 4.3) * 90.0;
            painter.line_segment([pos2(x_l, y0), pos2(x_l, y1)], Stroke::new(w, c));
            painter.line_segment([pos2(x_r, y0), pos2(x_r, y1)], Stroke::new(w, c));
        }
    }
    painter.rect_stroke(
        full.shrink(1.0),
        2.0,
        Stroke::new(2.0, theme::WOOD_EDGE),
        StrokeKind::Inside,
    );
    let panel = full.shrink(16.0);
    painter.rect_filled(panel, 4.0, theme::PANEL);
    painter.rect_stroke(
        panel,
        4.0,
        Stroke::new(1.5, theme::PANEL_EDGE),
        StrokeKind::Inside,
    );

    // Wordmarks. The RE-2 logo doubles as the settings latch. Logo, VU and
    // SETUP share one centerline (x = 993).
    painter.text(
        pos2(993.0, 36.0),
        Align2::CENTER_CENTER,
        "RE-2",
        FontId::monospace(22.0),
        theme::INK,
    );
    let logo = ui.allocate_rect(
        Rect::from_center_size(pos2(993.0, 35.0), vec2(76.0, 26.0)),
        Sense::click(),
    );
    if logo.clicked() {
        state.settings_open = !state.settings_open;
    }
    logo.on_hover_text("Tape stock, aging and machine setup.");
    painter.text(
        pos2(224.0, 404.0),
        Align2::CENTER_CENTER,
        "RE-DEEMER",
        FontId::monospace(28.0),
        theme::INK,
    );

    // Cassette window + VU, driven by the audio thread.
    let stock_profile = params.tape_stock.value().to_dsp().profile();
    let wear_eff = if params.aging_on.value() {
        params.tape_age.load(Ordering::Relaxed)
    } else {
        0.0
    };
    cassette::draw(
        ui,
        Rect::from_min_max(pos2(36.0, 32.0), pos2(412.0, 272.0)),
        &mut state.anim,
        dt,
        speed,
        footage,
        stock_profile.label,
        wear_eff,
    );
    vu::draw(
        ui,
        Rect::from_min_max(pos2(942.0, 50.0), pos2(1044.0, 118.0)),
        shared.vu.load(Ordering::Relaxed),
    );

    let position = shared.position.load(Ordering::Relaxed);

    // --- 1-8 position buttons (click = select, hold = RES gate) ---
    // Centered on the cassette's axis (x = 224), like the transport below.
    let mut any_held = false;
    for i in 0..8u8 {
        let cx = 77.0 + i as f32 * 42.0;
        let center = pos2(cx, 324.0);
        widgets::led(ui, pos2(cx, 298.0), position == i + 1, theme::LED_RED);
        let rect = Rect::from_center_size(center, vec2(28.0, 28.0));
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        if response.drag_started() || response.clicked() {
            setter.begin_set_parameter(&params.position);
            setter.set_parameter(&params.position, (i + 1) as i32);
            setter.end_set_parameter(&params.position);
        }
        any_held |= response.is_pointer_button_down_on();
        response.on_hover_text(
            "Select position. Position 1 is the panel itself; 2-8 are the fader columns. \
             Hold to gate the RES filter synth (with GATE on).",
        );
        let p = ui.painter();
        p.circle_filled(center, 13.0, theme::KNOB_BODY);
        p.circle_stroke(center, 13.0, Stroke::new(1.2, theme::KNOB_EDGE));
        p.text(
            pos2(cx, 324.0 + 24.0),
            Align2::CENTER_CENTER,
            format!("{}", i + 1),
            FontId::monospace(9.0),
            theme::INK,
        );
    }
    shared.ui_gate.store(any_held, Ordering::Relaxed);

    // --- Transport row: spans exactly the cassette window's width ---
    let ty = 474.0;
    let bsize = vec2(52.0, 32.0);
    let bx = |i: f32| pos2(62.0 + i * 54.0, ty);
    let transport = params.transport_mode.value();

    let stop_on = params.stop.value();
    let r = widgets::action_button(
        ui,
        Rect::from_center_size(bx(0.0), bsize),
        "STP/EJ",
        stop_on,
        None,
        "Stop the tape. Double-click: eject — wipes the tape and resets, \
         like dropping in a fresh cassette.",
    );
    if r.double_clicked() {
        shared.eject.store(true, Ordering::Relaxed);
    } else if r.clicked() {
        setter.begin_set_parameter(&params.stop);
        setter.set_parameter(&params.stop, !stop_on);
        setter.end_set_parameter(&params.stop);
    }

    use crate::params::TransportMode;
    for (i, (mode, text, accent, help)) in [
        (
            TransportMode::Echo,
            "REC/ECHO",
            Some(theme::LED_RED),
            "Normal mode: record + echo, erase head active.",
        ),
        (
            TransportMode::Play,
            "PLAY",
            Some(theme::LED_GREEN),
            "Playback manipulation only — nothing new is recorded. \
             TIME repitches what's on the tape (cassette synthesizer).",
        ),
        (
            TransportMode::Loop,
            "LOOP",
            Some(theme::LED_YELLOW),
            "Erase head lifted: sound-on-sound layering on a finite loop. \
             Old layers slowly compost under new ones.",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let r = widgets::action_button(
            ui,
            Rect::from_center_size(bx(1.0 + i as f32), bsize),
            text,
            transport == mode,
            accent,
            help,
        );
        if r.clicked() {
            setter.begin_set_parameter(&params.transport_mode);
            setter.set_parameter(&params.transport_mode, mode);
            setter.end_set_parameter(&params.transport_mode);
        }
    }

    // RW / FF: momentary wind while held.
    use crate::params::WindMode;
    for (i, (mode, text, help)) in [
        (
            WindMode::Rewind,
            "RW",
            "Hold: reverse playback (in PLAY mode), speed follows TIME.",
        ),
        (
            WindMode::FastForward,
            "FF",
            "Hold: high-speed playback (in PLAY mode), speed follows TIME.",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_center_size(bx(4.0 + i as f32), bsize);
        let response = ui.allocate_rect(rect, Sense::drag());
        if response.drag_started() {
            setter.begin_set_parameter(&params.wind);
            setter.set_parameter(&params.wind, mode);
        }
        if response.drag_stopped() {
            setter.set_parameter(&params.wind, WindMode::Off);
            setter.end_set_parameter(&params.wind);
        }
        let on = params.wind.value() == mode;
        let p = ui.painter();
        let body = if on { theme::KNOB_EDGE } else { theme::KNOB_BODY };
        p.rect_filled(rect, 3.0, body);
        p.rect_stroke(
            rect,
            3.0,
            Stroke::new(1.0, theme::PANEL_EDGE),
            StrokeKind::Outside,
        );
        p.text(
            pos2(rect.center().x, rect.bottom() + 8.0),
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(8.0),
            theme::INK,
        );
        response.on_hover_text(help);
    }

    widgets::toggle_button(
        ui,
        setter,
        &params.pause,
        Rect::from_center_size(bx(6.0), bsize),
        "PAUSE",
        Some(theme::LED_YELLOW),
        "Mechanical pause: tape stops fast, speed setting retained.",
    );

    // --- Top knob row. The whole cycle group lives together at the right
    // end: 1-8 length, DIV, CYCLE, with SYNC and CYC directly beneath.
    // With SYNC on the rate comes from the host tempo via DIV, so CYCLE
    // dims; free-running, DIV dims instead.
    let ky = 66.0;
    let synced = params.rate_sync.value();
    widgets::knob(
        ui,
        setter,
        &params.loop_len,
        pos2(466.0, ky),
        17.0,
        "LOOP",
        "Loop length for LOOP mode, in seconds of tape footage. \
         SYNC snaps it to whole beats.",
    );
    widgets::knob_capped(
        ui,
        setter,
        &params.white_drift,
        pos2(562.0, ky),
        17.0,
        "DRIFT",
        Some(theme::CAP_WHITE),
        false,
        "Glide time between positions for the White set (0-14 s).",
    );
    widgets::knob_capped(
        ui,
        setter,
        &params.gray_drift,
        pos2(626.0, ky),
        17.0,
        "DRIFT",
        Some(theme::CAP_GRAY),
        false,
        "Glide time between positions for the Gray set (0-14 s).",
    );
    widgets::knob_capped(
        ui,
        setter,
        &params.black_drift,
        pos2(690.0, ky),
        17.0,
        "DRIFT",
        Some(theme::CAP_BLACK),
        false,
        "Glide time between positions for the Black set (0-14 s).",
    );
    widgets::rotary_selector(
        ui,
        setter,
        &params.cycle_len,
        pos2(786.0, ky),
        18.0,
        8,
        "1-8",
        "How many positions the cycle rotates through. The Anomaly fires \
         on the final step.",
    );
    widgets::knob_capped(
        ui,
        setter,
        &params.rate_div,
        pos2(850.0, ky),
        17.0,
        "DIV",
        None,
        !synced,
        "Cycle rate as a host-tempo division, 1/1 down to 1/32 — active \
         when SYNC is on.",
    );
    widgets::knob_capped(
        ui,
        setter,
        &params.cycle_rate,
        pos2(914.0, ky),
        17.0,
        "CYCLE",
        None,
        synced,
        "Free-running cycle speed: 8 seconds per step up to 4,000 steps per \
         second — audio-rate stepping turns the faders into a waveform. \
         With SYNC on, the rate is the host tempo divided by DIV instead.",
    );

    let small = vec2(34.0, 16.0);
    widgets::toggle_button(
        ui,
        setter,
        &params.loop_sync,
        Rect::from_center_size(pos2(466.0, 112.0), small),
        "SYNC",
        Some(theme::LED_RED),
        "Snap the loop length to whole beats of the host tempo.",
    );
    widgets::toggle_button(
        ui,
        setter,
        &params.rate_sync,
        Rect::from_center_size(pos2(850.0, 112.0), small),
        "SYNC",
        Some(theme::LED_RED),
        "Lock the cycle to the host clock: rate = tempo divided by DIV, \
         steps phase-locked to the playhead while the transport rolls.",
    );
    widgets::toggle_button(
        ui,
        setter,
        &params.cycle_run,
        Rect::from_center_size(pos2(914.0, 112.0), small),
        "CYC",
        Some(theme::LED_RED),
        "Run the cycle: rotate through positions 1 to the 1-8 limit.",
    );

    // --- Set selectors + cycle length ---
    let sy = 158.0;
    widgets::switch3(
        ui,
        setter,
        &params.white_sel,
        pos2(470.0, sy),
        ["TM", "RS", "MS"],
        "What the White faders control: TiMe, ReSonance, or Mod Speed.",
    );
    widgets::toggle_button(
        ui,
        setter,
        &params.white_on,
        Rect::from_center_size(pos2(532.0, sy), vec2(26.0, 16.0)),
        "ON",
        Some(theme::LED_RED),
        "Engage the White set: its faders take over the selected control \
         on positions 2-8.",
    );
    widgets::switch3(
        ui,
        setter,
        &params.gray_sel,
        pos2(610.0, sy),
        ["FB", "MA", "LP"],
        "What the Gray faders control: FeedBack, Mod Amount, or LPF.",
    );
    widgets::toggle_button(
        ui,
        setter,
        &params.gray_on,
        Rect::from_center_size(pos2(672.0, sy), vec2(26.0, 16.0)),
        "ON",
        Some(theme::LED_RED),
        "Engage the Gray set.",
    );
    widgets::switch3(
        ui,
        setter,
        &params.black_sel,
        pos2(750.0, sy),
        ["TP", "DL", "HP"],
        "What the Black faders control: TaPe level, Dry Level, or HPF.",
    );
    widgets::toggle_button(
        ui,
        setter,
        &params.black_on,
        Rect::from_center_size(pos2(812.0, sy), vec2(26.0, 16.0)),
        "ON",
        Some(theme::LED_RED),
        "Engage the Black set.",
    );

    // --- Fader matrix: positions 2-8 x White/Gray/Black ---
    let whites = params.white_faders();
    let grays = params.gray_faders();
    let blacks = params.black_faders();
    for col in 0..7usize {
        let cx = 466.0 + col as f32 * 92.0;
        widgets::label(ui, pos2(cx, 198.0), &format!("{}", col + 2), 9.5, theme::INK);
        widgets::fader(
            ui,
            setter,
            whites[col],
            pos2(cx - 18.0, 260.0),
            96.0,
            theme::CAP_WHITE,
            "White set value for this position.",
        );
        widgets::fader(
            ui,
            setter,
            grays[col],
            pos2(cx, 260.0),
            96.0,
            theme::CAP_GRAY,
            "Gray set value for this position.",
        );
        widgets::fader(
            ui,
            setter,
            blacks[col],
            pos2(cx + 18.0, 260.0),
            96.0,
            theme::CAP_BLACK,
            "Black set value for this position.",
        );
        widgets::led(ui, pos2(cx, 322.0), position == (col + 2) as u8, theme::LED_RED);
    }
    // Position 1 = the panel itself.
    widgets::led(ui, pos2(440.0, 322.0), position == 1, theme::LED_RED);
    widgets::label(ui, pos2(440.0, 198.0), "1", 9.5, theme::INK_DIM);

    // --- Primary controls ---
    let ay = 366.0;
    let ax = |i: f32| pos2(452.0 + i * 64.0, ay);
    widgets::knob(
        ui,
        setter,
        &params.mod_amt,
        ax(0.0),
        18.0,
        "MOD AMT",
        "Sine modulation depth on motor speed — from gentle wobble to seasick.",
    );
    widgets::knob(
        ui,
        setter,
        &params.mod_spd,
        ax(1.0),
        18.0,
        "MOD SPD",
        "Modulation speed, 0.1 Hz up to 150 Hz (audio-rate FM territory).",
    );
    widgets::knob(
        ui,
        setter,
        &params.hpf,
        ax(2.0),
        18.0,
        "HPF",
        "24 dB/oct filter in the echo path. Cutting lows before the record \
         head frees up tape headroom — repeats stay cleaner.",
    );
    widgets::knob(
        ui,
        setter,
        &params.lpf,
        ax(3.0),
        18.0,
        "LPF",
        "24 dB/oct filter in the echo path. Every repeat passes through it \
         again, darkening as it regenerates.",
    );
    widgets::knob(
        ui,
        setter,
        &params.res,
        ax(4.0),
        18.0,
        "RES",
        "LPF resonance. Near the top it self-oscillates into a pure tone \
         whose pitch is the LPF cutoff.",
    );
    widgets::toggle_button(
        ui,
        setter,
        &params.res_gate,
        Rect::from_center_size(pos2(760.0, 366.0), vec2(30.0, 18.0)),
        "GATE",
        Some(theme::LED_RED),
        "Gate the resonance with the 1-8 buttons (or MIDI notes): play the \
         self-oscillating filter like an 8-pitch synth.",
    );

    let by = 452.0;
    let bxk = |i: f32| pos2(452.0 + i * 64.0, by);
    widgets::knob(
        ui,
        setter,
        &params.time,
        bxk(0.0),
        18.0,
        "TIME",
        "Delay time = tape speed. Changing it bends the pitch of everything \
         already on the tape — the signature move.",
    );
    widgets::knob(
        ui,
        setter,
        &params.feedback,
        bxk(1.0),
        18.0,
        "FDBK",
        "Echo sustain. Past 100% it runs away — the tape itself does the \
         limiting, not a digital clamp.",
    );
    widgets::knob(
        ui,
        setter,
        &params.tape_level,
        bxk(2.0),
        18.0,
        "TP LVL",
        "Level of all tape sound at the output.",
    );
    widgets::knob(
        ui,
        setter,
        &params.dry_level,
        bxk(3.0),
        18.0,
        "DRY LVL",
        "Level of the dry signal, taken from the input stage.",
    );
    widgets::knob(
        ui,
        setter,
        &params.out_drive,
        bxk(4.0),
        18.0,
        "OUT DRV",
        "Op-amp output drive on the tape + dry mix.",
    );

    // --- Right block: two rows aligned with the primary knob rows ---
    let setup = widgets::action_button(
        ui,
        Rect::from_center_size(pos2(993.0, 144.0), vec2(46.0, 22.0)),
        "SETUP",
        state.settings_open,
        Some(theme::LED_YELLOW),
        "Tape stock, aging, hiss, mechanism, quality, MIDI — the machine room.",
    );
    if setup.clicked() {
        state.settings_open = !state.settings_open;
    }

    widgets::switch3(
        ui,
        setter,
        &params.anomaly_pol,
        pos2(862.0, ay),
        ["-", "OFF", "+"],
        "Anomaly polarity: the tape hiccup bends pitch down (-) or up (+). \
         Middle = off.",
    );
    widgets::knob(
        ui,
        setter,
        &params.anomaly,
        pos2(944.0, ay),
        15.0,
        "ANMLY",
        "Anomaly amount: a single speed blip on the cycle's final step, from \
         a quick flick to a long singular wobble.",
    );
    widgets::momentary_button(
        ui,
        setter,
        &params.motor_kill,
        Rect::from_center_size(pos2(1026.0, ay), vec2(36.0, 22.0)),
        "MTR",
        Some(theme::LED_RED),
        "Hold: kill the motor — pitch drags down to a dead stop. Release to \
         wind back up.",
    );

    widgets::switch3(
        ui,
        setter,
        &params.tape_type,
        pos2(862.0, by),
        ["NM", "CH", "MT"],
        "The deck's tape-type setting (bias/EQ): I Normal (warm, saturates \
         early), II Chrome (cleaner), IV Metal (most headroom). Picking a \
         stock in SETUP sets this to the cassette's native type; moving it \
         away is a mis-set deck — a sound of its own.",
    );
    // Amber tell-tale when the deck setting doesn't match the cassette's
    // native formulation (deliberate mis-set or leftover).
    let native = match stock_profile.default_kind {
        te2_dsp::tape::TapeKind::I => crate::params::TapeType::Normal,
        te2_dsp::tape::TapeKind::II => crate::params::TapeType::Chrome,
        te2_dsp::tape::TapeKind::IV => crate::params::TapeType::Metal,
    };
    let mismatch = params.tape_type.value() != native;
    widgets::led(ui, pos2(824.0, by), mismatch, theme::LED_YELLOW);
    if mismatch {
        ui.allocate_rect(
            Rect::from_center_size(pos2(824.0, by), vec2(12.0, 12.0)),
            Sense::hover(),
        )
        .on_hover_text(format!(
            "Deck set to a different type than the {} in the well — \
             sounds like a mis-set deck. Re-pick the stock in SETUP to match.",
            stock_profile.label
        ));
    }
    widgets::knob(
        ui,
        setter,
        &params.tape_in,
        pos2(944.0, by),
        15.0,
        "TAPE IN",
        "How hot the signal hits the tape. Drives saturation and compression; \
         the VU reads this.",
    );
    widgets::knob(
        ui,
        setter,
        &params.out_level,
        pos2(1026.0, by),
        15.0,
        "OUT",
        "Output trim, -24 to +6 dB.",
    );

    // Quiet footer.
    painter.text(
        pos2(panel.center().x, panel.bottom() - 8.0),
        Align2::CENTER_CENTER,
        "CASSETTE  TAPE  ECHO",
        FontId::monospace(7.5),
        Color32::from_rgb(0x4A, 0x48, 0x44),
    );

    if state.settings_open {
        draw_settings_overlay(ui, setter, params, shared, state, scale);
    }
}


/// The TAPE & MACHINE overlay: a modal card over a dimmed faceplate, laid
/// out on a strict three-column grid (stocks | stocks + machine | aging +
/// scale), every group centered on its column.
fn draw_settings_overlay(
    ui: &mut egui::Ui,
    setter: &nice_plug::prelude::ParamSetter,
    params: &Te2Params,
    shared: &UiShared,
    state: &mut EditorState,
    scale: f32,
) {
    use crate::params::{TapeStockParam, TapeType};
    use te2_dsp::tape::TapeKind;

    let full = Rect::from_min_size(pos2(0.0, 0.0), CANVAS);
    let mut close = ui.input(|i| i.key_pressed(egui::Key::Escape));

    // The overlay layer scales with the panel.
    let overlay_id = egui::Id::new("te2-settings-overlay");
    let ctx = ui.ctx().clone();
    ctx.set_transform_layer(
        egui::LayerId::new(egui::Order::Foreground, overlay_id),
        egui::emath::TSTransform::from_scaling(scale),
    );

    egui::Area::new(overlay_id)
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(0.0, 0.0))
        .show(&ctx, |ui| {
            // Scrim: dims the faceplate, swallows its input, closes on click.
            let scrim = ui.allocate_rect(full, Sense::click());
            ui.painter()
                .rect_filled(full, 0.0, Color32::from_black_alpha(170));
            if scrim.clicked() {
                close = true;
            }

            // The card. Allocated so clicks in its gaps don't reach the scrim.
            let card = Rect::from_min_max(pos2(170.0, 70.0), pos2(910.0, 490.0));
            ui.allocate_rect(card, Sense::click());
            let p = ui.painter();
            p.rect_filled(card, 6.0, Color32::from_rgb(0x17, 0x17, 0x18));
            p.rect_stroke(
                card,
                6.0,
                Stroke::new(1.5, theme::KNOB_EDGE),
                StrokeKind::Inside,
            );
            p.text(
                pos2(194.0, 94.0),
                Align2::LEFT_CENTER,
                "TAPE  &  MACHINE",
                FontId::monospace(15.0),
                theme::INK,
            );

            // Close button.
            let x_rect = Rect::from_center_size(pos2(886.0, 94.0), vec2(20.0, 20.0));
            let x_resp = ui.allocate_rect(x_rect, Sense::click());
            let x_color = if x_resp.hovered() { theme::INK } else { theme::INK_DIM };
            let c = x_rect.center();
            for (a, b) in [
                (c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)),
                (c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)),
            ] {
                ui.painter().line_segment([a, b], Stroke::new(1.6, x_color));
            }
            if x_resp.clicked() {
                close = true;
            }

            // --- Three columns, 212 wide: A 194, B 426, C 658. Midlines
            // 300 / 532 / 764. Section headers share rows: y=124 and y=296.
            let header = |ui: &mut egui::Ui, mid: f32, y: f32, title: &str| {
                widgets::label(ui, pos2(mid, y), title, 9.0, theme::INK_DIM);
            };

            let selected = params.tape_stock.value();
            let stock_row = |ui: &mut egui::Ui, x: f32, y: f32, stock: TapeStockParam| {
                let rect = Rect::from_min_size(pos2(x, y), vec2(212.0, 20.0));
                let resp = ui.allocate_rect(rect, Sense::click());
                let profile = stock.to_dsp().profile();
                let is_sel = selected == stock;
                let p = ui.painter();
                let bg = if is_sel {
                    theme::KNOB_EDGE
                } else if resp.hovered() {
                    Color32::from_rgb(0x24, 0x24, 0x26)
                } else {
                    theme::KNOB_BODY
                };
                p.rect_filled(rect, 3.0, bg);
                widgets::led(ui, pos2(x + 11.0, rect.center().y), is_sel, theme::LED_YELLOW);
                ui.painter().text(
                    pos2(x + 22.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    profile.label,
                    FontId::monospace(8.5),
                    if is_sel { theme::INK } else { theme::INK_DIM },
                );
                let kind_badge = match profile.default_kind {
                    TapeKind::I => "I",
                    TapeKind::II => "II",
                    TapeKind::IV => "IV",
                };
                ui.painter().text(
                    pos2(rect.right() - 10.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    kind_badge,
                    FontId::monospace(8.0),
                    theme::INK_DIM,
                );
                if resp.clicked() && !is_sel {
                    setter.begin_set_parameter(&params.tape_stock);
                    setter.set_parameter(&params.tape_stock, stock);
                    setter.end_set_parameter(&params.tape_stock);
                    // A new cassette comes with its native formulation; the
                    // faceplate NM/CH/MT switch can still override it.
                    let tape_type = match profile.default_kind {
                        TapeKind::I => TapeType::Normal,
                        TapeKind::II => TapeType::Chrome,
                        TapeKind::IV => TapeType::Metal,
                    };
                    setter.begin_set_parameter(&params.tape_type);
                    setter.set_parameter(&params.tape_type, tape_type);
                    setter.end_set_parameter(&params.tape_type);
                }
                resp.on_hover_text(format!(
                    "~{:.0} min of rolling to fully worn · hiss ×{:.2} · drive ×{:.2}",
                    profile.aging_seconds / 60.0,
                    profile.noise_mul,
                    profile.drive_mul,
                ));
            };

            // --- Column A: premium + standard stocks ---
            header(ui, 300.0, 124.0, "PREMIUM  ·  ~1 H TO WORN");
            for (i, s) in [
                TapeStockParam::MaxellXlii,
                TapeStockParam::TdkSa,
                TapeStockParam::TdkMa,
                TapeStockParam::SonyMetalEs,
                TapeStockParam::BasfChromeMaxima,
                TapeStockParam::NakamichiExii,
            ]
            .into_iter()
            .enumerate()
            {
                stock_row(ui, 194.0, 136.0 + i as f32 * 24.0, s);
            }
            header(ui, 300.0, 296.0, "STANDARD  ·  ~40 MIN");
            for (i, s) in [
                TapeStockParam::TdkAd,
                TapeStockParam::MaxellUdii,
                TapeStockParam::SonyUx,
            ]
            .into_iter()
            .enumerate()
            {
                stock_row(ui, 194.0, 308.0 + i as f32 * 24.0, s);
            }

            // --- Column B: budget stocks + machine trims ---
            header(ui, 532.0, 124.0, "BUDGET  ·  ~20 MIN");
            for (i, s) in [
                TapeStockParam::TdkD,
                TapeStockParam::SonyHf,
                TapeStockParam::RealisticSupertape,
                TapeStockParam::Memorex,
                TapeStockParam::Generic,
            ]
            .into_iter()
            .enumerate()
            {
                stock_row(ui, 426.0, 136.0 + i as f32 * 24.0, s);
            }
            header(ui, 532.0, 296.0, "MACHINE");
            widgets::knob(
                ui,
                setter,
                &params.noise,
                pos2(492.0, 346.0),
                17.0,
                "NOISE",
                "Tape hiss level. The noise is recorded onto the tape, so it \
                 regenerates through the feedback loop like real hiss.",
            );
            widgets::knob(
                ui,
                setter,
                &params.mech,
                pos2(572.0, 346.0),
                17.0,
                "MECH",
                "Mechanism condition: wow, flutter, dropouts and bias sag. \
                 0 = freshly serviced, full = thrift-store wreck.",
            );
            widgets::switch3(
                ui,
                setter,
                &params.quality,
                pos2(490.0, 424.0),
                ["ECO", "STD", "ULT"],
                "Oversampling for the tape magnetics: 2x / 4x / 8x.",
            );
            widgets::toggle_button(
                ui,
                setter,
                &params.midi_enable,
                Rect::from_center_size(pos2(578.0, 424.0), vec2(36.0, 16.0)),
                "MIDI",
                Some(theme::LED_GREEN),
                "Let MIDI notes C3-G3 select and gate positions. Off by \
                 default so a keyboard routed to the track doesn't move the \
                 sequencer.",
            );

            // --- Column C: aging + interface scale ---
            header(ui, 764.0, 124.0, "TAPE AGING");
            widgets::toggle_button(
                ui,
                setter,
                &params.aging_on,
                Rect::from_center_size(pos2(736.0, 152.0), vec2(40.0, 18.0)),
                "AGING",
                Some(theme::LED_YELLOW),
                "Tape wears while the transport rolls: more wow, dropouts and \
                 hiss, less top end and headroom. Off = pristine forever.",
            );
            widgets::toggle_button(
                ui,
                setter,
                &params.aging_freeze,
                Rect::from_center_size(pos2(792.0, 152.0), vec2(44.0, 18.0)),
                "FREEZE",
                Some(theme::LED_GREEN),
                "Hold the wear exactly where it is — keep a worn character \
                 without it getting worse.",
            );

            let age = params.tape_age.load(Ordering::Relaxed).clamp(0.0, 1.0);
            let aging_on = params.aging_on.value();
            let bar = Rect::from_min_max(pos2(664.0, 186.0), pos2(864.0, 198.0));
            let bar_resp = ui.allocate_rect(bar, Sense::hover());
            let p = ui.painter();
            p.rect_filled(bar, 3.0, theme::FADER_TRACK);
            if age > 0.0 {
                let fill =
                    Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * age, bar.bottom()));
                let color = if aging_on {
                    Color32::from_rgb(0xB8, 0x86, 0x38)
                } else {
                    theme::KNOB_EDGE
                };
                p.rect_filled(fill, 3.0, color);
            }
            p.rect_stroke(bar, 3.0, Stroke::new(0.8, theme::PANEL_EDGE), StrokeKind::Outside);
            let profile = params.tape_stock.value().to_dsp().profile();
            ui.painter().text(
                pos2(764.0, 214.0),
                Align2::CENTER_CENTER,
                format!(
                    "WEAR {:>3.0}%{}",
                    age * 100.0,
                    if aging_on { "" } else { "  · BYPASSED" }
                ),
                FontId::monospace(8.5),
                theme::INK_DIM,
            );
            bar_resp.on_hover_text(format!(
                "This {} is ~{:.0} min of rolling from fresh to fully worn. \
                 Wear is saved with the project.",
                profile.label,
                profile.aging_seconds / 60.0,
            ));

            let fresh = widgets::action_button(
                ui,
                Rect::from_center_size(pos2(764.0, 246.0), vec2(110.0, 20.0)),
                "NEW CASSETTE",
                false,
                None,
                "Drop in a fresh tape: wipes everything on the loop and \
                 resets wear to zero.",
            );
            if fresh.clicked() {
                shared.eject.store(true, Ordering::Relaxed);
            }

            header(ui, 764.0, 296.0, "INTERFACE SCALE");
            for (i, s) in UI_SCALES.into_iter().enumerate() {
                let chip = Rect::from_center_size(
                    pos2(679.0 + i as f32 * 34.0, 322.0),
                    vec2(30.0, 18.0),
                );
                let resp = ui.allocate_rect(chip, Sense::click());
                let is_sel = (scale - s).abs() < 1e-3;
                let p = ui.painter();
                let bg = if is_sel {
                    theme::KNOB_EDGE
                } else if resp.hovered() {
                    Color32::from_rgb(0x24, 0x24, 0x26)
                } else {
                    theme::KNOB_BODY
                };
                p.rect_filled(chip, 3.0, bg);
                p.rect_stroke(chip, 3.0, Stroke::new(0.8, theme::PANEL_EDGE), StrokeKind::Outside);
                p.text(
                    chip.center(),
                    Align2::CENTER_CENTER,
                    format!("{:.0}", s * 100.0),
                    FontId::monospace(7.5),
                    if is_sel { theme::INK } else { theme::INK_DIM },
                );
                if resp.clicked() {
                    params.ui_scale.store(s, Ordering::Relaxed);
                }
                resp.on_hover_text(format!(
                    "Scale the interface to {:.0}% — the window resizes with it \
                     (host permitting). Saved with the plugin state.",
                    s * 100.0
                ));
            }
            widgets::label(
                ui,
                pos2(764.0, 348.0),
                "%  OF  1080 × 560",
                7.5,
                theme::INK_DIM,
            );

            // Footer hint.
            ui.painter().text(
                pos2(card.center().x, card.bottom() - 14.0),
                Align2::CENTER_CENTER,
                "STOCK SETS THE NM/CH/MT SWITCH TO ITS NATIVE TYPE  ·  STP/EJ DOUBLE-PRESS = FRESH CASSETTE",
                FontId::monospace(7.0),
                Color32::from_rgb(0x4A, 0x48, 0x44),
            );
        });

    if close {
        state.settings_open = false;
    }
}
