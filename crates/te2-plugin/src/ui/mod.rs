//! The RE-DEEMER panel: a vector tribute to the Space Case TE-2 faceplate.
//! Oiled maple frame, black panel, cassette window with a live transport,
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

struct EditorState {
    reel_angle: f32,
}

pub fn create(params: Arc<Te2Params>, shared: Arc<UiShared>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        EditorState { reel_angle: 0.0 },
        EguiSettings::default(),
        |_ctx, _queue, _state| {},
        move |ui, setter, _queue, state| {
            let ctx = ui.ctx().clone();
            // The reels never stop turning.
            ctx.request_repaint();
            let dt = ui.input(|i| i.stable_dt).min(0.1);
            let speed = shared.speed.load(Ordering::Relaxed);
            state.reel_angle += speed * dt * 6.0;

            draw_panel(ui, setter, &params, &shared, state.reel_angle);
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
) {
    draw_panel(ui, setter, params, shared, reel_angle);
}

fn draw_panel(
    ui: &mut egui::Ui,
    setter: &nice_plug::prelude::ParamSetter,
    params: &Te2Params,
    shared: &UiShared,
    reel_angle: f32,
) {
    let speed = shared.speed.load(Ordering::Relaxed);

    let full = ui.max_rect();
    let painter = ui.painter().clone();

    // Wood frame + faceplate.
    painter.rect_filled(full, 0.0, theme::WOOD);
    painter.rect_stroke(
        full.shrink(2.0),
        2.0,
        Stroke::new(3.0, theme::WOOD_EDGE),
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

    // Wordmarks.
    painter.text(
        pos2(1000.0, 36.0),
        Align2::CENTER_CENTER,
        "TE-2",
        FontId::monospace(22.0),
        theme::INK,
    );
    painter.text(
        pos2(208.0, 224.0),
        Align2::CENTER_CENTER,
        "RE-DEEMER",
        FontId::monospace(28.0),
        theme::INK,
    );

    // Cassette window + VU, driven by the audio thread.
    cassette::draw(
        ui,
        Rect::from_min_max(pos2(36.0, 32.0), pos2(380.0, 196.0)),
        reel_angle,
        speed,
    );
    vu::draw(
        ui,
        Rect::from_min_max(pos2(950.0, 50.0), pos2(1052.0, 118.0)),
        shared.vu.load(Ordering::Relaxed),
    );

    let position = shared.position.load(Ordering::Relaxed);

    // --- 1-8 position buttons (click = select, hold = RES gate) ---
    let mut any_held = false;
    for i in 0..8u8 {
        let cx = 56.0 + i as f32 * 42.0;
        let center = pos2(cx, 282.0);
        widgets::led(ui, pos2(cx, 258.0), position == i + 1, theme::LED_RED);
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
            pos2(cx, 282.0 + 22.0),
            Align2::CENTER_CENTER,
            format!("{}", i + 1),
            FontId::monospace(9.0),
            theme::INK,
        );
    }
    shared.ui_gate.store(any_held, Ordering::Relaxed);

    widgets::toggle_button(
        ui,
        setter,
        &params.midi_enable,
        Rect::from_center_size(pos2(398.0, 278.0), vec2(32.0, 16.0)),
        "MIDI",
        Some(theme::LED_GREEN),
        "Let MIDI notes C3-G3 select and gate positions. Off by default so a \
         keyboard routed to the track doesn't move the sequencer.",
    );

    // --- Transport row ---
    let ty = 500.0;
    let bsize = vec2(44.0, 26.0);
    let bx = |i: f32| pos2(62.0 + i * 50.0, ty);
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

    // --- Top knob row ---
    let ky = 66.0;
    let kx = |i: f32| pos2(446.0 + i * 64.0, ky);
    widgets::knob(
        ui,
        setter,
        &params.rate_div,
        kx(0.0),
        17.0,
        "DIV",
        "Cycle rate division when SYNC is on (1/1 down to 1/32).",
    );
    widgets::knob(
        ui,
        setter,
        &params.loop_len,
        kx(1.0),
        17.0,
        "LOOP",
        "Loop length for LOOP mode, in seconds of tape footage. \
         SYNC snaps it to whole beats.",
    );
    widgets::knob(
        ui,
        setter,
        &params.noise,
        kx(2.0),
        17.0,
        "NOISE",
        "Tape hiss level. The noise is recorded onto the tape, so it \
         regenerates through the feedback loop like real hiss.",
    );
    widgets::knob(
        ui,
        setter,
        &params.mech,
        kx(3.0),
        17.0,
        "MECH",
        "Mechanism condition: wow, flutter, dropouts and bias sag. \
         0 = freshly serviced, full = thrift-store wreck.",
    );
    widgets::knob(
        ui,
        setter,
        &params.white_drift,
        kx(4.0),
        17.0,
        "DRIFT",
        "Glide time between positions for the White set (0-14 s).",
    );
    widgets::knob(
        ui,
        setter,
        &params.gray_drift,
        kx(5.0),
        17.0,
        "DRIFT",
        "Glide time between positions for the Gray set (0-14 s).",
    );
    widgets::knob(
        ui,
        setter,
        &params.black_drift,
        kx(6.0),
        17.0,
        "DRIFT",
        "Glide time between positions for the Black set (0-14 s).",
    );
    widgets::knob(
        ui,
        setter,
        &params.cycle_rate,
        kx(7.0),
        17.0,
        "CYCLE",
        "Cycle speed: from 8 seconds per step up to 4,000 steps per second — \
         audio-rate stepping turns the faders into a waveform.",
    );

    let small = vec2(34.0, 16.0);
    widgets::toggle_button(
        ui,
        setter,
        &params.rate_sync,
        Rect::from_center_size(pos2(446.0, 112.0), small),
        "SYNC",
        Some(theme::LED_RED),
        "Sync the cycle rate to the host tempo using DIV.",
    );
    widgets::toggle_button(
        ui,
        setter,
        &params.loop_sync,
        Rect::from_center_size(pos2(510.0, 112.0), small),
        "SYNC",
        Some(theme::LED_RED),
        "Snap the loop length to whole beats of the host tempo.",
    );
    widgets::toggle_button(
        ui,
        setter,
        &params.cycle_run,
        Rect::from_center_size(pos2(894.0, 112.0), small),
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
    widgets::rotary_selector(
        ui,
        setter,
        &params.cycle_len,
        pos2(912.0, 170.0),
        18.0,
        8,
        "1-8",
        "How many positions the cycle rotates through. The Anomaly fires \
         on the final step.",
    );

    // --- Fader matrix: positions 2-8 x White/Gray/Black ---
    let whites = params.white_faders();
    let grays = params.gray_faders();
    let blacks = params.black_faders();
    for col in 0..7usize {
        let cx = 462.0 + col as f32 * 66.0;
        widgets::label(ui, pos2(cx, 200.0), &format!("{}", col + 2), 9.5, theme::INK);
        widgets::fader(
            ui,
            setter,
            whites[col],
            pos2(cx - 18.0, 256.0),
            72.0,
            theme::CAP_WHITE,
            "White set value for this position.",
        );
        widgets::fader(
            ui,
            setter,
            grays[col],
            pos2(cx, 256.0),
            72.0,
            theme::CAP_GRAY,
            "Gray set value for this position.",
        );
        widgets::fader(
            ui,
            setter,
            blacks[col],
            pos2(cx + 18.0, 256.0),
            72.0,
            theme::CAP_BLACK,
            "Black set value for this position.",
        );
        widgets::led(ui, pos2(cx, 302.0), position == (col + 2) as u8, theme::LED_RED);
    }
    // Position 1 = the panel itself.
    widgets::led(ui, pos2(430.0, 302.0), position == 1, theme::LED_RED);
    widgets::label(ui, pos2(430.0, 200.0), "1", 9.5, theme::INK_DIM);

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
        Rect::from_center_size(pos2(756.0, 360.0), vec2(28.0, 16.0)),
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

    // --- Right block ---
    widgets::switch3(
        ui,
        setter,
        &params.anomaly_pol,
        pos2(846.0, 354.0),
        ["-", "OFF", "+"],
        "Anomaly polarity: the tape hiccup bends pitch down (-) or up (+). \
         Middle = off.",
    );
    widgets::knob(
        ui,
        setter,
        &params.anomaly,
        pos2(922.0, 366.0),
        15.0,
        "ANMLY",
        "Anomaly amount: a single speed blip on the cycle's final step, from \
         a quick flick to a long singular wobble.",
    );
    widgets::momentary_button(
        ui,
        setter,
        &params.motor_kill,
        Rect::from_center_size(pos2(992.0, 360.0), vec2(36.0, 22.0)),
        "MTR",
        Some(theme::LED_RED),
        "Hold: kill the motor — pitch drags down to a dead stop. Release to \
         wind back up.",
    );

    widgets::switch3(
        ui,
        setter,
        &params.tape_type,
        pos2(846.0, 418.0),
        ["NM", "CH", "MT"],
        "Tape formulation: I Normal (warm, saturates early), II Chrome \
         (cleaner), IV Metal (most headroom).",
    );
    widgets::knob(
        ui,
        setter,
        &params.tape_in,
        pos2(922.0, 430.0),
        15.0,
        "TAPE IN",
        "How hot the signal hits the tape. Drives saturation and compression; \
         the VU reads this.",
    );
    widgets::switch3(
        ui,
        setter,
        &params.quality,
        pos2(1008.0, 418.0),
        ["ECO", "STD", "ULT"],
        "Oversampling for the tape magnetics: 2x / 4x / 8x.",
    );

    widgets::switch3(
        ui,
        setter,
        &params.input_char,
        pos2(846.0, 484.0),
        ["GT", "-10", "+4"],
        "Input level standard: guitar hi-Z, -10 dBV consumer, +4 dBu pro.",
    );
    widgets::knob(
        ui,
        setter,
        &params.out_level,
        pos2(922.0, 494.0),
        15.0,
        "OUT",
        "Output trim, -24 to +6 dB.",
    );

    // Quiet footer.
    painter.text(
        pos2(panel.center().x, panel.bottom() - 8.0),
        Align2::CENTER_CENTER,
        "A  SPACE CASE TE-2  TRIBUTE   ·   CASSETTE TAPE ECHO",
        FontId::monospace(7.5),
        Color32::from_rgb(0x4A, 0x48, 0x44),
    );
}
