# RE-DEEMER — Cassette Tape Echo

A software realization of the Space Case TE-2 cassette tape echo — the
instrument that was pre-ordered and never shipped. The preorder, redeemed.
Built in Rust on [nice-plug](https://codeberg.org/RustAudio/nice-plug),
faithful to the 2019 reference guide and prototype panel; the tribute lives
on the cassette label and the footer.

Bundle names: `RE-DEEMER.clap` / `RE-DEEMER.vst3` / `RE-DEEMER.component`
(AU id `aumf Rdmr Ntrm`). MIDI position control (notes C3–G3) is **off by
default** — enable it with the MIDI button next to the 1-8 row.

**Formats:** VST3 · CLAP · AUv2 (via clap-wrapper) — macOS universal
(arm64 + x86_64), Windows buildable from the same tree.

## What's inside

The tape is real, not a delay line: a position-indexed tape buffer at a fixed
96 kHz tape rate with fixed record/repro heads and a motor model. TIME is
motor speed — turning it repitches everything already on tape; MTR drags the
pitch to a dead stop; RW/FF shuttle the actual tape; the cassette-synth and
PLAY-mode manipulation fall out of the same physics.

- **Magnetics:** Jiles–Atherton hysteresis (Chowdhury, DAFx-19) solved with
  RK4 at 2×/4×/8× oversampling; bias blends the anhysteretic (ideally biased)
  path against the raw hysteresis loop. Tape types I/II/IV change saturation,
  EQ, self-erasure and hiss.
- **Heads & EQ:** record pre-emphasis (HF saturates first), de-emphasis,
  speed-tracking head bump and gap loss, spacing loss, level-dependent HF
  self-erasure.
- **Mechanism:** always-on wow/flutter/scrape calibrated to ~0.15% RMS
  (band-limited 0.2–30 Hz), dropouts and bias sag scaling with the MECH
  control; tape noise is recorded *onto* the tape so it regenerates through
  the feedback loop.
- **Filters:** 24 dB/oct OTA ladder HPF + LPF (ZDF), resonance to clean
  self-oscillation with pitch tracking; the RES gate turns the 1–8 buttons
  into an 8-pitch filter keyboard (MIDI C3–G3).
- **Positions & Sets:** 3 sets × 8 positions (21 faders), per-set DRIFT slew
  0–14 s, Cycle from 8 s/step to 4,000 steps/s with a sample-accurate clock —
  the multi-stage function generator of the original spec. Anomaly fires a
  motor hiccup on the cycle's final step.
- **Transport:** REC/ECHO, PLAY, LOOP (erase bypass = sound-on-sound
  layering), RW/FF, PAUSE, STP/EJ (double-press ejects to a fresh cassette).

## Building

```bash
rustup default stable          # 1.87+
cd spacecase-te2
cargo xtask bundle te2-plugin --release            # VST3 + CLAP
cargo xtask bundle-universal te2-plugin --release  # macOS universal
```

Bundles land in `target/bundled/`. Install:

- `Space Case TE-2.vst3` → `~/Library/Audio/Plug-Ins/VST3/`
- `Space Case TE-2.clap` → `~/Library/Audio/Plug-Ins/CLAP/`

### AUv2

The AU is a [clap-wrapper](https://github.com/free-audio/clap-wrapper)
component that loads the CLAP, so install the CLAP first.

```bash
cd wrapper-au
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
cp -r "build/Space Case TE-2.component" ~/Library/Audio/Plug-Ins/Components/
auval -v aumf Te2e Ntrm
```

### Dev tools

```bash
cargo test -p te2-dsp --release        # 39 DSP tests: timing, repitch, W&F,
                                       # aliasing, magnetics, sequencer, transport
cargo run -p te2-render --release -- gen pluck /tmp/p.wav
cargo run -p te2-render --release -- render /tmp/p.wav /tmp/echo.wav \
    --time 0.4 --feedback 0.8 --lpf 2500 --tail 6
cargo run -p te2-plugin --release --features standalone --bin te2-standalone
cargo run -p te2-plugin --release --features snapshot --bin te2-snapshot     # headless UI png
```

## Validation status

| Check | Result |
| --- | --- |
| clap-validator 0.3.2 | 18 run / 0 failed |
| pluginval 1.0.4, strictness 10 (VST3) | SUCCESS |
| auval (aumf Te2e Ntrm) | PASS |
| te2-dsp test suite | 39/39 |
| Engine CPU (48 kHz stereo, 4× OS) | ~13× realtime on one Apple-silicon core |

## Notes

- Distribution: nice-plug is ISC and its `vst3` bindings are MIT/Apache-2.0,
  but shipping VST3 binaries publicly still means agreeing to Steinberg's
  VST3 licensing terms (or GPLv3). CLAP and AU carry no such strings.
- For distribution beyond your own machine, codesign with a Developer ID and
  notarize; the xtask bundles are ad-hoc signed.
- `PRESETS.md` has 16 factory panel recipes.

*A tribute. You waited long enough.*
