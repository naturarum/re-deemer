//! Denormal-cost repro harness for the tape engine.
//!
//! Reproduces and measures the VCV Rack "hiccups / freezes" hypothesis: when
//! the echo and filter tails ring down with no noise floor to hold them up,
//! the engine's record-side IIR states (pre/post EQ, the record-bandwidth
//! biquads, and the oversampled hysteresis) fall into the subnormal range. On
//! a thread WITHOUT flush-to-zero each per-sample op on those subnormals runs
//! in slow CPU microcode. DAW hosts set flush-to-zero on their audio threads;
//! VCV Rack's engine thread does not — which is the bug.
//!
//! Run:
//!   cargo run -p te2-dsp --release --example denormal_bench
//!
//! For each oversampling factor it prints ns/sample for:
//!   - normal signal, FTZ off   (the ordinary cost; the noise floor is alive)
//!   - subnormal-pinned, FTZ off (simulates Rack's audio thread on a quiet tail)
//!   - subnormal-pinned, FTZ on  (the fix — what every DAW host already sets)
//!
//! A large off/on ratio confirms denormals are the cost, and "FTZ on" should
//! sit back down near the normal-signal cost.
//!
//! Why a subnormal *input*: with feedback and the noise floor off, the record
//! path is fed pure zero and its states sink into the denormal range on their
//! own — but only transiently, as they decay through it toward zero. Feeding
//! the smallest subnormal as input pins those same states in that range for
//! the whole window, so the per-op penalty can be measured cleanly. It is the
//! identical arithmetic the natural decay tails pay, just held steady.

use std::time::Instant;
use te2_dsp::denormals::set_flush_to_zero;
use te2_dsp::engine::{EngineParams, Te2Engine};
use te2_dsp::oversample::OsFactor;

const SR: f64 = 48_000.0;

/// Feedback and noise both off, so the record path is not propped up by the
/// OTA thermal floor or tape hiss — exactly the condition under which a quiet
/// pass denormalizes in Rack.
fn quiet_params(os: OsFactor) -> EngineParams {
    EngineParams {
        delay_time: 0.3,
        feedback: 0.0,
        dry_level: 0.0,
        tape_level: 1.0,
        condition: 0.0,
        noise_amount: 0.0,
        hpf_hz: 30.0,
        lpf_hz: 12_000.0,
        res: 0.0,
        os_factor: os,
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
enum Stim {
    /// An ordinary in-band tone — keeps every state normal.
    Normal,
    /// The smallest positive subnormal — pins the states denormal.
    Subnormal,
}

fn measure(os: OsFactor, stim: Stim, ftz: bool, secs: f64) -> f64 {
    let mut e = Te2Engine::new(SR);
    e.set_params(&quiet_params(os));
    set_flush_to_zero(ftz);

    let subnormal = f32::from_bits(1);
    let value = |k: u64| -> f32 {
        match stim {
            Stim::Normal => 0.2 * (std::f64::consts::TAU * 220.0 * k as f64 / SR).sin() as f32,
            Stim::Subnormal => subnormal,
        }
    };

    // Settle so the states reach their regime before the clock starts.
    for k in 0..(SR as u64 / 2) {
        let x = value(k);
        e.process(x, x);
    }

    let n = (SR * secs) as u64;
    let mut sink = 0.0f32;
    let t0 = Instant::now();
    for k in 0..n {
        let x = value(k);
        let (l, _r) = e.process(x, x);
        sink += l;
    }
    let elapsed = t0.elapsed();
    std::hint::black_box(sink);
    elapsed.as_nanos() as f64 / n as f64
}

fn run(os: OsFactor, label: &str) {
    let secs = 3.0;
    let normal_off = measure(os, Stim::Normal, false, secs);
    let denorm_off = measure(os, Stim::Subnormal, false, secs);
    let denorm_on = measure(os, Stim::Subnormal, true, secs);
    set_flush_to_zero(false);
    println!(
        "{label:>3}   normal {normal_off:7.1}   denormal(Rack) {denorm_off:7.1}   \
         denormal+fix {denorm_on:7.1}   penalty {:.1}x",
        denorm_off / denorm_on.max(1e-9)
    );
}

fn main() {
    println!("Tape engine silent-tail cost, ns/sample (lower is better)\n");
    println!("       {:>13}   {:>20}   {:>18}", "FTZ off", "FTZ off", "FTZ on");
    run(OsFactor::X2, "2x");
    run(OsFactor::X4, "4x");
    run(OsFactor::X8, "8x");
    println!(
        "\n'denormal(Rack)' is the bare audio thread (FTZ off); 'denormal+fix' is\n\
         FTZ on, which every audio-thread entry point now guarantees per callback.\n\
         A large off/on ratio = denormals are the cost (x86); ~1x = this CPU is\n\
         unaffected by subnormals (Apple Silicon)."
    );
}
