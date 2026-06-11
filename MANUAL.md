# RE-DEEMER — User Manual

**Cassette tape echo · v1.0.0 · macOS (VST3 / CLAP / AU) · free**

RE-DEEMER is a software realization of the Space Case TE-2, a cassette-based
tape echo instrument designed around 2018–2019 that was pre-ordered by many
people and never shipped. This is the machine those preorders were waiting
for — redeemed in software, free, for everyone who waited.

It is not a delay plugin with tape flavoring. Inside is a physical model of
the machine: a motor with inertia, tape moving past record and repro heads,
magnetic hysteresis, a worn mechanism. The delay time *is* the motor speed.
Everything follows from that.

---

## 1. Installation

Unzip the archive and either run the included `install.sh` (double-click or
run in Terminal), or copy by hand:

| File | Copy to |
| --- | --- |
| `RE-DEEMER.clap` | `~/Library/Audio/Plug-Ins/CLAP/` |
| `RE-DEEMER.vst3` | `~/Library/Audio/Plug-Ins/VST3/` |
| `RE-DEEMER.component` | `~/Library/Audio/Plug-Ins/Components/` |

**Important:** the AU is a thin wrapper that loads the CLAP — install the
CLAP even if you only use Logic.

**First launch on macOS:** this build is not notarized by Apple (it's a free
plugin, not an App Store product). If your DAW refuses to load it, run the
included `install.sh`, which clears the quarantine flag — or in Terminal:

```
xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/RE-DEEMER.clap \
    ~/Library/Audio/Plug-Ins/VST3/RE-DEEMER.vst3 \
    ~/Library/Audio/Plug-Ins/Components/RE-DEEMER.component
```

Then rescan plugins in your DAW. In Logic, the AU appears under
**naturarum → RE-DEEMER** (MIDI-controlled effects).

---

## 2. Quick start

1. Insert RE-DEEMER on a track. You'll hear your dry signal plus echoes.
2. **TIME** sets the echo time. Turn it *while echoes ring* — the repeats
   bend in pitch as the motor changes speed. That's the machine working.
3. **FDBK** sets how long echoes regenerate. Past 100% they grow instead of
   decay, and the tape itself — not a limiter — keeps it under control.
4. **TAPE IN** is how hard you hit the tape. Watch the VU: around 0 VU is
   warm; pinning it is crunchy and compressed (and the highs fold down).

Hover any control for an explanation and its current value.

---

## 3. Signal flow

```
IN ─┬─ TAPE IN ─(+ feedback)─ pre-emphasis ─ tape noise ─ MAGNETICS ─→ tape
    │                                                                   ↓ (motor speed = TIME)
    │              repro head ─ head EQ / gap loss ─ HPF ─ LPF+RES ──┐
    │                                                  feedback tap ──┘→ × FDBK back to record
    └─ DRY LVL ──────────────────────────┐            ↓ TP LVL
                                          └──→ Σ ─ OUT DRV ─ OUT
```

Things that matter about this topology:

- **The filters are inside the echo loop.** Every repeat passes the HPF and
  LPF again. Low LPF = repeats that dissolve into murk.
- **The HPF sits before the record head on regeneration.** Low frequencies
  saturate tape first, so cutting lows buys headroom — repeats stay cleaner
  even at high feedback.
- **The noise is on the tape.** Hiss echoes, filters, and pitch-bends along
  with your signal, because it is recorded like your signal.

---

## 4. The panel

### Primary controls (lower center)

| Control | What it does |
| --- | --- |
| **TIME** | Delay time = tape speed (60 ms – 1.5 s). Changing it repitches everything already on tape. |
| **FDBK** | Echo regeneration, 0–110%. Over 100% = self-oscillating runaway, tape-limited. |
| **TP LVL** | Tape (echo) level at the output. |
| **DRY LVL** | Dry signal level. |
| **OUT DRV** | Op-amp drive on the final mix. |
| **MOD AMT / MOD SPD** | Sine modulation of motor speed. 0.1 Hz (slow seasick) to 150 Hz (FM mangling). |
| **HPF / LPF / RES** | The 24 dB/oct OTA filters in the echo path. RES self-oscillates near max — a tunable sine source. |
| **GATE** | Links the resonance to the 1-8 buttons / MIDI notes: play the filter like a synth. |

### Tape & machine (right block)

| Control | What it does |
| --- | --- |
| **NM / CH / MT** | Tape formulation: I Normal (warm, saturates early), II Chrome (cleaner), IV Metal (most headroom). |
| **TAPE IN** | Record level into the tape. The VU reads this. |
| **MTR** | Hold: the motor dies and pitch drags to a stop. Release: it winds back up. |
| **ANMLY + (−/OFF/+)** | The Anomaly: one tape hiccup fired on the cycle's final step. Amount = quick blip → long wobble; polarity bends pitch down or up. |
| **ECO / STD / ULT** | Oversampling quality of the tape magnetics (2× / 4× / 8×). |
| **GT / −10 / +4** | Input level standard (guitar / consumer / pro). |
| **OUT** | Output trim. |

### Top row

