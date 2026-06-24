//! In-plugin preset manager: load the factory patches and save / load / delete
//! user presets.
//!
//! A preset is a *sound recipe*. It moves parameters through the host's normal
//! `ParamSetter` channel — the same path every UI widget uses — so loading one
//! re-dials the panel WITHOUT ejecting the tape, resetting wear, or resizing the
//! window (unlike a full state restore, which re-runs `initialize()`/`reset()`).
//!
//! What a preset captures is the *sound*: knobs, faders, sets, filters, mod,
//! anomaly, tape type/stock, noise/mech, loop, cycle, and transport mode.
//! Excluded (not a sound recipe): the `#[persist]` fields — tape wear, UI scale,
//! editor state — are absent from `param_map()` automatically; and by id we also
//! drop the momentary transport, live position, MIDI-enable (an IO preference),
//! quality (a CPU preference), the aging clock, and master output (so loading a
//! preset never causes a sudden volume jump).

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use nice_plug::prelude::*;
use serde::{Deserialize, Serialize};

use crate::params::Te2Params;

/// Param ids omitted from presets, in addition to the `#[persist]` fields (which
/// `param_map()` never returns). See the module docs for the rationale.
const EXCLUDE: &[&str] = &[
    "pos",    // live position selection
    "midi",   // MIDI position control — IO preference, deliberately off
    "qual",   // oversampling quality — CPU/system preference
    "aging",  // tape-aging clock on/off — wear process (wear itself is excluded)
    "agfrz",  // freeze the aging clock
    "mtr",    // momentary motor kill
    "pause",  // momentary pause
    "stop",   // momentary stop
    "wind",   // momentary RW/FF
    "outlvl", // master output — leave at the user's level, no jump on load
];

fn included(id: &str) -> bool {
    !EXCLUDE.contains(&id)
}

/// Snapshot the preset-eligible params as normalized (0..1) values.
pub fn capture(params: &Te2Params) -> BTreeMap<String, f32> {
    let mut map = BTreeMap::new();
    for (id, ptr, _group) in params.param_map() {
        if included(&id) {
            // SAFETY: `params` is borrowed for this call, so every ParamPtr it
            // produced points into a live Te2Params.
            let v = unsafe { ptr.unmodulated_normalized_value() };
            map.insert(id, v);
        }
    }
    map
}

/// Apply normalized values through the host's ParamSetter channel (GUI thread).
/// Unknown ids are ignored; missing ids are left at their current value.
pub fn apply(setter: &ParamSetter, params: &Te2Params, values: &BTreeMap<String, f32>) {
    for (id, ptr, _group) in params.param_map() {
        if !included(&id) {
            continue;
        }
        if let Some(&v) = values.get(&id) {
            // SAFETY: ParamPtr points into `params`, which is alive for this call.
            unsafe {
                setter.raw_context.raw_begin_set_parameter(ptr);
                setter.raw_context.raw_set_parameter_normalized(ptr, v);
                setter.raw_context.raw_end_set_parameter(ptr);
            }
        }
    }
}

/// Apply a factory preset's `(id, normalized)` table.
pub fn apply_factory(setter: &ParamSetter, params: &Te2Params, preset: &FactoryPreset) {
    let map: BTreeMap<String, f32> = preset
        .values
        .iter()
        .map(|(id, v)| ((*id).to_string(), *v))
        .collect();
    apply(setter, params, &map);
}

// ---------------------------------------------------------------------------
// Factory presets — compiled into the binary. Values are NORMALIZED 0..1.
// Empty for now: dialling the 16 sounds by ear is the deferred v1.1 content
// task. A `dump` mode for te2-snapshot (printing the live panel as a ready-to-
// paste `(id, value)` table) is planned to author them — not implemented yet.
// ---------------------------------------------------------------------------
pub struct FactoryPreset {
    pub name: &'static str,
    pub values: &'static [(&'static str, f32)],
}

pub static FACTORY: &[FactoryPreset] = &[];

// ---------------------------------------------------------------------------
// User presets — one JSON file per preset under the OS config dir.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
pub struct UserPreset {
    #[serde(default = "one")]
    pub format: u32,
    pub name: String,
    pub values: BTreeMap<String, f32>,
}
fn one() -> u32 {
    1
}

/// Per-OS user-preset directory: `RE-DEEMER/presets/` under Application Support
/// (macOS), %APPDATA% (Windows), or $XDG_CONFIG_HOME / ~/.config (Linux).
/// `None` if the home/config dir can't be resolved (e.g. a sandboxed AUv3) — the
/// UI must degrade gracefully (factory presets still work).
pub fn preset_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/Application Support/RE-DEEMER/presets"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA")?;
        Some(PathBuf::from(appdata).join("RE-DEEMER").join("presets"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            Some(PathBuf::from(xdg).join("RE-DEEMER").join("presets"))
        } else {
            let home = std::env::var_os("HOME")?;
            Some(PathBuf::from(home).join(".config/RE-DEEMER/presets"))
        }
    }
}

/// Filesystem-safe filename stem from a user-facing preset name.
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "preset".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

fn dir_or_err() -> io::Result<PathBuf> {
    preset_dir().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no preset directory"))
}

/// Save the current values under `name` (overwrites a same-named preset).
pub fn save(name: &str, values: &BTreeMap<String, f32>) -> io::Result<()> {
    let dir = dir_or_err()?;
    std::fs::create_dir_all(&dir)?;
    let preset = UserPreset {
        format: 1,
        name: name.to_string(),
        values: values.clone(),
    };
    let json = serde_json::to_string_pretty(&preset)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(dir.join(format!("{}.json", sanitize(name))), json)
}

/// All user presets, sorted by name (case-insensitive). Unreadable/corrupt files
/// are skipped rather than failing the whole list.
pub fn list_user() -> Vec<UserPreset> {
    let mut out = Vec::new();
    if let Some(dir) = preset_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if let Ok(preset) = serde_json::from_str::<UserPreset>(&text) {
                            out.push(preset);
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Delete the user preset with the given name.
pub fn delete(name: &str) -> io::Result<()> {
    let dir = dir_or_err()?;
    std::fs::remove_file(dir.join(format!("{}.json", sanitize(name))))
}
