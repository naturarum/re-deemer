//! Tape stock: the brand-grade of the cassette in the well. A separate axis
//! from the IEC formulation switch (NM/CH/MT) — stock sets how fast the tape
//! wears, its base hiss, and how much headroom the oxide has before it
//! squashes. Named after real, well-documented consumer cassettes.
//!
//! Maxell XL-II is the reference: the tape shown on the TE-2 prototype, and
//! the stock all profiles are normalized against (its multipliers are 1.0 so
//! the default sound is unchanged from before stocks existed).

use super::TapeKind;

/// The cassettes in the drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TapeStock {
    // Premium: slow aging, low hiss, generous headroom.
    #[default]
    MaxellXlii,
    TdkSa,
    TdkMa,
    SonyMetalEs,
    BasfChromeMaxima,
    NakamichiExii,
    // Standard: the tapes everyone actually bought.
    TdkAd,
    MaxellUdii,
    SonyUx,
    // Budget: sheds oxide fast, hissy, saturates early.
    TdkD,
    SonyHf,
    RealisticSupertape,
    Memorex,
    Generic,
}

/// What a stock does to the machinery.
#[derive(Debug, Clone, Copy)]
pub struct StockProfile {
    /// Transport-running seconds from fresh to fully worn (age 1.0).
    pub aging_seconds: f32,
    /// Added to the MECH condition (cheap shells warp and squeal).
    pub condition_add: f32,
    /// Scales the calibrated hiss floor.
    pub noise_mul: f32,
    /// Drive into the magnetics: >1 saturates earlier (less headroom).
    pub drive_mul: f32,
    /// Formulation this stock actually was — the UI pre-sets the NM/CH/MT
    /// switch from it (overridable, like mis-setting a real deck).
    pub default_kind: TapeKind,
    /// Label text, as printed on the shell.
    pub label: &'static str,
}

impl TapeStock {
    pub const ALL: [TapeStock; 14] = [
        TapeStock::MaxellXlii,
        TapeStock::TdkSa,
        TapeStock::TdkMa,
        TapeStock::SonyMetalEs,
        TapeStock::BasfChromeMaxima,
        TapeStock::NakamichiExii,
        TapeStock::TdkAd,
        TapeStock::MaxellUdii,
        TapeStock::SonyUx,
        TapeStock::TdkD,
        TapeStock::SonyHf,
        TapeStock::RealisticSupertape,
        TapeStock::Memorex,
        TapeStock::Generic,
    ];

    pub fn profile(self) -> StockProfile {
        use TapeKind::*;
        let p = |mins: f32, cond: f32, noise: f32, drive: f32, kind, label| StockProfile {
            aging_seconds: mins * 60.0,
            condition_add: cond,
            noise_mul: noise,
            drive_mul: drive,
            default_kind: kind,
            label,
        };
        match self {
            TapeStock::MaxellXlii => p(60.0, 0.00, 1.00, 1.00, II, "MAXELL XL-II"),
            TapeStock::TdkSa => p(60.0, 0.00, 0.97, 0.97, II, "TDK SA"),
            TapeStock::TdkMa => p(65.0, 0.00, 0.88, 0.85, IV, "TDK MA"),
            TapeStock::SonyMetalEs => p(65.0, 0.00, 0.90, 0.87, IV, "SONY METAL-ES"),
            TapeStock::BasfChromeMaxima => p(55.0, 0.02, 1.02, 0.98, II, "BASF CHROME MAXIMA"),
            TapeStock::NakamichiExii => p(55.0, 0.00, 0.98, 0.96, II, "NAKAMICHI EX-II"),
            TapeStock::TdkAd => p(40.0, 0.04, 1.10, 1.05, I, "TDK AD"),
            TapeStock::MaxellUdii => p(40.0, 0.03, 1.06, 1.03, II, "MAXELL UD-II"),
            TapeStock::SonyUx => p(40.0, 0.04, 1.08, 1.04, II, "SONY UX"),
            TapeStock::TdkD => p(25.0, 0.07, 1.18, 1.12, I, "TDK D"),
            TapeStock::SonyHf => p(22.0, 0.09, 1.22, 1.15, I, "SONY HF"),
            TapeStock::RealisticSupertape => p(18.0, 0.14, 1.32, 1.20, I, "REALISTIC SUPERTAPE"),
            TapeStock::Memorex => p(18.0, 0.16, 1.35, 1.22, I, "MEMOREX"),
            TapeStock::Generic => p(15.0, 0.20, 1.45, 1.28, I, "NO-NAME FERRIC"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_stock_is_transparent() {
        // Maxell XL-II must not color the default sound: it existed before
        // stocks did.
        let p = TapeStock::MaxellXlii.profile();
        assert_eq!(p.condition_add, 0.0);
        assert_eq!(p.noise_mul, 1.0);
        assert_eq!(p.drive_mul, 1.0);
    }

    #[test]
    fn grades_order_sensibly() {
        for stock in TapeStock::ALL {
            let p = stock.profile();
            assert!(
                (10.0 * 60.0..=80.0 * 60.0).contains(&p.aging_seconds),
                "{stock:?} aging out of range"
            );
            assert!((0.0..=0.3).contains(&p.condition_add));
            assert!((0.5..=2.0).contains(&p.noise_mul));
            assert!((0.5..=2.0).contains(&p.drive_mul));
        }
        let xlii = TapeStock::MaxellXlii.profile();
        let generic = TapeStock::Generic.profile();
        assert!(generic.aging_seconds < xlii.aging_seconds / 2.0);
        assert!(generic.noise_mul > xlii.noise_mul);
        assert!(generic.drive_mul > xlii.drive_mul);
    }
}
