//! Top-level engine: owns the full TE-2 signal topology.
//!
//! ```text
//! in -> [dry tap] -> TAPE IN -> (+ feedback) -> pre-emphasis -> + tape noise
//!     -> [oversampled hysteresis] -> self-erasure LP -> record bandwidth AA
//!     -> deposit on tape  ...  read head -> repro EQ (de-emph, head bump,
//!        gap loss, spacing) -> * dropout -> [fb tap] -> mix with dry -> out
//! ```
//!
//! The OTA filters (HPF/LPF/RES) and output drive arrive in phase 4 and sit
//! between the fb tap and the mix, per the hardware block diagram.

use crate::drive::OutputDrive;
use crate::filters::{Biquad, BiquadCoeffs, OnePole, OtaHighpass, OtaLowpass};
use crate::oversample::{OsFactor, Oversampler};
use crate::sequencer::{self, SeqConfig, Sequencer};
use crate::tape::heads_eq::{EqProfile, RecordEq, ReproEq};
use crate::tape::magnetics::{Hysteresis, MagParams};
use crate::tape::noise::{NoiseProfile, TapeNoise};
use crate::tape::reel::TapeReel;
use crate::tape::transport::{Mechanism, Motor};
use crate::tape::wow_flutter::WowFlutter;
use crate::tape::{HEAD_GAP, TAPE_RATE, TapeKind, speed_for_delay};

/// Maximum virtual tape loop, in seconds of tape at nominal speed.
pub const MAX_LOOP_SECONDS: f64 = 32.0;

/// What the cassette deck is set up to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportKind {
    /// REC/ECHO: record + echo, erase head active. The normal mode.
    #[default]
    Echo,
    /// PLAY: playback manipulation only, nothing recorded.
    Play,
    /// LOOP/ERASE BYPASS: record with the erase head lifted —
    /// sound-on-sound layering on a finite loop.
    Loop,
}

/// Held wind buttons (only act in Play mode, like holding RW/FF during
/// playback on the hardware).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wind {
    #[default]
    Off,
    Rewind,
    FastForward,
}

/// How much of the old recording survives an erase-bypassed record pass.
const LOOP_ERASE_KEEP: f32 = 0.72;

/// Control-rate parameters, set once per block by the plugin shell.
#[derive(Debug, Clone, Copy)]
pub struct EngineParams {
    /// Record-to-repro delay in seconds (maps to motor speed).
    pub delay_time: f32,
    /// Echo regeneration, 0..1.1 (>1 runs away into tape limiting).
    pub feedback: f32,
    /// Gain into tape — drives the magnetics. 1.0 = unity.
    pub tape_in: f32,
    /// Output level of tape sounds.
    pub tape_level: f32,
    /// Output level of the dry path.
    pub dry_level: f32,
    /// Motor sine modulation amount 0..1 and speed in Hz.
    pub mod_amount: f32,
    pub mod_speed_hz: f32,
    /// MTR button held: motor drags to a dead stop.
    pub motor_kill: bool,
    /// 24 dB/oct OTA highpass in the echo path (pre-record on regeneration).
    pub hpf_hz: f32,
    /// 24 dB/oct OTA lowpass in the echo path.
    pub lpf_hz: f32,
    /// LPF resonance 0..1, self-oscillation from ~0.93.
    pub res: f32,
    /// RES gate: when false, resonance is forced off (the 1-8 buttons gate
    /// it when the panel gate switch is on — wired by the sequencer).
    pub res_active: bool,
    /// OUT DRV 0..1, applied to tape + dry sum.
    pub out_drive: f32,
    /// Cassette in the well.
    pub tape_kind: TapeKind,
    /// Mechanism condition 0..1 (0 mint, 1 wreck): wow/flutter, dropouts,
    /// bias sag.
    pub condition: f32,
    /// Tape noise scale (1.0 = calibrated, 0.0 = sterile).
    pub noise_amount: f32,
    /// Oversampling for the magnetics (quality setting).
    pub os_factor: OsFactor,
    /// Positions / sets / cycle / anomaly configuration.
    pub seq: SeqConfig,
    /// Panel RES-gate switch: the 1-8 buttons gate the resonance.
    pub res_gate_enabled: bool,
    /// A position button (or MIDI note) is currently held.
    pub gate_held: bool,
    /// Transport mode (REC/ECHO, PLAY, LOOP).
    pub transport: TransportKind,
    /// PAUSE held: fast mechanical stop, speed setting retained.
    pub pause: bool,
    /// STP: tape motion stopped.
    pub stop: bool,
    /// Held RW/FF (Play mode).
    pub wind: Wind,
    /// Loop length in seconds of tape (Loop mode only).
    pub loop_len_s: f32,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            delay_time: 0.35,
            feedback: 0.45,
            tape_in: 1.0,
            tape_level: 0.8,
            dry_level: 1.0,
            mod_amount: 0.0,
            mod_speed_hz: 0.5,
            motor_kill: false,
            hpf_hz: 30.0,
            lpf_hz: 7_000.0,
            res: 0.0,
            res_active: true,
            out_drive: 0.0,
            tape_kind: TapeKind::I,
            condition: 0.35,
            noise_amount: 1.0,
            os_factor: OsFactor::X4,
            seq: SeqConfig::default(),
            res_gate_enabled: false,
            gate_held: false,
            transport: TransportKind::Echo,
            pause: false,
            stop: false,
            wind: Wind::Off,
            loop_len_s: 4.0,
        }
    }
}

