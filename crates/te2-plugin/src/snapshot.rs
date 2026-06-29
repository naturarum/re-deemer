//! Headless panel snapshot for development:
//! `cargo run -p te2-plugin --features snapshot --bin te2-snapshot -- out.png`
//! Pass `overlay` to render the settings card open, or `presets` for the
//! preset browser.

use nice_plug::prelude::*;
use std::sync::Arc;
use te2_plugin::{Te2Params, UiShared};

/// No-op GuiContext so a real ParamSetter can drive the widgets offline.
struct MockContext;

impl GuiContext for MockContext {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn request_resize(&self) -> bool {
        false
    }
    unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
    unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, _normalized: f32) {}
    unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
    fn get_state(&self) -> PluginState {
        unimplemented!("not used by the snapshot tool")
    }
    fn set_state(&self, _state: PluginState) {
        unimplemented!("not used by the snapshot tool")
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "te2-panel.png".to_string());
    let overlay = std::env::args().any(|a| a == "overlay");
    let presets = std::env::args().any(|a| a == "presets");

    let params = Arc::new(Te2Params::default());
    let shared = Arc::new(UiShared::default());
    let context: Arc<dyn GuiContext> = Arc::new(MockContext);

    if overlay {
        // Give the wear bar something to show.
        params
            .tape_age
            .store(0.37, std::sync::atomic::Ordering::Relaxed);
    }
    // Optional spool position, in seconds of tape footage (0..1800 = side A).
    if let Some(f) = std::env::args().find_map(|a| {
        a.strip_prefix("footage=")
            .and_then(|s| s.parse::<f32>().ok())
    }) {
        shared
            .footage
            .store(f, std::sync::atomic::Ordering::Relaxed);
    }
    // Preview the "update available" nudge (faceplate dot + SETUP line) without
    // a real newer release: pass `update` (optionally with `overlay`).
    if std::env::args().any(|a| a == "update") {
        shared
            .update_available
            .store(true, std::sync::atomic::Ordering::Relaxed);
        *shared.update_info.lock().unwrap() = Some((
            "1.1.2".to_string(),
            "https://naturarum.github.io/re-deemer/".to_string(),
        ));
    }

    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1080.0, 560.0))
        .build_ui(move |ui| {
            let setter = ParamSetter::new(&*context);
            te2_plugin::ui::draw_for_snapshot(ui, &setter, &params, &shared, 0.7, overlay, presets);
        });

    harness.run();
    let image = harness.render().expect("wgpu offscreen render");
    image.save(&out).expect("write png");
    eprintln!("wrote {out}");
}