| Control | What it does |
| --- | --- |
| **DIV + SYNC** | Cycle rate division and host-tempo sync. |
| **LOOP + SYNC** | Loop length for LOOP mode; SYNC snaps to beats. |
| **NOISE** | Tape hiss level (recorded onto the tape). |
| **MECH** | Mechanism condition: wow, flutter, dropouts, bias sag. 0 = serviced, full = thrift-store wreck. |
| **DRIFT ×3** | Per-set glide time between positions, 0–14 s. |
| **CYCLE + CYC** | Cycle speed (8 s/step → 4,000 steps/s) and run switch. |

---

## 5. Positions & Sets — the heart of the machine

This is the TE-2's signature idea, fully implemented.

There are **8 positions**. Position 1 is the panel itself — whatever the
knobs say. Positions 2–8 are the **21 faders**: seven columns, each holding a
White, Gray, and Black fader.

There are **3 sets**, each assignable to one of three controls and switched
ON independently:

| Set | Can control |
| --- | --- |
| **White** (TM / RS / MS) | TIME, RESonance, or Mod Speed |
| **Gray** (FB / MA / LP) | FeedBack, Mod Amount, or LPF |
| **Black** (TP / DL / HP) | TaPe level, Dry Level, or HPF |

Press a **1-8 button** and that position becomes active: each ON set's
control jumps to that position's fader value. With all three sets on, one
button press re-tunes three controls at once — eight stored "snapshots"
played like notes.

**The Cycle** rotates through positions automatically. The **1-8 rotary**
limits how many positions cycle; **CYCLE** sets the speed — from 8 seconds
per step (slow evolving phrases) all the way to 4,000 steps per second.
At audio rate, the fader pattern literally becomes a waveform shaping
whatever it controls — a multi-stage function generator.

**DRIFT** (one knob per set) glides between positions instead of jumping,
0–14 seconds, logarithmic. Slow cycle + long drift on TIME = feedbacks
mildly pitching up or down over long stretches. Fast cycle + a little drift
= rounded corners on your generated waveform.

**The Anomaly** fires once per cycle revolution, on the final step: a single
speed hiccup, like a tape machine with a sticky spot. It works even with all
sets off — run the cycle just to get a wobble every few seconds.

---

## 6. Transport

| Button | What it does |
| --- | --- |
| **REC/ECHO** | Normal mode: record + echo, erase head on. |
| **PLAY** | Nothing new is recorded — manipulate what's on the tape. TIME repitches it; the 1-8 buttons (White set → TM) play it like a sampler. |
| **LOOP** | Erase head lifted: sound-on-sound layering on a finite loop (length = LOOP knob, in *tape* seconds — faster TIME shortens the heard loop). Old layers decay ~28% per pass under new ones. |
| **RW / FF** | Hold to shuttle the tape (reverse / fast playback in PLAY mode). |
| **PAUSE** | Mechanical pause: instant stop, speed retained. |
| **STP/EJ** | Stop. **Double-click = eject**: wipes the tape, fresh cassette. |

---

## 7. MIDI

Enable the **MIDI** button (next to the 1-8 row; off by default) and notes
**C3–G3 (60–67)** select positions 1–8 and act as gates:

- With **GATE** on and **RES** high, each note plays the self-oscillating
  filter at that position's LPF pitch — an 8-note synth through the echo.
- In **PLAY** mode with the White set on TM, notes play the recorded tape at
  8 different speeds — a cassette sampler.

All panel controls are regular automatable parameters; use your DAW's MIDI
learn / automation for everything else.

---

## 8. Recipes

Sixteen starting points live in `PRESETS.md`, including: classic dub shots,
feedback runaway drones, the audio-rate function generator, the filter
keyboard, loop composting, and the broken-courier anomaly machine.

Three quick ones:

- **Dub:** TIME synced 1/4·, FDBK 78%, HPF 150 Hz, LPF 2.4 kHz — ride HPF
  and FDBK with your hands.
- **Self-oscillation as an instrument:** FDBK 104%, TAPE IN −6 dB, feed one
  note, then play TIME and LPF. The tape compresses the chaos musically.
- **Wind-down ending:** at the end of a phrase, hold **MTR**. Release for
  the wind-up. Instant ending, courtesy of physics.

---

## 9. Troubleshooting

| Symptom | Cause |
| --- | --- |
| Plugin won't load after download | macOS quarantine — run `install.sh` or the `xattr` command in §1. |
| AU missing in Logic | The AU needs the CLAP installed too (it loads it at runtime). Install both, then restart Logic. |
| Silence in PLAY mode | Nothing on the tape — record something in REC/ECHO first. |
| Keyboard moves the 1-8 buttons | That's the MIDI switch — turn it off (it's off by default). |
| Echo gets darker each repeat | That's tape. Open LPF, raise tape quality (CH/MT), lower TAPE IN. |
| Pitch warbles | MECH and/or MOD AMT — turn down for a serviced machine. |

**Specs:** stereo/mono, any sample rate, ~13× realtime on one core at 48 kHz
(STD quality). The plugin reports no latency; the tape path's group delay is
part of the echo time, as on hardware.

---

*RE-DEEMER is a free tribute to the Space Case TE-2 by Moto Modular /
Catskill Analog. Not affiliated. You waited long enough.*
