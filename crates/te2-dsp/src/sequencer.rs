//! Positions, Sets, Cycle, Drift, Anomaly — "kind of a multi-stage function
//! generator", per the TE-2 spec.
//!
//! - 8 positions. Position 1 is the panel itself; positions 2-8 are the 21
//!   fader values (7 per set).
//! - 3 sets (White / Gray / Black), each with an ON switch, a target
//!   selector, and an independent DRIFT slew (0-14 s, logarithmic feel).
//! - The Cycle rotates positions 1..len at 8 s/step up to 4,000 steps/s.
//!   Everything here runs per sample: at audio-rate stepping the fader
//!   pattern *is* a waveform and the drift knob rounds its corners.
//! - The Anomaly fires a single motor-speed hiccup when the cycle enters its
//!   final step.
//!
//! The sequencer outputs *override values in natural units* for whichever
//! targets are active; the engine merges them with the panel parameters.

/// What the White set can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhiteTarget {
    #[default]
    Time,
    Resonance,
    ModSpeed,
}

/// What the Gray set can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GrayTarget {
    #[default]
    Feedback,
    ModAmount,
    Lpf,
}

/// What the Black set can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlackTarget {
    #[default]
    TapeLevel,
    DryLevel,
    Hpf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnomalyPolarity {
    Minus,
    #[default]
    Off,
    Plus,
}

/// Sequencer configuration, set at control rate from the plugin parameters.
#[derive(Debug, Clone, Copy)]
pub struct SeqConfig {
    /// Fader values per set, positions 2..=8 (index 0 = position 2).
    pub white_faders: [f32; 7],
    pub gray_faders: [f32; 7],
    pub black_faders: [f32; 7],
    pub white_on: bool,
    pub gray_on: bool,
    pub black_on: bool,
    pub white_target: WhiteTarget,
    pub gray_target: GrayTarget,
    pub black_target: BlackTarget,
    /// Drift slew per set, seconds (0 = snap).
    pub white_drift: f32,
    pub gray_drift: f32,
    pub black_drift: f32,

    pub cycle_run: bool,
    /// Number of positions cycled, 1..=8.
    pub cycle_len: u8,
    /// Steps per second, 0.125..=4000.
    pub cycle_rate: f32,
    /// Host-clock lock: the absolute song position measured in cycle steps
    /// (beats / beats-per-step), when the host transport is rolling and rate
    /// sync is on. The cycle phase snaps to this at every block start, so
    /// steps land on the DAW grid instead of free-running at a matching rate.
    /// `None` = free-run (sync off, or transport stopped).
    pub host_step_pos: Option<f64>,

    /// Manual position 1..=8 (used when the cycle is stopped).
    pub manual_position: u8,

    pub anomaly_amount: f32,
    pub anomaly_polarity: AnomalyPolarity,

    /// Panel values expressed as normalized 0..1 in each target's mapping,
    /// so position 1 participates in drift glides seamlessly.
    pub panel_time_u: f32,
    pub panel_res_u: f32,
    pub panel_mod_spd_u: f32,
    pub panel_fdbk_u: f32,
    pub panel_mod_amt_u: f32,
    pub panel_lpf_u: f32,
    pub panel_tape_lvl_u: f32,
    pub panel_dry_lvl_u: f32,
    pub panel_hpf_u: f32,
}

impl Default for SeqConfig {
    fn default() -> Self {
        Self {
            white_faders: [0.5; 7],
            gray_faders: [0.5; 7],
            black_faders: [0.5; 7],
            white_on: false,
            gray_on: false,
            black_on: false,
            white_target: WhiteTarget::default(),
            gray_target: GrayTarget::default(),
            black_target: BlackTarget::default(),
            white_drift: 0.0,
            gray_drift: 0.0,
            black_drift: 0.0,
            cycle_run: false,
            cycle_len: 8,
            cycle_rate: 1.0,
            host_step_pos: None,
            manual_position: 1,
            anomaly_amount: 0.3,
            anomaly_polarity: AnomalyPolarity::Off,
            panel_time_u: 0.5,
            panel_res_u: 0.0,
            panel_mod_spd_u: 0.3,
            panel_fdbk_u: 0.41,
            panel_mod_amt_u: 0.0,
            panel_lpf_u: 0.82,
            panel_tape_lvl_u: 0.7,
            panel_dry_lvl_u: 0.85,
            panel_hpf_u: 0.06,
        }
    }
}

