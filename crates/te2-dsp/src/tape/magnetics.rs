//! Tape magnetization: Jiles-Atherton hysteresis solved with RK4 at the
//! oversampled rate, after Chowdhury, "Real-Time Physical Modelling for
//! Analog Tape Machines" (DAFx-19) — the CHOW Tape approach.
//!
//! Physics note on bias: with proper AC bias, recorded magnetization follows
//! the *anhysteretic* curve M_an(H) — a clean Langevin saturator. The full
//! J-A loop describes the under-biased response (crossover grit, remanence,
//! HF self-erasure). `bias` blends the two: 1.0 = ideally biased and clean,
//! lower = progressively under-biased. Tape types set their own defaults and
//! the mechanism-condition trim can sag the bias of a worn machine.
//!
//! All quantities are normalized: M_s = 1, signal -> field via `drive`.

/// J-A parameters relative to M_s = 1. Ratios from the DAFx-19 paper's tape
/// parameter set (M_s 3.5e5, a 2.2e4, k 2.7e4, c 0.17, alpha 1.6e-3).
#[derive(Debug, Clone, Copy)]
pub struct MagParams {
    /// Anhysteretic shape parameter (knee softness).
    pub a: f64,
    /// Mean-field coupling.
    pub alpha: f64,
    /// Coercivity (hysteresis loop width).
    pub k: f64,
    /// Reversible fraction.
    pub c: f64,
    /// Signal -> field scale: how hard 0 dBFS hits the tape.
    pub drive: f64,
    /// 1.0 = ideal AC bias (anhysteretic), 0.0 = fully under-biased (raw J-A).
    pub bias: f64,
    /// Output normalization so small signals pass at unity gain.
    pub out_norm: f64,
}

impl MagParams {
    /// Baseline ferric (Type I) profile. The drive scale puts 0 dBFS at
    /// Q ~= 4 on the Langevin curve: clearly saturated, not brickwalled.
    pub fn type_i() -> Self {
        let a = 2.2e4 / 3.5e5;
        Self {
            a,
            alpha: 1.6e-3,
            k: 2.7e4 / 3.5e5,
            c: 0.17,
            drive: 4.0 * a,
            bias: 0.82,
            out_norm: 1.0, // fixed up by `normalized()`
        }
    }

    /// Chrome (Type II): higher coercivity, more headroom, cleaner top.
    pub fn type_ii() -> Self {
        let base = Self::type_i();
        Self {
            k: base.k * 1.25,
            drive: base.drive * 0.78,
            bias: 0.88,
            ..base
        }
    }

    /// Metal (Type IV): the most headroom, stays composed when slammed.
    pub fn type_iv() -> Self {
        let base = Self::type_i();
        Self {
            k: base.k * 1.5,
            drive: base.drive * 0.6,
            bias: 0.92,
            ..base
        }
    }

    /// Compute `out_norm` so the small-signal gain of the *blended* output is
    /// unity. The hysteresis branch has lower small-signal susceptibility
    /// than the anhysteretic branch, so this is measured empirically with a
    /// short simulation. Costs ~10k iterations: call from `initialize()`,
    /// never from the audio thread.
    pub fn normalized(mut self) -> Self {
        self.out_norm = 1.0;
        let mut hyst = Hysteresis::default();
        let os_rate = 192_000.0;
        let dt = 1.0 / os_rate;
        let freq = 1_000.0;
        let amp = 0.05f64;
        let n = (os_rate / freq) as usize * 10;

        let mut in_sq = 0.0f64;
        let mut out_sq = 0.0f64;
        for k in 0..n {
            let x = amp * (std::f64::consts::TAU * freq * k as f64 / os_rate).sin();
            let y = hyst.process(&self, x as f32, dt) as f64;
            if k >= n / 2 {
                in_sq += x * x;
                out_sq += y * y;
            }
        }
        let gain = (out_sq / in_sq.max(1e-30)).sqrt();
        self.out_norm = 1.0 / gain.max(1e-6);
        self
    }
}

