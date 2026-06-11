//! The tape itself: a position-indexed circular buffer with a record head
//! that deposits host samples onto crossed cells and a repro head reading a
//! fixed gap behind it.
//!
//! Writing works like the physical process: as the tape advances by `delta`
//! cells in one host sample, every cell crossed gets the interpolated input
//! value at the moment it passed the head. Because positions are monotonic in
//! record modes, every cell is written exactly once per revolution — the
//! erase factor decides how much of the previous revolution survives
//! (~0 normally, more in loop/erase-bypass mode).
//!
//! Deposit interpolation runs one host sample behind so a full 4-point window
//! exists around the crossing time. One extra sample of delay inside a delay
//! line is immaterial.

/// 4-point, 3rd-order Hermite (Catmull-Rom) between y1 and y2.
#[inline]
fn hermite4(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + c0
}

pub struct TapeReel {
    /// Interleaved stereo cells [l, r, l, r, ...] for cache friendliness.
    cells: Vec<f32>,
    /// Active loop length in cells (echo mode: the full allocation).
    len: usize,
    capacity: usize,

    /// Record head position in cells, [0, len). This is the position of the
    /// *deposited frontier* (one host sample behind the physical head).
    write_pos: f64,
    /// Tape advance during the in-flight (not yet deposited) host sample.
    pending_delta: f64,

    /// Host-side history per channel for deposit interpolation:
    /// [x[k-3], x[k-2], x[k-1], x[k]].
    hist: [[f32; 4]; 2],
}

impl TapeReel {
    pub fn new(capacity_cells: usize) -> Self {
        Self {
            cells: vec![0.0; capacity_cells * 2],
            len: capacity_cells,
            capacity: capacity_cells,
            write_pos: 0.0,
            pending_delta: 0.0,
            hist: [[0.0; 4]; 2],
        }
    }

    /// Set the active loop length (loop mode). Clamped to capacity.
    pub fn set_loop_len(&mut self, cells: usize) {
        self.len = cells.clamp(4, self.capacity);
        if self.write_pos >= self.len as f64 {
            self.write_pos = self.write_pos.rem_euclid(self.len as f64);
        }
    }

    pub fn loop_len(&self) -> usize {
        self.len
    }

    /// Advance the tape by `delta` cells with the record head live.
    /// `input` is the current host sample after the record chain.
    /// `erase_keep` is how much of the old cell content survives the
    /// erase+record pass (0.0..1.0).
    pub fn advance_record(&mut self, delta: f64, input: [f32; 2], erase_keep: f32) {
        debug_assert!(delta >= 0.0);

        for ch in 0..2 {
            let h = &mut self.hist[ch];
            *h = [h[1], h[2], h[3], input[ch]];
        }

        // Deposit the *previous* interval (between hist[1] and hist[2]).
        let step = self.pending_delta;
        self.pending_delta = delta;
        if step <= 0.0 {
            return;
        }

        let start = self.write_pos;
        let end = start + step;
        let len = self.len as f64;

        // Cells crossed: integers in (start, end].
        let mut c = start.floor() + 1.0;
        while c <= end {
            let t = ((c - start) / step) as f32;
            let idx = (c.rem_euclid(len)) as usize % self.len;
            for ch in 0..2 {
                let h = &self.hist[ch];
                let value = hermite4(h[0], h[1], h[2], h[3], t);
                let cell = &mut self.cells[idx * 2 + ch];
                *cell = *cell * erase_keep + value;
            }
            c += 1.0;
        }

        self.write_pos = end.rem_euclid(len);
    }

    /// Advance the tape with the record head off (play / wind modes).
    /// `delta` may be negative.
    pub fn advance_play(&mut self, delta: f64) {
        self.pending_delta = 0.0;
        self.write_pos = (self.write_pos + delta).rem_euclid(self.len as f64);
    }

    /// Read the tape at `offset` cells behind the record head.
    pub fn read(&self, offset: f64) -> [f32; 2] {
        let len = self.len as f64;
        let pos = (self.write_pos - offset).rem_euclid(len);
        let base = pos.floor();
        let t = (pos - base) as f32;
        let i = base as usize % self.len;

        let im1 = (i + self.len - 1) % self.len;
        let ip1 = (i + 1) % self.len;
        let ip2 = (i + 2) % self.len;

        let mut out = [0.0f32; 2];
        for ch in 0..2 {
            out[ch] = hermite4(
                self.cells[im1 * 2 + ch],
                self.cells[i * 2 + ch],
                self.cells[ip1 * 2 + ch],
                self.cells[ip2 * 2 + ch],
                t,
            );
        }
        out
    }

    pub fn reset(&mut self) {
        self.cells.fill(0.0);
        self.write_pos = 0.0;
        self.pending_delta = 0.0;
        self.hist = [[0.0; 4]; 2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_speed_roundtrip_delay() {
        // At delta=2 cells/sample and a gap of 200 cells, an impulse should
        // come back exactly 100 host samples later.
        let mut reel = TapeReel::new(10_000);
        let gap = 200.0;
        let mut impulse_seen_at = None;

        for k in 0..400 {
            let x = if k == 10 { 1.0 } else { 0.0 };
            reel.advance_record(2.0, [x, x], 0.0);
            let y = reel.read(gap)[0];
            if y.abs() > 0.5 && impulse_seen_at.is_none() {
                impulse_seen_at = Some(k);
            }
        }

        // Deposit pipeline adds one host sample; impulse at k=10 lands around
        // cell 22, read head reaches it 100 samples later.
        let seen = impulse_seen_at.expect("echo never arrived");
        assert!(
            (seen as i64 - 111).unsigned_abs() <= 2,
            "echo arrived at {seen}, expected ~111"
        );
    }

    #[test]
    fn sine_roundtrip_is_clean() {
        // A 1 kHz sine through deposit+read at an awkward speed ratio should
        // come back with very low error against an ideally delayed copy.
        let sr = 48_000.0f64;
        let f = 1000.0f64;
        let delta = 1.7919; // deliberately irrational-ish cells/sample
        let gap = 4000.0; // cells -> gap/delta host samples of delay
        let delay_samples = gap / delta + 1.0; // +1 for deposit pipeline

        let mut reel = TapeReel::new(100_000);
        let n = 20_000usize;
        let mut err_energy = 0.0f64;
        let mut sig_energy = 0.0f64;

        for k in 0..n {
            let x = (std::f64::consts::TAU * f * k as f64 / sr).sin() as f32;
            reel.advance_record(delta, [x, x], 0.0);
            let y = reel.read(gap)[0] as f64;

            // Skip until the delay line is fully primed.
            let k_src = k as f64 - delay_samples;
            if k_src > delay_samples + 64.0 {
                let ideal = (std::f64::consts::TAU * f * k_src / sr).sin();
                err_energy += (y - ideal) * (y - ideal);
                sig_energy += ideal * ideal;
            }
        }

        let snr_db = 10.0 * (sig_energy / err_energy.max(1e-30)).log10();
        assert!(snr_db > 90.0, "round-trip SNR too low: {snr_db:.1} dB");
    }
}
