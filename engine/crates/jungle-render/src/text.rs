//! Small text rasteriser over ab_glyph with tiny-skia blending.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use tiny_skia::{Color, Pixmap};

pub struct Fonts {
    pub display: FontVec,
    pub regular: FontVec,
    pub bold: FontVec,
    pub black: FontVec,
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

impl Fonts {
    pub fn new() -> Self {
        Fonts {
            display: FontVec::try_from_vec(crate::theme::FONT_DISPLAY.to_vec())
                .expect("display font"),
            regular: FontVec::try_from_vec(crate::theme::FONT_REGULAR.to_vec())
                .expect("regular font"),
            bold: FontVec::try_from_vec(crate::theme::FONT_BOLD.to_vec()).expect("bold font"),
            black: FontVec::try_from_vec(crate::theme::FONT_BLACK.to_vec()).expect("black font"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub fn measure(font: &FontVec, text: &str, size: f32) -> f32 {
    let scaled = font.as_scaled(PxScale::from(size));
    text.chars()
        .map(|ch| {
            let id = scaled.scaled_glyph(ch).id;
            scaled.h_advance(id)
        })
        .sum()
}

/// Draws `text` with its baseline at `baseline_y`. Returns the advance width.
#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    pixmap: &mut Pixmap,
    font: &FontVec,
    text: &str,
    x: f32,
    baseline_y: f32,
    size: f32,
    color: Color,
    align: Align,
) -> f32 {
    let width = measure(font, text, size);
    let mut caret = match align {
        Align::Left => x,
        Align::Center => x - width / 2.0,
        Align::Right => x - width,
    };
    let scaled = font.as_scaled(PxScale::from(size));
    let alpha = color.alpha();
    if alpha <= 0.0 {
        return width;
    }
    for ch in text.chars() {
        let mut glyph = scaled.scaled_glyph(ch);
        glyph.position = ab_glyph::point(caret, baseline_y);
        let glyph_id = glyph.id;
        if let Some(outline) = scaled.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pixmap.width() as i32 || py >= pixmap.height() as i32 {
                    return;
                }
                let a = coverage * alpha;
                if a <= 0.0 {
                    return;
                }
                let idx = ((py as u32 * pixmap.width()) + px as u32) as usize * 4;
                let data = pixmap.data_mut();
                let dst = &mut data[idx..idx + 4];
                let src_r = color.red() * 255.0 * a;
                let src_g = color.green() * 255.0 * a;
                let src_b = color.blue() * 255.0 * a;
                let inv = 1.0 - a;
                dst[0] = (src_r + dst[0] as f32 * inv).min(255.0) as u8;
                dst[1] = (src_g + dst[1] as f32 * inv).min(255.0) as u8;
                dst[2] = (src_b + dst[2] as f32 * inv).min(255.0) as u8;
                dst[3] = 255;
            });
        }
        caret += scaled.h_advance(glyph_id);
    }
    width
}
