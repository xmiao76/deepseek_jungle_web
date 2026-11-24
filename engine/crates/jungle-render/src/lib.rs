//! Board renderer: deterministic 448x576 (or scaled) RGBA frames for the
//! standalone canvas and the Arena component.

pub mod art;
pub mod text;
pub mod theme;
mod theme_tokens;

use std::f32::consts::TAU;

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point,
    Rect, SpreadMode, Stroke, Transform,
};

use text::{Align, Fonts};

pub use text::Align as TextAlign;

pub const COLS: u32 = 7;
pub const ROWS: u32 = 9;
pub const CELL: u32 = 64;
pub const BASE_W: u32 = COLS * CELL;
pub const BASE_H: u32 = ROWS * CELL;
pub const FRAME_BYTES: usize = (BASE_W * BASE_H * 4) as usize;

const WATER: [u8; 12] = [22, 23, 25, 26, 29, 30, 32, 33, 36, 37, 39, 40];
const TRAP_BLUE: [u8; 3] = [2, 4, 10];
const TRAP_RED: [u8; 3] = [52, 58, 60];
const DEN_BLUE: u8 = 3;
const DEN_RED: u8 = 59;

fn is_water(sq: u8) -> bool {
    WATER.contains(&sq)
}
fn is_trap(sq: u8) -> bool {
    TRAP_BLUE.contains(&sq) || TRAP_RED.contains(&sq)
}
fn trap_owner(sq: u8) -> u8 {
    if TRAP_BLUE.contains(&sq) {
        0
    } else {
        1
    }
}
fn is_den(sq: u8) -> bool {
    sq == DEN_BLUE || sq == DEN_RED
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Playing,
    Won,
    Drawn,
    Forfeited,
}

/// Alias used by the component to avoid a name clash with the WIT `Phase`.
pub type FramePhase = Phase;

pub struct View<'a> {
    pub cells: &'a [u8; 63],
    pub to_play: u8,
    pub first_side: u8,
    pub phase: Phase,
    pub winner: Option<u8>,
    pub reason: &'a str,
    pub plies: &'a [(u8, u8)],
    pub selected: Option<u8>,
    pub hover: Option<u8>,
    pub cursor: Option<u8>,
    pub legal: &'a [(u8, u8)],
    /// Piece id of the captured unit, for the standalone capture animation.
    pub captured: Option<u8>,
    /// The Arena component draws the result in-frame; the standalone shell draws
    /// its own result card above the board and sets this to false.
    pub show_result: bool,
}

pub struct Renderer {
    fonts: Fonts,
    background: Option<Background>,
    frame: Option<Pixmap>,
}

