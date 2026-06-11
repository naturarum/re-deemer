//! Headless panel snapshot for development:
//! `cargo run -p te2-plugin --features snapshot --bin te2-snapshot -- out.png`
//! Pass `overlay` as a second argument to render with the settings card open.

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
    let scale = std::env::args()
        .find_map(|a| a.strip_prefix("scale=").and_then(|s| s.parse::<f32>().ok()))
        .unwrap_or(1.0);

    let params = Arc::new(Te2Params::default());
    let shared = Arc::new(UiShared::default());
    let context: Arc<dyn GuiContext> = Arc::new(MockContext);

    if overlay {
        // Give the wear bar something to show.
        params.tape_age.store(0.37, std::sync::atomic::Ordering::Relaxed);
    }
    params.ui_scale.store(scale, std::sync::atomic::Ordering::Relaxed);
    if (scale - 1.0).abs() > 1e-3 {
        // The panel transform follows the window size, so mirror it here.
        let scaled = nice_plug_egui::EguiState::from_size(
            (1080.0 * scale).round() as u32,
            (560.0 * scale).round() as u32,
        );
        nice_plug::params::persist::PersistentField::set(
            &params.editor_state,
            Arc::try_unwrap(scaled).expect("fresh state"),
        );
    }
    // Optional spool position, in seconds of tape footage (0..1800 = side A).
    if let Some(f) = std::env::args()
        .find_map(|a| a.strip_prefix("footage=").and_then(|s| s.parse::<f32>().ok()))
    {
        shared.footage.store(f, std::sync::atomic::Ordering::Relaxed);
    }

    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1080.0 * scale, 560.0 * scale))
        .build_ui(move |ui| {
            let setter = ParamSetter::new(&*context);
            te2_plugin::ui::draw_for_snapshot(ui, &setter, &params, &shared, 0.7, overlay);
        });

    harness.run();
    let image = harness.render().expect("wgpu offscreen render");
    image.save(&out).expect("write png");
    eprintln!("wrote {out}");
}
