//! Space Case TE-2 — cassette tape echo plugin shell (nice-plug).

mod params;
pub mod presets;
pub mod ui;
mod update;

use nice_plug::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use te2_dsp::{EngineParams, Te2Engine};

pub use params::Te2Params;

/// State shared between the audio thread and the editor: meters, the live
/// motor speed for the reel animation, and UI-originated one-shots.
pub struct UiShared {
    pub vu: AtomicF32,
    pub speed: AtomicF32,
    pub position: AtomicU8,
    /// Signed seconds of tape past the heads (spool animation source).
    pub footage: AtomicF32,
    /// STP/EJ double-press: swap in a fresh cassette.
    pub eject: AtomicBool,
    /// A 1-8 panel button is held (RES gate).
    pub ui_gate: AtomicBool,
    /// A newer version is on the website (set by the background update check;
    /// read by the editor for the faceplate nudge + the SETUP line).
    pub update_available: AtomicBool,
    /// (latest version, download URL) when an update is available.
    pub update_info: Mutex<Option<(String, String)>>,
}

impl Default for UiShared {
    fn default() -> Self {
        Self {
            vu: AtomicF32::new(0.0),
            speed: AtomicF32::new(1.0),
            position: AtomicU8::new(1),
            footage: AtomicF32::new(0.0),
            eject: AtomicBool::new(false),
            ui_gate: AtomicBool::new(false),
            update_available: AtomicBool::new(false),
            update_info: Mutex::new(None),
        }
    }
}

pub struct SpaceCaseTe2 {
    params: Arc<Te2Params>,
    engine: Option<Te2Engine>,
    ui_shared: Arc<UiShared>,
    /// Position selected by a MIDI note, overriding the position parameter
    /// while active.
    midi_position: Option<u8>,
    /// Notes currently held (for RES gating); lowest-numbered wins position.
    held_notes: u32,
    /// Last tape-wear value we wrote into the persisted field. When the
    /// field no longer matches (state load), the engine gets re-seeded.
    age_written: f32,
}

impl Default for SpaceCaseTe2 {
    fn default() -> Self {
        Self {
            params: Arc::new(Te2Params::default()),
            engine: None,
            ui_shared: Arc::new(UiShared::default()),
            midi_position: None,
            held_notes: 0,
            age_written: -1.0,
        }
    }
}

/// MIDI notes C3..G3 (60..=67) select positions 1..=8.
const POSITION_BASE_NOTE: u8 = 60;

