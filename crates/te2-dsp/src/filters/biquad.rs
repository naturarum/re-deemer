//! Utility filter sections: TDF2 biquad and a one-pole. Used for EQ stages,
//! anti-alias/bandwidth filters, and DC blocking. The character filters
//! (OTA ladder) live in `ota_ladder.rs`.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiquadCoeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl BiquadCoeffs {
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// RBJ cookbook lowpass.
    pub fn lowpass(sample_rate: f32, fc: f32, q: f32) -> Self {
        let fc = fc.clamp(10.0, sample_rate * 0.49);
        let w0 = std::f32::consts::TAU * fc / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos) * 0.5) / a0,
            b1: (1.0 - cos) / a0,
            b2: ((1.0 - cos) * 0.5) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    /// RBJ cookbook highpass.
    pub fn highpass(sample_rate: f32, fc: f32, q: f32) -> Self {
        let fc = fc.clamp(1.0, sample_rate * 0.49);
        let w0 = std::f32::consts::TAU * fc / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 + cos) * 0.5) / a0,
            b1: -(1.0 + cos) / a0,
            b2: ((1.0 + cos) * 0.5) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    /// RBJ cookbook peaking EQ, gain in dB.
    pub fn peaking(sample_rate: f32, fc: f32, q: f32, gain_db: f32) -> Self {
        let fc = fc.clamp(10.0, sample_rate * 0.49);
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = std::f32::consts::TAU * fc / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha / a;
        Self {
            b0: (1.0 + alpha * a) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }

    /// RBJ cookbook high shelf, gain in dB.
    pub fn high_shelf(sample_rate: f32, fc: f32, slope: f32, gain_db: f32) -> Self {
        let fc = fc.clamp(10.0, sample_rate * 0.49);
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = std::f32::consts::TAU * fc / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        let a0 = (a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha;
        Self {
            b0: (a * ((a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha)) / a0,
            b1: (-2.0 * a * ((a - 1.0) + (a + 1.0) * cos)) / a0,
            b2: (a * ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha)) / a0,
            a1: (2.0 * ((a - 1.0) - (a + 1.0) * cos)) / a0,
            a2: ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha) / a0,
        }
    }

    /// RBJ cookbook low shelf, gain in dB.
    pub fn low_shelf(sample_rate: f32, fc: f32, slope: f32, gain_db: f32) -> Self {
        let fc = fc.clamp(10.0, sample_rate * 0.49);
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = std::f32::consts::TAU * fc / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        let a0 = (a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha;
        Self {
            b0: (a * ((a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha)) / a0,
            b1: (2.0 * a * ((a - 1.0) - (a + 1.0) * cos)) / a0,
            b2: (a * ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha)) / a0,
            a1: (-2.0 * ((a - 1.0) + (a + 1.0) * cos)) / a0,
            a2: ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha) / a0,
        }
    }
}

/// Transposed direct form II biquad.
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    z1: f32,
    z2: f32,
}

impl Biquad {
    #[inline]
    pub fn process(&mut self, c: &BiquadCoeffs, x: f32) -> f32 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// One-pole lowpass/highpass building block.
#[derive(Debug, Clone, Copy, Default)]
pub struct OnePole {
    state: f32,
}

impl OnePole {
    pub fn coeff(sample_rate: f32, fc: f32) -> f32 {
        let fc = fc.clamp(0.01, sample_rate * 0.49);
        1.0 - (-std::f32::consts::TAU * fc / sample_rate).exp()
    }

    #[inline]
    pub fn lowpass(&mut self, coeff: f32, x: f32) -> f32 {
        self.state += coeff * (x - self.state);
        self.state
    }

    #[inline]
    pub fn highpass(&mut self, coeff: f32, x: f32) -> f32 {
        x - self.lowpass(coeff, x)
    }

    pub fn reset(&mut self) {
        self.state = 0.0;
    }
}
