//! Tape noise: hiss shaped per tape type plus asperity (modulation) noise
//! that rides the signal level. Injected into the *record* path so it lands
//! on tape — it regenerates through the feedback loop, gets filtered and
//! pitch-bent like everything else on the tape. That behavior is most of why
//! real tape echoes sound alive.

use crate::filters::OnePole;
use super::magnetics::MagParams;

/// Noise voicing per tape type.
#[derive(Debug, Clone, Copy)]
pub struct NoiseProfile {
    /// Hiss RMS level (linear, relative to full scale).
    pub hiss_level: f32,
    /// Hiss spectrum top end.
    pub hiss_cutoff_hz: f32,
    /// How strongly the noise floor rides the signal envelope.
    pub asperity: f32,
}

impl NoiseProfile {
    pub fn type_i() -> Self {
        Self {
            hiss_level: 2.2e-3, // ~ -53 dBFS
            hiss_cutoff_hz: 9_000.0,
            asperity: 2.2,
        }
    }

    pub fn type_ii() -> Self {
        Self {
            hiss_level: 1.3e-3, // ~ -58 dBFS
            hiss_cutoff_hz: 11_000.0,
            asperity: 1.8,
        }
    }

    pub fn type_iv() -> Self {
        Self {
            hiss_level: 0.8e-3, // ~ -62 dBFS
            hiss_cutoff_hz: 13_000.0,
            asperity: 1.5,
        }
    }

    pub fn for_params(_mag: &MagParams, tape_type: super::TapeKind) -> Self {
        match tape_type {
            super::TapeKind::I => Self::type_i(),
            super::TapeKind::II => Self::type_ii(),
            super::TapeKind::IV => Self::type_iv(),
        }
    }
}

/// Per-channel noise generator.
pub struct TapeNoise {
    rng: u32,
    hiss_lp: OnePole,
    hiss_hp: OnePole,
    env: OnePole,
    lp_coeff: f32,
    hp_coeff: f32,
    env_coeff: f32,
}

impl TapeNoise {
    pub fn new(sample_rate: f64, seed: u32) -> Self {
        Self {
            rng: seed | 1,
            hiss_lp: OnePole::default(),
            hiss_hp: OnePole::default(),
            env: OnePole::default(),
            lp_coeff: OnePole::coeff(sample_rate as f32, 9_000.0),
            hp_coeff: OnePole::coeff(sample_rate as f32, 90.0),
            env_coeff: OnePole::coeff(sample_rate as f32, 12.0),
        }
    }

    pub fn set_profile(&mut self, sample_rate: f64, profile: &NoiseProfile) {
        self.lp_coeff = OnePole::coeff(sample_rate as f32, profile.hiss_cutoff_hz);
    }

    /// Noise sample to add into the record path. `signal` is the current
    /// record-path sample (for asperity tracking), `amount` is the overall
    /// noise scale (settings drawer / condition), 1.0 = calibrated level.
    #[inline]
    pub fn process(&mut self, profile: &NoiseProfile, signal: f32, amount: f32) -> f32 {
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let white = (self.rng >> 8) as f32 / (1 << 23) as f32 - 1.0;

        // Shape: lowpass for the oxide spectrum, highpass since heads don't
        // record DC rumble.
        let shaped = self
            .hiss_hp
            .highpass(self.hp_coeff, self.hiss_lp.lowpass(self.lp_coeff, white));

        // Asperity: the floor rises with the recorded level.
        let env = self.env.lowpass(self.env_coeff, signal.abs());
        let floor = 1.0 + profile.asperity * env.min(1.5);

        // White noise scaling: the one-pole chain leaves roughly 1/3 of full-
        // scale white RMS (~0.58), so compensate to hit the calibrated level.
        shaped * profile.hiss_level * floor * amount * 3.0
    }

    pub fn reset(&mut self) {
        self.hiss_lp.reset();
        self.hiss_hp.reset();
        self.env.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiss_level_calibrated() {
        let profile = NoiseProfile::type_i();
        let mut noise = TapeNoise::new(48_000.0, 0xDEADBEEF);
        let n = 48_000 * 4;
        let mut sum_sq = 0.0f64;
        for _ in 0..n {
            let y = noise.process(&profile, 0.0, 1.0) as f64;
            sum_sq += y * y;
        }
        let rms_db = 10.0 * (sum_sq / n as f64).log10();
        assert!(
            (-58.0..=-48.0).contains(&rms_db),
            "type I hiss RMS {rms_db:.1} dB, expected ~-53"
        );
    }

    #[test]
    fn asperity_raises_floor_with_signal() {
        let profile = NoiseProfile::type_i();
        let measure = |sig: f32| {
            let mut noise = TapeNoise::new(48_000.0, 0xDEADBEEF);
            let n = 48_000;
            let mut sum_sq = 0.0f64;
            for _ in 0..n {
                let y = noise.process(&profile, sig, 1.0) as f64;
                sum_sq += y * y;
            }
            (sum_sq / n as f64).sqrt()
        };
        let quiet = measure(0.0);
        let loud = measure(0.8);
        assert!(
            loud > quiet * 2.0,
            "asperity too weak: quiet {quiet:.5} loud {loud:.5}"
        );
    }
}
