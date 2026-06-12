# Changelog

## Unreleased

- **Windows and Linux builds.** VST3 + CLAP for both, built and
  clap-validated in CI on every release (the v1.0.3 archives are already
  up). Unsigned and younger than the macOS builds — reports welcome.
- VCV Rack module in development (separate project).


## 1.0.3 — 2026-06-12

- **FDBK now reaches 150%.** The old 110% ceiling left the loop gain barely
  over unity, so runaway crept up over many seconds instead of taking off.
  At 150% (~3.5 dB of excess gain) it blooms within a couple of repeats,
  slams into the tape and stays there — properly unhinged, still bounded by
  the tape itself. Note for existing sessions: saved knob *values* load
  unchanged, but normalized automation curves of FDBK written against
  earlier versions will read ~36% lower (the range grew); re-scale them if
  a track relied on them.
- **Interface scaling.** SETUP gained an INTERFACE SCALE selector:
  50–200% of the native 1080×560 in six steps. The window resizes live
  (host permitting) and the choice is saved with the plugin state.
- **SETUP overlay re-laid on a strict three-column grid** — stocks |
  budget + machine | aging + scale — with section headers on shared rows
  and every group centered on its column.
- **RW/FF now wind like a real deck.** Held wind was only 4x play speed, so
  the spools barely moved even fast-forwarding (a real mechanism winds a
  C60 side in a minute or two). Wind is now ~12x nominal (capped ~24x at
  fast TIME settings) — the spools visibly fly, and the shuttle sound got
  correspondingly wilder.
- The website's download button now serves the *versioned* archive
  (`RE-DEEMER-x.y.z-macos.zip`) so downloads identify their version; the
  stable link remains for old bookmarks.

## 1.0.2 — 2026-06-11

- **Alignment & symmetry pass:** the 1-8 buttons and transport row are
  centered on the cassette's axis (the transport spans exactly the window's
  width); the fader matrix, VU, RE-2 logo, SETUP and the right block share
  standardized margins and a common right edge; the wordmark/VU column sits
  on one centerline.
- **The cycle group is now one group.** 1-8 length, DIV and CYCLE sit
  together at the right end of the top row with SYNC and CYC directly
  beneath. SYNC locks the cycle to the host clock (tempo ÷ DIV,
  phase-locked to the playhead); the knob that isn't driving — CYCLE when
  synced, DIV when free — renders dimmed so the mode is visible at a
  glance.
- **Dark walnut frame** with subtle procedural grain (a few dozen vector
  strokes — no textures, no extra rendering cost).

## 1.0.1 — 2026-06-11

**If you use the AU in Logic or GarageBand, update: the v1.0.0 download's
AU never registered with macOS** (details below). Plus tape stocks, tape
aging, a settings overlay, live spools, and a round of panel fixes.

- **Live spools.** The cassette animation now runs on real tape motion from
  the engine: the supply pack winds onto the take-up over a C60 side
  (30 minutes at nominal speed), pack radii follow tape-area conservation,
  and each reel's spin rate tracks its current radius — the supply reel
  audibly-fast as it runs low. RW winds it backwards, MTR stalls it, the
  side ping-pongs at the end like an auto-reverse deck, and eject drops in
  a fully-wound fresh cassette. The shell picked up a clear hub window,
  ruled label with side-A marker, and shell screws.
- **Deck type vs. cassette type, reconciled.** The faceplate NM/CH/MT switch
  is the *deck's* tape-type setting; the stock in SETUP is the *cassette*.
  Picking a stock sets the deck to match; setting them apart is a usable
  mis-set-deck effect, flagged by a small amber tell-tale next to the
  switch. New instances now default to CH to match the default Maxell XL-II
  (existing sessions keep their saved value).
- **Layout fixes** from testing: the fader matrix spans the full panel
  width; the 1-8 cycle-length selector joined the top row beside CYCLE; the
  MIDI switch moved off the faceplate into TAPE & MACHINE.
- **Panel redesign.** The cassette window is much larger (and everything in
  it scales properly), the 1-8 position buttons moved below it, and the
  transport buttons grew. Faders got a third more throw (96 px) for finer
  mouse control. The three DRIFT knobs now wear their set's cap color
  (white/gray/black) so they can be told apart. The right-hand block sits on
  two rows aligned with the main knob rows. The wordmark above the VU is now
  RE-2, and third-party hardware branding was removed from the faceplate —
  the cassette label simply shows the loaded stock.
- **Tape stock**: fourteen real-world cassettes (Maxell XL-II default,
  TDK SA/MA/AD/D, Sony Metal-ES/UX/HF, BASF Chrome Maxima, Nakamichi EX-II,
  Maxell UD-II, Realistic Supertape, Memorex, no-name ferric) in three
  grades setting base hiss, headroom, and how fast the tape wears.
  Selecting a stock pre-sets the NM/CH/MT switch to its native formulation;
  the cassette in the window is labelled with what's loaded.
- **Tape aging**: the tape wears while the transport rolls — wow, dropouts
  and hiss rise; top end, output and headroom fade. Premium stock takes
  about an hour to go lo-fi, budget stock ~20 minutes. Wear is saved with
  the project, FREEZE holds it, eject (or NEW CASSETTE) resets it, and the
  AGING switch turns the whole thing off.
- **TAPE & MACHINE overlay**: click the TE-2 logo (or the new SETUP button)
  for the machine room — tape stock, aging, NOISE, MECH and quality, moved
  off the faceplate.
- **Cycle is now phase-locked to the DAW clock.** SYNC previously matched
  only the step *rate* to the tempo, so the cycle drifted against the song.
  With SYNC on and the transport rolling, steps now land on the grid and
  follow loops, jumps and tempo changes; with the transport stopped the
  cycle free-runs at the synced rate.
- **Fixed: the RES-gate filter synth was silent with NOISE turned down.**
  Self-oscillation was seeded only by tape hiss, so gating the filter open
  produced nothing on a quiet machine. The OTA input stage now carries its
  own thermal noise floor (≈−90 dB, inaudible) — the filter sings from true
  silence, like the real circuit. Gate open/close also ramps over ~3 ms
  instead of snapping (no clicks).
- **Removed the GT/−10/+4 input switch.** It was a hardware
  impedance/level-matching control and never affected the audio here —
  TAPE IN is the input level. Old sessions load fine; the saved value is
  ignored.
- **Fixed: the Audio Unit registered on first build only.** An incremental
  rebuild quietly stripped the component's AudioComponents registration, so
  the AU shipped in the v1.0.0 zip never appears in Logic/GarageBand. The
  build now restores the registration on every build and packaging refuses
  to ship a component without it. (CLAP and VST3 were unaffected; the AU
  loads the installed CLAP, so reinstalling fixes both.)

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
