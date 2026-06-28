//! Offline render harness: wav in -> Te2Engine -> wav out, plus test signal
//! generation. The listening/analysis companion to the unit tests.
//!
//! Usage:
//!   te2-render render <in.wav> <out.wav> [--time 0.35] [--feedback 0.45]
//!       [--tape-in 1.0] [--tape-level 0.8] [--dry-level 1.0]
//!       [--mod 0.0] [--mod-speed 0.5] [--tail 4.0]
//!   te2-render gen <pluck|sweep|impulse> <out.wav> [--seconds 2.0]

use te2_dsp::{EngineParams, Te2Engine};

fn main() {
    // Match the plugin's audio thread: flush subnormals so x86 render runs over
    // long decaying tails don't crawl in denormal microcode.
    te2_dsp::denormals::ensure_flush_to_zero();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("render") if args.len() >= 3 => render(&args[1], &args[2], &args[3..]),
        Some("gen") if args.len() >= 3 => gen(&args[1], &args[2], &args[3..]),
        _ => {
            eprintln!("usage: te2-render render <in.wav> <out.wav> [param flags]");
            eprintln!("       te2-render gen <pluck|sweep|impulse> <out.wav> [--seconds N]");
            std::process::exit(2);
        }
    }
}

fn flag(args: &[String], name: &str, default: f32) -> f32 {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn render(input: &str, output: &str, flags: &[String]) {
    let mut reader = hound::WavReader::open(input).expect("open input wav");
    let spec = reader.spec();
    let mut engine = Te2Engine::new(spec.sample_rate as f64);

    engine.set_params(&EngineParams {
        delay_time: flag(flags, "--time", 0.35),
        feedback: flag(flags, "--feedback", 0.45),
        tape_in: flag(flags, "--tape-in", 1.0),
        tape_level: flag(flags, "--tape-level", 0.8),
        dry_level: flag(flags, "--dry-level", 1.0),
        mod_amount: flag(flags, "--mod", 0.0),
        mod_speed_hz: flag(flags, "--mod-speed", 0.5),
        condition: flag(flags, "--condition", 0.35),
        noise_amount: flag(flags, "--noise", 1.0),
        hpf_hz: flag(flags, "--hpf", 30.0),
        lpf_hz: flag(flags, "--lpf", 7000.0),
        res: flag(flags, "--res", 0.0),
        out_drive: flag(flags, "--drive", 0.0),
        ..Default::default()
    });

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 * scale)
                .collect()
        }
    };

    let out_spec = hound::WavSpec {
        channels: 2,
        sample_rate: spec.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(output, out_spec).expect("create output wav");

    // Settle the motor before audio starts so renders are deterministic.
    for _ in 0..spec.sample_rate {
        engine.process(0.0, 0.0);
    }

    let ch = spec.channels as usize;
    let mut write = |l: f32, r: f32| {
        let (ol, or) = engine.process(l, r);
        writer.write_sample(ol).unwrap();
        writer.write_sample(or).unwrap();
    };
    for frame in samples.chunks(ch) {
        let l = frame[0];
        let r = if ch > 1 { frame[1] } else { frame[0] };
        write(l, r);
    }
    // Let the echo tail ring out.
    let tail = (flag(flags, "--tail", 4.0) * spec.sample_rate as f32) as usize;
    for _ in 0..tail {
        write(0.0, 0.0);
    }
    writer.finalize().unwrap();
    eprintln!("rendered {output}");
}

fn gen(kind: &str, output: &str, flags: &[String]) {
    let sr = 48_000u32;
    let seconds = flag(flags, "--seconds", 2.0);
    let n = (seconds * sr as f32) as usize;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(output, spec).expect("create wav");

    match kind {
        // Four staccato filtered-noise plucks: the classic tape echo test.
        "pluck" => {
            let mut rng = 0x12345678u32;
            let mut lp = 0.0f32;
            for k in 0..n {
                let t = k as f32 / sr as f32;
                let in_pluck = (t % 0.5) < 0.012;
                let x = if in_pluck {
                    rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                    let noise = (rng >> 9) as f32 / (1 << 23) as f32 - 1.0;
                    lp += 0.25 * (noise - lp);
                    lp * 0.9
                } else {
                    lp *= 0.995;
                    lp
                };
                writer.write_sample(x).unwrap();
            }
        }
        // Log sweep 20 Hz - 20 kHz for aliasing inspection.
        "sweep" => {
            let f0 = 20.0f64;
            let f1 = 20_000.0f64;
            let k_rate = (f1 / f0).ln() / seconds as f64;
            let mut phase = 0.0f64;
            for k in 0..n {
                let t = k as f64 / sr as f64;
                let f = f0 * (k_rate * t).exp();
                phase += std::f64::consts::TAU * f / sr as f64;
                writer.write_sample((phase.sin() * 0.5) as f32).unwrap();
            }
        }
        "impulse" => {
            for k in 0..n {
                writer
                    .write_sample(if k == 100 { 0.9f32 } else { 0.0 })
                    .unwrap();
            }
        }
        other => {
            eprintln!("unknown generator: {other}");
            std::process::exit(2);
        }
    }
    writer.finalize().unwrap();
    eprintln!("generated {output}");
}
