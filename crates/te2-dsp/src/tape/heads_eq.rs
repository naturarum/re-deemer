//! Record/repro head and tape EQ: pre-emphasis before the magnetics so HF
//! saturates first (that's where tape warmth comes from), matching
//! de-emphasis plus head bump and the speed-dependent loss filters on
//! playback, and the level-dependent HF self-erasure when the tape is hit
//! hot.

use super::TapeKind;
use crate::filters::{Biquad, BiquadCoeffs, OnePole};

/// EQ voicing per tape type. The emphasis pair nets out flat at low levels;
/// its purpose is what it does *around* the saturator.
#[derive(Debug, Clone, Copy)]
pub struct EqProfile {
    pub emphasis_db: f32,
    pub emphasis_fc: f32,
    /// Head bump center at nominal speed (scales with tape speed).
    pub bump_fc: f32,
    pub bump_db: f32,
    /// Playback gap-loss corner at nominal speed (scales with speed).
    pub gap_fc: f32,
    /// Fixed spacing/azimuth HF shelf.
    pub spacing_db: f32,
    pub spacing_fc: f32,
}

impl EqProfile {
    pub fn for_kind(kind: TapeKind) -> Self {
        match kind {
            // Ferric: strong emphasis (120 us era), pronounced bump, dull top.
            TapeKind::I => Self {
                emphasis_db: 6.0,
                emphasis_fc: 5_200.0,
                bump_fc: 85.0,
                bump_db: 2.6,
                gap_fc: 13_000.0,
                spacing_db: -2.5,
                spacing_fc: 9_000.0,
            },
            // Chrome: 70 us EQ, brighter, slightly leaner bump.
            TapeKind::II => Self {
                emphasis_db: 4.5,
                emphasis_fc: 6_300.0,
                bump_fc: 80.0,
                bump_db: 2.2,
                gap_fc: 15_000.0,
                spacing_db: -1.8,
                spacing_fc: 10_000.0,
            },
            // Metal: most extended, tightest.
            TapeKind::IV => Self {
                emphasis_db: 3.5,
                emphasis_fc: 7_000.0,
                bump_fc: 76.0,
                bump_db: 1.8,
                gap_fc: 16_500.0,
                spacing_db: -1.2,
                spacing_fc: 11_000.0,
            },
        }
    }
}

/// Record-side EQ: pre-emphasis shelf + dynamic self-erasure lowpass.
pub struct RecordEq {
    sample_rate: f32,
    pre_emph_coeffs: BiquadCoeffs,
    pre_emph: Biquad,

    // Self-erasure: hot recordings erase their own highs. Envelope of the
    // magnetics output steers a one-pole cutoff.
    erase_env: OnePole,
    erase_env_coeff: f32,
    erase_lp: OnePole,

    /// Tape wear 0..1: shed oxide self-erases sooner and harder.
    wear: f32,
}

impl RecordEq {
    pub fn new(sample_rate: f32, profile: &EqProfile) -> Self {
        let mut eq = Self {
            sample_rate,
            pre_emph_coeffs: BiquadCoeffs::IDENTITY,
            pre_emph: Biquad::default(),
            erase_env: OnePole::default(),
            erase_env_coeff: OnePole::coeff(sample_rate, 25.0),
            erase_lp: OnePole::default(),
            wear: 0.0,
        };
        eq.set_profile(profile);
        eq
    }

    /// Tape wear 0..1 (aging): lowers the self-erasure knee and cutoff.
    pub fn set_wear(&mut self, wear: f32) {
        self.wear = wear.clamp(0.0, 1.0);
    }

    pub fn set_profile(&mut self, profile: &EqProfile) {
        self.pre_emph_coeffs = BiquadCoeffs::high_shelf(
            self.sample_rate,
            profile.emphasis_fc,
            0.9,
            profile.emphasis_db,
        );
    }

    /// Before the magnetics.
    #[inline]
    pub fn pre(&mut self, x: f32) -> f32 {
        self.pre_emph.process(&self.pre_emph_coeffs, x)
    }