/// How often (in samples) the speed-tracking filter coefficients refresh.
const CONTROL_INTERVAL: u32 = 16;

/// Gain smoothing time constant.
const GAIN_SMOOTH_SECONDS: f64 = 0.005;

pub struct Te2Engine {
    sample_rate: f64,
    motor: Motor,
    reel: TapeReel,
    wow_flutter: WowFlutter,
    sequencer: Sequencer,

    // Magnetics: pre-normalized parameter sets per tape type, current copy
    // with condition-adjusted bias.
    mag_by_kind: [MagParams; 3],
    mag: MagParams,
    hysteresis: [Hysteresis; 2],
    oversamplers: [Oversampler; 2],

    record_eq: [RecordEq; 2],
    repro_eq: [ReproEq; 2],
    noise_profile: NoiseProfile,
    noise: [TapeNoise; 2],

    // Record-side bandwidth: what the tape can hold at the current speed.
    // Doubles as the deposit anti-alias filter — slow tape physically cannot
    // record highs. 4th-order Butterworth (two cascaded biquads).
    record_bw_coeffs: [BiquadCoeffs; 2],
    record_bw: [[Biquad; 2]; 2],

    // The echo-path OTA filters (post-repro, pre-feedback-origin).
    ota_hp: [OtaHighpass; 2],
    ota_lp: [OtaLowpass; 2],

    // OUT DRV on the final mix.
    out_drive: OutputDrive,

    // DC blocker inside the feedback loop.
    fb_dc_coeff: f32,
    fb_dc: [OnePole; 2],

    /// Last repro output, fed back into the record sum (the loop transit time
    /// is the head gap, so one-sample-late feedback is physical).
    fb_sample: [f32; 2],

    /// VU ballistics source: envelope of the recorded (post-magnetics) level.
    vu_env: f32,
    vu_coeff_up: f32,
    vu_coeff_down: f32,

    // Smoothed gains.
    feedback_g: Smoothed,
    tape_in_g: Smoothed,
    tape_level_g: Smoothed,
    dry_level_g: Smoothed,

    params: EngineParams,
    control_countdown: u32,
}

impl Te2Engine {
    pub fn new(sample_rate: f64) -> Self {
        let capacity = (MAX_LOOP_SECONDS * TAPE_RATE) as usize;
        let smooth = Smoothed::coeff(sample_rate, GAIN_SMOOTH_SECONDS);
        let params = EngineParams::default();
        let sr = sample_rate as f32;

        // The empirical gain normalization runs a short sim per type; this
        // constructor is only ever called from initialize(), never process().
        let mag_by_kind = [
            MagParams::type_i().normalized(),
            MagParams::type_ii().normalized(),
            MagParams::type_iv().normalized(),
        ];
        let eq_profile = EqProfile::for_kind(params.tape_kind);

        let mut engine = Self {
            sample_rate,
            motor: Motor::new(sample_rate),
            reel: TapeReel::new(capacity),
            wow_flutter: WowFlutter::new(sample_rate),
            sequencer: Sequencer::new(sample_rate),
            mag_by_kind,
            mag: mag_by_kind[0],
            hysteresis: [Hysteresis::default(), Hysteresis::default()],
            oversamplers: [Oversampler::default(), Oversampler::default()],
            record_eq: [RecordEq::new(sr, &eq_profile), RecordEq::new(sr, &eq_profile)],
            repro_eq: [ReproEq::new(sr, &eq_profile), ReproEq::new(sr, &eq_profile)],
            noise_profile: NoiseProfile::type_i(),
            noise: [
                TapeNoise::new(sample_rate, 0x1234_5678),
                TapeNoise::new(sample_rate, 0x8765_4321),
            ],
            record_bw_coeffs: [BiquadCoeffs::IDENTITY; 2],
            record_bw: [[Biquad::default(); 2]; 2],
            ota_hp: [OtaHighpass::default(), OtaHighpass::default()],
            ota_lp: [OtaLowpass::default(), OtaLowpass::default()],
            out_drive: OutputDrive::new(sample_rate),
            fb_dc_coeff: OnePole::coeff(sr, 10.0),
            fb_dc: [OnePole::default(); 2],
            fb_sample: [0.0; 2],
            vu_env: 0.0,
            vu_coeff_up: OnePole::coeff(sr, 8.0),
            vu_coeff_down: OnePole::coeff(sr, 1.6),
            feedback_g: Smoothed::new(params.feedback, smooth),
            tape_in_g: Smoothed::new(params.tape_in, smooth),
            tape_level_g: Smoothed::new(params.tape_level, smooth),
            dry_level_g: Smoothed::new(params.dry_level, smooth),
            params,
            control_countdown: 0,
        };
        engine.apply_params();
        engine.update_speed_filters(engine.motor.current_speed().abs().max(0.05));
        engine
    }

    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Relative motor speed right now (for the UI reel animation and tests).
    pub fn motor_speed(&self) -> f64 {
        self.motor.current_speed()
    }

