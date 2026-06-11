//! Panel palette, lifted from the TE-2 prototype photo: oiled maple frame,
//! matte black faceplate, off-white silkscreen, ARP-style fader caps.

use egui::Color32;

// Dark oiled walnut, not blonde maple: base, grain streaks, edge shadow.
pub const WOOD: Color32 = Color32::from_rgb(0x6E, 0x4E, 0x34);
pub const WOOD_GRAIN_DARK: Color32 = Color32::from_rgb(0x55, 0x3A, 0x26);
pub const WOOD_GRAIN_LIGHT: Color32 = Color32::from_rgb(0x86, 0x62, 0x42);
pub const WOOD_EDGE: Color32 = Color32::from_rgb(0x3E, 0x2B, 0x1C);
pub const PANEL: Color32 = Color32::from_rgb(0x12, 0x12, 0x13);
pub const PANEL_EDGE: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2C);
pub const INK: Color32 = Color32::from_rgb(0xD8, 0xD4, 0xCC);
pub const INK_DIM: Color32 = Color32::from_rgb(0x8A, 0x88, 0x82);
pub const KNOB_BODY: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x20);
pub const KNOB_EDGE: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3E);
pub const KNOB_MARK: Color32 = Color32::from_rgb(0xEA, 0xE8, 0xE2);
pub const LED_RED: Color32 = Color32::from_rgb(0xE8, 0x30, 0x20);
pub const LED_RED_OFF: Color32 = Color32::from_rgb(0x40, 0x12, 0x10);
pub const LED_GREEN: Color32 = Color32::from_rgb(0x40, 0xD8, 0x48);
pub const LED_YELLOW: Color32 = Color32::from_rgb(0xE8, 0xD0, 0x40);
pub const FADER_TRACK: Color32 = Color32::from_rgb(0x08, 0x08, 0x09);
pub const CAP_WHITE: Color32 = Color32::from_rgb(0xE8, 0xE6, 0xE0);
pub const CAP_GRAY: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x96);
pub const CAP_BLACK: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x48);
pub const VU_FACE: Color32 = Color32::from_rgb(0xE9, 0xDF, 0xC0);
pub const VU_NEEDLE: Color32 = Color32::from_rgb(0x18, 0x14, 0x10);
pub const VU_RED: Color32 = Color32::from_rgb(0xC8, 0x32, 0x20);
pub const CASSETTE_SHELL: Color32 = Color32::from_rgb(0x20, 0x1C, 0x1A);
pub const TAPE_PACK: Color32 = Color32::from_rgb(0x4A, 0x32, 0x28);
pub const HUB: Color32 = Color32::from_rgb(0xD8, 0xD4, 0xCC);