struct Background {
    scale: u32,
    flipped: bool,
    pixmap: Pixmap,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        Renderer {
            fonts: Fonts::new(),
            background: None,
            frame: None,
        }
    }

    /// Renders one frame and returns straight RGBA (alpha is always 255).
    pub fn render(&mut self, view: &View, flipped: bool, progress: u16, scale: u32) -> &[u8] {
        let scale = scale.clamp(1, 4);
        let w = BASE_W * scale;
        let h = BASE_H * scale;
        let need_bg = match &self.background {
            Some(bg) => bg.scale != scale || bg.flipped != flipped,
            None => true,
        };
        if need_bg {
            self.background = Some(Background {
                scale,
                flipped,
                pixmap: make_background(scale, flipped),
            });
        }
        let bg = self.background.as_ref().expect("background");
        if self.frame.as_ref().map(|f| f.width()) != Some(w)
            || self.frame.as_ref().map(|f| f.height()) != Some(h)
        {
            self.frame = Pixmap::new(w, h);
        }
        let frame = self.frame.as_mut().expect("frame");
        frame.data_mut().copy_from_slice(bg.pixmap.data());

        let progress = progress.min(1000);
        let scale_f = scale as f32;
        let anim = last_ply(view).filter(|_| progress < 1000);

        draw_last_move(frame, view, flipped, scale_f);
        draw_side_bar(frame, view, flipped, scale_f);

        let mut moving_id: Option<u8> = None;
        if let Some((_, to)) = anim {
            if let Some(piece) = view.cells.get(to as usize).copied().filter(|p| *p != 0) {
                moving_id = Some(piece);
            }
        }

        for sq in 0..63u8 {
            let id = view.cells[sq as usize];
            if id == 0 {
                continue;
            }
            if anim.map(|(_, to)| to) == Some(sq) {
                continue; // drawn at the interpolated position below
            }
            let (cx, cy) = cell_center(display_sq(sq, flipped), scale_f);
            draw_piece(
                frame,
                &self.fonts,
                cx,
                cy,
                id,
                scale_f,
                1.0,
                is_trap(sq),
                for_side(sq),
                is_water(sq),
            );
        }

        if let Some((from, to)) = anim {
            let t = ease(progress as f32 / 1000.0);
            let (fx, fy) = cell_center(display_sq(from, flipped), scale_f);
            let (tx, ty) = cell_center(display_sq(to, flipped), scale_f);
            let cx = fx + (tx - fx) * t;
            let cy = fy + (ty - fy) * t;
            if let Some(captured) = view.captured {
                draw_piece(
                    frame,
                    &self.fonts,
                    tx,
                    ty,
                    captured,
                    scale_f,
                    (1.0 - t) * 0.85,
                    is_trap(to),
                    for_side(to),
                    is_water(to),
                );
            }
            if let Some(piece) = moving_id.or_else(|| Some(view.cells[to as usize])) {
                draw_piece(
                    frame,
                    &self.fonts,
                    cx,
                    cy,
                    piece,
                    scale_f,
                    1.0,
                    is_trap(to),
                    for_side(to),
                    is_water(to),
                );
            }
        }

        draw_hints(frame, view, flipped, scale_f);

        if view.show_result && matches!(view.phase, Phase::Won | Phase::Drawn | Phase::Forfeited) {
            draw_result(frame, &self.fonts, view, progress, scale_f);
        }

        frame.data()
    }
}

fn for_side(sq: u8) -> u8 {
    if TRAP_BLUE.contains(&sq) {
        0
    } else {
        1
    }
}

fn last_ply(view: &View) -> Option<(u8, u8)> {
    view.plies.last().copied()
}

fn display_sq(sq: u8, flipped: bool) -> u8 {
    if flipped {
        62 - sq
    } else {
        sq
    }
}

fn cell_center(display: u8, scale: f32) -> (f32, f32) {
    let col = (display % 7) as f32;
    let row = (display / 7) as f32;
    (
        (col + 0.5) * CELL as f32 * scale,
        (row + 0.5) * CELL as f32 * scale,
    )
}

fn cell_rect(display: u8, scale: f32) -> Rect {
    let col = (display % 7) as f32;
    let row = (display / 7) as f32;
    Rect::from_xywh(
        col * CELL as f32 * scale,
        row * CELL as f32 * scale,
        CELL as f32 * scale,
        CELL as f32 * scale,
    )
    .expect("cell")
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---- Primitives ------------------------------------------------------------

fn fill_path(pixmap: &mut Pixmap, path: &Path, color: Color) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    pixmap.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
}

fn stroke_path(pixmap: &mut Pixmap, path: &Path, color: Color, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let stroke = Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    pixmap.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn rect_path(rect: Rect) -> Path {
    let mut pb = PathBuilder::new();
    pb.push_rect(rect);
    pb.finish().expect("rect")
}

fn round_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish().expect("round rect")
}

fn fill_round_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    fill_path(pixmap, &round_rect_path(x, y, w, h, r), color);
}

#[allow(clippy::too_many_arguments)]
fn stroke_round_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: Color,
    width: f32,
) {
    stroke_path(pixmap, &round_rect_path(x, y, w, h, r), color, width);
}

fn fill_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Color) {
    fill_path(pixmap, &art::ellipse(cx, cy, r, r), color);
}

fn stroke_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Color, width: f32) {
    stroke_path(pixmap, &art::ellipse(cx, cy, r, r), color, width);
}

fn line(pixmap: &mut Pixmap, pts: &[(f32, f32)], color: Color, width: f32) {
    let mut pb = PathBuilder::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        if i == 0 {
            pb.move_to(*x, *y);
        } else {
            pb.line_to(*x, *y);
        }
    }
    if let Some(path) = pb.finish() {
        stroke_path(pixmap, &path, color, width);
    }
}

