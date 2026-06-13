//! Realistic per-sample CPU cost of the tape engine, the way a RE-DEEMER
//! module actually runs inside a VCV Rack patch: a live signal through the
//! full default chain — tape noise on, feedback loop active, wow/flutter and
//! dropouts live — at each oversampling factor.
//!
//! Run:
//!   cargo run -p te2-dsp --release --example cpu_cost
//!
//! Prints ns/sample for the engine as Rack drives it (stereo, both channels),
//! and the same running only one channel — the headroom a mono fast-path
//! would buy when the patch feeds a mono source (the common case). The % of a
//! single core is derived for the sample rates Rack is commonly run at.

use std::time::Instant;
use te2_dsp::denormals::set_flush_to_zero;
use te2_dsp::engine::{EngineParams, Te2Engine};
use te2_dsp::oversample::OsFactor;

const SR: f64 = 48_000.0;

/// What one instance runs when it just sits in a patch with audio going
/// through it: Rack defaults (aging off), feedback and noise live.
fn rack_default(os: OsFactor) -> EngineParams {
    EngineParams {
        os_factor: os,
        aging_on: false,
        ..Default::default()
    }
}

fn measure(os: OsFactor, secs: f64, drive_both: bool) -> f64 {
    let mut e = Te2Engine::new(SR);
    e.set_params(&rack_default(os));
    set_flush_to_zero(true); // match the post-fix audio thread

    let tone = |k: u64| 0.25 * (std::f64::consts::TAU * 220.0 * k as f64 / SR).sin() as f32;

    // Warm the loop to steady state.
    for k in 0..(SR as u64) {
        let x = tone(k);
        e.process(x, if drive_both { x } else { 0.0 });
    }

    let n = (SR * secs) as u64;
    let mut sink = 0.0f32;
    let t0 = Instant::now();
    for k in 0..n {
        let x = tone(k);
        let (l, r) = e.process(x, if drive_both { x } else { 0.0 });
        sink += l + r;
    }
    let elapsed = t0.elapsed();
    std::hint::black_box(sink);
    elapsed.as_nanos() as f64 / n as f64
}

fn pct(ns: f64, sr: f64) -> f64 {
    ns * sr / 1e9 * 100.0
}

fn run(os: OsFactor, label: &str) {
    let ns = measure(os, 4.0, true);
    println!(
        "{label:>3}  {ns:7.1} ns/sample   core load: 48k {:4.1}%   96k {:4.1}%   192k {:4.1}%",
        pct(ns, 48_000.0),
        pct(ns, 96_000.0),
        pct(ns, 192_000.0),
    );
}

fn main() {
    println!("RE-DEEMER engine cost as Rack drives it (stereo, full default chain)\n");
    run(OsFactor::X2, "2x");
    run(OsFactor::X4, "4x");
    run(OsFactor::X8, "8x");

    // What a mono fast-path (run one channel, copy to both outs) would cost
    // at the default 4x — most modular patches feed a mono source.
    let stereo_4x = measure(OsFactor::X4, 4.0, true);
    let mono_4x = measure(OsFactor::X4, 4.0, false);
    println!(
        "\nmono fast-path @4x: {mono_4x:.0} ns vs stereo {stereo_4x:.0} ns  \
         ({:.0}% of stereo)",
        mono_4x / stereo_4x * 100.0
    );
    println!(
        "(note: this measures one driven channel; a true mono path would also \
         skip the second channel's filters/hysteresis entirely.)"
    );
}
