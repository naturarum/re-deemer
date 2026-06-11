//! Polyphase halfband oversampling for the nonlinear stages (tape hysteresis,
//! output drive). 47-tap Kaiser (beta=9) halfband FIR: passband flat to
//! 0.18*fs, aliases suppressed >84 dB above 0.32*fs at each stage's rate.

/// Odd-index taps (k = 1, 3, 5, ... 23) of the 47-tap halfband; the center
/// tap is exactly 0.5 and all other even taps are zero.
const ODD_TAPS: [f32; 12] = [
    3.157_664_2e-1,
    -9.868_68e-2,
    5.197_895_7e-2,
    -3.042_467_5e-2,
    1.801_273_4e-2,
    -1.034_221_7e-2,
    5.596_731e-3,
    -2.776_692_9e-3,
    1.218_266_7e-3,
    -4.450_039e-4,
    1.181_470_4e-4,
    -1.265_518_2e-5,
]; // sums to ~0.25 per side; full DC gain = 1.0

/// Number of input-rate samples of history the odd branch needs.
const HIST: usize = 24;

/// One halfband stage: interpolate by 2 and decimate by 2.
#[derive(Clone)]
pub struct Halfband {
    /// Interpolator input history (input rate).
    up_hist: [f32; HIST],
    /// Decimator history of odd-phase inputs (high rate odd samples).
    down_odd_hist: [f32; HIST],
    /// Decimator center-branch delay line (high rate even samples).
    down_even_hist: [f32; HIST],
}

impl Default for Halfband {
    fn default() -> Self {
        Self {
            up_hist: [0.0; HIST],
            down_odd_hist: [0.0; HIST],
            down_even_hist: [0.0; HIST],
        }
    }
}

impl Halfband {
    /// One input sample in, two output samples (at 2x rate) out.
    /// Gain-compensated for interpolation (x2).
    ///
    /// With the center tap at the (odd) index 23, the zero-stuffed
    /// convolution splits so that even-phase outputs come from the FIR
    /// branch (the interpolated midpoint at input time n-11.5) and odd-phase
    /// outputs from the pure-delay branch (x[n-11]). Returned in time order.
    #[inline]
    pub fn up(&mut self, x: f32) -> [f32; 2] {
        self.up_hist.copy_within(1.., 0);
        self.up_hist[HIST - 1] = x;

        // FIR branch: pairs (x[n-12-i], x[n-11+i]) symmetric around n-11.5.
        let mut mid = 0.0f32;
        for (i, tap) in ODD_TAPS.iter().enumerate() {
            let a = self.up_hist[11 - i];
            let b = self.up_hist[12 + i];
            mid += tap * (a + b);
        }
        // Delay branch: x[n-11].
        let on_grid = self.up_hist[12];

        [2.0 * mid, on_grid]
    }

    /// Two input samples (at 2x rate, in time order) in, one output out.
    #[inline]
    pub fn down(&mut self, x: [f32; 2]) -> f32 {
        self.down_even_hist.copy_within(1.., 0);
        self.down_even_hist[HIST - 1] = x[0];
        self.down_odd_hist.copy_within(1.., 0);
        self.down_odd_hist[HIST - 1] = x[1];

        // Center tap picks the odd-phase stream; FIR taps the even phase.
        let center = 0.5 * self.down_odd_hist[11];
        let mut fir = 0.0f32;
        for (i, tap) in ODD_TAPS.iter().enumerate() {
            let a = self.down_even_hist[11 - i];
            let b = self.down_even_hist[12 + i];
            fir += tap * (a + b);
        }
        center + fir
    }

    pub fn reset(&mut self) {
        self.up_hist = [0.0; HIST];
        self.down_odd_hist = [0.0; HIST];
        self.down_even_hist = [0.0; HIST];
    }
}

/// Oversampling factor for the nonlinear stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsFactor {
    X2,
    X4,
    X8,
}

impl OsFactor {
    pub fn factor(self) -> usize {
        match self {
            OsFactor::X2 => 2,
            OsFactor::X4 => 4,
            OsFactor::X8 => 8,
        }
    }
}

/// Up to 8x oversampler built from cascaded halfband stages. The nonlinear
/// callback runs once per high-rate sample.
#[derive(Clone, Default)]
pub struct Oversampler {
    stage1: Halfband,
    stage2: Halfband,
    stage3: Halfband,
}