fn poly_path(pts: &[(f32, f32)]) -> Path {
    let mut pb = PathBuilder::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        if i == 0 {
            pb.move_to(*x, *y);
        } else {
            pb.line_to(*x, *y);
        }
    }
    pb.close();
    pb.finish().expect("poly")
}

fn checker(sq: u8) -> bool {
    let col = sq % 7;
    let row = sq / 7;
    (col + row).is_multiple_of(2)
}

// ---- Background ------------------------------------------------------------

fn make_background(scale: u32, flipped: bool) -> Pixmap {
    let mut pixmap = Pixmap::new(BASE_W * scale, BASE_H * scale).expect("background");
    pixmap.fill(theme::c(theme::LACQUER, 255));
    let scale_f = scale as f32;
    let cell = CELL as f32 * scale_f;

    for display in 0..63u8 {
        let sq = display_sq(display, flipped);
        let rect = cell_rect(display, scale_f);
        let base = if checker(sq) {
            theme::PARCHMENT
        } else {
            theme::PARCHMENT_ALT
        };
        let mut paint = Paint::default();
        paint.set_color(theme::c(base, 255));
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);

        // Deterministic paper flecks.
        let seed = (sq as u32).wrapping_mul(2_654_435_761);
        for i in 0..3u32 {
            let h = seed.rotate_left(i * 7 + 3);
            let fx = rect.x() + ((h & 0xff) as f32 / 255.0) * cell;
            let fy = rect.y() + (((h >> 8) & 0xff) as f32 / 255.0) * cell;
            fill_circle(&mut pixmap, fx, fy, 0.9 * scale_f, theme::c(theme::INK, 18));
        }

        if is_water(sq) {
            draw_water(&mut pixmap, rect, scale_f, display);
        }
        if is_den(sq) {
            draw_den(&mut pixmap, rect, scale_f, sq == DEN_RED);
        }
        if is_trap(sq) {
            draw_trap(&mut pixmap, rect, scale_f, trap_owner(sq));
        }

        // Cell edge.
        let mut edge = Paint::default();
        edge.set_color(theme::c(theme::INK, 34));
        edge.anti_alias = false;
        let stroke = Stroke {
            width: 1.0 * scale_f,
            ..Default::default()
        };
        pixmap.stroke_path(
            &rect_path(rect),
            &edge,
            &stroke,
            Transform::identity(),
            None,
        );
    }

    // Border + vignette.
    let fw = pixmap.width() as f32;
    let fh = pixmap.height() as f32;
    stroke_round_rect(
        &mut pixmap,
        2.0 * scale_f,
        2.0 * scale_f,
        fw - 4.0 * scale_f,
        fh - 4.0 * scale_f,
        6.0 * scale_f,
        theme::c(theme::GOLD, 120),
        2.0 * scale_f,
    );
    stroke_round_rect(
        &mut pixmap,
        5.0 * scale_f,
        5.0 * scale_f,
        fw - 10.0 * scale_f,
        fh - 10.0 * scale_f,
        4.0 * scale_f,
        theme::c(theme::INK, 80),
        1.5 * scale_f,
    );

    pixmap
}

fn draw_water(pixmap: &mut Pixmap, rect: Rect, scale: f32, display: u8) {
    let cell = CELL as f32 * scale;
    let stops = vec![
        tiny_skia::GradientStop::new(0.0, theme::c(theme::RIVER_MID, 255)),
        tiny_skia::GradientStop::new(0.55, theme::c(theme::RIVER_DEEP, 255)),
        tiny_skia::GradientStop::new(1.0, theme::c(theme::RIVER_MID, 255)),
    ];
    let shader = LinearGradient::new(
        Point::from_xy(rect.x(), rect.y()),
        Point::from_xy(rect.x(), rect.y() + cell),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    );
    if let Some(shader) = shader {
        let paint = Paint {
            shader,
            anti_alias: true,
            ..Default::default()
        };
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }

    // Continuous wave lines across the two-cell-wide river.
    let row = display / 7;
    for k in 0..3 {
        let base_y = rect.y() + (14.0 + k as f32 * 15.0) * scale;
        let mut pb = PathBuilder::new();
        let steps = 10;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = rect.x() + t * cell;
            let phase = (display as f32 * 1.7) + (row as f32 * 0.8) + k as f32 * 1.3;
            let y = base_y + (phase + t * TAU * 1.2).sin() * 1.8 * scale;
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        if let Some(path) = pb.finish() {
            stroke_path(pixmap, &path, theme::c(theme::RIVER_LIGHT, 90), 1.2 * scale);
        }
    }

    // Water sheen.
    fill_round_rect(
        pixmap,
        rect.x() + 4.0 * scale,
        rect.y() + 4.0 * scale,
        cell - 8.0 * scale,
        cell * 0.35,
        4.0 * scale,
        theme::c(theme::CREAM, 12),
    );
    let _ = is_water(display);
}

