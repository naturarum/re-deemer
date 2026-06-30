//! 24 dB/oct OTA cascade filters in zero-delay-feedback (TPT) form — the
//! "classic Roland 4-poles of the late 70s" the TE-2 spec calls for.
//!
//! The lowpass closes global negative feedback around four one-pole stages;
//! k = 4 is the linear self-oscillation threshold and the tanh on the loop
//! input bounds oscillation into a clean sine whose frequency tracks the
//! cutoff (that's what makes the RES-gate filter-synth playable). The
//! highpass is the same cascade without resonance.

/// One TPT one-pole state. y = G*x + (1-G)*s with G = g/(1+g), g = tan(pi*fc/fs).
#[derive(Debug, Clone, Copy, Default)]
struct Stage {
    s: f32,
}

impl Stage {
    /// Lowpass output; updates state.
    #[inline]
    fn lp(&mut self, x: f32, big_g: f32) -> f32 {
        let v = (x - self.s) * big_g;
        let y = v + self.s;
        self.s = y + v;
        y
    }
}

/// OTA input-stage thermal noise, peak linear (~-90 dBFS). Inaudible under
/// any program material, but it's what lets the filter self-oscillate from
/// true silence — a real OTA sings at full resonance even with no input,
/// it doesn't wait for tape hiss to arrive and seed it.
const THERMAL_NOISE: f32 = 3.0e-5;

/// Resonance low-end compensation, 0..1 (production default). The ladder's
/// `−k·y4` feedback drains the passband — the lows especially — by ~1/(1+k) as
/// resonance rises (the classic Moog bass loss), which makes big regenerating
/// feedbacks hard to build. Scaling the loop *input* by `(1 + comp·k)` pushes the
/// passband gain back toward unity: 0.0 = the bare ladder (most bass loss,
/// original voicing), 1.0 = fully restored (flat passband, least character). We
/// keep it partial — enough low end for feedback to bloom without flattening the
/// filter. Only the program input is scaled, so self-oscillation (which builds
/// from x≈0) is completely unaffected. This default is the shipped value across
/// all products; the offline render harness can override it per-instance via
/// `set_comp` to A/B candidates by ear.
///
/// **0.3** chosen by ear (subtle — lows return and feedback breathes while the
/// OTA/Moog voicing stays put). Measured ~2.5× more sub-150 Hz energy in a
/// regenerating echo at moderate resonance vs the bare ladder.
const DEFAULT_RES_COMP: f32 = 0.3;

/// 4-pole OTA lowpass with resonance to self-oscillation.
pub struct OtaLowpass {
    stages: [Stage; 4],
    big_g: f32,
    /// Feedback amount, 0..~4.3 (4.0 = oscillation threshold).
    k: f32,
    /// Low-end compensation amount (see `DEFAULT_RES_COMP`). Separate from `set`
    /// so re-setting cutoff/res never disturbs it.
    comp: f32,
    rng: u32,
}

impl Default for OtaLowpass {
    fn default() -> Self {
        Self {
            stages: [Stage::default(); 4],
            big_g: 0.5,
            k: 0.0,
            comp: DEFAULT_RES_COMP,
            rng: 0x6A09_E667,
        }
    }
}