impl Oversampler {
    /// Run `f` at the oversampled rate around one host sample.
    #[inline]
    pub fn process(&mut self, factor: OsFactor, x: f32, mut f: impl FnMut(f32) -> f32) -> f32 {
        match factor {
            OsFactor::X2 => {
                let [a, b] = self.stage1.up(x);
                self.stage1.down([f(a), f(b)])
            }
            OsFactor::X4 => {
                let [a, b] = self.stage1.up(x);
                let [a0, a1] = self.stage2.up(a);
                let [b0, b1] = self.stage2.up(b);
                let ya = self.stage2.down([f(a0), f(a1)]);
                let yb = self.stage2.down([f(b0), f(b1)]);
                self.stage1.down([ya, yb])
            }
            OsFactor::X8 => {
                let [a, b] = self.stage1.up(x);
                let mut out2 = [0.0f32; 2];
                for (i, s) in [a, b].into_iter().enumerate() {
                    let [s0, s1] = self.stage2.up(s);
                    let [u0, u1] = self.stage3.up(s0);
                    let [u2, u3] = self.stage3.up(s1);
                    let y0 = self.stage3.down([f(u0), f(u1)]);
                    let y1 = self.stage3.down([f(u2), f(u3)]);
                    out2[i] = self.stage2.down([y0, y1]);
                }
                self.stage1.down(out2)
            }
        }
    }

    pub fn reset(&mut self) {
        self.stage1.reset();
        self.stage2.reset();
        self.stage3.reset();
    }

    /// Round-trip latency in host-rate samples. Each stage's up+down pair
    /// contributes 2 * 23 samples of group delay at its own output rate.
    pub fn latency(factor: OsFactor) -> f32 {
        match factor {
            OsFactor::X2 => 23.0,
            OsFactor::X4 => 23.0 + 11.5,
            OsFactor::X8 => 23.0 + 11.5 + 5.75,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_identity_snr() {
        // Identity nonlinearity: output must equal a delayed input to within
        // the filter's passband accuracy for an in-band tone.
        let sr = 48_000.0f64;
        let f = 4_000.0f64;
        let mut os = Oversampler::default();
        let latency = Oversampler::latency(OsFactor::X4) as f64;

        let n = 8192;
        let mut err = 0.0f64;
        let mut sig = 0.0f64;
        for k in 0..n {
            let x = (std::f64::consts::TAU * f * k as f64 / sr).sin() as f32;
            let y = os.process(OsFactor::X4, x, |s| s) as f64;
            if k > 200 {
                let ideal = (std::f64::consts::TAU * f * (k as f64 - latency) / sr).sin();
                err += (y - ideal) * (y - ideal);
                sig += ideal * ideal;
            }
        }
        let snr = 10.0 * (sig / err.max(1e-30)).log10();
        assert!(snr > 70.0, "oversampler passthrough SNR {snr:.1} dB");
    }

    #[test]
    fn cubic_distortion_aliasing_suppressed() {
        // Drive a 15 kHz tone through x^3 (harmonic at 45 kHz). Without
        // oversampling it aliases to 3 kHz; with 4x it must be buried.
        let sr = 48_000.0f64;
        let f = 15_000.0f64;
        let mut os = Oversampler::default();
        let n = 16384usize;
        let mut buf = Vec::with_capacity(n);
        for k in 0..n {
            let x = (std::f64::consts::TAU * f * k as f64 / sr).sin() as f32;
            buf.push(os.process(OsFactor::X4, x, |s| s * s * s) as f64);
        }

        // Goertzel power at the alias frequency (3*15k - 48k = -3k -> 3 kHz)
        // vs the fundamental.
        let goertzel = |freq: f64| {
            let w = std::f64::consts::TAU * freq / sr;
            let coeff = 2.0 * w.cos();
            let (mut s0, mut s1, mut s2) = (0.0f64, 0.0, 0.0);
            for &x in &buf[2048..] {
                s0 = x + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            (s1 * s1 + s2 * s2 - coeff * s1 * s2).sqrt()
        };
        let fundamental = goertzel(15_000.0);
        let alias = goertzel(3_000.0);
        let ratio_db = 20.0 * (alias / fundamental.max(1e-30)).log10();
        assert!(
            ratio_db < -70.0,
            "alias at 3 kHz only {ratio_db:.1} dB below fundamental"
        );
    }
}
