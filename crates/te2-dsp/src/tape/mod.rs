pub mod heads_eq;
pub mod magnetics;
pub mod noise;
pub mod reel;
pub mod stock;
pub mod transport;
pub mod wow_flutter;

pub use stock::{StockProfile, TapeStock};

/// IEC cassette tape types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TapeKind {
    #[default]
    I,
    II,
    IV,
}

/// Tape cells per second of tape at nominal motor speed (1.0).
///
/// Fixed regardless of host sample rate: cassette physics tops out far below
/// 48 kHz of bandwidth, and a fixed rate keeps tape content host-rate
/// independent. 2x headroom over the audible band keeps read/write
/// interpolation images out of the way.
pub const TAPE_RATE: f64 = 96_000.0;

/// Delay between record and repro heads at nominal speed, in seconds.
pub const NOMINAL_DELAY: f64 = 0.35;

/// Record-to-repro head gap in tape cells (fixed, like the physical heads).
pub const HEAD_GAP: f64 = NOMINAL_DELAY * TAPE_RATE;

/// Relative motor speed that produces the given record-to-repro delay.
#[inline]
pub fn speed_for_delay(delay_seconds: f64) -> f64 {
    NOMINAL_DELAY / delay_seconds.max(1e-3)
}
