//! The mechanism: always-on speed instability calibrated to real cassette
//! figures, plus dropouts. One instance per engine — there is one capstan —
//! with per-channel dropout generators (oxide differs per track).
//!
//! Components of the speed multiplier:
//! - wow: capstan once-around sine (~0.8 Hz) + bounded random drift
//! - flutter: incommensurate rotational rates (motor pole, pinch roller,
//!   idler) between ~6 and 30 Hz
//! - scrape flutter: tape-on-head friction noise, band-limited high-rate
//!   speed jitter — the "hairiness" most plugins miss
//!
//! `condition` 0..1 scales everything: 0.15 is a serviced deck, 0.35 a fair
//! used machine (default), 1.0 a thrift-store wreck. At the default the
//! combined weighted figure lands near 0.15% WRMS.

pub struct WowFlutter {
    sample_rate: f64,
    condition: f64,

    // Wow: capstan sine + random drift.
    wow_phase: f64,
    drift_state: f64,
    drift_lp_coeff: f64,

    // Flutter sines (phases advance at fixed incommensurate rates).
    flutter_phases: [f64; 4],

    // Scrape flutter: bandpassed noise on the speed signal.
    scrape_bp: [f64; 2],

    rng: u32,

    // Dropout state per channel: remaining samples and depth curve position.
    dropout_pos: [f64; 2],
    dropout_len: [f64; 2],
    dropout_depth: [f32; 2],
}

/// Flutter component rates in Hz (motor pole, pinch roller once-around,
/// idler, second harmonic of the roller — deliberately incommensurate).
const FLUTTER_RATES: [f64; 4] = [6.3, 10.7, 17.9, 29.4];
/// Relative amplitude of each flutter component.
const FLUTTER_AMPS: [f64; 4] = [1.0, 0.7, 0.45, 0.25];

impl WowFlutter {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            condition: 0.35,
            wow_phase: 0.0,
            drift_state: 0.0,
            drift_lp_coeff: 1.0 - (-std::f64::consts::TAU * 0.4 / sample_rate).exp(),
            flutter_phases: [0.0, 1.3, 2.9, 4.1],
            scrape_bp: [0.0; 2],
            rng: 0x9E3779B9,
            dropout_pos: [0.0; 2],
            dropout_len: [0.0; 2],
            dropout_depth: [0.0; 2],
        }
    }

    pub fn set_condition(&mut self, condition: f64) {
        self.condition = condition.clamp(0.0, 1.0);
    }

    #[inline]
    fn next_noise(&mut self) -> f64 {
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng >> 8) as f64 / (1 << 23) as f64 - 1.0
    }

    /// Advance one host sample. Returns (speed multiplier, [dropout gains]).
    #[inline]
    pub fn process(&mut self) -> (f64, [f32; 2]) {
        let c = self.condition;
        let dt = 1.0 / self.sample_rate;

        // Wow sine: +/-0.10% at default condition, growing to ~0.5%.
        let wow_depth = 1.0e-3 * (c / 0.35) * (1.0 + c);
        self.wow_phase += std::f64::consts::TAU * 0.83 * dt;
        if self.wow_phase >= std::f64::consts::TAU {
            self.wow_phase -= std::f64::consts::TAU;
        }
        let wow = wow_depth * self.wow_phase.sin();

        // Random drift: one-pole filtered noise, bounded by its own gain.
        let drift_target = self.next_noise() * 0.8e-3 * (c / 0.35);
        self.drift_state += self.drift_lp_coeff * (drift_target - self.drift_state);
        let drift = self.drift_state;

        // Flutter: stacked incommensurate sines, ~0.03% each at default.
        let flutter_depth = 0.30e-3 * (c / 0.35);
        let mut flutter = 0.0;
        for (i, rate) in FLUTTER_RATES.iter().enumerate() {
            self.flutter_phases[i] += std::f64::consts::TAU * rate * dt;
            if self.flutter_phases[i] >= std::f64::consts::TAU {
                self.flutter_phases[i] -= std::f64::consts::TAU;
            }
            flutter += FLUTTER_AMPS[i] * flutter_depth * self.flutter_phases[i].sin();
        }

        // Scrape flutter: noise bandpassed around ~2 kHz via a cheap
        // two-state resonator, very small depth.
        let noise = self.next_noise();
        let f_bp = 2_000.0 / self.sample_rate;
        let r = 0.995;
        let w = std::f64::consts::TAU * f_bp;
        let bp_in = noise - 2.0 * r * w.cos() * self.scrape_bp[0] - r * r * self.scrape_bp[1];
        let scrape_raw = bp_in - self.scrape_bp[1];
        self.scrape_bp[1] = self.scrape_bp[0];
        self.scrape_bp[0] = bp_in;
        let scrape = scrape_raw * 0.02e-3 * (c / 0.35);

        let speed_mult = 1.0 + wow + drift + flutter + scrape;

        // Dropouts: Poisson arrivals, raised-cosine dips 8-50 ms deep with
        // condition. Rare on a good deck, frequent on a wreck.
        let mut gains = [1.0f32; 2];
        let rate_per_sample = c * c * 0.4 * dt;
        for ch in 0..2 {
            if self.dropout_len[ch] > 0.0 {
                self.dropout_pos[ch] += 1.0;
                let t = self.dropout_pos[ch] / self.dropout_len[ch];
                if t >= 1.0 {
                    self.dropout_len[ch] = 0.0;
                } else {
                    // Raised cosine dip.
                    let dip = 0.5 - 0.5 * (std::f64::consts::TAU * t).cos();
                    gains[ch] = 1.0 - self.dropout_depth[ch] * dip as f32;
                }
            } else if self.next_noise().abs() < rate_per_sample {
                let len_ms = 8.0 + self.next_noise().abs() * 42.0;
                self.dropout_len[ch] = len_ms * 1e-3 * self.sample_rate;
                self.dropout_pos[ch] = 0.0;
                self.dropout_depth[ch] = (0.3 + 0.6 * self.next_noise().abs() * c) as f32;
            }
        }

        (speed_mult, gains)
    }

    pub fn reset(&mut self) {
        let condition = self.condition;
        *self = Self::new(self.sample_rate);
        self.condition = condition;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_deviation_in_cassette_range() {
        let mut wf = WowFlutter::new(48_000.0);
        wf.set_condition(0.35);
        let n = 48_000 * 10;
        let mut sum_sq = 0.0f64;
        for _ in 0..n {
            let (m, _) = wf.process();
            let dev = m - 1.0;
            sum_sq += dev * dev;
        }
        let rms_pct = (sum_sq / n as f64).sqrt() * 100.0;
        // Unweighted RMS speed deviation: cassette decks land roughly in
        // 0.05%..0.35%; we target the middle for a fair used machine.
        assert!(
            rms_pct > 0.05 && rms_pct < 0.4,
            "speed deviation {rms_pct:.3}% out of cassette range"
        );
    }

    #[test]
    fn mint_condition_is_steadier_than_wreck() {
        let measure = |cond: f64| {
            let mut wf = WowFlutter::new(48_000.0);
            wf.set_condition(cond);
            let n = 48_000 * 5;
            let mut sum_sq = 0.0f64;
            for _ in 0..n {
                let (m, _) = wf.process();
                sum_sq += (m - 1.0) * (m - 1.0);
            }
            (sum_sq / n as f64).sqrt()
        };
        let mint = measure(0.1);
        let wreck = measure(1.0);
        assert!(
            wreck > mint * 3.0,
            "condition scaling too weak: mint {mint:.6} wreck {wreck:.6}"
        );
    }
}