    /// VU meter level: envelope of what's hitting the tape.
    pub fn vu_level(&self) -> f32 {
        self.vu_env
    }

    /// Current sequencer position 1..=8 (for the UI LEDs).
    pub fn current_position(&self) -> u8 {
        self.sequencer.position()
    }

    /// STP/EJ pressed twice: pop the cassette and put in a fresh one. Also
    /// flushes the signal in flight — with no tape in the well there is no
    /// loop for it to live in.
    pub fn eject(&mut self) {
        self.reel.reset();
        self.fb_sample = [0.0; 2];
        self.vu_env = 0.0;
        for ch in 0..2 {
            self.fb_dc[ch].reset();
            self.ota_hp[ch].reset();
            self.ota_lp[ch].reset();
            self.record_eq[ch].reset();
            self.repro_eq[ch].reset();
            self.hysteresis[ch].reset();
            self.oversamplers[ch].reset();
            for stage in 0..2 {
                self.record_bw[ch][stage].reset();
            }
        }
    }

    pub fn set_params(&mut self, params: &EngineParams) {
        let kind_changed = params.tape_kind != self.params.tape_kind;
        let condition_changed = (params.condition - self.params.condition).abs() > 1e-6;
        self.params = *params;
        if kind_changed {
            let idx = match params.tape_kind {
                TapeKind::I => 0,
                TapeKind::II => 1,
                TapeKind::IV => 2,
            };
            self.mag = self.mag_by_kind[idx];
            self.noise_profile = NoiseProfile::for_params(&self.mag, params.tape_kind);
            let eq = EqProfile::for_kind(params.tape_kind);
            for ch in 0..2 {
                self.record_eq[ch].set_profile(&eq);
                self.repro_eq[ch].set_profile(&eq);
                self.noise[ch].set_profile(self.sample_rate, &self.noise_profile);
            }
        }
        if kind_changed || condition_changed {
            // A worn machine's bias sags: more hysteresis grit. Physical, so
            // the small level shift that comes with it is kept.
            let idx = match params.tape_kind {
                TapeKind::I => 0,
                TapeKind::II => 1,
                TapeKind::IV => 2,
            };
            let base = self.mag_by_kind[idx];
            self.mag = MagParams {
                bias: base.bias * (1.0 - 0.30 * params.condition as f64),
                ..base
            };
            self.wow_flutter.set_condition(params.condition as f64);
        }
        self.apply_params();
    }

    /// TIME-knob inertia (settings drawer trim).
    pub fn set_motor_inertia(&mut self, settle_seconds: f64) {
        self.motor.set_inertia(settle_seconds);
    }

    /// MTR wind-down/up ramp (panel trim screw).
    pub fn set_motor_ramp(&mut self, down_seconds: f64, up_seconds: f64) {
        self.motor.set_kill_ramp(down_seconds, up_seconds);
    }

    fn apply_params(&mut self) {
        let p = &self.params;
        self.motor
            .set_target_speed(speed_for_delay(p.delay_time as f64));
        self.motor.set_motor_kill(p.motor_kill);
        self.motor
            .set_modulation(p.mod_amount as f64, p.mod_speed_hz as f64);
        self.feedback_g.target = p.feedback;
        self.tape_in_g.target = p.tape_in;
        self.tape_level_g.target = p.tape_level;
        self.dry_level_g.target = p.dry_level;

        let sr = self.sample_rate as f32;
        let res = if p.res_active { p.res } else { 0.0 };
        for ch in 0..2 {
            self.ota_hp[ch].set(sr, p.hpf_hz);
            self.ota_lp[ch].set(sr, p.lpf_hz, res);
        }
        self.out_drive.set_amount(p.out_drive);

        // Transport state machine: stop/pause dominate, wind only acts in
        // Play mode (like holding RW/FF during playback on the hardware).
        self.motor.mechanism = if p.stop {
            Mechanism::Stopped
        } else if p.pause {
            Mechanism::Paused
        } else {
            match (p.transport, p.wind) {
                (TransportKind::Play, Wind::Rewind) => Mechanism::Rewinding,
                (TransportKind::Play, Wind::FastForward) => Mechanism::FastForwarding,
                (TransportKind::Play, Wind::Off) => Mechanism::Playing,
                (TransportKind::Echo | TransportKind::Loop, _) => Mechanism::Recording,
            }
        };

        // Loop mode shortens the virtual tape to the loop length; echo mode
        // uses the whole reel (effectively endless).
        let loop_cells = match p.transport {
            TransportKind::Loop => {
                ((p.loop_len_s as f64).clamp(0.5, MAX_LOOP_SECONDS - 1.0) * TAPE_RATE) as usize
            }
            _ => (MAX_LOOP_SECONDS * TAPE_RATE) as usize,
        };
        if loop_cells != self.reel.loop_len() {
            self.reel.set_loop_len(loop_cells);
        }

        // Hand the sequencer its config, with the panel values expressed in
        // each target's normalized mapping so position 1 (the panel) blends
        // into drift glides seamlessly.
        let mut seq = p.seq;
        seq.panel_time_u = sequencer::unmap_time_s(p.delay_time);
        seq.panel_res_u = sequencer::unmap_unit(p.res);
        seq.panel_mod_spd_u = sequencer::unmap_mod_spd_hz(p.mod_speed_hz);
        seq.panel_fdbk_u = sequencer::unmap_feedback(p.feedback);
        seq.panel_mod_amt_u = sequencer::unmap_unit(p.mod_amount);
        seq.panel_lpf_u = sequencer::unmap_lpf_hz(p.lpf_hz);
        seq.panel_tape_lvl_u = sequencer::unmap_level(p.tape_level);
        seq.panel_dry_lvl_u = sequencer::unmap_level(p.dry_level);
        seq.panel_hpf_u = sequencer::unmap_hpf_hz(p.hpf_hz);
        self.sequencer.set_config(&seq);
    }

