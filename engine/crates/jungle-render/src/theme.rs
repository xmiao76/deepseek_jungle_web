//! Shared visual identity: palette and typography. The standalone CSS and the
//! component both use these values (the CSS is generated from the same tokens).

use tiny_skia::Color;

pub use crate::theme_tokens::*;

pub const FONT_DISPLAY: &[u8] = include_bytes!("../../../../assets/fonts/Marcellus-Regular.ttf");
pub const FONT_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/Lato-Regular.ttf");
pub const FONT_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/Lato-Bold.ttf");
pub const FONT_BLACK: &[u8] = include_bytes!("../../../../assets/fonts/Lato-Black.ttf");

#[inline]
pub fn c(hex: u32, alpha: u8) -> Color {
    Color::from_rgba8(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
        alpha,
    )
}

#[inline]
pub fn side_color(owner: u8) -> u32 {
    if owner == 0 {
        INDIGO
    } else {
        VERMILION
    }
}

#[inline]
pub fn side_light(owner: u8) -> u32 {
    if owner == 0 {
        INDIGO_LIGHT
    } else {
        VERMILION_LIGHT
    }
}

pub fn reason_label(reason: &str) -> &'static str {
    match reason {
        "den" => "Den reached",
        "elimination" => "All pieces captured",
        "immobilization" => "No legal moves",
        "repetition" => "Threefold repetition",
        "no-capture" => "100 plies without a capture",
        "timeout" => "Timeout",
        "trap" => "Illegal trap move",
        "invalid-move" => "Invalid move",
        _ => "",
    }
}