impl Plugin for SpaceCaseTe2 {
    const NAME: &'static str = "RE-DEEMER";
    const VENDOR: &'static str = "naturarum";
    const URL: &'static str = "https://spacecasetapeecho.com";
    const EMAIL: &'static str = "deepchord@icloud.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::create(self.params.clone(), self.ui_shared.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine = Some(Te2Engine::new(buffer_config.sample_rate as f64));
        true
    }

    fn reset(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Subnormal floats in the engine's feedback/filter/J-A tails are ~free
        // on Apple Silicon but 10-100x slower on x86, and hosts do NOT reliably
        // set flush-to-zero on the audio thread (the VST3/CLAP/AU specs make no
        // such promise, and a host may run process on worker threads). Guarantee
        // it once per callback; this also covers the standalone and the
        // clap-wrapper AU, which route through this same process().
        te2_dsp::denormals::ensure_flush_to_zero();

        // Consume note events: notes select positions and gate the RES synth.
        // Only when the panel MIDI switch is on — otherwise a keyboard routed
        // to the track would yank the 1-8 positions around uninvited.
        let midi_enabled = self.params.midi_enable.value();
        if !midi_enabled && self.held_notes != 0 {
            // Switch was just turned off with notes held: release everything.
            self.held_notes = 0;
            self.midi_position = None;
        }
        while let Some(event) = context.next_event() {
            if !midi_enabled {
                continue;
            }
            match event {
                NoteEvent::NoteOn { note, .. } => {
                    if (POSITION_BASE_NOTE..POSITION_BASE_NOTE + 8).contains(&note) {
                        let idx = note - POSITION_BASE_NOTE;
                        self.held_notes |= 1 << idx;
                        self.midi_position = Some(idx + 1);
                    }
                }
                NoteEvent::NoteOff { note, .. }
                    if (POSITION_BASE_NOTE..POSITION_BASE_NOTE + 8).contains(&note) =>
                {
                    self.held_notes &= !(1 << (note - POSITION_BASE_NOTE));
                    if self.held_notes == 0 {
                        self.midi_position = None;
                    } else {
                        // Fall back to the lowest still-held note.
                        let low = self.held_notes.trailing_zeros() as u8;
                        self.midi_position = Some(low + 1);
                    }
                }
                _ => {}
            }
        }

        let Some(engine) = &mut self.engine else {
            return ProcessStatus::Error("plugin not initialized");
        };

        // Tape wear lives in the engine; the persisted field mirrors it.
        // If the field changed under us (project/preset load), re-seed the
        // engine; otherwise the engine value is authoritative.
        let persisted_age = self
            .params
            .tape_age
            .load(std::sync::atomic::Ordering::Relaxed);
        if persisted_age != self.age_written {
            engine.set_age(persisted_age);
        }

        // Cycle rate: free-running or synced to the host tempo. When synced
        // and the transport is rolling, the cycle is also phase-locked to
        // the playhead so steps land on the DAW grid (and survive loops,
        // jumps and tempo changes).
        let beats_per_step = match self.params.rate_div.value() {
            params::RateDivision::Whole => 4.0,
            params::RateDivision::Half => 2.0,
            params::RateDivision::Quarter => 1.0,
            params::RateDivision::QuarterTriplet => 2.0 / 3.0,
            params::RateDivision::Eighth => 0.5,
            params::RateDivision::EighthTriplet => 1.0 / 3.0,
            params::RateDivision::Sixteenth => 0.25,
            params::RateDivision::ThirtySecond => 0.125,
        };
        let rate_synced = self.params.rate_sync.value();
        let cycle_rate = if rate_synced {
            let tempo = context.transport().tempo.unwrap_or(120.0) as f32;
            (tempo / 60.0 / beats_per_step).clamp(0.05, 4_000.0)
        } else {
            self.params.cycle_rate.value()
        };
        let host_step_pos = if rate_synced && context.transport().playing {
            context
                .transport()
                .pos_beats()
                .map(|beats| beats / beats_per_step as f64)
        } else {
            None
        };

        let fader = |p: &FloatParam| p.value();
        let seq = te2_dsp::sequencer::SeqConfig {
            white_faders: self.params.white_faders().map(fader),
            gray_faders: self.params.gray_faders().map(fader),
            black_faders: self.params.black_faders().map(fader),
            white_on: self.params.white_on.value(),
            gray_on: self.params.gray_on.value(),
            black_on: self.params.black_on.value(),
            white_target: match self.params.white_sel.value() {
                params::WhiteTarget::Time => te2_dsp::sequencer::WhiteTarget::Time,
                params::WhiteTarget::Resonance => te2_dsp::sequencer::WhiteTarget::Resonance,
                params::WhiteTarget::ModSpeed => te2_dsp::sequencer::WhiteTarget::ModSpeed,
            },
            gray_target: match self.params.gray_sel.value() {
                params::GrayTarget::Feedback => te2_dsp::sequencer::GrayTarget::Feedback,
                params::GrayTarget::ModAmount => te2_dsp::sequencer::GrayTarget::ModAmount,
                params::GrayTarget::Lpf => te2_dsp::sequencer::GrayTarget::Lpf,
            },
            black_target: match self.params.black_sel.value() {
                params::BlackTarget::TapeLevel => te2_dsp::sequencer::BlackTarget::TapeLevel,
                params::BlackTarget::DryLevel => te2_dsp::sequencer::BlackTarget::DryLevel,
                params::BlackTarget::Hpf => te2_dsp::sequencer::BlackTarget::Hpf,
            },
            white_drift: self.params.white_drift.value(),
            gray_drift: self.params.gray_drift.value(),
            black_drift: self.params.black_drift.value(),
            cycle_run: self.params.cycle_run.value(),
            cycle_len: self.params.cycle_len.value() as u8,
            cycle_rate,
            host_step_pos,
            manual_position: self
                .midi_position
                .unwrap_or(self.params.position.value() as u8),
            anomaly_amount: self.params.anomaly.value(),
            anomaly_polarity: match self.params.anomaly_pol.value() {
                params::AnomalyPolarity::Minus => te2_dsp::sequencer::AnomalyPolarity::Minus,
                params::AnomalyPolarity::Off => te2_dsp::sequencer::AnomalyPolarity::Off,
                params::AnomalyPolarity::Plus => te2_dsp::sequencer::AnomalyPolarity::Plus,
            },
            // Panel-u values are filled in by the engine.
            ..Default::default()
        };

        // Engine parameters update at control rate; with sample-accurate
        // automation enabled the host splits buffers on param changes anyway.
        engine.set_params(&EngineParams {
            delay_time: self.params.time.value(),
            feedback: self.params.feedback.value(),
            tape_in: self.params.tape_in.value(),
            tape_level: self.params.tape_level.value(),
            dry_level: self.params.dry_level.value(),
            mod_amount: self.params.mod_amt.value(),
            mod_speed_hz: self.params.mod_spd.value(),
            motor_kill: self.params.motor_kill.value(),
            slip: 0.0,
            hpf_hz: self.params.hpf.value(),
            lpf_hz: self.params.lpf.value(),
            res: self.params.res.value(),
            // RES gating happens in the engine via res_gate_enabled +
            // gate_held; this is a separate hard kill we never use.
            res_active: true,
            out_drive: self.params.out_drive.value(),
            tape_kind: match self.params.tape_type.value() {
                params::TapeType::Normal => te2_dsp::tape::TapeKind::I,
                params::TapeType::Chrome => te2_dsp::tape::TapeKind::II,
                params::TapeType::Metal => te2_dsp::tape::TapeKind::IV,
            },
            os_factor: match self.params.quality.value() {
                params::Quality::Eco => te2_dsp::oversample::OsFactor::X2,
                params::Quality::Standard => te2_dsp::oversample::OsFactor::X4,
                params::Quality::Ultra => te2_dsp::oversample::OsFactor::X8,
            },
            condition: self.params.mech.value(),
            noise_amount: self.params.noise.value(),
            stock: self.params.tape_stock.value().to_dsp(),
            aging_on: self.params.aging_on.value(),
            aging_freeze: self.params.aging_freeze.value(),
            seq,
            res_gate_enabled: self.params.res_gate.value(),
            gate_held: self.held_notes != 0 || self.ui_shared.ui_gate.load(Ordering::Relaxed),
            transport: match self.params.transport_mode.value() {
                params::TransportMode::Echo => te2_dsp::engine::TransportKind::Echo,
                params::TransportMode::Play => te2_dsp::engine::TransportKind::Play,
                params::TransportMode::Loop => te2_dsp::engine::TransportKind::Loop,
            },
            pause: self.params.pause.value(),
            stop: self.params.stop.value(),
            wind: match self.params.wind.value() {
                params::WindMode::Off => te2_dsp::engine::Wind::Off,
                params::WindMode::Rewind => te2_dsp::engine::Wind::Rewind,
                params::WindMode::FastForward => te2_dsp::engine::Wind::FastForward,
            },
            loop_len_s: if self.params.loop_sync.value() {
                // Snap the loop to whole beats of the host tempo.
                let tempo = context.transport().tempo.unwrap_or(120.0) as f32;
                let beat = 60.0 / tempo.max(20.0);
                let beats = (self.params.loop_len.value() / beat).round().max(1.0);
                beats * beat
            } else {
                self.params.loop_len.value()
            },
        });

        if self.ui_shared.eject.swap(false, Ordering::Relaxed) {
            engine.eject();
        }

        for mut frame in buffer.iter_samples() {
            let out_gain = self.params.out_level.smoothed.next();

            let left = frame.get_mut(0).map(|s| *s).unwrap_or(0.0);
            let right = frame.get_mut(1).map(|s| *s).unwrap_or(left);
            let (out_l, out_r) = engine.process(left, right);

            if let Some(s) = frame.get_mut(0) {
                *s = out_l * out_gain;
            }
            if let Some(s) = frame.get_mut(1) {
                *s = out_r * out_gain;
            }
        }

        // Mirror the engine's wear back into the persisted field (also what
        // the editor's wear bar reads).
        let age_now = engine.age();
        self.params
            .tape_age
            .store(age_now, std::sync::atomic::Ordering::Relaxed);
        self.age_written = age_now;

        // Feed the editor: meter, reel speed, position LEDs.
        if self.params.editor_state.is_open() {
            self.ui_shared
                .vu
                .store(engine.vu_level(), Ordering::Relaxed);
            self.ui_shared
                .speed
                .store(engine.motor_speed() as f32, Ordering::Relaxed);
            self.ui_shared
                .position
                .store(engine.current_position(), Ordering::Relaxed);
            self.ui_shared
                .footage
                .store(engine.tape_footage_seconds() as f32, Ordering::Relaxed);
        }

        // The tape keeps making sound (noise, feedback tails) after input stops.
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for SpaceCaseTe2 {
    const CLAP_ID: &'static str = "com.naturarum.re-deemer";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("RE-DEEMER cassette tape echo — a Space Case TE-2 tribute");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Delay,
        ClapFeature::Distortion,
    ];
}

impl Vst3Plugin for SpaceCaseTe2 {
    const VST3_CLASS_ID: [u8; 16] = *b"ReDeemerTapeEcho";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Delay];
}

nice_export_clap!(SpaceCaseTe2);
nice_export_vst3!(SpaceCaseTe2);