    /// Recompute the speed-tracking bandwidth filters.
    fn update_speed_filters(&mut self, speed: f64) {
        let sr = self.sample_rate as f32;

        // Local tape Nyquist in host terms is 0.5 * v * TAPE_RATE. Stay under
        // it for anti-aliasing; the 20 kHz cap is the electronics' own limit.
        let record_fc = (0.42 * speed * TAPE_RATE) as f32;
        let record_fc = record_fc.min(0.45 * sr).min(20_000.0);
        // Butterworth Q values for a 4th-order cascade.
        self.record_bw_coeffs = [
            BiquadCoeffs::lowpass(sr, record_fc, 0.5412),
            BiquadCoeffs::lowpass(sr, record_fc, 1.3066),
        ];

        for ch in 0..2 {
            self.repro_eq[ch].set_speed(speed as f32);
        }
    }

    /// Process one stereo frame.
    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.control_countdown == 0 {
            self.control_countdown = CONTROL_INTERVAL;
            let speed = self.motor.current_speed().abs().max(0.05);
            self.update_speed_filters(speed);
        }
        self.control_countdown -= 1;

        // Sequencer first: it may override the motor target, the filters,
        // and the level smoother targets for this sample.
        let seq_out = self.sequencer.process();
        let seq_active =
            self.params.seq.white_on || self.params.seq.gray_on || self.params.seq.black_on;
        if seq_active {
            if let Some(t) = seq_out.time_s {
                self.motor.set_target_speed(speed_for_delay(t as f64));
            }
            let mod_amt = seq_out.mod_amt.unwrap_or(self.params.mod_amount);
            let mod_spd = seq_out.mod_spd_hz.unwrap_or(self.params.mod_speed_hz);
            if seq_out.mod_amt.is_some() || seq_out.mod_spd_hz.is_some() {
                self.motor.set_modulation(mod_amt as f64, mod_spd as f64);
            }
            if let Some(fb) = seq_out.feedback {
                self.feedback_g.target = fb;
            }
            if let Some(tl) = seq_out.tape_level {
                self.tape_level_g.target = tl;
            }
            if let Some(dl) = seq_out.dry_level {
                self.dry_level_g.target = dl;
            }
        }

        // RES gating + any sequencer filter overrides need per-sample filter
        // updates; otherwise the control-interval refresh is enough.
        let res_gated_off = self.params.res_gate_enabled && !self.params.gate_held;
        let res_eff = if res_gated_off || !self.params.res_active {
            0.0
        } else {
            seq_out.res.unwrap_or(self.params.res)
        };
        let lpf_eff = seq_out.lpf_hz.unwrap_or(self.params.lpf_hz);
        let hpf_eff = seq_out.hpf_hz.unwrap_or(self.params.hpf_hz);
        if seq_out.lpf_hz.is_some()
            || seq_out.hpf_hz.is_some()
            || seq_out.res.is_some()
            || self.params.res_gate_enabled
        {
            let sr = self.sample_rate as f32;
            for ch in 0..2 {
                self.ota_lp[ch].set(sr, lpf_eff, res_eff);
                self.ota_hp[ch].set(sr, hpf_eff);
            }
        }

        let (wf_mult, dropout_gains) = self.wow_flutter.process();
        let v = self.motor.process() * (wf_mult + seq_out.anomaly_speed);
        let delta = v * TAPE_RATE / self.sample_rate;

        let feedback = self.feedback_g.next();
        let tape_in = self.tape_in_g.next();
        let tape_level = self.tape_level_g.next();
        let dry_level = self.dry_level_g.next();

        let os_factor = self.params.os_factor;
        let dt_os = 1.0 / (os_factor.factor() as f64 * self.sample_rate);
        let noise_amount = self.params.noise_amount;

        let dry = [left, right];
        let mut rec = [0.0f32; 2];
        let mut vu_in = 0.0f32;
        for ch in 0..2 {
            let fb = self.fb_dc[ch].highpass(self.fb_dc_coeff, self.fb_sample[ch]) * feedback;
            let sum = dry[ch] * tape_in + fb;

            // Record electronics: emphasis, then noise onto the bus so it is
            // recorded (and regenerates through the loop like real hiss).
            let emphasized = self.record_eq[ch].pre(sum);
            let with_noise =
                emphasized + self.noise[ch].process(&self.noise_profile, emphasized, noise_amount);

            // The tape itself, at the oversampled rate.
            let mag = &self.mag;
            let hyst = &mut self.hysteresis[ch];
            let magnetized = self.oversamplers[ch]
                .process(os_factor, with_noise, |s| hyst.process(mag, s, dt_os));

            vu_in = vu_in.max(magnetized.abs());

            // Self-erasure of hot highs, then the speed bandwidth limit.
            let mut shaped = self.record_eq[ch].post(magnetized);
            for stage in 0..2 {
                shaped = self.record_bw[ch][stage].process(&self.record_bw_coeffs[stage], shaped);
            }
            rec[ch] = shaped;
        }

