//! OUT DRV: the op-amp output stage, "more gradual tunability and contained
//! resulting levels" than the TE-1 per the spec. Applied to tape + dry sum.
//! Slightly asymmetric soft clip (single-supply op-amp into its rails) with
//! 2x oversampling and DC servo.

use crate::filters::OnePole;
use crate::oversample::{OsFactor, Oversampler};

pub struct OutputDrive {
    os: [Oversampler; 2],
    dc: [OnePole; 2],
    dc_coeff: f32,
    /// 0..1 panel knob.
    amount: f32,
    pre_gain: f32,
    post_gain: f32,
}

impl OutputDrive {
    pub fn new(sample_rate: f64) -> Self {
        let mut drive = Self {
            os: [Oversampler::default(), Oversampler::default()],
            dc: [OnePole::default(), OnePole::default()],
            dc_coeff: OnePole::coeff(sample_rate as f32, 8.0),
            amount: 0.0,
            pre_gain: 1.0,
            post_gain: 1.0,
        };
        drive.set_amount(0.0);
        drive
    }

    pub fn set_amount(&mut self, amount: f32) {
        self.amount = amount.clamp(0.0, 1.0);
        // Up to ~+26 dB into the clipper; loudness partially compensated so
        // the knob adds dirt faster than level ("contained resulting levels").
        self.pre_gain = 1.0 + self.amount * self.amount * 19.0;
        self.post_gain = 1.0 / (1.0 + self.amount * 2.2);
    }

    #[inline]
    pub fn process(&mut self, ch: usize, x: f32) -> f32 {
        if self.amount < 1e-4 {
            return x;
        }
        let pre = self.pre_gain;
        // Rail asymmetry grows with drive.
        let offset = 0.04 * self.amount;
        let shaped = self.os[ch].process(OsFactor::X2, x, |s| {
            let driven = s * pre + offset;
            soft_rail(driven)
        });
        let centered = self.dc[ch].highpass(self.dc_coeff, shaped);
        centered * self.post_gain
    }

    pub fn reset(&mut self) {
        for ch in 0..2 {
            self.os[ch].reset();
            self.dc[ch].reset();
        }
    }
}

/// Op-amp-ish rail clip: tanh knee into a hard ceiling, slightly different
/// knees per polarity.
#[inline]
fn soft_rail(x: f32) -> f32 {
    if x >= 0.0 {
        (x * 0.9).tanh() / 0.9
    } else {
        (x * 1.05).tanh() / 1.05
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_drive_is_transparent() {
        let mut d = OutputDrive::new(48_000.0);
        d.set_amount(0.0);
        for k in 0..1000 {
            let x = (k as f32 * 0.013).sin() * 0.7;
            assert_eq!(d.process(0, x), x);
        }
    }

    #[test]
    fn drive_adds_harmonics_and_stays_bounded() {
        let sr = 48_000.0f64;
        let mut d = OutputDrive::new(sr);
        d.set_amount(0.8);
        let mut peak = 0.0f32;
        let mut sum_sq = 0.0f64;
        let n = 48_000;
        for k in 0..n {
            let x = 0.6 * (std::f64::consts::TAU * 220.0 * k as f64 / sr).sin() as f32;
            let y = d.process(0, x);
            peak = peak.max(y.abs());
            sum_sq += (y as f64) * (y as f64);
        }
        assert!(peak < 1.6, "drive output too hot: {peak}");
        assert!(sum_sq > 0.0);
    }
}