impl OtaLowpass {
    /// `res` 0..1; oscillation onset around 0.93.
    pub fn set(&mut self, sample_rate: f32, fc: f32, res: f32) {
        let fc = fc.clamp(20.0, sample_rate * 0.45);
        let g = (std::f32::consts::PI * fc / sample_rate).tan();
        self.big_g = g / (1.0 + g);
        self.k = res.clamp(0.0, 1.0) * 4.3;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let big_g = self.big_g;
        let g2 = big_g * big_g;
        let g4 = g2 * g2;

        // Linear ZDF prediction of y4 to resolve the instantaneous loop.
        let b = |st: &Stage| (1.0 - big_g) * st.s;
        let sigma = ((b(&self.stages[0]) * big_g + b(&self.stages[1])) * big_g
            + b(&self.stages[2]))
            * big_g
            + b(&self.stages[3]);
        // Low-end compensation: scale the loop input so the resonance feedback
        // drains the passband less (see DEFAULT_RES_COMP). Input-only — at x≈0
        // this is a no-op, so self-oscillation is unchanged. The ZDF prediction
        // uses the same compensated input to stay consistent.
        let xc = x * (1.0 + self.comp * self.k);
        let y4_lin = (g4 * xc + sigma) / (1.0 + self.k * g4);

        // OTA input stage saturates the closed loop — this is what bounds
        // self-oscillation into a sine. Its thermal noise rides along.
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let thermal = ((self.rng >> 8) as f32 / (1 << 23) as f32 - 1.0) * THERMAL_NOISE;
        let u = fast_tanh(xc + thermal - self.k * y4_lin);

        let y1 = self.stages[0].lp(u, big_g);
        let y2 = self.stages[1].lp(y1, big_g);
        let y3 = self.stages[2].lp(y2, big_g);
        self.stages[3].lp(y3, big_g)
    }

    pub fn reset(&mut self) {
        self.stages = [Stage::default(); 4];
    }

    /// Override the low-end compensation amount (see `DEFAULT_RES_COMP`). Dev/
    /// tuning only — production uses the default; not wired to any parameter.
    pub fn set_comp(&mut self, comp: f32) {
        self.comp = comp;
    }

    /// Tiny excitation so self-oscillation starts reliably from silence.
    pub fn ping(&mut self) {
        self.stages[0].s += 1e-4;
    }
}

/// 4-pole OTA highpass (no resonance on the TE-2).
pub struct OtaHighpass {
    stages: [Stage; 4],
    big_g: f32,
}

impl Default for OtaHighpass {
    fn default() -> Self {
        Self {
            stages: [Stage::default(); 4],
            big_g: 0.5,
        }
    }
}

impl OtaHighpass {
    pub fn set(&mut self, sample_rate: f32, fc: f32) {
        let fc = fc.clamp(10.0, sample_rate * 0.45);
        let g = (std::f32::consts::PI * fc / sample_rate).tan();
        self.big_g = g / (1.0 + g);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let mut y = x;
        for stage in &mut self.stages {
            y -= stage.lp(y, self.big_g);
        }
        y
    }

    pub fn reset(&mut self) {
        self.stages = [Stage::default(); 4];
    }
}

