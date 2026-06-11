//! The complete TE-2 parameter tree. IDs are stable forever — never rename them.
//!
//! Control names and ranges follow the 2019 TE-2 quick reference guide:
//! - 12 primary controls (TIME..ANMLY)
//! - 21 position faders (positions 2-8 x White/Gray/Black sets; position 1 is the panel)
//! - 3 sets with ON / parameter selector / DRIFT slew (0-14 s, logarithmic)
//! - Cycle: run, length 1-8, rate 8 s/step .. 4000 steps/s (+ host sync, a plugin addition)
//! - Anomaly polarity, RES gate, tape type I/II/IV, input character, transport mode

use nice_plug::prelude::*;
use nice_plug_egui::EguiState;
use std::sync::Arc;

/// White set fader target: TM / RS / MS per the panel switch.
#[derive(Enum, PartialEq, Clone, Copy)]
pub enum WhiteTarget {
    #[id = "tm"]
    #[name = "TM (Time)"]
    Time,
    #[id = "rs"]
    #[name = "RS (Resonance)"]
    Resonance,
    #[id = "ms"]
    #[name = "MS (Mod Speed)"]
    ModSpeed,
}

/// Gray set fader target: FB / MA / LP.
#[derive(Enum, PartialEq, Clone, Copy)]
pub enum GrayTarget {
    #[id = "fb"]
    #[name = "FB (Feedback)"]
    Feedback,
    #[id = "ma"]
    #[name = "MA (Mod Amount)"]
    ModAmount,
    #[id = "lp"]
    #[name = "LP (LPF)"]
    Lpf,
}