        // VU ballistics (attack faster than release).
        let coeff = if vu_in > self.vu_env {
            self.vu_coeff_up
        } else {
            self.vu_coeff_down
        };
        self.vu_env += coeff * (vu_in - self.vu_env);

        match self.motor.mechanism {
            Mechanism::Recording => {
                // Loop mode lifts the erase head: old material survives each
                // pass, slowly composting under new layers.
                let erase_keep = match self.params.transport {
                    TransportKind::Loop => LOOP_ERASE_KEEP,
                    _ => 0.0,
                };
                self.reel.advance_record(delta, rec, erase_keep);
            }
            _ => self.reel.advance_play(delta),
        }

        let tape_raw = self.reel.read(HEAD_GAP);
        let mut out = [0.0f32; 2];
        for ch in 0..2 {
            let tape = self.repro_eq[ch].process(tape_raw[ch]) * dropout_gains[ch];
            // Block diagram order: repro -> HPF -> LPF(+res) -> feedback
            // origin. Every regeneration is re-filtered, and the HPF lands
            // before the record head on the next pass.
            let filtered = self.ota_lp[ch].process(self.ota_hp[ch].process(tape));
            self.fb_sample[ch] = filtered;

            let mixed = dry[ch] * dry_level + filtered * tape_level;
            let driven = self.out_drive.process(ch, mixed);
            // Final safety clamp: protects the host from pathological states
            // without ever engaging in normal use.
            out[ch] = driven.clamp(-4.0, 4.0);
        }

        (out[0], out[1])
    }

    pub fn reset(&mut self) {
        self.motor.reset();
        self.reel.reset();
        self.wow_flutter.reset();
        self.sequencer.reset();
        for ch in 0..2 {
            for stage in 0..2 {
                self.record_bw[ch][stage].reset();
            }
            self.record_eq[ch].reset();
            self.repro_eq[ch].reset();
            self.hysteresis[ch].reset();
            self.oversamplers[ch].reset();
            self.noise[ch].reset();
            self.fb_dc[ch].reset();
            self.ota_hp[ch].reset();
            self.ota_lp[ch].reset();
        }
        self.out_drive.reset();
        self.fb_sample = [0.0; 2];
        self.vu_env = 0.0;
    }
}

/// One-pole parameter smoother.
struct Smoothed {
    value: f32,
    target: f32,
    coeff: f32,
}

impl Smoothed {
    fn coeff(sample_rate: f64, tau: f64) -> f32 {
        (1.0 - (-1.0 / (tau * sample_rate)).exp()) as f32
    }

    fn new(value: f32, coeff: f32) -> Self {
        Self {
            value,
            target: value,
            coeff,
        }
    }

