//! Standalone build for development: `cargo run -p te2-plugin --features
//! standalone --bin te2-standalone -- --backend dummy`

fn main() {
    nice_plug::wrapper::standalone::nice_export_standalone::<te2_plugin::SpaceCaseTe2>();
}
