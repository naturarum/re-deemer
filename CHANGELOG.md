# Changelog

## 1.0.0 — 2026-06-11

First release. The preorder, redeemed.

- Physical tape model: position-indexed tape at a fixed tape rate, motor
  with inertia, Jiles-Atherton hysteresis magnetics (2×/4×/8× oversampled),
  record/repro head EQ, speed-tracking gap loss and head bump, self-erasure,
  calibrated wow/flutter/scrape, dropouts, tape noise recorded onto tape.
- Tape types I / II / IV; mechanism condition (MECH) and noise level.
- 24 dB/oct OTA HPF + LPF inside the echo loop; resonance to self-oscillation
  with pitch tracking; RES gate (filter keyboard via 1-8 buttons / MIDI).
- Positions & Sets: 3 sets × 8 positions (21 faders), per-set DRIFT 0–14 s,
  Cycle 8 s/step → 4,000 steps/s (sample-accurate), host sync, Anomaly.
- Transport: REC/ECHO, PLAY (tape manipulation), LOOP (erase-bypass
  layering), RW/FF, PAUSE, MTR motor kill, double-press eject.
- MIDI position select/gating (C3–G3), off by default.
- Animated panel UI with hover help on every control.
- Formats: VST3, CLAP, AUv2 (macOS universal). Validation: clap-validator,
  pluginval strictness 10, auval — all passing.
