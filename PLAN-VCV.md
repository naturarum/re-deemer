# RE-DEEMER for VCV Rack — plan

Decisions (June 2026): **one big module** (~50–60HP, the whole instrument
like the hardware), **hardware patchpoints + Rack-native extras**,
**self-hosted beta first** then VCV Library submission, **stocks/aging
included** via the context menu.

The point of the Rack version: the hardware was semi-modular — this is the
form where the patchpoints come back for real.

---

## Architecture: bridge, don't port

`te2-dsp` stays the single source of truth (it has zero plugin-framework
dependencies for exactly this reason). A new `te2-capi` crate exposes a C
ABI over `Te2Engine`:

- `te2_create(sample_rate)` / `te2_destroy`
- `te2_process(l, r, *out_l, *out_r)` — Rack runs per-sample; this is
  already the engine's native shape
- `te2_set_params(const Te2Params*)` — plain-C mirror of `EngineParams`,
  set at control rate (every N samples)
- `te2_eject`, `te2_age`, `te2_set_age`, `te2_footage_seconds`,
  `te2_vu`, `te2_motor_speed`, `te2_position`
- builds as `staticlib`; `cbindgen` generates `te2.h`

The Rack plugin is a thin C++ shim (Rack SDK, GPLv3-compatible — our ISC
core is fine) linking `libte2_capi.a` per platform. The Makefile drives
`cargo build --release` for the current target before the C++ build.

## Patchpoints

**Audio:** IN L / IN R (R normalled to L) · OUT L / OUT R.

**CV inputs — front five (from the 2019 reference):**

| Jack | Behavior |
| --- | --- |
| MTR | gate: high kills the motor, release winds back up |
| REC | trigger: toggles record/echo vs play |
| TP OUT | tape level CV |
| RES | resonance CV, ±5 V |
| TAP | trigger: tap tempo sets TIME *(new DSP: inter-tap interval → delay_time)* |

**Modulation row — eight inputs, each with an attenuverter (the hardware's
"internal CV" row, restored to actual jacks):**

DRY LVL · OUT DRV · TAPE IN · MOD AMT · MOD SPD · ANMLY ·
MTR WOW *(new DSP: slip-clutch drag — CV adds momentary motor wow)* ·
LOOP (trigger toggles loop mode)

**CV outputs (the semi-modular heart):**

| Jack | Behavior |
| --- | --- |
| WHITE / GRAY / BLACK | each set's drift-slewed value as ±5 V CV — the fader rows sequence the rest of the rack *(new DSP: expose per-set values from the sequencer)* |
| EOC | trigger on the cycle's final-step entry (when the Anomaly would fire) |

**Rack-native extras:**

| Jack | Behavior |
| --- | --- |
| TIME (1 V/oct) | exponential motor-speed CV: +1 V doubles tape speed — recorded material tracks pitch like an oscillator (cassette synthesizer) |
| CLOCK | trigger advances the cycle one step *(new DSP: external-step mode replaces the DAW phase-lock in Rack land)* |
| POS | 0–10 V selects position 1–8 (gate behavior with RES GATE, like MIDI notes in the DAW plugin) |

## Panel & widgets

- SVG faceplate in the RE-DEEMER style (wood ends, black panel, cream ink).
- Custom nanovg widgets: ARP-style fader (21×), the live cassette window
  (the vector spool animation ports directly — footage/wear already exposed),
  VU needle, LED ring knobs where useful.
- Wear bar on the panel; stock picker, AGING/FREEZE, quality, side length
  in the right-click context menu (Rack's "machine room"). Wear persists in
  the patch via `dataToJson`/`dataFromJson`.

## DSP work items (all in te2-dsp, shared with the DAW plugin)

1. Tap tempo → `delay_time`
2. Sequencer external-step mode (advance on demand)
3. Slip-clutch wow input (momentary motor drag amount)
4. Per-set CV value getters on the sequencer
5. `te2-capi` crate + cbindgen header

## Phases

1. **Bridge + skeleton** — te2-capi, Rack plugin compiles, audio passes,
   TIME/FDBK knobs work. *Proof of life.*
2. **Full param surface** — all knobs/switches/faders, context menu,
   state save/load incl. wear.
3. **Patchpoints** — jack field + the four DSP work items above.
4. **Panel art & cassette widget** — the lovable part.
5. **Beta** — self-hosted `.vcvplugin` for mac (arm+x64), Windows, Linux
   from GitHub releases (forces the Windows/Linux te2-dsp builds — already
   a roadmap item). Announce on the site.
6. **VCV Library submission** — needs their sign-off on a cargo step in
   the Makefile (precedent exists; ask the Library team early, during
   phase 2, so the answer arrives before beta).

## Open questions (not blocking phase 1)

- Module name: "RE-DEEMER" vs a Rack-specific variant.
- HP count lands wherever the panel design says — guess 52–58HP.
- Polyphony: out of scope (one tape machine = one tape).