/// Per-sample sequencer output. `None` = set is off, use the panel value.
#[derive(Debug, Clone, Copy, Default)]
pub struct SeqOut {
    pub position: u8,
    /// Additive motor speed perturbation from the anomaly.
    pub anomaly_speed: f64,
    pub time_s: Option<f32>,
    pub res: Option<f32>,
    pub mod_spd_hz: Option<f32>,
    pub feedback: Option<f32>,
    pub mod_amt: Option<f32>,
    pub lpf_hz: Option<f32>,
    pub tape_level: Option<f32>,
    pub dry_level: Option<f32>,
    pub hpf_hz: Option<f32>,
}

// --- Normalized (0..1) <-> natural unit mappings per target. Frequencies and
// times are exponential, levels tapered, the rest linear. ---

pub fn map_time_s(u: f32) -> f32 {
    const MIN: f32 = 0.06;
    const MAX: f32 = 1.5;
    MIN * (MAX / MIN).powf(u.clamp(0.0, 1.0))
}

pub fn unmap_time_s(v: f32) -> f32 {
    const MIN: f32 = 0.06;
    const MAX: f32 = 1.5;
    (v.clamp(MIN, MAX) / MIN).ln() / (MAX / MIN).ln()
}

pub fn map_lpf_hz(u: f32) -> f32 {
    100.0 * (18_000.0f32 / 100.0).powf(u.clamp(0.0, 1.0))
}

pub fn unmap_lpf_hz(v: f32) -> f32 {
    (v.clamp(100.0, 18_000.0) / 100.0).ln() / (18_000.0f32 / 100.0).ln()
}

pub fn map_hpf_hz(u: f32) -> f32 {
    20.0 * (2_000.0f32 / 20.0).powf(u.clamp(0.0, 1.0))
}

pub fn unmap_hpf_hz(v: f32) -> f32 {
    (v.clamp(20.0, 2_000.0) / 20.0).ln() / (2_000.0f32 / 20.0).ln()
}

pub fn map_mod_spd_hz(u: f32) -> f32 {
    0.1 * (150.0f32 / 0.1).powf(u.clamp(0.0, 1.0))
}

pub fn unmap_mod_spd_hz(v: f32) -> f32 {
    (v.clamp(0.1, 150.0) / 0.1).ln() / (150.0f32 / 0.1).ln()
}

pub fn map_feedback(u: f32) -> f32 {
    u.clamp(0.0, 1.0) * 1.5
}

pub fn unmap_feedback(v: f32) -> f32 {
    (v / 1.5).clamp(0.0, 1.0)
}

pub fn map_level(u: f32) -> f32 {
    // Audio-taper: gentle near the bottom, 1.25 max like the level params.
    let u = u.clamp(0.0, 1.0);
    u * u.sqrt() * 1.25
}

pub fn unmap_level(v: f32) -> f32 {
    ((v / 1.25).clamp(0.0, 1.0)).powf(2.0 / 3.0)
}

pub fn map_unit(u: f32) -> f32 {
    u.clamp(0.0, 1.0)
}

