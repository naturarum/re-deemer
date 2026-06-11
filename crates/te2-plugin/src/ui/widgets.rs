//! Custom panel widgets: knobs, ARP-style faders, slide switches, tactile
//! buttons, LEDs, rotary selectors. All painter-drawn vectors bound to
//! nice-plug parameters through the `ParamSetter`.

use super::theme;
use nice_plug::prelude::{Param, ParamSetter};
use egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2,
    vec2,
};

pub fn label(ui: &Ui, pos: Pos2, text: &str, size: f32, color: Color32) {
    ui.painter().text(
        pos,
        Align2::CENTER_CENTER,
        text,
        FontId::monospace(size),
        color,
    );
}

/// Vertical-drag parameter interaction shared by knobs and faders.
/// Returns the new normalized value while dragging.
fn drag_normalized<P: Param>(
    ui: &Ui,
    response: &Response,
    setter: &ParamSetter,
    param: &P,
    pixels_full_range: f32,
) {
    if response.drag_started() {
        setter.begin_set_parameter(param);
    }
    if response.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        let scale = if fine {
            pixels_full_range * 8.0
        } else {
            pixels_full_range
        };
        let delta = -response.drag_delta().y / scale;
        if delta != 0.0 {
            let next = (param.unmodulated_normalized_value() + delta).clamp(0.0, 1.0);
            setter.set_parameter_normalized(param, next);
        }
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, param.default_normalized_value());
        setter.end_set_parameter(param);
    }
}

/// Hover tooltip: control explanation plus the current value.
fn help_tip<P: Param>(response: Response, param: &P, help: &str) {
    if !help.is_empty() {
        let value = param
            .normalized_value_to_string(param.unmodulated_normalized_value(), true)
            .to_string();
        response.on_hover_ui(|ui| {
            ui.label(
                egui::RichText::new(format!("{}  ·  {}", param.name(), value))
                    .monospace()
                    .strong(),
            );
            ui.label(egui::RichText::new(help).size(11.0));
        });
    }
}

/// Show the parameter's formatted value while dragging.
fn value_tip<P: Param>(ui: &Ui, response: &Response, param: &P, center: Pos2, offset_y: f32) {
    if response.dragged() {
        let text = param.normalized_value_to_string(param.unmodulated_normalized_value(), true).to_string();
        let pos = pos2(center.x, center.y + offset_y);
        let painter = ui.painter();
        let galley_rect = Rect::from_center_size(pos, vec2(text.len() as f32 * 7.0 + 10.0, 16.0));
        painter.rect_filled(galley_rect, 3.0, Color32::from_black_alpha(220));
        painter.text(
            pos,
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(11.0),
            theme::INK,
        );
    }
}

/// Rotary knob. `radius` is the body radius; label drawn underneath.
pub fn knob<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    center: Pos2,
    radius: f32,
    text: &str,
    help: &str,
) {
    let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.2));
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    drag_normalized(ui, &response, setter, param, 220.0);

    let painter = ui.painter();
    painter.circle_filled(center, radius, theme::KNOB_BODY);
    painter.circle_stroke(center, radius, Stroke::new(1.5, theme::KNOB_EDGE));

    // Pointer: 7 o'clock to 5 o'clock sweep.
    let norm = param.unmodulated_normalized_value();
    let angle = (-225.0 + norm * 270.0).to_radians();
    let dir = vec2(angle.cos(), angle.sin());
    painter.line_segment(
        [center + dir * (radius * 0.25), center + dir * (radius * 0.92)],
        Stroke::new(2.0, theme::KNOB_MARK),
    );
    // End-of-travel ticks.
    for t in [-225.0f32, 45.0] {
        let a = t.to_radians();
        let d = vec2(a.cos(), a.sin());
        painter.line_segment(
            [center + d * (radius + 2.0), center + d * (radius + 5.0)],
            Stroke::new(1.2, theme::INK_DIM),
        );
    }

    label(
        ui,
        pos2(center.x, center.y + radius + 11.0),
        text,
        9.5,
        theme::INK,
    );
    value_tip(ui, &response, param, center, -(radius + 14.0));
    help_tip(response, param, help);
}

/// ARP-style mini fader.
pub fn fader<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    center: Pos2,
    height: f32,
    cap_color: Color32,
    help: &str,
) {
    let rect = Rect::from_center_size(center, vec2(18.0, height + 12.0));
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    drag_normalized(ui, &response, setter, param, height);

    let painter = ui.painter();
    let track = Rect::from_center_size(center, vec2(4.0, height));
    painter.rect_filled(track, 2.0, theme::FADER_TRACK);
    painter.rect_stroke(
        track.expand(0.5),
        2.0,
        Stroke::new(0.8, theme::PANEL_EDGE),
        StrokeKind::Outside,
    );

    let norm = param.unmodulated_normalized_value();
    let cap_y = center.y + height * 0.5 - norm * height;
    let cap = Rect::from_center_size(pos2(center.x, cap_y), vec2(15.0, 9.0));
    painter.rect_filled(cap, 2.0, cap_color);
    painter.rect_stroke(
        cap,
        2.0,
        Stroke::new(0.8, Color32::from_black_alpha(160)),
        StrokeKind::Inside,
    );
    value_tip(ui, &response, param, center, -(height * 0.5 + 16.0));
    help_tip(response, param, help);
}

