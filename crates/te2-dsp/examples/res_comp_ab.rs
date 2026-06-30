//! Offline A/B for the LPF resonance low-end compensation. Renders the same
//! dub-feedback patch (high feedback + moderate resonance, wet only) at several
//! compensation amounts so the change can be judged by ear. Writes 16-bit WAVs
//! to /tmp. Dev tool — run: `cargo run -p te2-dsp --example res_comp_ab --release`.

use te2_dsp::{EngineParams, Te2Engine};

fn write_wav16(path: &str, frames: &[(f32, f32)], sr: u32) {
    let data_bytes = frames.len() * 4; // 2 ch * 2 bytes
    let mut v: Vec<u8> = Vec::with_capacity(44 + data_bytes);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((36 + data_bytes) as u32).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&2u16.to_le_bytes()); // stereo
    v.extend_from_slice(&sr.to_le_bytes());
    v.extend_from_slice(&(sr * 4).to_le_bytes()); // byte rate
    v.extend_from_slice(&4u16.to_le_bytes()); // block align
    v.extend_from_slice(&16u16.to_le_bytes()); // bits
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    let q = |s: f32| (s.clamp(-1.0, 1.0) * 32767.0) as i16;
    for &(l, r) in frames {
        v.extend_from_slice(&q(l).to_le_bytes());
        v.extend_from_slice(&q(r).to_le_bytes());
    }
    std::fs::write(path, v).unwrap();
    eprintln!("wrote {path}");
}

fn main() {
    te2_dsp::denormals::ensure_flush_to_zero();
    let sr = 48_000u32;

    for &comp in &[0.0f32, 0.3, 0.5, 0.7] {
        let mut e = Te2Engine::new(sr as f64);
        e.set_params(&EngineParams {
            delay_time: 0.45,
            feedback: 0.85,
            tape_in: 1.0,
            tape_level: 1.0,
            dry_level: 0.0, // wet only — judge the feedback character
            hpf_hz: 30.0,
            lpf_hz: 1800.0,
            res: 0.45, // ~a third up: where the user reports the lows vanish
            condition: 0.0,
            noise_amount: 0.0,
            ..Default::default()
        });
        e.set_res_comp(comp);

        // Settle motor/tape.
        for _ in 0..(sr / 2) {
            e.process(0.0, 0.0);
        }

        // Filtered-noise plucks every 0.5 s for 4 s, then 3 s tail.
        let mut frames = Vec::new();
        let mut rng = 0x1234_5678u32;
        let mut lp = 0.0f32;
        let total = sr as usize * 7;
        let signal = sr as usize * 4;
        for k in 0..total {
            let x = if k < signal && (k % (sr as usize / 2)) < (sr as usize / 80) {
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = (rng >> 9) as f32 / (1 << 23) as f32 - 1.0;
                lp += 0.25 * (noise - lp);
                lp * 0.9
            } else {
                lp *= 0.995;
                lp
            };
            frames.push(e.process(x, x));
        }

        let tag = (comp * 100.0) as u32;
        write_wav16(&format!("/tmp/redeemer_rescomp_{tag:03}.wav"), &frames, sr);
    }
}
