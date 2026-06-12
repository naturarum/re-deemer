//! C ABI over the te2-dsp engine, for non-Rust hosts (the VCV Rack module
//! links this as a static library). The header lives at `include/te2.h` and
//! is maintained BY HAND — if you change anything exported here, change the
//! header to match, and bump the `TE2_ABI_VERSION` in both places.
//!
//! Conventions:
//! - One opaque handle per engine instance; create/destroy from a non-audio
//!   thread (creation runs the magnetics calibration sims).
//! - `te2_process` is realtime-safe; everything else is control-rate.
//! - Enums cross the boundary as `int32_t` with the mappings documented in
//!   the header; out-of-range values clamp to something sane.

use te2_dsp::oversample::OsFactor;
use te2_dsp::sequencer::{AnomalyPolarity, BlackTarget, GrayTarget, SeqConfig, WhiteTarget};
use te2_dsp::tape::{TapeKind, TapeStock};
use te2_dsp::{EngineParams, Te2Engine};
use te2_dsp::engine::{TransportKind, Wind};

/// Bumped whenever the exported surface or `Te2CParams` layout changes.
pub const TE2_ABI_VERSION: u32 = 2;

pub struct Te2Handle {
    engine: Te2Engine,
}

/// Plain-C mirror of `EngineParams` (+ the sequencer config flattened).
/// Field-for-field documentation lives in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Te2CParams {
    pub delay_time: f32,
    pub feedback: f32,
    pub tape_in: f32,
    pub tape_level: f32,
    pub dry_level: f32,
    pub mod_amount: f32,
    pub mod_speed_hz: f32,
    pub motor_kill: bool,
    pub hpf_hz: f32,
    pub lpf_hz: f32,
    pub res: f32,
    pub out_drive: f32,
    /// 0 = I (normal), 1 = II (chrome), 2 = IV (metal).
    pub tape_kind: i32,
    /// Index into the stock list, header order (0 = Maxell XL-II).
    pub stock: i32,
    pub aging_on: bool,
    pub aging_freeze: bool,
    pub condition: f32,
    pub noise_amount: f32,
    /// 0 = 2x, 1 = 4x, 2 = 8x oversampling.
    pub os_factor: i32,

    // Sequencer.
    pub white_faders: [f32; 7],
    pub gray_faders: [f32; 7],
    pub black_faders: [f32; 7],
    pub white_on: bool,
    pub gray_on: bool,
    pub black_on: bool,
    /// White: 0 = time, 1 = resonance, 2 = mod speed.
    pub white_target: i32,
    /// Gray: 0 = feedback, 1 = mod amount, 2 = LPF.
    pub gray_target: i32,
    /// Black: 0 = tape level, 1 = dry level, 2 = HPF.
    pub black_target: i32,
    pub white_drift: f32,
    pub gray_drift: f32,
    pub black_drift: f32,
    pub cycle_run: bool,
    /// 1..=8.
    pub cycle_len: i32,
    /// Steps per second.
    pub cycle_rate: f32,
    /// When true, `host_step_pos` phase-locks the cycle (absolute position
    /// in steps). Rack uses the CLOCK input instead and leaves this false.
    pub host_step_valid: bool,
    pub host_step_pos: f64,
    /// 1..=8.
    pub manual_position: i32,
    pub anomaly_amount: f32,
    /// -1 = minus, 0 = off, 1 = plus.
    pub anomaly_polarity: i32,
    pub res_gate_enabled: bool,
    pub gate_held: bool,
    /// 0 = echo, 1 = play, 2 = loop.
    pub transport: i32,
    pub pause: bool,
    pub stop: bool,
    /// 0 = off, 1 = rewind, 2 = fast-forward.
    pub wind: i32,
    pub loop_len_s: f32,
    /// Slip-clutch drag 0..1 (irregular speed sag; 0 = off). ABI v2.
    pub slip: f32,
    /// External cycle clocking: the internal step clock stands down and
    /// steps arrive via te2_clock_step(). ABI v2.
    pub external_clock: bool,
}

fn stock_from_i32(v: i32) -> TapeStock {
    *TapeStock::ALL
        .get(v.clamp(0, TapeStock::ALL.len() as i32 - 1) as usize)
        .unwrap_or(&TapeStock::MaxellXlii)
}