/// Langevin function and derivative, numerically safe near zero.
#[inline]
fn langevin(x: f64) -> f64 {
    if x.abs() < 1e-4 {
        x / 3.0 - x * x * x / 45.0
    } else {
        1.0 / x.tanh() - 1.0 / x
    }
}

#[inline]
fn langevin_deriv(x: f64) -> f64 {
    if x.abs() < 1e-4 {
        1.0 / 3.0 - x * x / 15.0
    } else {
        let coth = 1.0 / x.tanh();
        1.0 - coth * coth + 1.0 / (x * x)
    }
}

/// Per-channel hysteresis state. Call `process()` once per oversampled
/// sample.
#[derive(Debug, Clone, Default)]
pub struct Hysteresis {
    /// J-A magnetization state.
    m: f64,
    /// Previous field value.
    h_prev: f64,
    /// Anhysteretic implicit state (for the mean-field iteration).
    m_an_prev: f64,
}

impl Hysteresis {
    /// J-A dM/dt as a function of state and field slew, eq. (5) of DAFx-19.
    #[inline]
    fn dm_dt(p: &MagParams, m: f64, h: f64, dh_dt: f64) -> f64 {
        let q = (h + p.alpha * m) / p.a;
        let m_an = langevin(q);
        let l_prime = langevin_deriv(q);
        let delta = if dh_dt >= 0.0 { 1.0 } else { -1.0 };
        let diff = m_an - m;
        // delta_M kills the irreversible term when it would move against the
        // field direction (no negative susceptibility).
        let delta_m = if diff * delta >= 0.0 { 1.0 } else { 0.0 };

        let denom_irr = (1.0 - p.c) * delta * p.k - p.alpha * diff;
        let irr = if denom_irr.abs() < 1e-12 {
            0.0
        } else {
            (1.0 - p.c) * delta_m * diff / denom_irr
        };
        let rev = p.c * l_prime / p.a;
        let denom = 1.0 - p.c * p.alpha * l_prime / p.a;
        ((irr + rev) / denom.max(1e-9)) * dh_dt
    }

    /// Process one oversampled sample through the magnetic model.
    /// `dt` is the oversampled sample period (1 / (factor * sample_rate)).
    #[inline]
    pub fn process(&mut self, p: &MagParams, x: f32, dt: f64) -> f32 {
        let h = x as f64 * p.drive;
        let dh_dt = (h - self.h_prev) / dt;
        let h_half = 0.5 * (h + self.h_prev);

        // RK4 on the J-A ODE.
        let m = self.m;
        let k1 = dt * Self::dm_dt(p, m, self.h_prev, dh_dt);
        let k2 = dt * Self::dm_dt(p, m + 0.5 * k1, h_half, dh_dt);
        let k3 = dt * Self::dm_dt(p, m + 0.5 * k2, h_half, dh_dt);
        let k4 = dt * Self::dm_dt(p, m + k3, h, dh_dt);
        let mut m_next = m + (k1 + 2.0 * k2 + 2.0 * k3 + k4) / 6.0;
        if !m_next.is_finite() {
            m_next = 0.0;
        }
        // The physical state can never exceed saturation.
        self.m = m_next.clamp(-1.0, 1.0);
        self.h_prev = h;

        // Anhysteretic path (ideally biased recording): solve
        // M = L((H + alpha*M)/a) by fixed point from the previous value
        // (alpha is tiny, one refinement is plenty).
        let mut m_an = langevin((h + p.alpha * self.m_an_prev) / p.a);
        m_an = langevin((h + p.alpha * m_an) / p.a);
        self.m_an_prev = m_an;

        let blended = p.bias * m_an + (1.0 - p.bias) * self.m;
        (blended * p.out_norm) as f32
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: run a sine at the given amplitude through the model at 4x 48k
    /// and return (gain, thd) measured over whole cycles.
    fn measure(p: &MagParams, amp: f64, freq: f64) -> (f64, f64) {
        let os_rate = 192_000.0;
        let dt = 1.0 / os_rate;
        let mut hyst = Hysteresis::default();
        let cycles = 40.0;
        let n = (os_rate / freq * cycles) as usize;

        // Goertzel at the fundamental + total power for THD.
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            let x = amp * (std::f64::consts::TAU * freq * k as f64 / os_rate).sin();
            out.push(hyst.process(p, x as f32, dt) as f64);
        }
        let settle = n / 2;
        let tail = &out[settle..];
        let len = tail.len() as f64;

        let mut re = 0.0;
        let mut im = 0.0;
        let mut power = 0.0;
        let mut mean = 0.0;
        for (k, &y) in tail.iter().enumerate() {
            let ph = std::f64::consts::TAU * freq * k as f64 / os_rate;
            re += y * ph.cos();
            im += y * ph.sin();
            power += y * y;
            mean += y;
        }
        mean /= len;
        let fund_amp = 2.0 * (re * re + im * im).sqrt() / len;
        let fund_power = fund_amp * fund_amp / 2.0;
        let total_power = power / len - mean * mean;
        let harm_power = (total_power - fund_power).max(0.0);
        let thd = (harm_power / fund_power).sqrt();
        (fund_amp / amp, thd)
    }

