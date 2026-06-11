//! Pure DSP for the Space Case TE-2 tape echo. No plugin framework dependencies.

pub mod drive;
pub mod engine;
pub mod filters;
pub mod oversample;
pub mod sequencer;
pub mod tape;

pub use engine::{EngineParams, Te2Engine};