fn to_engine_params(c: &Te2CParams) -> EngineParams {
    let seq = SeqConfig {
        white_faders: c.white_faders,
        gray_faders: c.gray_faders,
        black_faders: c.black_faders,
        white_on: c.white_on,
        gray_on: c.gray_on,
        black_on: c.black_on,
        white_target: match c.white_target {
            1 => WhiteTarget::Resonance,
            2 => WhiteTarget::ModSpeed,
            _ => WhiteTarget::Time,
        },
        gray_target: match c.gray_target {
            1 => GrayTarget::ModAmount,
            2 => GrayTarget::Lpf,
            _ => GrayTarget::Feedback,
        },
        black_target: match c.black_target {
            1 => BlackTarget::DryLevel,
            2 => BlackTarget::Hpf,
            _ => BlackTarget::TapeLevel,
        },
        white_drift: c.white_drift,
        gray_drift: c.gray_drift,
        black_drift: c.black_drift,
        cycle_run: c.cycle_run,
        cycle_len: c.cycle_len.clamp(1, 8) as u8,
        cycle_rate: c.cycle_rate,
        host_step_pos: c.host_step_valid.then_some(c.host_step_pos),
        external_clock: c.external_clock,
        manual_position: c.manual_position.clamp(1, 8) as u8,
        anomaly_amount: c.anomaly_amount,
        anomaly_polarity: match c.anomaly_polarity {
            v if v < 0 => AnomalyPolarity::Minus,
            v if v > 0 => AnomalyPolarity::Plus,
            _ => AnomalyPolarity::Off,
        },
        ..Default::default()
    };

    EngineParams {
        delay_time: c.delay_time,
        feedback: c.feedback,
        tape_in: c.tape_in,
        tape_level: c.tape_level,
        dry_level: c.dry_level,
        mod_amount: c.mod_amount,
        mod_speed_hz: c.mod_speed_hz,
        motor_kill: c.motor_kill,
        slip: c.slip,
        hpf_hz: c.hpf_hz,
        lpf_hz: c.lpf_hz,
        res: c.res,
        res_active: true,
        out_drive: c.out_drive,
        tape_kind: match c.tape_kind {
            1 => TapeKind::II,
            2 => TapeKind::IV,
            _ => TapeKind::I,
        },
        stock: stock_from_i32(c.stock),
        aging_on: c.aging_on,
        aging_freeze: c.aging_freeze,
        condition: c.condition,
        noise_amount: c.noise_amount,
        os_factor: match c.os_factor {
            0 => OsFactor::X2,
            2 => OsFactor::X8,
            _ => OsFactor::X4,
        },
        seq,
        res_gate_enabled: c.res_gate_enabled,
        gate_held: c.gate_held,
        transport: match c.transport {
            1 => TransportKind::Play,
            2 => TransportKind::Loop,
            _ => TransportKind::Echo,
        },
        pause: c.pause,
        stop: c.stop,
        wind: match c.wind {
            1 => Wind::Rewind,
            2 => Wind::FastForward,
            _ => Wind::Off,
        },
        loop_len_s: c.loop_len_s,
    }
}

fn from_engine_params(p: &EngineParams) -> Te2CParams {
    Te2CParams {
        delay_time: p.delay_time,
        feedback: p.feedback,
        tape_in: p.tape_in,
        tape_level: p.tape_level,
        dry_level: p.dry_level,
        mod_amount: p.mod_amount,
        mod_speed_hz: p.mod_speed_hz,
        motor_kill: p.motor_kill,
        hpf_hz: p.hpf_hz,
        lpf_hz: p.lpf_hz,
        res: p.res,
        out_drive: p.out_drive,
        tape_kind: match p.tape_kind {
            TapeKind::I => 0,
            TapeKind::II => 1,
            TapeKind::IV => 2,
        },
        stock: TapeStock::ALL
            .iter()
            .position(|s| *s == p.stock)
            .unwrap_or(0) as i32,
        aging_on: p.aging_on,
        aging_freeze: p.aging_freeze,
        condition: p.condition,
        noise_amount: p.noise_amount,
        os_factor: match p.os_factor {
            OsFactor::X2 => 0,
            OsFactor::X4 => 1,
            OsFactor::X8 => 2,
        },
        white_faders: p.seq.white_faders,
        gray_faders: p.seq.gray_faders,
        black_faders: p.seq.black_faders,
        white_on: p.seq.white_on,
        gray_on: p.seq.gray_on,
        black_on: p.seq.black_on,
        white_target: 0,
        gray_target: 0,
        black_target: 0,
        white_drift: p.seq.white_drift,
        gray_drift: p.seq.gray_drift,
        black_drift: p.seq.black_drift,
        cycle_run: p.seq.cycle_run,
        cycle_len: p.seq.cycle_len as i32,
        cycle_rate: p.seq.cycle_rate,
        host_step_valid: false,
        host_step_pos: 0.0,
        manual_position: p.seq.manual_position as i32,
        anomaly_amount: p.seq.anomaly_amount,
        anomaly_polarity: 0,
        res_gate_enabled: p.res_gate_enabled,
        gate_held: p.gate_held,
        transport: 0,
        pause: p.pause,
        stop: p.stop,
        wind: 0,
        loop_len_s: p.loop_len_s,
        slip: p.slip,
        external_clock: p.seq.external_clock,
    }
}