    #[inline]
    fn next(&mut self) -> f32 {
        self.value += self.coeff * (self.target - self.value);
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine_48k() -> Te2Engine {
        let mut e = Te2Engine::new(48_000.0);
        e.set_motor_inertia(0.02); // fast settle for tests
        e
    }

    /// Params with the mechanism stilled and filters wide open for
    /// deterministic timing tests.
    fn clinical(delay: f32, feedback: f32) -> EngineParams {
        EngineParams {
            delay_time: delay,
            feedback,
            dry_level: 0.0,
            tape_level: 1.0,
            condition: 0.0,
            noise_amount: 0.0,
            hpf_hz: 20.0,
            lpf_hz: 18_000.0,
            res: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn echo_arrives_at_delay_time() {
        let mut e = engine_48k();
        e.set_params(&clinical(0.25, 0.0));
        for _ in 0..24_000 {
            e.process(0.0, 0.0);
        }

        let mut first_echo = None;
        for k in 0..24_000 {
            let x = if k == 0 { 1.0 } else { 0.0 };
            let (y, _) = e.process(x, 0.0);
            if y.abs() > 0.02 && first_echo.is_none() {
                first_echo = Some(k);
            }
        }
        let k = first_echo.expect("no echo") as f64 / 48_000.0;
        assert!(
            (k - 0.25).abs() < 0.01,
            "echo at {k:.4}s, expected 0.250s"
        );
    }

    #[test]
    fn time_change_repitches_tape_content() {
        // Record a 1 kHz tone at one speed, halve the speed (double the
        // delay), and the already-recorded material must come back an octave
        // down — the signature tape echo behavior.
        let sr = 48_000.0;
        let mut e = engine_48k();
        e.set_params(&clinical(0.3, 0.0));
        for _ in 0..48_000 {
            e.process(0.0, 0.0);
        }

        let mut k = 0u64;
        for _ in 0..48_000 {
            let x = 0.35 * (std::f64::consts::TAU * 1000.0 * k as f64 / sr).sin() as f32;
            e.process(x, x);
            k += 1;
        }

        e.set_params(&clinical(0.6, 0.0));
        for _ in 0..7_200 {
            e.process(0.0, 0.0);
        }
        let mut crossings = 0u32;
        let mut prev = 0.0f32;
        for _ in 0..9_600 {
            let (y, _) = e.process(0.0, 0.0);
            if prev <= 0.0 && y > 0.0 {
                crossings += 1;
            }
            prev = y;
        }
        let freq = crossings as f64 / 0.2;
        assert!(
            (freq - 500.0).abs() < 25.0,
            "repitched tone measured {freq:.1} Hz, expected ~500 Hz"
        );
    }

    #[test]
    fn glide_is_click_free() {
        let mut e = engine_48k();
        e.set_params(&clinical(0.2, 0.6));

        let sr = 48_000.0;
        let mut k = 0u64;
        let mut max_step = 0.0f32;
        let mut prev = 0.0f32;
        let mut tone = |k: u64| 0.3 * (std::f64::consts::TAU * 800.0 * k as f64 / sr).sin() as f32;

        for _ in 0..48_000 {
            let x = tone(k);
            k += 1;
            let (y, _) = e.process(x, x);
            prev = y;
        }
        e.set_params(&clinical(0.45, 0.6));
        for _ in 0..48_000 {
            let x = tone(k);
            k += 1;
            let (y, _) = e.process(x, x);
            let step = (y - prev).abs();
            if step > max_step {
                max_step = step;
            }
            prev = y;
        }

        assert!(
            max_step < 0.2,
            "output discontinuity during TIME glide: step {max_step:.3}"
        );
    }

    #[test]
    fn runaway_feedback_stays_bounded_by_tape() {
        let mut e = engine_48k();
        e.set_params(&EngineParams {
            delay_time: 0.15,
            feedback: 1.08,
            dry_level: 0.0,
            tape_level: 1.0,
            condition: 0.0,
            noise_amount: 0.0,
            ..Default::default()
        });
        let mut peak_late = 0.0f32;
        for k in 0..(8 * 48_000) {
            let x = if k < 100 { 0.8 } else { 0.0 };
            let (y, _) = e.process(x, x);
            assert!(y.is_finite(), "non-finite output at sample {k}");
            if k > 6 * 48_000 {
                peak_late = peak_late.max(y.abs());
            }
        }
        assert!(peak_late < 2.5, "runaway exploded: peak {peak_late:.2}");
        assert!(
            peak_late > 0.05,
            "runaway died out: peak {peak_late:.4} (feedback >1 should self-oscillate)"
        );
    }

    #[test]
    fn repeats_degrade_progressively() {
        // Each pass re-records through emphasis + magnetics + losses, so a
        // bright transient's repeats must lose HF energy monotonically-ish.
        let sr = 48_000.0f64;
        let mut e = engine_48k();
        e.set_params(&clinical(0.25, 0.85));
        for _ in 0..24_000 {
            e.process(0.0, 0.0);
        }

        // One sharp click.
        let mut out = Vec::new();
        for k in 0..(5 * 48_000) {
            let x = if k < 24 { 0.9 } else { 0.0 };
            let (y, _) = e.process(x, x);
            out.push(y);
        }

        // HF energy (above ~3 kHz via simple first difference) in windows
        // around each expected repeat.
        let hf_energy = |start: usize| {
            let w = &out[start..(start + 4_800).min(out.len())];
            let mut acc = 0.0f64;
            for pair in w.windows(2) {
                let d = (pair[1] - pair[0]) as f64;
                acc += d * d;
            }
            acc
        };
        let r1 = hf_energy((0.25 * sr) as usize - 200);
        let r3 = hf_energy((0.75 * sr) as usize - 200);
        let r6 = hf_energy((1.50 * sr) as usize - 200);
        assert!(
            r1 > r3 && r3 > r6,
            "repeats should darken: r1 {r1:.2e} r3 {r3:.2e} r6 {r6:.2e}"
        );
    }

    #[test]
    fn noise_floor_present_and_calibrated() {
        let mut e = engine_48k();
        e.set_params(&EngineParams {
            delay_time: 0.3,
            feedback: 0.0,
            dry_level: 0.0,
            tape_level: 1.0,
            condition: 0.35,
            noise_amount: 1.0,
            ..Default::default()
        });
        for _ in 0..48_000 {
            e.process(0.0, 0.0);
        }
        let n = 4 * 48_000;
        let mut sum_sq = 0.0f64;
        for _ in 0..n {
            let (y, _) = e.process(0.0, 0.0);
            sum_sq += (y as f64) * (y as f64);
        }
        let rms_db = 10.0 * (sum_sq / n as f64).log10();
        assert!(
            (-65.0..=-45.0).contains(&rms_db),
            "tape noise floor {rms_db:.1} dBFS, expected around -53"
        );
    }

    #[test]
    fn lpf_in_loop_darkens_repeats_faster() {
        // With the LPF low, each regeneration loses more HF than with it
        // open — the decay of HF energy across repeats must be steeper.
        let hf_decay = |lpf_hz: f32| {
            let mut e = engine_48k();
            e.set_params(&EngineParams {
                lpf_hz,
                ..clinical(0.25, 0.85)
            });
            for _ in 0..24_000 {
                e.process(0.0, 0.0);
            }
            let mut out = Vec::new();
            for k in 0..(3 * 48_000) {
                let x = if k < 24 { 0.9 } else { 0.0 };
                let (y, _) = e.process(x, x);
                out.push(y);
            }
            let hf = |start: usize| {
                let w = &out[start..start + 4_800];
                w.windows(2).map(|p| {
                    let d = (p[1] - p[0]) as f64;
                    d * d
                }).sum::<f64>()
            };
            let r1 = hf((0.25 * 48_000.0) as usize - 200);
            let r4 = hf((1.0 * 48_000.0) as usize - 200);
            r4 / r1.max(1e-30)
        };
        let open = hf_decay(18_000.0);
        let closed = hf_decay(1_500.0);
        // Repeat 1 has already passed the LPF once, so the r4/r1 ratio
        // understates the per-pass loss; ~35% extra decay is solid evidence
        // the filter sits inside the regeneration loop.
        assert!(
            closed < open * 0.65,
            "low LPF should steepen HF decay: open {open:.3e} closed {closed:.3e}"
        );
    }

    #[test]
    fn filter_self_oscillates_at_full_res() {
        let mut e = engine_48k();
        e.set_params(&EngineParams {
            lpf_hz: 600.0,
            res: 1.0,
            // The tape noise floor is what kicks oscillation off from
            // silence, exactly like the hardware.
            noise_amount: 0.05,
            ..clinical(0.3, 0.0)
        });
        // No input at all; the filter must sing on its own.
        for _ in 0..(2 * 48_000) {
            e.process(0.0, 0.0);
        }
        let mut peak = 0.0f32;
        let mut crossings = 0u32;
        let mut prev = 0.0f32;
        for _ in 0..48_000 {
            let (y, _) = e.process(0.0, 0.0);
            peak = peak.max(y.abs());
            if prev <= 0.0 && y > 0.0 {
                crossings += 1;
            }
            prev = y;
        }
        assert!(peak > 0.05, "filter should self-oscillate: peak {peak}");
        let freq = crossings as f64;
        let cents = 1200.0 * (freq / 600.0).log2();
        assert!(
            cents.abs() < 60.0,
            "self-osc pitch off cutoff: {freq:.0} Hz ({cents:+.0} cents)"
        );
    }

    #[test]
    fn cycle_drives_motor_speed_between_positions() {
        use crate::sequencer::{SeqConfig, WhiteTarget};
        let mut e = engine_48k();
        let mut seq = SeqConfig {
            cycle_run: true,
            cycle_len: 2,
            cycle_rate: 1.0, // 1 step per second
            white_on: true,
            white_target: WhiteTarget::Time,
            ..Default::default()
        };
        // Position 2 fader low (fast tape), position... len 2 cycles
        // positions 1 and 2: position 1 = panel time (0.3 s).
        seq.white_faders = [sequencer::unmap_time_s(0.9), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        e.set_params(&EngineParams {
            seq,
            ..clinical(0.3, 0.0)
        });

        // Sample motor speed across 4 seconds; it must visit two distinct
        // speed plateaus (0.35/0.3 ~= 1.17 and 0.35/0.9 ~= 0.39).
        let mut speeds = Vec::new();
        for k in 0..(4 * 48_000) {
            e.process(0.0, 0.0);
            if k % 4_800 == 0 {
                speeds.push(e.motor_speed());
            }
        }
        let fast = speeds.iter().cloned().fold(f64::MIN, f64::max);
        let slow = speeds.iter().cloned().fold(f64::MAX, f64::min);
        assert!(
            (fast - 1.1667).abs() < 0.1,
            "fast plateau {fast:.3}, expected ~1.167"
        );
        assert!(
            (slow - 0.3889).abs() < 0.1,
            "slow plateau {slow:.3}, expected ~0.389"
        );
    }

    #[test]
    fn res_gate_silences_oscillation_until_held() {
        let mut e = engine_48k();
        let base = EngineParams {
            lpf_hz: 700.0,
            res: 1.0,
            noise_amount: 0.05,
            res_gate_enabled: true,
            gate_held: false,
            ..clinical(0.3, 0.0)
        };
        e.set_params(&base);
        for _ in 0..(2 * 48_000) {
            e.process(0.0, 0.0);
        }
        let mut peak = 0.0f32;
        for _ in 0..24_000 {
            let (y, _) = e.process(0.0, 0.0);
            peak = peak.max(y.abs());
        }
        assert!(peak < 0.02, "gated-off filter should not sing: {peak}");

        e.set_params(&EngineParams {
            gate_held: true,
            ..base
        });
        for _ in 0..(2 * 48_000) {
            e.process(0.0, 0.0);
        }
        let mut peak = 0.0f32;
        for _ in 0..24_000 {
            let (y, _) = e.process(0.0, 0.0);
            peak = peak.max(y.abs());
        }
        assert!(peak > 0.05, "held gate should open oscillation: {peak}");
    }

    #[test]
    fn play_mode_does_not_record() {
        let mut e = engine_48k();
        e.set_params(&EngineParams {
            transport: TransportKind::Play,
            ..clinical(0.3, 0.5)
        });
        for _ in 0..24_000 {
            e.process(0.0, 0.0);
        }
        // Feed loud audio in Play; nothing must land on tape.
        let mut peak = 0.0f32;
        for k in 0..(2 * 48_000) {
            let x = 0.8 * ((k as f32 * 0.13).sin());
            let (y, _) = e.process(x, x);
            peak = peak.max(y.abs());
        }
        assert!(peak < 1e-3, "play mode leaked input onto tape: {peak}");
    }

    #[test]
    fn loop_mode_layers_and_composts() {
        // A burst recorded onto a 1-second loop with feedback OFF must keep
        // coming back every second, decaying by the erase-bypass keep factor
        // — layering, not echo.
        // Delay 0.35 s = nominal motor speed, so the 1 s tape loop passes in
        // exactly 1 s of host time (loop length is tape footage, not host
        // time — faster TIME settings shorten the heard loop, like hardware).
        let mut e = engine_48k();
        e.set_params(&EngineParams {
            transport: TransportKind::Loop,
            loop_len_s: 1.0,
            ..clinical(0.35, 0.0)
        });
        for _ in 0..24_000 {
            e.process(0.0, 0.0);
        }

        let mut out = Vec::new();
        for k in 0..(4 * 48_000) {
            let x = if k < 480 { 0.7 } else { 0.0 };
            let (y, _) = e.process(x, x);
            out.push(y.abs());
        }
        // Peak around each loop pass (burst plays back 0.25 s after record
        // [the head gap], then every 1.0 s).
        let window_peak = |t: f64| {
            let c = (t * 48_000.0) as usize;
            out[c.saturating_sub(2_400)..(c + 2_400).min(out.len())]
                .iter()
                .cloned()
                .fold(0.0f32, f32::max)
        };
        let p1 = window_peak(0.35);
        let p2 = window_peak(1.35);
        let p3 = window_peak(2.35);
        assert!(p1 > 0.1, "first pass missing: {p1}");
        assert!(
            p2 > p1 * 0.4 && p2 < p1 * 0.95,
            "second pass should be the keep-factor quieter: p1 {p1:.3} p2 {p2:.3}"
        );
        assert!(
            p3 > p2 * 0.4 && p3 < p2 * 0.95,
            "third pass should compost further: p2 {p2:.3} p3 {p3:.3}"
        );
    }

    #[test]
    fn rewind_plays_recorded_tape_backwards() {
        let mut e = engine_48k();
        let base = clinical(0.3, 0.0);
        e.set_params(&base);
        for _ in 0..24_000 {
            e.process(0.0, 0.0);
        }
        // Record a second of tone in Echo mode.
        for k in 0..48_000 {
            let x = 0.5 * (std::f64::consts::TAU * 500.0 * k as f64 / 48_000.0).sin() as f32;
            e.process(x, x);
        }
        // Switch to Play + RW: the tape runs backward over the recording.
        e.set_params(&EngineParams {
            transport: TransportKind::Play,
            wind: Wind::Rewind,
            ..base
        });
        let mut energy = 0.0f64;
        for _ in 0..24_000 {
            let (y, _) = e.process(0.0, 0.0);
            assert!(y.is_finite());
            energy += (y as f64) * (y as f64);
        }
        assert!(
            energy > 0.1,
            "rewind over recorded tape should produce sound: {energy:.4}"
        );
    }

    #[test]
    fn eject_clears_the_tape() {
        let mut e = engine_48k();
        e.set_params(&clinical(0.25, 0.8));
        for _ in 0..24_000 {
            e.process(0.0, 0.0);
        }
        for k in 0..24_000 {
            let x = if k < 480 { 0.8 } else { 0.0 };
            e.process(x, x);
        }
        e.eject();
        let mut peak = 0.0f32;
        for _ in 0..(2 * 48_000) {
            let (y, _) = e.process(0.0, 0.0);
            peak = peak.max(y.abs());
        }
        assert!(peak < 1e-3, "tape should be blank after eject: {peak}");
    }

    #[test]
    fn motor_kill_drags_to_stop_and_recovers() {
        let mut e = engine_48k();
        let base = clinical(0.3, 0.0);
        e.set_params(&base);
        for _ in 0..24_000 {
            e.process(0.0, 0.0);
        }
        let v_run = e.motor_speed();
        assert!(v_run > 0.5, "motor should be running: {v_run}");

        e.set_params(&EngineParams {
            motor_kill: true,
            ..base
        });
        for _ in 0..(2 * 48_000) {
            e.process(0.0, 0.0);
        }
        assert!(
            e.motor_speed().abs() < 0.01,
            "motor should be dead-stopped: {}",
            e.motor_speed()
        );

        e.set_params(&base);
        for _ in 0..(2 * 48_000) {
            e.process(0.0, 0.0);
        }
        assert!(
            (e.motor_speed() - v_run).abs() < 0.05,
            "motor should recover to {v_run}: {}",
            e.motor_speed()
        );
    }
}
