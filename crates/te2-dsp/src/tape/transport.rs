//! Motor model: speed targets with mechanical inertia, transport states, and
//! the intentional sine speed modulation (MOD AMT / MOD SPD).
//!
//! All speed-affecting features end up as one number per sample: the relative
//! tape speed `v` (1.0 = nominal). Everything downstream (delay time, repitch,
//! wind-down effects, bandwidth) derives from it.

/// What the mechanism is currently doing. Phase 6 wires the full transport
/// button logic; the motor already understands all the states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mechanism {
    Stopped,
    /// Tape rolling, record head live (echo or loop layering).
    #[default]
    Recording,
    /// Tape rolling, playback only.
    Playing,
    /// Pinch roller lifted: tape stops fast, speed setting retained.
    Paused,
    /// Held rewind: reverse playback, proportional to TIME.
    Rewinding,
    /// Held fast-forward: high speed playback, proportional to TIME.
    FastForwarding,
}

/// Speed multipliers for held wind transports (the PDF: "proportionally
/// faster than playback speed").
const WIND_MULT: f64 = 4.0;

pub struct Motor {
    sample_rate: f64,

    /// Critically damped 2nd-order follower for the TIME-knob speed target.
    speed: f64,
    speed_vel: f64,
    target_speed: f64,
    /// Natural frequency of the follower (rad/s); settle time ~= 4/omega.
    omega: f64,

    /// Motor-kill (MTR button) multiplier, one-pole slewed 1 -> 0 -> 1.
    kill_gain: f64,
    kill_engaged: bool,
    kill_down_coeff: f64,
    kill_up_coeff: f64,

    /// Pause multiplier: much faster mechanical ramp.
    pause_gain: f64,
    pause_coeff: f64,

    pub mechanism: Mechanism,

    // Intentional sine modulation of motor speed.
    mod_amount: f64,
    mod_phase: f64,
    mod_inc: f64,
}

impl Motor {
    pub fn new(sample_rate: f64) -> Self {
        let mut motor = Self {
            sample_rate,
            speed: 1.0,
            speed_vel: 0.0,
            target_speed: 1.0,
            omega: 0.0,
            kill_gain: 1.0,
            kill_engaged: false,
            kill_down_coeff: 0.0,
            kill_up_coeff: 0.0,
            pause_gain: 1.0,
            pause_coeff: 0.0,
            mechanism: Mechanism::default(),
            mod_amount: 0.0,
            mod_phase: 0.0,
            mod_inc: 0.0,
        };
        motor.set_inertia(0.12);
        motor.set_kill_ramp(0.4, 0.25);
        motor.pause_coeff = one_pole_coeff(0.03, sample_rate);
        motor
    }

    /// TIME-knob inertia: approximate settle time in seconds.
    pub fn set_inertia(&mut self, settle_seconds: f64) {
        self.omega = 4.0 / settle_seconds.max(0.01);
    }

    /// MTR button wind-down / wind-up times in seconds (panel trim screw).
    pub fn set_kill_ramp(&mut self, down_seconds: f64, up_seconds: f64) {
        self.kill_down_coeff = one_pole_coeff(down_seconds.max(0.01) / 4.0, self.sample_rate);
        self.kill_up_coeff = one_pole_coeff(up_seconds.max(0.01) / 4.0, self.sample_rate);
    }

    pub fn set_target_speed(&mut self, speed: f64) {
        self.target_speed = speed;
    }

    pub fn set_motor_kill(&mut self, engaged: bool) {
        self.kill_engaged = engaged;
    }

    pub fn set_modulation(&mut self, amount: f64, speed_hz: f64) {
        self.mod_amount = amount.clamp(0.0, 1.0);
        self.mod_inc = std::f64::consts::TAU * speed_hz / self.sample_rate;
    }

    /// Current speed without modulation, for UI reel animation and metering.
    pub fn current_speed(&self) -> f64 {
        let dir = match self.mechanism {
            Mechanism::Rewinding => -WIND_MULT,
            Mechanism::FastForwarding => WIND_MULT,
            Mechanism::Stopped => 0.0,
            _ => 1.0,
        };
        self.speed * self.kill_gain * self.pause_gain * dir
    }

    /// Advance one sample; returns the relative tape speed `v` for this sample.
    /// May be negative (rewind) or zero (stopped/paused/killed).
    #[inline]
    pub fn process(&mut self) -> f64 {
        let dt = 1.0 / self.sample_rate;

        // Critically damped follower toward the TIME target.
        let accel =
            self.omega * self.omega * (self.target_speed - self.speed) - 2.0 * self.omega * self.speed_vel;
        self.speed_vel += accel * dt;
        self.speed += self.speed_vel * dt;

        // MTR kill: drag down to a dead stop, ramp back up on release.
        let (kill_target, kill_coeff) = if self.kill_engaged {
            (0.0, self.kill_down_coeff)
        } else {
            (1.0, self.kill_up_coeff)
        };
        self.kill_gain += kill_coeff * (kill_target - self.kill_gain);

        // Pause / stop: fast mechanical ramp.
        let pause_target = match self.mechanism {
            Mechanism::Paused | Mechanism::Stopped => 0.0,
            _ => 1.0,
        };
        self.pause_gain += self.pause_coeff * (pause_target - self.pause_gain);

        // Intentional sine modulation. Full AMT is a deep, obviously musical
        // wobble; tapered so the lower half of the knob stays subtle.
        let modulation = if self.mod_amount > 0.0 {
            self.mod_phase += self.mod_inc;
            if self.mod_phase >= std::f64::consts::TAU {
                self.mod_phase -= std::f64::consts::TAU;
            }
            let depth = self.mod_amount * self.mod_amount * 0.5;
            1.0 + depth * self.mod_phase.sin()
        } else {
            1.0
        };

        let dir = match self.mechanism {
            Mechanism::Rewinding => -WIND_MULT,
            Mechanism::FastForwarding => WIND_MULT,
            _ => 1.0,
        };

        (self.speed * modulation * self.kill_gain * self.pause_gain * dir).clamp(-12.0, 12.0)
    }

    pub fn reset(&mut self) {
        self.speed = self.target_speed;
        self.speed_vel = 0.0;
        self.kill_gain = 1.0;
        self.pause_gain = 1.0;
        self.mod_phase = 0.0;
    }
}

#[inline]
fn one_pole_coeff(tau_seconds: f64, sample_rate: f64) -> f64 {
    1.0 - (-1.0 / (tau_seconds * sample_rate)).exp()
}
