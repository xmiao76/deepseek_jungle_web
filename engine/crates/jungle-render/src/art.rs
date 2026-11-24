//! Original vector animal emblems. Each animal is drawn as an ink silhouette
//! inside a token; `w` is the emblem width in pixels.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

const KAPPA: f32 = 0.552_284_8;

fn fill(pixmap: &mut Pixmap, path: &Path, color: Color) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    pixmap.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
}

fn stroke(pixmap: &mut Pixmap, path: &Path, color: Color, width: f32) {
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

pub(crate) fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
    let mut pb = PathBuilder::new();
    let ox = rx * KAPPA;
    let oy = ry * KAPPA;
    pb.move_to(cx - rx, cy);
    pb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
    pb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
    pb.close();
    pb.finish().expect("ellipse")
}

fn polygon(points: &[(f32, f32)]) -> Path {
    let mut pb = PathBuilder::new();
    for (i, (x, y)) in points.iter().enumerate() {
        if i == 0 {
            pb.move_to(*x, *y);
        } else {
            pb.line_to(*x, *y);
        }
    }
    pb.close();
    pb.finish().expect("polygon")
}

fn line_path(points: &[(f32, f32)]) -> Path {
    let mut pb = PathBuilder::new();
    for (i, (x, y)) in points.iter().enumerate() {
        if i == 0 {
            pb.move_to(*x, *y);
        } else {
            pb.line_to(*x, *y);
        }
    }
    pb.finish().expect("line")
}

struct Canvas {
    cx: f32,
    cy: f32,
    s: f32,
    ink: Color,
}

impl Canvas {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.cx + x * self.s, self.cy + y * self.s)
    }

    fn ellipse(&self, x: f32, y: f32, rx: f32, ry: f32) -> Path {
        let (cx, cy) = self.p(x, y);
        ellipse(cx, cy, rx * self.s, ry * self.s)
    }

    fn poly(&self, pts: &[(f32, f32)]) -> Path {
        let mapped: Vec<(f32, f32)> = pts.iter().map(|(x, y)| self.p(*x, *y)).collect();
        polygon(&mapped)
    }

    fn line(&self, pts: &[(f32, f32)]) -> Path {
        let mapped: Vec<(f32, f32)> = pts.iter().map(|(x, y)| self.p(*x, *y)).collect();
        line_path(&mapped)
    }
}

/// `kind` is the animal rank 1..=8. `w` is the emblem width.
pub fn draw_animal(pixmap: &mut Pixmap, kind: u8, cx: f32, cy: f32, w: f32, ink: Color) {
    let s = w / 32.0;
    let c = Canvas { cx, cy, s, ink };
    match kind {
        1 => rat(pixmap, &c),
        2 => cat(pixmap, &c),
        3 => dog(pixmap, &c),
        4 => wolf(pixmap, &c),
        5 => leopard(pixmap, &c),
        6 => tiger(pixmap, &c),
        7 => lion(pixmap, &c),
        8 => elephant(pixmap, &c),
        _ => {}
    }
}

fn eyes(pixmap: &mut Pixmap, c: &Canvas) {
    let cream = Color::from_rgba8(247, 238, 219, 255);
    for x in [-4.2, 4.2] {
        let (px, py) = c.p(x, -1.5);
        fill(pixmap, &ellipse(px, py, 1.1 * c.s, 1.3 * c.s), cream);
    }
}