/// ABI version of this library; check against TE2_ABI_VERSION in the header.
#[no_mangle]
pub extern "C" fn te2_abi_version() -> u32 {
    TE2_ABI_VERSION
}

/// Size of `Te2CParams` as Rust sees it — compare against sizeof(te2_params)
/// on the C side to catch layout drift between the hand-maintained header
/// and this file.
#[no_mangle]
pub extern "C" fn te2_params_size() -> usize {
    std::mem::size_of::<Te2CParams>()
}

/// Create an engine. NOT realtime-safe (runs calibration sims); call from a
/// setup thread. Returns null on a non-finite/absurd sample rate.
#[no_mangle]
pub extern "C" fn te2_create(sample_rate: f64) -> *mut Te2Handle {
    if !(8_000.0..=768_000.0).contains(&sample_rate) {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(Te2Handle {
        engine: Te2Engine::new(sample_rate),
    }))
}

/// # Safety
/// `h` must be a handle from `te2_create` that hasn't been destroyed.
#[no_mangle]
pub unsafe extern "C" fn te2_destroy(h: *mut Te2Handle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

/// Fill `out` with the engine defaults.
///
/// # Safety
/// `out` must point to a valid `Te2CParams`.
#[no_mangle]
pub unsafe extern "C" fn te2_default_params(out: *mut Te2CParams) {
    if !out.is_null() {
        *out = from_engine_params(&EngineParams::default());
    }
}

/// Apply parameters (control rate; realtime-safe).
///
/// # Safety
/// `h` and `p` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn te2_set_params(h: *mut Te2Handle, p: *const Te2CParams) {
    if let (Some(h), Some(p)) = (h.as_mut(), p.as_ref()) {
        h.engine.set_params(&to_engine_params(p));
    }
}

/// Process one stereo frame (realtime-safe).
///
/// # Safety
/// All pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_process(
    h: *mut Te2Handle,
    in_l: f32,
    in_r: f32,
    out_l: *mut f32,
    out_r: *mut f32,
) {
    if let Some(h) = h.as_mut() {
        let (l, r) = h.engine.process(in_l, in_r);
        if !out_l.is_null() {
            *out_l = l;
        }
        if !out_r.is_null() {
            *out_r = r;
        }
    }
}

/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_eject(h: *mut Te2Handle) {
    if let Some(h) = h.as_mut() {
        h.engine.eject();
    }
}

/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_reset(h: *mut Te2Handle) {
    if let Some(h) = h.as_mut() {
        h.engine.reset();
    }
}

/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_vu(h: *const Te2Handle) -> f32 {
    h.as_ref().map_or(0.0, |h| h.engine.vu_level())
}

/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_motor_speed(h: *const Te2Handle) -> f64 {
    h.as_ref().map_or(0.0, |h| h.engine.motor_speed())
}

/// Current sequencer position, 1..=8.
///
/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_position(h: *const Te2Handle) -> i32 {
    h.as_ref().map_or(1, |h| h.engine.current_position() as i32)
}

/// Tape wear, 0..1.
///
/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_age(h: *const Te2Handle) -> f32 {
    h.as_ref().map_or(0.0, |h| h.engine.age())
}