fn draw_den(pixmap: &mut Pixmap, rect: Rect, scale: f32, red: bool) {
    let cell = CELL as f32 * scale;
    let x = rect.x() + 7.0 * scale;
    let y = rect.y() + 7.0 * scale;
    let w = cell - 14.0 * scale;
    let h = cell - 14.0 * scale;
    fill_round_rect(pixmap, x, y, w, h, 9.0 * scale, theme::c(theme::INK, 215));
    stroke_round_rect(
        pixmap,
        x,
        y,
        w,
        h,
        9.0 * scale,
        theme::c(theme::GOLD, 210),
        2.0 * scale,
    );

    // Arch.
    let ax = rect.x() + cell * 0.5;
    let ay = rect.y() + cell * 0.72;
    let mut pb = PathBuilder::new();
    pb.move_to(ax - 13.0 * scale, ay);
    pb.line_to(ax - 13.0 * scale, ay - 12.0 * scale);
    pb.cubic_to(
        ax - 13.0 * scale,
        ay - 24.0 * scale,
        ax + 13.0 * scale,
        ay - 24.0 * scale,
        ax + 13.0 * scale,
        ay - 12.0 * scale,
    );
    pb.line_to(ax + 13.0 * scale, ay);
    pb.close();
    if let Some(path) = pb.finish() {
        fill_path(pixmap, &path, theme::c(theme::LACQUER, 255));
        stroke_path(
            pixmap,
            &path,
            theme::c(
                if red {
                    theme::VERMILION_LIGHT
                } else {
                    theme::INDIGO_LIGHT
                },
                170,
            ),
            1.5 * scale,
        );
    }
    // Crown marks.
    let cy = rect.y() + 12.0 * scale;
    for i in -1..=1 {
        let cx = rect.x() + cell * 0.5 + i as f32 * 7.0 * scale;
        let pts = [
            (cx, cy + 5.0 * scale),
            (cx - 3.0 * scale, cy - 2.0 * scale),
            (cx + 3.0 * scale, cy - 2.0 * scale),
        ];
        fill_path(pixmap, &poly_path(&pts), theme::c(theme::GOLD, 220));
    }
}

fn draw_trap(pixmap: &mut Pixmap, rect: Rect, scale: f32, owner: u8) {
    let cell = CELL as f32 * scale;
    let cx = rect.x() + cell * 0.5;
    let cy = rect.y() + cell * 0.5;
    let r = cell * 0.34;
    let color = theme::c(
        if owner == 0 {
            theme::INDIGO
        } else {
            theme::VERMILION
        },
        95,
    );

    let diamond = [(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)];
    stroke_path(pixmap, &poly_path(&diamond), color, 1.4 * scale);
    for k in 0..3 {
        let yy = cy - r * 0.45 + k as f32 * r * 0.42;
        let half = r * (0.62 - k as f32 * 0.12);
        line(
            pixmap,
            &[(cx - half, yy), (cx, yy + half * 0.65), (cx + half, yy)],
            color,
            1.2 * scale,
        );
    }
    fill_circle(pixmap, cx, cy, 2.2 * scale, color);
}

// ---- Pieces ----------------------------------------------------------------

fn piece_rank(id: u8) -> u8 {
    ((id.saturating_sub(1)) % 16) / 2 + 1
}

fn piece_owner(id: u8) -> u8 {
    (id.saturating_sub(1)) & 1
}