/// Three-position slide switch for 3-variant enum params.
pub fn switch3<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    center: Pos2,
    labels: [&str; 3],
    help: &str,
) {
    let w = 54.0;
    let rect = Rect::from_center_size(center, vec2(w, 14.0));
    let response = ui.allocate_rect(
        Rect::from_center_size(center, vec2(w + 6.0, 26.0)),
        Sense::click(),
    );

    let norm = param.unmodulated_normalized_value();
    let idx = (norm * 2.0).round() as usize;

    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let rel = ((pos.x - rect.left()) / w * 3.0).floor().clamp(0.0, 2.0);
            setter.begin_set_parameter(param);
            setter.set_parameter_normalized(param, rel / 2.0);
            setter.end_set_parameter(param);
        }
    }

    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, theme::FADER_TRACK);
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(0.8, theme::PANEL_EDGE),
        StrokeKind::Outside,
    );
    let slot_w = w / 3.0;
    let thumb_x = rect.left() + slot_w * (idx as f32 + 0.5);
    let thumb = Rect::from_center_size(pos2(thumb_x, center.y), vec2(slot_w - 4.0, 10.0));
    painter.rect_filled(thumb, 2.0, theme::CAP_WHITE);

    for (i, l) in labels.iter().enumerate() {
        let x = rect.left() + slot_w * (i as f32 + 0.5);
        let color = if i == idx { theme::INK } else { theme::INK_DIM };
        label(ui, pos2(x, center.y - 15.0), l, 8.0, color);
    }
    help_tip(response, param, help);
}

/// Small round LED.
pub fn led(ui: &Ui, center: Pos2, on: bool, color_on: Color32) {
    let painter = ui.painter();
    let color = if on { color_on } else { theme::LED_RED_OFF };
    painter.circle_filled(center, 3.2, color);
    if on {
        painter.circle_filled(center, 5.5, color.gamma_multiply(0.25));
    }
}

/// Tactile latching button bound to a bool param. Returns the response.
pub fn toggle_button<P: Param<Plain = bool>>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    rect: Rect,
    text: &str,
    accent: Option<Color32>,
    help: &str,
) -> Response {
    let response = ui.allocate_rect(rect, Sense::click());
    let on = param.unmodulated_normalized_value() > 0.5;
    if response.clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, !on);
        setter.end_set_parameter(param);
    }
    draw_button(ui, rect, text, on, accent);
    if !help.is_empty() {
        response.clone().on_hover_text(help);
    }
    response
}

/// Momentary button: true while held.
pub fn momentary_button<P: Param<Plain = bool>>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    rect: Rect,
    text: &str,
    accent: Option<Color32>,
    help: &str,
) -> Response {
    let response = ui.allocate_rect(rect, Sense::drag());
    if response.drag_started() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, true);
    }
    if response.drag_stopped() {
        setter.set_parameter(param, false);
        setter.end_set_parameter(param);
    }
    let on = param.unmodulated_normalized_value() > 0.5;
    draw_button(ui, rect, text, on, accent);
    if !help.is_empty() {
        response.clone().on_hover_text(help);
    }
    response
}

/// Plain action button (no param); returns clicked.
pub fn action_button(
    ui: &mut Ui,
    rect: Rect,
    text: &str,
    lit: bool,
    accent: Option<Color32>,
    help: &str,
) -> Response {
    let response = ui.allocate_rect(rect, Sense::click());
    draw_button(ui, rect, text, lit, accent);
    if !help.is_empty() {
        response.clone().on_hover_text(help);
    }
    response
}

fn draw_button(ui: &Ui, rect: Rect, text: &str, on: bool, accent: Option<Color32>) {
    let painter = ui.painter();
    let body = if on {
        theme::KNOB_EDGE
    } else {
        theme::KNOB_BODY
    };
    painter.rect_filled(rect, 3.0, body);
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, theme::PANEL_EDGE),
        StrokeKind::Outside,
    );
    if let Some(color) = accent {
        let dot = pos2(rect.center().x, rect.top() + 5.0);
        ui.painter()
            .circle_filled(dot, 2.6, if on { color } else { color.gamma_multiply(0.35) });
    }
    painter.text(
        pos2(rect.center().x, rect.bottom() + 8.0),
        Align2::CENTER_CENTER,
        text,
        FontId::monospace(8.0),
        theme::INK,
    );
}

/// N-position rotary selector for int/enum params (e.g. cycle length 1-8).
pub fn rotary_selector<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    center: Pos2,
    radius: f32,
    positions: usize,
    text: &str,
    help: &str,
) {
    let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.4));
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    drag_normalized(ui, &response, setter, param, 160.0);

    let painter = ui.painter();
    painter.circle_filled(center, radius, theme::KNOB_BODY);
    painter.circle_stroke(center, radius, Stroke::new(1.5, theme::KNOB_EDGE));

    let norm = param.unmodulated_normalized_value();
    let idx = (norm * (positions - 1) as f32).round();
    let angle = (-225.0 + idx / (positions - 1) as f32 * 270.0).to_radians();
    let dir = vec2(angle.cos(), angle.sin());
    painter.line_segment(
        [center + dir * (radius * 0.2), center + dir * (radius * 0.9)],
        Stroke::new(2.4, theme::KNOB_MARK),
    );
    for i in 0..positions {
        let a = (-225.0 + i as f32 / (positions - 1) as f32 * 270.0).to_radians();
        let d = vec2(a.cos(), a.sin());
        painter.text(
            center + d * (radius + 8.0),
            Align2::CENTER_CENTER,
            format!("{}", i + 1),
            FontId::monospace(7.5),
            theme::INK_DIM,
        );
    }
    label(
        ui,
        pos2(center.x, center.y + radius + 15.0),
        text,
        9.0,
        theme::INK,
    );
    help_tip(response, param, help);
}
