//! Flush-to-zero / denormals-are-zero control for the audio thread.
//!
//! Subnormal (denormal) floats arise naturally in IIR feedback and filter
//! tails as they decay toward silence. On x86-64 the FPU handles subnormals in
//! microcode — often 10–100× slower per operation than a normal float — so an
//! engine that idles cheaply can spike the CPU for as long as a tail lingers in
//! the subnormal range. (AArch64 / Apple Silicon is far less affected: this
//! engine measures ~1× with or without flushing — see `examples/denormal_bench`.)
//!
//! Hosts are NOT guaranteed to set flush-to-zero on the thread that calls the
//! plugin — the VST3 / CLAP / AU specs make no such promise, and a host may run
//! `process` on worker threads whose control word was never set — and VCV
//! Rack's bare engine thread certainly does not. So the engine cannot rely on
//! the host. Every audio-thread entry point calls [`ensure_flush_to_zero`] once
//! per callback: the plugin's `process` (also covering the standalone and the
//! clap-wrapper AU), the offline render `main`, and the C ABI's `te2_process`
//! (Rack). Cheap cross-platform insurance for x86 hosts and thread migration.

/// Is flush-to-zero currently enabled on the calling thread?
#[inline]
pub fn flush_to_zero_enabled() -> bool {
    imp::enabled()
}

/// Enable or disable flush-to-zero (and denormals-are-zero, where the
/// architecture distinguishes them) on the CALLING thread.
///
/// This is per-thread CPU state. Production audio code wants it enabled and
/// should call [`ensure_flush_to_zero`]; this explicit setter exists mainly so
/// benchmarks and tests can measure the with/without difference.
#[inline]
pub fn set_flush_to_zero(enabled: bool) {
    imp::set(enabled);
}

/// Ensure flush-to-zero is enabled on the calling thread, as cheaply as
/// possible: the FPU control word is read first and rewritten only when the
/// bits aren't already set, so the steady-state cost is a single register
/// read. Cheap enough to call once per audio-thread entry — which also makes
/// it robust to a host that migrates the work between worker threads.
#[inline]
pub fn ensure_flush_to_zero() {
    if !imp::enabled() {
        imp::set(true);
    }
}

#[cfg(target_arch = "x86_64")]
mod imp {
    // `_mm_getcsr`/`_mm_setcsr` are deprecated (the docs suggest inline asm),
    // but they are stable, correct, and the clearest way to touch FTZ/DAZ.
    // We keep them rather than ship hand-written MXCSR asm that can't be
    // exercised from an arm64 dev machine.
    #![allow(deprecated)]
    use core::arch::x86_64::{_mm_getcsr, _mm_setcsr};

    // MXCSR: FTZ = flush-to-zero (bit 15), DAZ = denormals-are-zero (bit 6).
    const FTZ: u32 = 1 << 15;
    const DAZ: u32 = 1 << 6;

    #[inline]
    pub fn enabled() -> bool {
        let csr = unsafe { _mm_getcsr() };
        (csr & (FTZ | DAZ)) == (FTZ | DAZ)
    }

    #[inline]
    pub fn set(on: bool) {
        unsafe {
            let csr = _mm_getcsr();
            let csr = if on { csr | FTZ | DAZ } else { csr & !(FTZ | DAZ) };
            _mm_setcsr(csr);
        }
    }
}

#[cfg(target_arch = "aarch64")]
mod imp {
    // FPCR bit 24 = FZ (flush-to-zero for single/double precision). AArch64
    // has no separate "denormals-are-zero": FZ flushes both subnormal inputs
    // and subnormal results to zero.
    const FZ: u64 = 1 << 24;

    #[inline]
    fn read_fpcr() -> u64 {
        let v: u64;
        // SAFETY: reads the floating-point control register; no memory access.
        unsafe {
            core::arch::asm!("mrs {0}, fpcr", out(reg) v, options(nomem, nostack, preserves_flags));
        }
        v
    }

    #[inline]
    fn write_fpcr(v: u64) {
        // SAFETY: writes the floating-point control register; no memory access.
        unsafe {
            core::arch::asm!("msr fpcr, {0}", in(reg) v, options(nomem, nostack, preserves_flags));
        }
    }

    #[inline]
    pub fn enabled() -> bool {
        (read_fpcr() & FZ) == FZ
    }

    #[inline]
    pub fn set(on: bool) {
        let v = read_fpcr();
        let v = if on { v | FZ } else { v & !FZ };
        write_fpcr(v);
    }
}

// Other architectures: nothing to do. Report "enabled" so `ensure_*` is a
// no-op and never spins trying to set bits it can't.
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
mod imp {
    #[inline]
    pub fn enabled() -> bool {
        true
    }
    #[inline]
    pub fn set(_on: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn flush_to_zero_actually_flushes() {
        let subnormal = f32::from_bits(1); // smallest positive subnormal
        assert!(subnormal.is_subnormal(), "test setup: not a subnormal");

        // Baseline: with flushing off, a subnormal survives an FPU multiply.
        set_flush_to_zero(false);
        assert!(!flush_to_zero_enabled());
        let kept = black_box(subnormal) * black_box(1.0_f32);
        assert!(kept.is_subnormal(), "without FTZ a subnormal should survive");

        // With flushing on, the same runtime op collapses to zero.
        set_flush_to_zero(true);
        assert!(flush_to_zero_enabled());
        let flushed = black_box(subnormal) * black_box(1.0_f32);
        assert_eq!(flushed, 0.0, "FTZ/DAZ failed to flush a subnormal to zero");

        // ensure_* is idempotent and leaves it enabled.
        ensure_flush_to_zero();
        assert!(flush_to_zero_enabled());

        // Restore this thread's default so sibling tests are unaffected.
        set_flush_to_zero(false);
    }
}