    #[test]
    fn small_signal_unity_and_clean() {
        let p = MagParams::type_i().normalized();
        let (gain, thd) = measure(&p, 0.03, 1000.0); // ~-30 dBFS
        assert!(
            (gain - 1.0).abs() < 0.1,
            "small-signal gain should be ~1.0, got {gain:.3}"
        );
        assert!(thd < 0.02, "small-signal THD too high: {:.2}%", thd * 100.0);
    }

    #[test]
    fn saturation_compresses_and_distorts() {
        let p = MagParams::type_i().normalized();
        let (gain_low, _) = measure(&p, 0.05, 1000.0);
        let (gain_hot, thd_hot) = measure(&p, 1.0, 1000.0); // 0 dBFS
        assert!(
            gain_hot < gain_low * 0.75,
            "0 dBFS should compress: low {gain_low:.3} hot {gain_hot:.3}"
        );
        assert!(
            thd_hot > 0.03,
            "0 dBFS should distort audibly: THD {:.2}%",
            thd_hot * 100.0
        );
        // ... but the output stays bounded (tape, not a fuzz pedal).
        assert!(gain_hot * 1.0 < 1.2, "output exploded: {gain_hot:.3}");
    }

    #[test]
    fn tape_types_have_increasing_headroom() {
        let amp = 0.7;
        let thd = |p: MagParams| measure(&p.normalized(), amp, 1000.0).1;
        let t1 = thd(MagParams::type_i());
        let t2 = thd(MagParams::type_ii());
        let t4 = thd(MagParams::type_iv());
        assert!(
            t1 > t2 && t2 > t4,
            "headroom order wrong: I {t1:.4} II {t2:.4} IV {t4:.4}"
        );
    }

    #[test]
    fn under_bias_adds_hysteresis_grit() {
        let mut clean = MagParams::type_i().normalized();
        clean.bias = 1.0;
        let mut dirty = clean;
        dirty.bias = 0.4;
        let (_, thd_clean) = measure(&clean, 0.12, 1000.0);
        let (_, thd_dirty) = measure(&dirty, 0.12, 1000.0);
        assert!(
            thd_dirty > thd_clean * 1.5,
            "under-bias should add low-level distortion: clean {thd_clean:.4} dirty {thd_dirty:.4}"
        );
    }

    #[test]
    fn state_stays_finite_under_abuse() {
        let p = MagParams::type_i().normalized();
        let mut hyst = Hysteresis::default();
        let dt = 1.0 / 192_000.0;
        let mut rng = 0x2545F491u32;
        for _ in 0..200_000 {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = ((rng >> 8) as f32 / (1 << 23) as f32 - 1.0) * 8.0; // +18 dB abuse
            let y = hyst.process(&p, x, dt);
            assert!(y.is_finite());
            assert!(y.abs() < 4.0, "runaway output {y}");
        }
    }
}