fn rat(pixmap: &mut Pixmap, c: &Canvas) {
    fill(pixmap, &c.ellipse(-8.0, -8.5, 5.0, 5.0), c.ink);
    fill(pixmap, &c.ellipse(8.0, -8.5, 5.0, 5.0), c.ink);
    fill(
        pixmap,
        &c.ellipse(-8.0, -8.5, 2.4, 2.4),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(
        pixmap,
        &c.ellipse(8.0, -8.5, 2.4, 2.4),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(pixmap, &c.ellipse(0.0, 0.0, 10.5, 9.0), c.ink);
    fill(
        pixmap,
        &c.poly(&[(0.0, 6.0), (4.8, 12.5), (-4.8, 12.5)]),
        c.ink,
    );
    fill(
        pixmap,
        &c.ellipse(0.0, 12.5, 1.4, 1.2),
        Color::from_rgba8(247, 238, 219, 255),
    );
    eyes(pixmap, c);
    for (a, b) in [(-5.0, 7.0), (-2.0, 8.5)] {
        stroke(
            pixmap,
            &c.line(&[(a, b), (a - 6.0, b + 1.5)]),
            c.ink,
            0.8 * c.s,
        );
        stroke(
            pixmap,
            &c.line(&[(a, b), (a - 6.0, b - 1.5)]),
            c.ink,
            0.8 * c.s,
        );
        stroke(
            pixmap,
            &c.line(&[(-a, b), (-a + 6.0, b + 1.5)]),
            c.ink,
            0.8 * c.s,
        );
        stroke(
            pixmap,
            &c.line(&[(-a, b), (-a + 6.0, b - 1.5)]),
            c.ink,
            0.8 * c.s,
        );
    }
}

fn cat(pixmap: &mut Pixmap, c: &Canvas) {
    fill(
        pixmap,
        &c.poly(&[(-9.0, -5.0), (-7.0, -16.0), (-1.0, -8.0)]),
        c.ink,
    );
    fill(
        pixmap,
        &c.poly(&[(9.0, -5.0), (7.0, -16.0), (1.0, -8.0)]),
        c.ink,
    );
    fill(pixmap, &c.ellipse(0.0, 0.0, 10.5, 9.5), c.ink);
    fill(
        pixmap,
        &c.ellipse(0.0, 4.5, 4.5, 3.0),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(
        pixmap,
        &c.poly(&[(0.0, 4.0), (1.6, 6.0), (-1.6, 6.0)]),
        c.ink,
    );
    eyes(pixmap, c);
    for y in [4.5, 6.5] {
        stroke(
            pixmap,
            &c.line(&[(3.0, y), (12.0, y - 1.0)]),
            c.ink,
            0.7 * c.s,
        );
        stroke(
            pixmap,
            &c.line(&[(-3.0, y), (-12.0, y - 1.0)]),
            c.ink,
            0.7 * c.s,
        );
    }
}

fn dog(pixmap: &mut Pixmap, c: &Canvas) {
    fill(pixmap, &c.ellipse(-9.5, 1.0, 3.6, 8.0), c.ink);
    fill(pixmap, &c.ellipse(9.5, 1.0, 3.6, 8.0), c.ink);
    fill(pixmap, &c.ellipse(0.0, -1.0, 9.5, 8.5), c.ink);
    fill(
        pixmap,
        &c.ellipse(0.0, 5.0, 5.0, 4.0),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(pixmap, &c.ellipse(0.0, 3.5, 1.8, 1.3), c.ink);
    fill(
        pixmap,
        &c.ellipse(0.0, 8.5, 1.5, 2.2),
        Color::from_rgba8(200, 90, 70, 255),
    );
    eyes(pixmap, c);
}

fn wolf(pixmap: &mut Pixmap, c: &Canvas) {
    fill(
        pixmap,
        &c.poly(&[(-10.0, -4.0), (-9.0, -18.0), (-2.0, -9.0)]),
        c.ink,
    );
    fill(
        pixmap,
        &c.poly(&[(10.0, -4.0), (9.0, -18.0), (2.0, -9.0)]),
        c.ink,
    );
    fill(pixmap, &c.ellipse(0.0, -1.0, 9.5, 8.5), c.ink);
    fill(
        pixmap,
        &c.poly(&[(-4.0, 2.0), (4.0, 2.0), (2.0, 13.0), (-2.0, 13.0)]),
        c.ink,
    );
    fill(
        pixmap,
        &c.ellipse(0.0, 12.5, 2.0, 1.5),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(
        pixmap,
        &c.poly(&[(-10.0, 2.0), (-13.0, 7.0), (-8.0, 8.0)]),
        c.ink,
    );
    fill(
        pixmap,
        &c.poly(&[(10.0, 2.0), (13.0, 7.0), (8.0, 8.0)]),
        c.ink,
    );
    eyes(pixmap, c);
}

fn leopard(pixmap: &mut Pixmap, c: &Canvas) {
    fill(pixmap, &c.ellipse(-7.5, -8.0, 4.0, 4.0), c.ink);
    fill(pixmap, &c.ellipse(7.5, -8.0, 4.0, 4.0), c.ink);
    fill(pixmap, &c.ellipse(0.0, 0.0, 10.5, 9.5), c.ink);
    for (x, y) in [(-6.0, -3.5), (6.0, -3.5), (-3.0, 3.0), (4.0, 2.0)] {
        fill(
            pixmap,
            &c.ellipse(x, y, 1.1, 1.1),
            Color::from_rgba8(247, 238, 219, 255),
        );
    }
    fill(
        pixmap,
        &c.ellipse(0.0, 5.0, 4.6, 3.2),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(pixmap, &c.ellipse(0.0, 3.8, 1.6, 1.2), c.ink);
    eyes(pixmap, c);
}

fn tiger(pixmap: &mut Pixmap, c: &Canvas) {
    fill(pixmap, &c.ellipse(-8.0, -8.0, 4.8, 4.8), c.ink);
    fill(pixmap, &c.ellipse(8.0, -8.0, 4.8, 4.8), c.ink);
    fill(pixmap, &c.ellipse(0.0, 0.0, 10.5, 9.5), c.ink);
    fill(
        pixmap,
        &c.ellipse(0.0, 5.2, 4.8, 3.2),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(pixmap, &c.ellipse(0.0, 4.0, 1.7, 1.2), c.ink);
    for x in [-4.5, 0.0, 4.5] {
        stroke(
            pixmap,
            &c.line(&[(x, -8.5), (x * 0.7, -4.0)]),
            Color::from_rgba8(247, 238, 219, 255),
            1.1 * c.s,
        );
    }
    for (x, y) in [(-8.0, 1.0), (8.0, 1.0)] {
        stroke(
            pixmap,
            &c.line(&[(x, y), (x * 0.6, y + 3.0)]),
            Color::from_rgba8(247, 238, 219, 255),
            1.0 * c.s,
        );
    }
    eyes(pixmap, c);
}

fn lion(pixmap: &mut Pixmap, c: &Canvas) {
    let mut mane = PathBuilder::new();
    for i in 0..14 {
        let a = i as f32 / 14.0 * std::f32::consts::TAU;
        let b = (i as f32 + 0.5) / 14.0 * std::f32::consts::TAU;
        let (x1, y1) = c.p(a.cos() * 9.0, a.sin() * 9.0);
        let (x2, y2) = c.p(b.cos() * 15.0, b.sin() * 15.0);
        let (x3, y3) = c.p(
            (b + 0.5 / 14.0 * std::f32::consts::TAU).cos() * 9.0,
            (b + 0.5 / 14.0 * std::f32::consts::TAU).sin() * 9.0,
        );
        if i == 0 {
            mane.move_to(x1, y1);
        } else {
            mane.line_to(x1, y1);
        }
        mane.line_to(x2, y2);
        mane.line_to(x3, y3);
    }
    mane.close();
    if let Some(path) = mane.finish() {
        fill(pixmap, &path, c.ink);
    }
    fill(
        pixmap,
        &c.ellipse(0.0, 0.0, 8.8, 8.8),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(pixmap, &c.ellipse(0.0, 1.0, 7.0, 7.0), c.ink);
    fill(
        pixmap,
        &c.ellipse(0.0, 5.0, 4.4, 3.2),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(pixmap, &c.ellipse(0.0, 3.8, 1.6, 1.2), c.ink);
    eyes(pixmap, c);
}

fn elephant(pixmap: &mut Pixmap, c: &Canvas) {
    fill(pixmap, &c.ellipse(-10.5, -1.0, 7.5, 9.5), c.ink);
    fill(pixmap, &c.ellipse(10.5, -1.0, 7.5, 9.5), c.ink);
    fill(
        pixmap,
        &c.ellipse(-10.5, -1.0, 4.8, 6.6),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(
        pixmap,
        &c.ellipse(10.5, -1.0, 4.8, 6.6),
        Color::from_rgba8(247, 238, 219, 255),
    );
    fill(pixmap, &c.ellipse(0.0, -1.0, 8.0, 8.5), c.ink);
    let mut trunk = PathBuilder::new();
    let (x0, y0) = c.p(0.0, 4.0);
    let (x1, y1) = c.p(3.5, 10.0);
    let (x2, y2) = c.p(3.0, 15.5);
    let (x3, y3) = c.p(-1.0, 16.0);
    trunk.move_to(x0 - 2.2 * c.s, y0);
    trunk.cubic_to(x1 - 2.0 * c.s, y1, x2 - 2.0 * c.s, y2, x3, y3);
    trunk.line_to(x3 + 1.0 * c.s, y3);
    trunk.cubic_to(
        x2 + 1.0 * c.s,
        y2 - 1.0 * c.s,
        x1 + 1.0 * c.s,
        y1 - 1.0 * c.s,
        x0 + 2.2 * c.s,
        y0,
    );
    trunk.close();
    if let Some(path) = trunk.finish() {
        fill(pixmap, &path, c.ink);
    }
    for x in [-5.5, 5.5] {
        stroke(
            pixmap,
            &c.line(&[(x, 3.0), (x, 9.0)]),
            Color::from_rgba8(247, 238, 219, 255),
            4.0 * c.s,
        );
    }
    eyes(pixmap, c);
}

/// Draws a tiny 448x576 board-like swatch for menus/social cards? Kept simple:
/// a rounded rectangle in the brand palette with a subtle diagonal.
pub fn draw_swatch(pixmap: &mut Pixmap, color: Color) {
    let mut paint = Paint::default();
    paint.set_color(color);
    if let Some(rect) = Rect::from_xywh(0.0, 0.0, pixmap.width() as f32, pixmap.height() as f32) {
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}
