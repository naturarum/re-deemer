# Space Case TE-2 — Roadmap

Post-1.0 features, in rough priority order.

> **Shipped (v1.0.1):** §1 tape aging, §2 tape stock selector, §3 settings
> overlay — released together as one feature ("TAPE & MACHINE"). Details
> drifted slightly from the sketches below (the overlay opens from the TE-2
> logo *and* a SETUP button; standard-grade stocks were added; the short-lived
> interface-scale selector from 1.0.3 was later removed). §4 the preset browser
> is the **v1.1** work in progress — the save/load mechanism is built; the 16
> factory sounds remain. The sections are kept for the original reasoning.

---

## 1. Tape aging (settings overlay) — DONE

Tape wears as it runs. The longer the transport rolls, the more the sound
degrades toward lo-fi — exactly what a real cassette loop does as the oxide
sheds and the same stretch of tape passes the heads thousands of times.

**Model.** An internal `age` value (0.0 = fresh, 1.0 = fully worn) ramps up
while the motor is running (record/echo/loop/play), paused while stopped. Age
then drives the *existing* degradation parameters — it's a time modulator on
machinery we already have, not a new DSP path:

- raises effective `condition` (→ more wow/flutter, dropouts, bias sag)
- rolls off HF (repro gap-loss corner drops, self-erasure knee lowers)
- lifts the hiss/asperity floor
- nudges saturation softer (oxide loses output)

**Aging rate is set by the tape stock (see §2).** Targets the user specified,
measured in *transport-running* time:

| Stock quality | Time to "lo-fi" | Notes |
| --- | --- | --- |
| Good tape | ~60 min | premium oxide, holds up |
| Bad tape | ~20 min | cheap stock, sheds fast |

**Controls (in the settings overlay, not the faceplate):**
- **Tape Aging** on/off — when off, tape stays pristine forever.
- **Freeze Aging** — pauses the age clock at its current value (keep a worn
  character without it getting worse).
- Age is **persisted with the project** (it's physical wear on *this* loop).
- **STP/EJ double-press (eject)** already wipes the tape → also resets `age`
  to 0.0 (fresh cassette in the well). Clean tie-in with existing transport.

**Implementation pointers (when we build it):**
- Add `age: f32` to engine state; advance in `process()` by
  `dt / aging_seconds` while `mechanism` is rolling. `aging_seconds` comes
  from the stock.
- Fold `age` into `condition`/noise/EQ where `EngineParams.condition` and the
  repro gap-loss / self-erasure corners are computed (`engine.rs`,
  `tape/heads_eq.rs`, `tape/wow_flutter.rs`).
- Persist via a `#[persist]` field on `Te2Params` (serde), surfaced to the
  engine each block alongside the settings-overlay trims.
- `eject()` in `engine.rs` resets `age`.

---

## 2. Tape stock selector (settings overlay) — DONE

A new axis distinct from the faceplate NM/CH/MT switch (which is IEC
*formulation*: I Normal / II Chrome / IV Metal). "Stock" is the *brand-grade*
that sets aging rate, base noise floor, and headroom. Named after real,
well-documented consumer cassettes so it reads as authentic.

**Good / premium (slow aging ≈ 60 min, low noise, high headroom):**
- **Maxell XLII** (Type II) — the tape shown on the TE-2 prototype; natural hero/default
- **TDK SA** (Type II, Super Avilyn) — the chrome-position benchmark
- **TDK MA / MA-R** (Type IV metal) — maximum headroom
- **Sony Metal-ES** (Type IV)
- **BASF Chrome Maxima** (Type II)
- **Nakamichi EXII** (Type II)

**Standard / mid (medium aging ≈ 40 min):**
- **TDK AD** (Type I, premium ferric)
- **Maxell UDII** (Type II)
- **Sony UX** (Type II)

**Budget / bad (fast aging ≈ 20 min, higher hiss, less headroom):**
- **TDK D** (Type I) — the entry-level ferric
- **Sony HF** (Type I)
- **Realistic Supertape** (RadioShack house brand)
- **Memorex (generic)** (Type I)
- generic / no-name ferric

Each stock maps to: `aging_seconds`, base `condition` offset, base
`noise_amount`, headroom (drive scale), and a default IEC formulation that
pre-sets the NM/CH/MT switch (overridable on the faceplate).

---

## 3. Settings overlay panel — DONE

A panel that **overlays the main interface** (dim the faceplate behind it,
draw a centered card) rather than living on the faceplate. Opened by clicking
the **TE-2 logo** (hook already reserved). egui draws it as a modal `Area`
over the panel with a scrim.

Initial contents (room to grow — more will land here later):
- Tape Aging on/off + Freeze (§1)
- Tape Stock selector (§2)
- Existing trims currently mapped to faceplate knobs but really "setup":
  MECH condition, motor wind-down/up ramp, noise level, oversampling quality,
  UI scale.

This lets us reclaim faceplate space (the NOISE/MECH knobs were placed on the
panel as a stopgap — they belong here).

---

## 4. In-plugin preset browser — SHIPPED (v1.1.0)

A preset is a *sound recipe*: it moves params through the host's normal
per-parameter channel (not a full state restore), so loading one re-dials the
panel without ejecting the tape or resetting wear. It lives as a **PRESETS
tab** inside the SETUP overlay (alongside MACHINE), not a faceplate strip.

- **User presets** — save / load / delete, one JSON file each under the OS
  config dir. **Done** (the PRESETS tab is user-only for now).
- **Factory presets** — a **future addition**, not surfaced in the UI yet (the
  PRESETS tab leaves no space for them). When added: a compiled `(id, value)`
  table plus a column in the tab; the 16 `PRESETS.md` recipes are the content,
  dialled in by ear. Backend stub exists; it's basic UI work + content later.

---

## 5. AUv3 (iOS / modern macOS) — only if iOS is a goal

Today the plugin ships VST3 + CLAP + **AUv2** (via clap-wrapper; passes
`auval`, loads in Logic/GarageBand/Live on desktop). **AUv3 is not built** and
is a separate effort:

- AUv3 is an *app-extension* (`.appex`) inside a container `.app`, sandboxed,
  built through Xcode — a fundamentally different model from AUv2.
- No Rust/CLAP tool produces it (`clap-wrapper` is AUv2-only). It needs a
  Swift/ObjC Xcode wrapper that hosts the engine (the `te2-dsp` crate compiles
  to a static lib for that).
- **Required for iOS/iPadOS**; on desktop it adds little over the existing
  AUv2. Prioritize only if mobile is on the table.

---

## Later / smaller

- Windows build + CI matrix (buildable from this tree; untested).
- Developer ID codesign + notarization for distribution.
- Per-set CV-style cross-patching (the hardware's "semi-modular" routing),
  reimagined as internal mod routing since plugins have no CV jacks.
- Tap-tempo / footswitch-style step advance mapping.