#[inline]
fn fast_tanh(x: f32) -> f32 {
    // Padé-ish rational tanh, accurate to ~1e-4 over +/-4, hard clamp beyond.
    let x = x.clamp(-4.0, 4.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Low-end compensation lifts sub-cutoff energy at high resonance without
    /// touching self-oscillation. Guards the COMP feature against regressions.
    #[test]
    fn compensation_restores_low_end() {
        let sr = 48_000.0f64;
        let low_rms = |comp: f32| {
            let mut f = OtaLowpass::default();
            f.set(sr as f32, 1500.0, 0.9); // k = 0.9*4.3 = 3.87, lows heavily drained
            f.set_comp(comp);
            let (mut sumsq, mut cnt) = (0.0f64, 0usize);
            for n in 0..(sr as usize) {
                let x = (std::f64::consts::TAU * 100.0 * n as f64 / sr).sin() as f32 * 0.3;
                let y = f.process(x);
                if n > sr as usize / 2 {
                    sumsq += (y as f64).powi(2);
                    cnt += 1;
                }
            }
            (sumsq / cnt as f64).sqrt()
        };
        let bare = low_rms(0.0);
        let comped = low_rms(0.5);
        assert!(
            comped > bare * 1.8,
            "compensation should lift 100 Hz at high res: bare={bare:.5} comped={comped:.5}"
        );
    }

    fn tone_response(filter: &mut OtaLowpass, sr: f64, freq: f64) -> f64 {
        let n = (sr * 0.5) as usize;
        let mut sum_sq = 0.0f64;
        let mut count = 0usize;
        for k in 0..n {
            let x = (std::f64::consts::TAU * freq * k as f64 / sr).sin() as f32;
            let y = filter.process(x * 0.05) as f64; // small signal: stay linear
            if k > n / 2 {
                sum_sq += y * y;
                count += 1;
            }
        }
        (sum_sq / count as f64).sqrt() / (0.05 / std::f64::consts::SQRT_2)
    }

    #[test]
    fn lowpass_is_24_db_per_octave() {
        let sr = 48_000.0;
        let mut f = OtaLowpass::default();
        f.set(sr as f32, 1_000.0, 0.0);
        let pass = 20.0 * tone_response(&mut f, sr, 100.0).log10();
        f.reset();
        let two_oct = 20.0 * tone_response(&mut f, sr, 4_000.0).log10();
        let drop = pass - two_oct;
        assert!(
            (40.0..56.0).contains(&drop),
            "2-octave drop {drop:.1} dB, expected ~48"
        );
    }

    #[test]
    fn self_oscillation_pitch_tracks_cutoff() {
        let sr = 48_000.0f64;
        for fc in [220.0f32, 440.0, 1760.0] {
            let mut f = OtaLowpass::default();
            f.set(sr as f32, fc, 1.0);
            f.ping();
            // Let oscillation build.
            for _ in 0..(sr as usize) {
                f.process(0.0);
            }
            let mut crossings = 0u32;
            let mut prev = 0.0f32;
            let n = sr as usize;
            for _ in 0..n {
                let y = f.process(0.0);
                if prev <= 0.0 && y > 0.0 {
                    crossings += 1;
                }
                prev = y;
            }
            let freq = crossings as f64;
            let cents = 1200.0 * (freq / fc as f64).log2();
            assert!(
                cents.abs() < 30.0,
                "self-osc at fc={fc}: measured {freq:.1} Hz ({cents:+.0} cents)"
            );
        }
    }

    #[test]
    fn self_oscillation_amplitude_bounded() {
        let mut f = OtaLowpass::default();
        f.set(48_000.0, 800.0, 1.0);
        f.ping();
        let mut peak = 0.0f32;
        for _ in 0..96_000 {
            peak = peak.max(f.process(0.0).abs());
        }
        assert!(peak > 0.1, "oscillation never built: peak {peak}");
        assert!(peak < 1.5, "oscillation unbounded: peak {peak}");
    }

    #[test]
    fn below_threshold_resonance_decays() {
        let mut f = OtaLowpass::default();
        f.set(48_000.0, 800.0, 0.7);
        f.ping();
        for _ in 0..48_000 {
            f.process(0.0);
        }
        let mut peak = 0.0f32;
        for _ in 0..24_000 {
            peak = peak.max(f.process(0.0).abs());
        }
        assert!(peak < 1e-3, "res 0.7 should not self-oscillate: {peak}");
    }

    #[test]
    fn highpass_cuts_lows() {
        let sr = 48_000.0;
        let mut f = OtaHighpass::default();
        f.set(sr as f32, 1_000.0);
        let measure = |f: &mut OtaHighpass, freq: f64| {
            let n = (sr * 0.5) as usize;
            let mut sum = 0.0f64;
            let mut cnt = 0usize;
            for k in 0..n {
                let x = (std::f64::consts::TAU * freq * k as f64 / sr).sin() as f32;
                let y = f.process(x) as f64;
                if k > n / 2 {
                    sum += y * y;
                    cnt += 1;
                }
            }
            (sum / cnt as f64).sqrt()
        };
        let hi = measure(&mut f, 8_000.0);
        f.reset();
        let lo = measure(&mut f, 250.0);
        let drop = 20.0 * (hi / lo).log10();
        assert!(drop > 40.0, "HPF 2-oct-below rejection only {drop:.1} dB");
    }
}