pub fn unmap_unit(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// One set's drift-slewed normalized value.
#[derive(Debug, Clone, Copy, Default)]
struct SetSlew {
    value: f32,
    initialized: bool,
}

impl SetSlew {
    #[inline]
    fn next(&mut self, target: f32, coeff: f32) -> f32 {
        if !self.initialized {
            self.value = target;
            self.initialized = true;
        }
        self.value += coeff * (target - self.value);
        self.value
    }
}

pub struct Sequencer {
    sample_rate: f64,
    config: SeqConfig,

    /// Cycle phase 0..1 within the current step.
    phase: f64,
    /// Current cycle position index 0-based (position = idx + 1).
    cycle_idx: u8,

    white: SetSlew,
    gray: SetSlew,
    black: SetSlew,
    white_coeff: f32,
    gray_coeff: f32,
    black_coeff: f32,

    // Anomaly pulse state.
    anomaly_pos: f64,
    anomaly_len: f64,
    anomaly_amp: f64,
}

impl Sequencer {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            config: SeqConfig::default(),
            phase: 0.0,
            cycle_idx: 0,
            white: SetSlew::default(),
            gray: SetSlew::default(),
            black: SetSlew::default(),
            white_coeff: 1.0,
            gray_coeff: 1.0,
            black_coeff: 1.0,
            anomaly_pos: 0.0,
            anomaly_len: 0.0,
            anomaly_amp: 0.0,
        }
    }

    pub fn set_config(&mut self, config: &SeqConfig) {
        let was_running = self.config.cycle_run;
        self.config = *config;
        if config.cycle_run && !was_running {
            self.phase = 0.0;
            self.cycle_idx = 0;
        }
        // Host-clock lock: land exactly where the DAW playhead says we are.
        // The per-sample clock still advances within the block; this snap at
        // block rate is what keeps steps on the grid through loops, jumps
        // and tempo changes.
        if let (true, Some(step_pos)) = (config.cycle_run, config.host_step_pos) {
            let len = config.cycle_len.clamp(1, 8);
            let step_pos = step_pos.max(0.0);
            let idx = (step_pos.floor() as u64 % len as u64) as u8;
            self.phase = step_pos.fract();
            if idx != self.cycle_idx {
                self.cycle_idx = idx;
                // Landing on the final step counts as entering it.
                if config.anomaly_polarity != AnomalyPolarity::Off && idx == len - 1 {
                    self.trigger_anomaly();
                }
            }
        }
        let coeff = |drift_s: f32| -> f32 {
            if drift_s <= 1e-3 {
                1.0
            } else {
                // Slew settles in roughly the drift time (one-pole, tau =
                // drift/4 gives the "logarhythmic" approach of the spec).
                let tau = (drift_s / 4.0).max(1e-4) as f64;
                (1.0 - (-1.0 / (tau * self.sample_rate)).exp()) as f32
            }
        };
        self.white_coeff = coeff(config.white_drift);
        self.gray_coeff = coeff(config.gray_drift);
        self.black_coeff = coeff(config.black_drift);
    }

    /// Current position, 1..=8.
    pub fn position(&self) -> u8 {
        if self.config.cycle_run {
            self.cycle_idx + 1
        } else {
            self.config.manual_position.clamp(1, 8)
        }
    }

    /// Normalized fader/panel value for a position within a set.
    #[inline]
    fn value_u(faders: &[f32; 7], panel_u: f32, position: u8) -> f32 {
        if position <= 1 {
            panel_u
        } else {
            faders[(position - 2) as usize]
        }
    }

    /// Advance one sample.
    #[inline]
    pub fn process(&mut self) -> SeqOut {
        // Cycle clock, sample-accurate.
        if self.config.cycle_run {
            let len = self.config.cycle_len.clamp(1, 8);
            self.phase += self.config.cycle_rate as f64 / self.sample_rate;
            while self.phase >= 1.0 {
                self.phase -= 1.0;
                let prev = self.cycle_idx;
                self.cycle_idx = (self.cycle_idx + 1) % len;
                // Entering the final step of the cycle fires the anomaly.
                if self.config.anomaly_polarity != AnomalyPolarity::Off
                    && self.cycle_idx == len - 1
                    && prev != self.cycle_idx
                {
                    self.trigger_anomaly();
                }
            }
            if self.cycle_idx >= len {
                self.cycle_idx = 0;
            }
        }
        let cfg = &self.config;

        let position = self.position();

        // Anomaly envelope: raised-cosine speed blip.
        let anomaly_speed = if self.anomaly_len > 0.0 {
            self.anomaly_pos += 1.0;
            let t = self.anomaly_pos / self.anomaly_len;
            if t >= 1.0 {
                self.anomaly_len = 0.0;
                0.0
            } else {
                let win = 0.5 - 0.5 * (std::f64::consts::TAU * t).cos();
                self.anomaly_amp * win
            }
        } else {
            0.0
        };

        let mut out = SeqOut {
            position,
            anomaly_speed,
            ..Default::default()
        };

        if cfg.white_on {
            let target = Self::value_u(&cfg.white_faders, cfg.panel_time_u, position);
            // Use the matching panel value for whichever target is selected.
            let target = match cfg.white_target {
                WhiteTarget::Time => target,
                WhiteTarget::Resonance => {
                    Self::value_u(&cfg.white_faders, cfg.panel_res_u, position)
                }
                WhiteTarget::ModSpeed => {
                    Self::value_u(&cfg.white_faders, cfg.panel_mod_spd_u, position)
                }
            };
            let u = self.white.next(target, self.white_coeff);
            match cfg.white_target {
                WhiteTarget::Time => out.time_s = Some(map_time_s(u)),
                WhiteTarget::Resonance => out.res = Some(map_unit(u)),
                WhiteTarget::ModSpeed => out.mod_spd_hz = Some(map_mod_spd_hz(u)),
            }
        }

        if cfg.gray_on {
            let target = match cfg.gray_target {
                GrayTarget::Feedback => {
                    Self::value_u(&cfg.gray_faders, cfg.panel_fdbk_u, position)
                }
                GrayTarget::ModAmount => {
                    Self::value_u(&cfg.gray_faders, cfg.panel_mod_amt_u, position)
                }
                GrayTarget::Lpf => Self::value_u(&cfg.gray_faders, cfg.panel_lpf_u, position),
            };
            let u = self.gray.next(target, self.gray_coeff);
            match cfg.gray_target {
                GrayTarget::Feedback => out.feedback = Some(map_feedback(u)),
                GrayTarget::ModAmount => out.mod_amt = Some(map_unit(u)),
                GrayTarget::Lpf => out.lpf_hz = Some(map_lpf_hz(u)),
            }
        }

        if cfg.black_on {
            let target = match cfg.black_target {
                BlackTarget::TapeLevel => {
                    Self::value_u(&cfg.black_faders, cfg.panel_tape_lvl_u, position)
                }
                BlackTarget::DryLevel => {
                    Self::value_u(&cfg.black_faders, cfg.panel_dry_lvl_u, position)
                }
                BlackTarget::Hpf => Self::value_u(&cfg.black_faders, cfg.panel_hpf_u, position),
            };
            let u = self.black.next(target, self.black_coeff);
            match cfg.black_target {
                BlackTarget::TapeLevel => out.tape_level = Some(map_level(u)),
                BlackTarget::DryLevel => out.dry_level = Some(map_level(u)),
                BlackTarget::Hpf => out.hpf_hz = Some(map_hpf_hz(u)),
            }
        }

        out
    }

    fn trigger_anomaly(&mut self) {
        let amt = self.config.anomaly_amount.clamp(0.0, 1.0) as f64;
        if amt <= 0.0 {
            return;
        }
        // Quick blip -> long singular wobble.
        let len_s = 0.008 + 0.4 * amt * amt;
        self.anomaly_len = len_s * self.sample_rate;
        self.anomaly_pos = 0.0;
        let amp = 0.005 + 0.20 * amt;
        self.anomaly_amp = match self.config.anomaly_polarity {
            AnomalyPolarity::Plus => amp,
            AnomalyPolarity::Minus => -amp,
            AnomalyPolarity::Off => 0.0,
        };
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.cycle_idx = 0;
        self.white = SetSlew::default();
        self.gray = SetSlew::default();
        self.black = SetSlew::default();
        self.anomaly_len = 0.0;
        self.anomaly_pos = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_rotates_within_length() {
        let mut seq = Sequencer::new(48_000.0);
        seq.set_config(&SeqConfig {
            cycle_run: true,
            cycle_len: 3,
            cycle_rate: 100.0,
            ..Default::default()
        });
        let mut seen = [false; 9];
        for _ in 0..48_000 {
            let out = seq.process();
            seen[out.position as usize] = true;
        }
        assert!(seen[1] && seen[2] && seen[3], "positions 1-3 should cycle");
        assert!(!seen[4..].iter().any(|&s| s), "positions beyond len visited");
    }

    #[test]
    fn audio_rate_stepping_is_sample_accurate() {
        // 2-step cycle at 1000 steps/s = 500 Hz square on the target value.
        let mut seq = Sequencer::new(48_000.0);
        let mut cfg = SeqConfig {
            cycle_run: true,
            cycle_len: 2,
            cycle_rate: 1_000.0,
            black_on: true,
            black_target: BlackTarget::TapeLevel,
            ..Default::default()
        };
        cfg.black_faders = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        cfg.panel_tape_lvl_u = 0.0;
        seq.set_config(&cfg);

        // Count level transitions over 1 second: 1000 steps/s = 1000 flips.
        let mut flips = 0u32;
        let mut prev = None;
        for _ in 0..48_000 {
            let out = seq.process();
            let level = out.tape_level.unwrap();
            if let Some(p) = prev {
                if (level - p as f32).abs() > 0.3 {
                    flips += 1;
                }
            }
            prev = Some(level);
        }
        assert!(
            (980..=1020).contains(&flips),
            "expected ~1000 flips/s at 1000 steps/s, got {flips}"
        );
    }

    #[test]
    fn drift_smooths_steps() {
        let measure_jump = |drift: f32| {
            let mut seq = Sequencer::new(48_000.0);
            let mut cfg = SeqConfig {
                cycle_run: true,
                cycle_len: 2,
                cycle_rate: 4.0,
                gray_on: true,
                gray_target: GrayTarget::Feedback,
                gray_drift: drift,
                ..Default::default()
            };
            cfg.gray_faders = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            cfg.panel_fdbk_u = 0.0;
            seq.set_config(&cfg);
            let mut max_step = 0.0f32;
            let mut prev = 0.0f32;
            for k in 0..48_000 {
                let out = seq.process();
                let v = out.feedback.unwrap();
                if k > 0 {
                    max_step = max_step.max((v - prev).abs());
                }
                prev = v;
            }
            max_step
        };
        let snap = measure_jump(0.0);
        let slewed = measure_jump(2.0);
        assert!(snap > 0.5, "drift 0 should snap: max step {snap}");
        assert!(
            slewed < 0.001,
            "drift 2s should glide smoothly: max step {slewed}"
        );
    }

    #[test]
    fn host_sync_locks_position_to_the_playhead() {
        let mut seq = Sequencer::new(48_000.0);
        let mut cfg = SeqConfig {
            cycle_run: true,
            cycle_len: 4,
            cycle_rate: 2.0,
            ..Default::default()
        };
        // Free-run somewhere arbitrary first.
        seq.set_config(&cfg);
        for _ in 0..10_000 {
            seq.process();
        }

        // Host says we're 6.25 steps into the song: 6 % 4 = step index 2,
        // position 3, a quarter of the way through the step.
        cfg.host_step_pos = Some(6.25);
        seq.set_config(&cfg);
        assert_eq!(seq.position(), 3, "position must land on the host grid");

        // A later block reports the playhead jumped back (loop region):
        // the cycle follows instead of free-running past it.
        cfg.host_step_pos = Some(0.5);
        seq.set_config(&cfg);
        assert_eq!(seq.position(), 1, "loop jump must re-lock the cycle");

        // Between blocks the per-sample clock still advances: half a step
        // at 2 steps/s = 0.25 s to the next boundary.
        let mut steps_seen = vec![seq.position()];
        for _ in 0..(48_000 / 2) {
            let out = seq.process();
            if *steps_seen.last().unwrap() != out.position {
                steps_seen.push(out.position);
            }
        }
        assert_eq!(
            steps_seen,
            vec![1, 2],
            "per-sample clock should advance to the next step between snaps"
        );
    }

    #[test]
    fn anomaly_fires_on_final_step_only() {
        let mut seq = Sequencer::new(48_000.0);
        seq.set_config(&SeqConfig {
            cycle_run: true,
            cycle_len: 4,
            cycle_rate: 8.0, // 0.5 s per full cycle
            anomaly_polarity: AnomalyPolarity::Plus,
            anomaly_amount: 0.8,
            ..Default::default()
        });
        // Record where anomalies begin.
        let mut bursts: Vec<(u8, usize)> = Vec::new();
        let mut prev_speed = 0.0f64;
        for k in 0..(4 * 48_000) {
            let out = seq.process();
            if prev_speed == 0.0 && out.anomaly_speed > 0.0 {
                bursts.push((out.position, k));
            }
            prev_speed = out.anomaly_speed;
        }
        assert!(bursts.len() >= 6, "anomaly should fire each cycle: {bursts:?}");
        assert!(
            bursts.iter().all(|&(pos, _)| pos == 4),
            "anomaly must fire on the final step: {bursts:?}"
        );
    }
}