    /// After the magnetics: level-dependent HF self-erasure.
    #[inline]
    pub fn post(&mut self, x: f32) -> f32 {
        let env = self.erase_env.lowpass(self.erase_env_coeff, x.abs());
        // Below the knee the filter sits out of band; above it the top end
        // folds down toward ~5 kHz as the tape squashes. Worn oxide starts
        // erasing sooner and from a lower ceiling.
        let excess = (env - (0.55 - 0.20 * self.wear)).max(0.0);
        let fc = 20_000.0 * (1.0 - 0.55 * self.wear) / (1.0 + 6.0 * excess);
        let coeff = OnePole::coeff(self.sample_rate, fc);
        self.erase_lp.lowpass(coeff, x)
    }

    pub fn reset(&mut self) {
        self.pre_emph.reset();
        self.erase_env.reset();
        self.erase_lp.reset();
    }
}

/// Repro-side EQ: de-emphasis, head bump, gap loss, spacing loss.
/// The bump and gap corners track tape speed — update at control rate.
pub struct ReproEq {
    sample_rate: f32,
    profile: EqProfile,
    /// Gap-loss corner multiplier from tape wear (1.0 fresh, lower worn).
    wear_gap_mul: f32,

    de_emph_coeffs: BiquadCoeffs,
    de_emph: Biquad,
    bump_coeffs: BiquadCoeffs,
    bump: Biquad,
    gap_coeffs: BiquadCoeffs,
    gap: Biquad,
    spacing_coeffs: BiquadCoeffs,
    spacing: Biquad,
}

impl ReproEq {
    pub fn new(sample_rate: f32, profile: &EqProfile) -> Self {
        let mut eq = Self {
            sample_rate,
            profile: *profile,
            wear_gap_mul: 1.0,
            de_emph_coeffs: BiquadCoeffs::IDENTITY,
            de_emph: Biquad::default(),
            bump_coeffs: BiquadCoeffs::IDENTITY,
            bump: Biquad::default(),
            gap_coeffs: BiquadCoeffs::IDENTITY,
            gap: Biquad::default(),
            spacing_coeffs: BiquadCoeffs::IDENTITY,
            spacing: Biquad::default(),
        };
        eq.set_profile(profile);
        eq.set_speed(1.0);
        eq
    }

    pub fn set_profile(&mut self, profile: &EqProfile) {
        self.profile = *profile;
        self.de_emph_coeffs = BiquadCoeffs::high_shelf(
            self.sample_rate,
            profile.emphasis_fc,
            0.9,
            -profile.emphasis_db,
        );
        self.spacing_coeffs = BiquadCoeffs::high_shelf(
            self.sample_rate,
            profile.spacing_fc,
            0.8,
            profile.spacing_db,
        );
    }

    /// Tape wear 0..1 (aging): the gap-loss corner falls as oxide sheds.
    /// Takes effect on the next `set_speed` (control rate).
    pub fn set_wear(&mut self, wear: f32) {
        self.wear_gap_mul = 1.0 - 0.62 * wear.clamp(0.0, 1.0);
    }

    /// Update the speed-tracking sections (call at control rate).
    pub fn set_speed(&mut self, speed: f32) {
        let speed = speed.abs().max(0.05);
        let bump_fc = (self.profile.bump_fc * speed).clamp(25.0, 400.0);
        self.bump_coeffs = BiquadCoeffs::peaking(self.sample_rate, bump_fc, 1.1, self.profile.bump_db);
        let gap_fc =
            (self.profile.gap_fc * speed * self.wear_gap_mul).clamp(1_200.0, 0.45 * self.sample_rate);
        self.gap_coeffs = BiquadCoeffs::lowpass(self.sample_rate, gap_fc, 0.6);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let x = self.de_emph.process(&self.de_emph_coeffs, x);
        let x = self.bump.process(&self.bump_coeffs, x);
        let x = self.gap.process(&self.gap_coeffs, x);
        self.spacing.process(&self.spacing_coeffs, x)
    }

    pub fn reset(&mut self) {
        self.de_emph.reset();
        self.bump.reset();
        self.gap.reset();
        self.spacing.reset();
    }
}