/// Restore tape wear (patch load).
///
/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_set_age(h: *mut Te2Handle, age: f32) {
    if let Some(h) = h.as_mut() {
        h.engine.set_age(age);
    }
}

/// Advance the cycle one step (external clock; realtime-safe).
///
/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_clock_step(h: *mut Te2Handle) {
    if let Some(h) = h.as_mut() {
        h.engine.clock_step();
    }
}

/// True once per cycle final-step entry (EOC trigger source).
///
/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_take_eoc(h: *mut Te2Handle) -> bool {
    h.as_mut().is_some_and(|h| h.engine.take_eoc())
}

/// The three sets' drift-slewed values, normalized 0..1 (Set CV outputs).
///
/// # Safety
/// All pointers must be valid (outputs may be null to skip).
#[no_mangle]
pub unsafe extern "C" fn te2_set_values(
    h: *const Te2Handle,
    white: *mut f32,
    gray: *mut f32,
    black: *mut f32,
) {
    let (w, g, b) = h.as_ref().map_or((0.0, 0.0, 0.0), |h| h.engine.set_values());
    if !white.is_null() {
        *white = w;
    }
    if !gray.is_null() {
        *gray = g;
    }
    if !black.is_null() {
        *black = b;
    }
}

/// Label of a tape stock (static string), or "?" out of range.
#[no_mangle]
pub extern "C" fn te2_stock_label(index: i32) -> *const std::os::raw::c_char {
    // Static, NUL-terminated copies of the stock labels, header order.
    const LABELS: [&[u8]; 14] = [
        b"MAXELL XL-II ",
        b"TDK SA ",
        b"TDK MA ",
        b"SONY METAL-ES ",
        b"BASF CHROME MAXIMA ",
        b"NAKAMICHI EX-II ",
        b"TDK AD ",
        b"MAXELL UD-II ",
        b"SONY UX ",
        b"TDK D ",
        b"SONY HF ",
        b"REALISTIC SUPERTAPE ",
        b"MEMOREX ",
        b"NO-NAME FERRIC ",
    ];
    const FALLBACK: &[u8] = b"? ";
    let label = LABELS
        .get(index.clamp(0, 13) as usize)
        .copied()
        .unwrap_or(FALLBACK);
    label.as_ptr() as *const std::os::raw::c_char
}

/// Number of tape stocks.
#[no_mangle]
pub extern "C" fn te2_stock_count() -> i32 {
    14
}

/// Signed seconds of tape past the heads (spool animation).
///
/// # Safety
/// `h` must be valid.
#[no_mangle]
pub unsafe extern "C" fn te2_footage_seconds(h: *const Te2Handle) -> f64 {
    h.as_ref().map_or(0.0, |h| h.engine.tape_footage_seconds())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_roundtrip_passes_audio() {
        let h = te2_create(48_000.0);
        assert!(!h.is_null());
        unsafe {
            let mut p = std::mem::zeroed::<Te2CParams>();
            te2_default_params(&mut p);
            p.dry_level = 0.0;
            p.tape_level = 1.0;
            p.delay_time = 0.25;
            p.condition = 0.0;
            p.noise_amount = 0.0;
            te2_set_params(h, &p);

            let (mut l, mut r) = (0.0f32, 0.0f32);
            for _ in 0..24_000 {
                te2_process(h, 0.0, 0.0, &mut l, &mut r);
            }
            // A click must come back as an echo ~0.25 s later.
            let mut heard = false;
            for k in 0..24_000 {
                let x = if k == 0 { 1.0 } else { 0.0 };
                te2_process(h, x, x, &mut l, &mut r);
                if k > 10_000 && l.abs() > 0.02 {
                    heard = true;
                }
                assert!(l.is_finite() && r.is_finite());
            }
            assert!(heard, "echo never arrived through the C ABI");
            te2_destroy(h);
        }
    }

    #[test]
    fn null_handles_are_harmless() {
        unsafe {
            te2_destroy(std::ptr::null_mut());
            te2_set_params(std::ptr::null_mut(), std::ptr::null());
            let (mut l, mut r) = (0.0, 0.0);
            te2_process(std::ptr::null_mut(), 0.0, 0.0, &mut l, &mut r);
            assert_eq!(te2_vu(std::ptr::null()), 0.0);
        }
        assert!(te2_create(f64::NAN).is_null());
    }
}