/// Black set fader target: TP / DL / HP.
#[derive(Enum, PartialEq, Clone, Copy)]
pub enum BlackTarget {
    #[id = "tp"]
    #[name = "TP (Tape Level)"]
    TapeLevel,
    #[id = "dl"]
    #[name = "DL (Dry Level)"]
    DryLevel,
    #[id = "hp"]
    #[name = "HP (HPF)"]
    Hpf,
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum AnomalyPolarity {
    #[id = "minus"]
    #[name = "−"]
    Minus,
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "plus"]
    #[name = "+"]
    Plus,
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum TapeType {
    #[id = "normal"]
    #[name = "I Normal"]
    Normal,
    #[id = "chrome"]
    #[name = "II Chrome"]
    Chrome,
    #[id = "metal"]
    #[name = "IV Metal"]
    Metal,
}

/// Brand-grade of the cassette in the well — a separate axis from the IEC
/// NM/CH/MT formulation switch. Sets aging rate, base hiss, and headroom.
#[derive(Enum, PartialEq, Clone, Copy)]
pub enum TapeStockParam {
    #[id = "xlii"]
    #[name = "Maxell XL-II"]
    MaxellXlii,
    #[id = "sa"]
    #[name = "TDK SA"]
    TdkSa,
    #[id = "ma"]
    #[name = "TDK MA"]
    TdkMa,
    #[id = "mes"]
    #[name = "Sony Metal-ES"]
    SonyMetalEs,
    #[id = "bcm"]
    #[name = "BASF Chrome Maxima"]
    BasfChromeMaxima,
    #[id = "exii"]
    #[name = "Nakamichi EX-II"]
    NakamichiExii,
    #[id = "ad"]
    #[name = "TDK AD"]
    TdkAd,
    #[id = "udii"]
    #[name = "Maxell UD-II"]
    MaxellUdii,
    #[id = "ux"]
    #[name = "Sony UX"]
    SonyUx,
    #[id = "d"]
    #[name = "TDK D"]
    TdkD,
    #[id = "hf"]
    #[name = "Sony HF"]
    SonyHf,
    #[id = "rst"]
    #[name = "Realistic Supertape"]
    RealisticSupertape,
    #[id = "mrx"]
    #[name = "Memorex"]
    Memorex,
    #[id = "gen"]
    #[name = "No-Name Ferric"]
    Generic,
}

impl TapeStockParam {
    pub fn to_dsp(self) -> te2_dsp::tape::TapeStock {
        use te2_dsp::tape::TapeStock as S;
        match self {
            TapeStockParam::MaxellXlii => S::MaxellXlii,
            TapeStockParam::TdkSa => S::TdkSa,
            TapeStockParam::TdkMa => S::TdkMa,
            TapeStockParam::SonyMetalEs => S::SonyMetalEs,
            TapeStockParam::BasfChromeMaxima => S::BasfChromeMaxima,
            TapeStockParam::NakamichiExii => S::NakamichiExii,
            TapeStockParam::TdkAd => S::TdkAd,
            TapeStockParam::MaxellUdii => S::MaxellUdii,
            TapeStockParam::SonyUx => S::SonyUx,
            TapeStockParam::TdkD => S::TdkD,
            TapeStockParam::SonyHf => S::SonyHf,
            TapeStockParam::RealisticSupertape => S::RealisticSupertape,
            TapeStockParam::Memorex => S::Memorex,
            TapeStockParam::Generic => S::Generic,
        }
    }
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum TransportMode {
    #[id = "echo"]
    #[name = "Rec/Echo"]
    Echo,
    #[id = "play"]
    #[name = "Play"]
    Play,
    #[id = "loop"]
    #[name = "Loop (Erase Bypass)"]
    Loop,
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum Quality {
    #[id = "eco"]
    Eco,
    #[id = "standard"]
    Standard,
    #[id = "ultra"]
    Ultra,
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum WindMode {
    #[id = "off"]
    Off,
    #[id = "rw"]
    #[name = "RW"]
    Rewind,
    #[id = "ff"]
    #[name = "FF"]
    FastForward,
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum RateDivision {
    #[id = "d1"]
    #[name = "1/1"]
    Whole,
    #[id = "d2"]
    #[name = "1/2"]
    Half,
    #[id = "d4"]
    #[name = "1/4"]
    Quarter,
    #[id = "d4t"]
    #[name = "1/4T"]
    QuarterTriplet,
    #[id = "d8"]
    #[name = "1/8"]
    Eighth,
    #[id = "d8t"]
    #[name = "1/8T"]
    EighthTriplet,
    #[id = "d16"]
    #[name = "1/16"]
    Sixteenth,
    #[id = "d32"]
    #[name = "1/32"]
    ThirtySecond,
}

#[derive(Params)]
pub struct Te2Params {
    // --- Primary controls (quick guide callouts 1-12) ---
    /// Delay time / tape speed. Maps to motor speed: longer time = slower tape.
    #[id = "time"]
    pub time: FloatParam,
    /// Echo sustain. >1.0 runs away into tape-limited self-oscillation.
    #[id = "fdbk"]
    pub feedback: FloatParam,
    /// 24 dB/oct OTA HPF in the echo path, ahead of the record head on regeneration.
    #[id = "hpf"]
    pub hpf: FloatParam,
    /// 24 dB/oct OTA LPF in the echo path.
    #[id = "lpf"]
    pub lpf: FloatParam,
    /// LPF resonance, reaches self-oscillation near the top.
    #[id = "res"]
    pub res: FloatParam,
    /// Level of source sent to tape — drives tape saturation. VU reads this.
    #[id = "tapein"]
    pub tape_in: FloatParam,
    /// Sine modulation amount on motor speed.
    #[id = "mod"]
    pub mod_amt: FloatParam,
    /// Sine modulation speed.
    #[id = "modspd"]
    pub mod_spd: FloatParam,
    /// Output level of all tape sounds.
    #[id = "tplvl"]
    pub tape_level: FloatParam,
    /// Output level of dry sound, from the input stage.
    #[id = "drylvl"]
    pub dry_level: FloatParam,
    /// Op-amp output drive, applied to tape and dry.
    #[id = "outdrv"]
    pub out_drive: FloatParam,
    /// Amount of anomaly pulse sent to motor time/speed.
    #[id = "anmly"]
    pub anomaly: FloatParam,

    // --- Position faders: positions 2-8 for each set (position 1 = panel knobs) ---
    #[id = "w2"]
    pub w2: FloatParam,
    #[id = "w3"]
    pub w3: FloatParam,
    #[id = "w4"]
    pub w4: FloatParam,
    #[id = "w5"]
    pub w5: FloatParam,
    #[id = "w6"]
    pub w6: FloatParam,
    #[id = "w7"]
    pub w7: FloatParam,
    #[id = "w8"]
    pub w8: FloatParam,
    #[id = "g2"]
    pub g2: FloatParam,
    #[id = "g3"]
    pub g3: FloatParam,
    #[id = "g4"]
    pub g4: FloatParam,
    #[id = "g5"]
    pub g5: FloatParam,
    #[id = "g6"]
    pub g6: FloatParam,
    #[id = "g7"]
    pub g7: FloatParam,
    #[id = "g8"]
    pub g8: FloatParam,
    #[id = "b2"]
    pub b2: FloatParam,
    #[id = "b3"]
    pub b3: FloatParam,
    #[id = "b4"]
    pub b4: FloatParam,
    #[id = "b5"]
    pub b5: FloatParam,
    #[id = "b6"]
    pub b6: FloatParam,
    #[id = "b7"]
    pub b7: FloatParam,
    #[id = "b8"]
    pub b8: FloatParam,

    // --- Sets ---
    #[id = "won"]
    pub white_on: BoolParam,
    #[id = "wsel"]
    pub white_sel: EnumParam<WhiteTarget>,
    #[id = "wdrift"]
    pub white_drift: FloatParam,
    #[id = "gon"]
    pub gray_on: BoolParam,
    #[id = "gsel"]
    pub gray_sel: EnumParam<GrayTarget>,
    #[id = "gdrift"]
    pub gray_drift: FloatParam,
    #[id = "bon"]
    pub black_on: BoolParam,
    #[id = "bsel"]
    pub black_sel: EnumParam<BlackTarget>,
    #[id = "bdrift"]
    pub black_drift: FloatParam,

    // --- Cycle ---
    #[id = "cyc"]
    pub cycle_run: BoolParam,
    #[id = "cyclen"]
    pub cycle_len: IntParam,
    /// Steps per second, 0.125 (8 s/step) to 4000 (audio rate).
    #[id = "rate"]
    pub cycle_rate: FloatParam,
    #[id = "rsync"]
    pub rate_sync: BoolParam,
    #[id = "rdiv"]
    pub rate_div: EnumParam<RateDivision>,
    /// Current position 1-8. Manual selection; the cycle overrides while running.
    #[id = "pos"]
    pub position: IntParam,

    // --- Switches & modes ---
    #[id = "apol"]
    pub anomaly_pol: EnumParam<AnomalyPolarity>,
    /// Gate switch: 1-8 buttons gate the LPF resonance (playable filter synth).
    #[id = "resgt"]
    pub res_gate: BoolParam,
    /// MIDI notes C3-G3 select/gate positions. Off by default so a keyboard
    /// routed to the track doesn't yank the sequencer around uninvited.
    #[id = "midi"]
    pub midi_enable: BoolParam,
    #[id = "ttype"]
    pub tape_type: EnumParam<TapeType>,
    // "inchr" (input character GT/-10/+4) was removed: it was a hardware
    // impedance/level-matching switch and never drove any DSP here — TAPE IN
    // is the input level. The id stays retired; old sessions ignore it.
    #[id = "tmode"]
    pub transport_mode: EnumParam<TransportMode>,
    /// Momentary motor kill: pitch drags to a dead stop, ramps back on release.
    #[id = "mtr"]
    pub motor_kill: BoolParam,
    /// PAUSE: fast mechanical stop retaining speed.
    #[id = "pause"]
    pub pause: BoolParam,
    /// STP: tape motion stopped.
    #[id = "stop"]
    pub stop: BoolParam,
    /// Held RW / FF (acts in Play mode).
    #[id = "wind"]
    pub wind: EnumParam<WindMode>,
    /// Loop length for Loop (erase bypass) mode, in seconds when not synced.
    #[id = "loopln"]
    pub loop_len: FloatParam,
    #[id = "loopsy"]
    pub loop_sync: BoolParam,

    // --- Global ---
    #[id = "outlvl"]
    pub out_level: FloatParam,
    #[id = "qual"]
    pub quality: EnumParam<Quality>,
    /// Tape noise level (1.0 = calibrated cassette hiss).
    #[id = "noise"]
    pub noise: FloatParam,
    /// Mechanism condition: 0 = serviced deck, 1 = thrift-store wreck.
    #[id = "mech"]
    pub mech: FloatParam,

    // --- Tape stock & aging (settings overlay) ---
    /// Which cassette is in the well (brand-grade; sets aging rate,
    /// base hiss and headroom).
    #[id = "stock"]
    pub tape_stock: EnumParam<TapeStockParam>,
    /// Tape aging master switch: off = pristine forever.
    #[id = "aging"]
    pub aging_on: BoolParam,
    /// Freeze the wear clock at its current value.
    #[id = "agfrz"]
    pub aging_freeze: BoolParam,
    /// Accumulated tape wear 0..1 — engine state, not a host parameter.
    /// Persisted with the project: it's physical wear on *this* loop.
    #[persist = "age"]
    pub tape_age: Arc<AtomicF32>,

    /// Editor scale factor (0.5..2.0), from the SETUP overlay. The window
    /// resizes to 1080x560 times this; the canvas scales with it.
    #[persist = "ui-scale"]
    pub ui_scale: Arc<AtomicF32>,

    /// Editor window state.
    #[persist = "egui-state"]
    pub editor_state: Arc<EguiState>,
}

impl Te2Params {
    pub fn white_faders(&self) -> [&FloatParam; 7] {
        [
            &self.w2, &self.w3, &self.w4, &self.w5, &self.w6, &self.w7, &self.w8,
        ]
    }

    pub fn gray_faders(&self) -> [&FloatParam; 7] {
        [
            &self.g2, &self.g3, &self.g4, &self.g5, &self.g6, &self.g7, &self.g8,
        ]
    }

    pub fn black_faders(&self) -> [&FloatParam; 7] {
        [
            &self.b2, &self.b3, &self.b4, &self.b5, &self.b6, &self.b7, &self.b8,
        ]
    }
}

/// Nominal delay range in seconds; TIME sweeps motor speed across this span.
pub const TIME_MIN: f32 = 0.06;
pub const TIME_MAX: f32 = 1.5;
pub const TIME_DEFAULT: f32 = 0.35;

fn fader(set: &str, position: usize) -> FloatParam {
    FloatParam::new(
        format!("{set} {position}"),
        0.5,
        FloatRange::Linear { min: 0.0, max: 1.0 },
    )
    .with_value_to_string(formatters::v2s_f32_rounded(3))
}

fn drift(name: &str) -> FloatParam {
    FloatParam::new(
        name.to_string(),
        0.0,
        FloatRange::Skewed {
            min: 0.0,
            max: 14.0,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_unit(" s")
    .with_value_to_string(formatters::v2s_f32_rounded(2))
}

impl Default for Te2Params {
    fn default() -> Self {
        Self {
            time: FloatParam::new(
                "Time",
                TIME_DEFAULT,
                FloatRange::Skewed {
                    min: TIME_MIN,
                    max: TIME_MAX,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            // Motor inertia lives in the DSP engine; no param smoothing on top.
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            // Past 100% the loop gain genuinely exceeds unity and the tape
            // does the limiting. 150% is ~3.5 dB of excess gain — runaway
            // blooms in a couple of repeats instead of creeping up.
            feedback: FloatParam::new(
                "Feedback",
                0.45,
                FloatRange::Linear { min: 0.0, max: 1.5 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            hpf: FloatParam::new(
                "HPF",
                30.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 2_000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),

            lpf: FloatParam::new(
                "LPF",
                7_000.0,
                FloatRange::Skewed {
                    min: 100.0,
                    max: 18_000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),

            res: FloatParam::new("Resonance", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(1))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            tape_in: FloatParam::new(
                "Tape In",
                1.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 8.0, // up to +18 dB into tape
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            mod_amt: FloatParam::new("Mod Amt", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(1))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            mod_spd: FloatParam::new(
                "Mod Spd",
                0.5,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 150.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            tape_level: FloatParam::new(
                "Tape Level",
                0.8,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 1.25,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            dry_level: FloatParam::new(
                "Dry Level",
                1.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 1.25,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            out_drive: FloatParam::new(
                "Out Drive",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            anomaly: FloatParam::new("Anomaly", 0.3, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(1))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            w2: fader("White", 2),
            w3: fader("White", 3),
            w4: fader("White", 4),
            w5: fader("White", 5),
            w6: fader("White", 6),
            w7: fader("White", 7),
            w8: fader("White", 8),
            g2: fader("Gray", 2),
            g3: fader("Gray", 3),
            g4: fader("Gray", 4),
            g5: fader("Gray", 5),
            g6: fader("Gray", 6),
            g7: fader("Gray", 7),
            g8: fader("Gray", 8),
            b2: fader("Black", 2),
            b3: fader("Black", 3),
            b4: fader("Black", 4),
            b5: fader("Black", 5),
            b6: fader("Black", 6),
            b7: fader("Black", 7),
            b8: fader("Black", 8),

            white_on: BoolParam::new("White On", false),
            white_sel: EnumParam::new("White Target", WhiteTarget::Time),
            white_drift: drift("White Drift"),
            gray_on: BoolParam::new("Gray On", false),
            gray_sel: EnumParam::new("Gray Target", GrayTarget::Feedback),
            gray_drift: drift("Gray Drift"),
            black_on: BoolParam::new("Black On", false),
            black_sel: EnumParam::new("Black Target", BlackTarget::TapeLevel),
            black_drift: drift("Black Drift"),

            cycle_run: BoolParam::new("Cycle", false),
            cycle_len: IntParam::new("Cycle Length", 8, IntRange::Linear { min: 1, max: 8 }),
            cycle_rate: FloatParam::new(
                "Cycle Rate",
                1.0,
                FloatRange::Skewed {
                    min: 0.125,
                    max: 4_000.0,
                    factor: FloatRange::skew_factor(-2.5),
                },
            )
            .with_unit(" steps/s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),
            rate_sync: BoolParam::new("Rate Sync", false),
            rate_div: EnumParam::new("Rate Division", RateDivision::Quarter),
            position: IntParam::new("Position", 1, IntRange::Linear { min: 1, max: 8 }),

            anomaly_pol: EnumParam::new("Anomaly Polarity", AnomalyPolarity::Off),
            res_gate: BoolParam::new("Res Gate", false),
            midi_enable: BoolParam::new("MIDI Positions", false),
            // Chrome: matches the default stock (Maxell XL-II, Type II) so a
            // fresh instance isn't born with a mis-set deck. Old sessions
            // carry their own saved value.
            tape_type: EnumParam::new("Tape Type", TapeType::Chrome),
            transport_mode: EnumParam::new("Transport", TransportMode::Echo),
            motor_kill: BoolParam::new("Motor", false),
            pause: BoolParam::new("Pause", false),
            stop: BoolParam::new("Stop", false),
            wind: EnumParam::new("Wind", WindMode::Off),
            loop_len: FloatParam::new(
                "Loop Length",
                4.0,
                FloatRange::Skewed {
                    min: 0.5,
                    max: 30.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            loop_sync: BoolParam::new("Loop Sync", false),

            out_level: FloatParam::new(
                "Output",
                1.0,
                FloatRange::Skewed {
                    min: util::db_to_gain(-24.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-24.0, 6.0),
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            quality: EnumParam::new("Quality", Quality::Standard),
            noise: FloatParam::new("Noise", 1.0, FloatRange::Linear { min: 0.0, max: 1.5 })
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            mech: FloatParam::new("Mechanism", 0.35, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            tape_stock: EnumParam::new("Tape Stock", TapeStockParam::MaxellXlii),
            aging_on: BoolParam::new("Tape Aging", true),
            aging_freeze: BoolParam::new("Freeze Aging", false),
            tape_age: Arc::new(AtomicF32::new(0.0)),
            ui_scale: Arc::new(AtomicF32::new(1.0)),

            editor_state: EguiState::from_size(1080, 560),
        }
    }
}