fn piece_name(rank: u8) -> &'static str {
    match rank {
        1 => "Rat",
        2 => "Cat",
        3 => "Dog",
        4 => "Wolf",
        5 => "Leopard",
        6 => "Tiger",
        7 => "Lion",
        8 => "Elephant",
        _ => "?",
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_piece(
    pixmap: &mut Pixmap,
    fonts: &Fonts,
    cx: f32,
    cy: f32,
    id: u8,
    scale: f32,
    opacity: f32,
    on_trap: bool,
    _trap_side: u8,
    in_water: bool,
) {
    if opacity <= 0.01 {
        return;
    }
    let owner = piece_owner(id);
    let rank = piece_rank(id);
    let side = theme::side_color(owner);
    let side_light = theme::side_light(owner);
    let r = 22.0 * scale;
    let a = |base: u8| -> Color { theme::c(side, (base as f32 * opacity) as u8) };

    // Shadow.
    fill_circle(
        pixmap,
        cx,
        cy + 19.0 * scale,
        r * 0.82,
        theme::c(theme::INK, (55.0 * opacity) as u8),
    );

    if in_water {
        stroke_circle(
            pixmap,
            cx,
            cy + 2.0 * scale,
            r + 4.0 * scale,
            theme::c(theme::RIVER_LIGHT, (120.0 * opacity) as u8),
            1.5 * scale,
        );
        stroke_circle(
            pixmap,
            cx,
            cy + 2.0 * scale,
            r + 8.0 * scale,
            theme::c(theme::RIVER_LIGHT, (70.0 * opacity) as u8),
            1.2 * scale,
        );
    }

    // Side-coloured rim, then cream face.
    fill_circle(pixmap, cx, cy, r, a(255));
    fill_circle(
        pixmap,
        cx,
        cy,
        r - 2.0 * scale,
        theme::c(theme::side_light(owner), (255.0 * opacity) as u8),
    );
    fill_circle(
        pixmap,
        cx,
        cy,
        r - 3.6 * scale,
        theme::c(theme::CREAM, (255.0 * opacity) as u8),
    );
    stroke_circle(
        pixmap,
        cx,
        cy,
        r - 1.0 * scale,
        theme::c(theme::INK, (70.0 * opacity) as u8),
        1.0 * scale,
    );
    let _ = side_light;

    // Animal emblem.
    art::draw_animal(
        pixmap,
        rank,
        cx,
        cy - 1.0 * scale,
        27.0 * scale,
        theme::c(theme::CHARCOAL, (255.0 * opacity) as u8),
    );

    // Rank pip.
    let pip_x = cx + 15.0 * scale;
    let pip_y = cy - 15.0 * scale;
    fill_circle(
        pixmap,
        pip_x,
        pip_y,
        6.4 * scale,
        theme::c(theme::CREAM, (255.0 * opacity) as u8),
    );
    stroke_circle(pixmap, pip_x, pip_y, 6.4 * scale, a(220), 1.2 * scale);
    text::draw_text(
        pixmap,
        &fonts.black,
        &rank.to_string(),
        pip_x,
        pip_y + 2.9 * scale,
        8.0 * scale,
        theme::c(theme::INK, (255.0 * opacity) as u8),
        Align::Center,
    );

    // Label plate.
    let plate_w = 50.0 * scale;
    let plate_h = 12.0 * scale;
    let plate_x = cx - plate_w / 2.0;
    let plate_y = cy + 21.0 * scale;
    fill_round_rect(
        pixmap,
        plate_x,
        plate_y,
        plate_w,
        plate_h,
        3.0 * scale,
        theme::c(theme::INK, (215.0 * opacity) as u8),
    );
    stroke_round_rect(
        pixmap,
        plate_x,
        plate_y,
        plate_w,
        plate_h,
        3.0 * scale,
        a(140),
        0.8 * scale,
    );
    text::draw_text(
        pixmap,
        &fonts.bold,
        piece_name(rank),
        cx,
        plate_y + 9.0 * scale,
        8.0 * scale,
        theme::c(theme::CREAM, (255.0 * opacity) as u8),
        Align::Center,
    );

    if on_trap {
        fill_circle(
            pixmap,
            cx - 15.0 * scale,
            cy - 15.0 * scale,
            6.0 * scale,
            theme::c(theme::VERMILION, (255.0 * opacity) as u8),
        );
        text::draw_text(
            pixmap,
            &fonts.black,
            "!",
            cx - 15.0 * scale,
            cy - 12.0 * scale,
            9.0 * scale,
            theme::c(theme::CREAM, (255.0 * opacity) as u8),
            Align::Center,
        );
    }

    // Side ring accent.
    stroke_circle(
        pixmap,
        cx,
        cy,
        r - 0.8 * scale,
        theme::c(theme::GOLD, (35.0 * opacity) as u8),
        1.0 * scale,
    );
    let _ = side;
}

// ---- Overlays --------------------------------------------------------------

fn draw_last_move(pixmap: &mut Pixmap, view: &View, flipped: bool, scale: f32) {
    let Some((from, to)) = last_ply(view) else {
        return;
    };
    for (sq, alpha) in [(from, 38u8), (to, 68u8)] {
        let rect = cell_rect(display_sq(sq, flipped), scale);
        let mut paint = Paint::default();
        paint.set_color(theme::c(theme::GOLD, alpha));
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
    let rect = cell_rect(display_sq(to, flipped), scale);
    stroke_round_rect(
        pixmap,
        rect.x() + 5.0 * scale,
        rect.y() + 5.0 * scale,
        rect.width() - 10.0 * scale,
        rect.height() - 10.0 * scale,
        6.0 * scale,
        theme::c(theme::GOLD_LIGHT, 150),
        1.4 * scale,
    );
}

fn draw_side_bar(pixmap: &mut Pixmap, view: &View, flipped: bool, scale: f32) {
    if view.phase != Phase::Playing {
        return;
    }
    let color = theme::c(theme::side_color(view.to_play), 210);
    let mut paint = Paint::default();
    paint.set_color(color);
    let home_row = if view.to_play == 0 {
        if flipped {
            8.0
        } else {
            0.0
        }
    } else if flipped {
        0.0
    } else {
        8.0
    };
    let y = home_row * CELL as f32 * scale;
    if let Some(rect) = Rect::from_xywh(0.0, y, BASE_W as f32 * scale, 4.0 * scale) {
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn draw_hints(pixmap: &mut Pixmap, view: &View, flipped: bool, scale: f32) {
    if let Some(cursor) = view.cursor {
        let rect = cell_rect(display_sq(cursor, flipped), scale);
        stroke_round_rect(
            pixmap,
            rect.x() + 4.0 * scale,
            rect.y() + 4.0 * scale,
            rect.width() - 8.0 * scale,
            rect.height() - 8.0 * scale,
            5.0 * scale,
            theme::c(theme::CREAM, 190),
            1.4 * scale,
        );
    }
    if let Some(hover) = view.hover {
        let rect = cell_rect(display_sq(hover, flipped), scale);
        let mut paint = Paint::default();
        paint.set_color(theme::c(theme::CREAM, 18));
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
    if let Some(selected) = view.selected {
        let rect = cell_rect(display_sq(selected, flipped), scale);
        stroke_round_rect(
            pixmap,
            rect.x() + 3.0 * scale,
            rect.y() + 3.0 * scale,
            rect.width() - 6.0 * scale,
            rect.height() - 6.0 * scale,
            7.0 * scale,
            theme::c(theme::GOLD_LIGHT, 240),
            3.0 * scale,
        );
        // Corner ticks.
        let x0 = rect.x() + 1.0 * scale;
        let y0 = rect.y() + 1.0 * scale;
        let x1 = rect.x() + rect.width() - 1.0 * scale;
        let y1 = rect.y() + rect.height() - 1.0 * scale;
        let t = 9.0 * scale;
        for pts in [
            [(x0, y0 + t), (x0, y0), (x0 + t, y0)],
            [(x1 - t, y0), (x1, y0), (x1, y0 + t)],
            [(x0, y1 - t), (x0, y1), (x0 + t, y1)],
            [(x1 - t, y1), (x1, y1), (x1, y1 - t)],
        ] {
            line(pixmap, &pts, theme::c(theme::GOLD, 240), 2.0 * scale);
        }
    }

    for &(from, to) in view.legal {
        if view.selected != Some(from) {
            continue;
        }
        let rect = cell_rect(display_sq(to, flipped), scale);
        let cx = rect.x() + rect.width() / 2.0;
        let cy = rect.y() + rect.height() / 2.0;
        if view.cells[to as usize] == 0 {
            fill_circle(pixmap, cx, cy, 6.0 * scale, theme::c(theme::JADE, 200));
            stroke_circle(
                pixmap,
                cx,
                cy,
                7.5 * scale,
                theme::c(theme::CREAM, 90),
                1.0 * scale,
            );
        } else {
            stroke_circle(
                pixmap,
                cx,
                cy,
                20.0 * scale,
                theme::c(theme::VERMILION, 230),
                2.6 * scale,
            );
            line(
                pixmap,
                &[
                    (cx - 6.0 * scale, cy - 6.0 * scale),
                    (cx + 6.0 * scale, cy + 6.0 * scale),
                ],
                theme::c(theme::VERMILION, 230),
                2.0 * scale,
            );
            line(
                pixmap,
                &[
                    (cx + 6.0 * scale, cy - 6.0 * scale),
                    (cx - 6.0 * scale, cy + 6.0 * scale),
                ],
                theme::c(theme::VERMILION, 230),
                2.0 * scale,
            );
        }
    }
}

// ---- Result ----------------------------------------------------------------

fn draw_result(pixmap: &mut Pixmap, fonts: &Fonts, view: &View, progress: u16, scale: f32) {
    let fade = if progress >= 1000 {
        1.0
    } else {
        let t = (progress as f32 / 1000.0 - 0.55) / 0.45;
        t.clamp(0.0, 1.0)
    };
    if fade <= 0.0 {
        return;
    }
    let w = BASE_W as f32 * scale;
    let h = BASE_H as f32 * scale;
    let mut scrim = Paint::default();
    scrim.set_color(theme::c(theme::INK, (170.0 * fade) as u8));
    if let Some(rect) = Rect::from_xywh(0.0, 0.0, w, h) {
        pixmap.fill_rect(rect, &scrim, Transform::identity(), None);
    }

    let card_w = 328.0 * scale;
    let card_h = 158.0 * scale;
    let x = (w - card_w) / 2.0;
    let y = (h - card_h) / 2.0;
    fill_round_rect(
        pixmap,
        x,
        y,
        card_w,
        card_h,
        14.0 * scale,
        theme::c(theme::LACQUER, (250.0 * fade) as u8),
    );
    stroke_round_rect(
        pixmap,
        x,
        y,
        card_w,
        card_h,
        14.0 * scale,
        theme::c(theme::GOLD, (240.0 * fade) as u8),
        2.0 * scale,
    );
    stroke_round_rect(
        pixmap,
        x + 5.0 * scale,
        y + 5.0 * scale,
        card_w - 10.0 * scale,
        card_h - 10.0 * scale,
        10.0 * scale,
        theme::c(theme::GOLD, (80.0 * fade) as u8),
        1.0 * scale,
    );

    let (title, accent) = match view.phase {
        Phase::Drawn => ("DRAW".to_string(), theme::GOLD_LIGHT),
        _ => match view.winner {
            Some(0) => ("BLUE WINS".to_string(), theme::INDIGO_LIGHT),
            Some(1) => ("RED WINS".to_string(), theme::VERMILION_LIGHT),
            _ => ("GAME OVER".to_string(), theme::GOLD_LIGHT),
        },
    };

    text::draw_text(
        pixmap,
        &fonts.bold,
        "MATCH RESULT",
        w / 2.0,
        y + 30.0 * scale,
        9.0 * scale,
        theme::c(theme::GOLD, (230.0 * fade) as u8),
        Align::Center,
    );
    text::draw_text(
        pixmap,
        &fonts.display,
        &title,
        w / 2.0,
        y + 78.0 * scale,
        30.0 * scale,
        theme::c(accent, (255.0 * fade) as u8),
        Align::Center,
    );

    line(
        pixmap,
        &[
            (w / 2.0 - 46.0 * scale, y + 92.0 * scale),
            (w / 2.0 + 46.0 * scale, y + 92.0 * scale),
        ],
        theme::c(theme::GOLD, (140.0 * fade) as u8),
        1.0 * scale,
    );

    let reason = theme::reason_label(view.reason);
    let subtitle = if view.phase == Phase::Forfeited {
        if reason.is_empty() {
            "Forfeit".to_string()
        } else {
            format!("Forfeit · {reason}")
        }
    } else {
        reason.to_string()
    };
    text::draw_text(
        pixmap,
        &fonts.regular,
        &subtitle,
        w / 2.0,
        y + 114.0 * scale,
        12.0 * scale,
        theme::c(theme::CREAM, (235.0 * fade) as u8),
        Align::Center,
    );
    text::draw_text(
        pixmap,
        &fonts.display,
        "JUNGLE",
        w / 2.0,
        y + 140.0 * scale,
        10.0 * scale,
        theme::c(theme::GOLD, (190.0 * fade) as u8),
        Align::Center,
    );
}

// Re-export a convenient frame size helper for the component/webengine.
pub fn frame_size(scale: u32) -> (u32, u32) {
    let scale = scale.clamp(1, 4);
    (BASE_W * scale, BASE_H * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    fn opening_cells() -> [u8; 63] {
        jungle_board_opening()
    }

    // Duplicated minimal canonical opening for renderer tests (renderer has no
    // dependency on jungle-core).
    fn jungle_board_opening() -> [u8; 63] {
        let mut cells = [0u8; 63];
        let red = [
            (1u8, 48u8),
            (2, 50),
            (3, 54),
            (4, 44),
            (5, 46),
            (6, 56),
            (7, 62),
            (8, 42),
        ];
        let blue = [
            (1u8, 14u8),
            (2, 12),
            (3, 8),
            (4, 18),
            (5, 16),
            (6, 6),
            (7, 0),
            (8, 20),
        ];
        for (rank, sq) in blue {
            cells[sq as usize] = (rank - 1) * 2 + 1;
        }
        for (rank, sq) in red {
            cells[sq as usize] = (rank - 1) * 2 + 1 + 1;
        }
        cells
    }

    fn view<'a>(cells: &'a [u8; 63], legal: &'a [(u8, u8)]) -> View<'a> {
        View {
            cells,
            to_play: 1,
            first_side: 1,
            phase: Phase::Playing,
            winner: None,
            reason: "",
            plies: &[],
            selected: None,
            hover: None,
            cursor: None,
            legal,
            captured: None,
            show_result: false,
        }
    }

    #[test]
    fn frame_is_exact_and_opaque() {
        let cells = opening_cells();
        let legal = [];
        let mut renderer = Renderer::new();
        let frame = renderer
            .render(&view(&cells, &legal), false, 1000, 1)
            .to_vec();
        assert_eq!(frame.len(), FRAME_BYTES);
        assert!(frame.chunks_exact(4).all(|px| px[3] == 255));
        let again = renderer
            .render(&view(&cells, &legal), false, 1000, 1)
            .to_vec();
        assert_eq!(frame, again, "unflipped render must be deterministic");
        let flipped = renderer
            .render(&view(&cells, &legal), true, 1000, 1)
            .to_vec();
        assert_ne!(frame, flipped, "flip must change the view");
    }

    #[test]
    fn scaled_frame_and_settled_terminal() {
        let cells = opening_cells();
        let legal = [];
        let mut renderer = Renderer::new();
        let (w, h) = frame_size(2);
        let frame = renderer
            .render(&view(&cells, &legal), false, 1000, 2)
            .to_vec();
        assert_eq!(frame.len(), (w * h * 4) as usize);

        let terminal = View {
            cells: &cells,
            to_play: 0,
            first_side: 1,
            phase: Phase::Drawn,
            winner: None,
            reason: "repetition",
            plies: &[(42, 43)],
            selected: None,
            hover: None,
            cursor: None,
            legal: &[],
            captured: None,
            show_result: true,
        };
        let settled = renderer.render(&terminal, false, 1000, 1).to_vec();
        assert_eq!(settled.len(), FRAME_BYTES);
        assert!(settled.chunks_exact(4).all(|px| px[3] == 255));
        let _ = theme::GOLD;
    }
}
