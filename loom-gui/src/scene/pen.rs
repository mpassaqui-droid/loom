//! A tiny vector "pen" for the scenes: everything a frame draws goes into one
//! coloured triangle mesh (one draw call), with the canvas-2D tricks the
//! browser uses rebuilt from triangles: glow (`shadowBlur`) = a wide faint
//! additive stroke under a thin bright one, gradients = per-vertex colours,
//! `lighter` compositing = premultiplied colours with zero alpha.

use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Vec2};

use super::logic::hsl_to_rgb;

/// A colour from HSL (hue 0..360, saturation/lightness 0..1).
pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Color32 {
    let (r, g, b) = hsl_to_rgb(hue, saturation, lightness);
    Color32::from_rgb(r, g, b)
}

/// `color` at opacity `alpha` (0..1), normal blending.
pub fn alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

/// `color` at strength `amount` (0..1), added to what is below (light adds up, like neon).
pub fn glow(color: Color32, amount: f32) -> Color32 {
    let a = amount.clamp(0.0, 1.0);
    let s = |v: u8| (v as f32 * a) as u8;
    Color32::from_rgba_premultiplied(s(color.r()), s(color.g()), s(color.b()), 0)
}

/// Collects triangles for one frame; [`Pen::flush`] hands them to egui.
#[derive(Default)]
pub struct Pen {
    mesh: Mesh,
    /// Triangles per frame last time, to reserve the right size up front.
    last_size: (usize, usize),
}

impl Pen {
    /// Send what was drawn so far to the painter (call before drawing text on top).
    pub fn flush(&mut self, painter: &Painter) {
        if self.mesh.is_empty() {
            return;
        }
        let mesh = std::mem::take(&mut self.mesh);
        self.last_size = (self.last_size.0.max(mesh.vertices.len()), self.last_size.1.max(mesh.indices.len()));
        painter.add(Shape::mesh(mesh));
        self.mesh.vertices.reserve(self.last_size.0 / 2);
        self.mesh.indices.reserve(self.last_size.1 / 2);
    }

    fn vertex(&mut self, pos: Pos2, color: Color32) -> u32 {
        let i = self.mesh.vertices.len() as u32;
        self.mesh.colored_vertex(pos, color);
        i
    }

    /// A four-cornered shape with a colour per corner (a, b, c, d in order).
    pub fn quad(&mut self, p: [Pos2; 4], c: [Color32; 4]) {
        let i = self.vertex(p[0], c[0]);
        self.vertex(p[1], c[1]);
        self.vertex(p[2], c[2]);
        self.vertex(p[3], c[3]);
        self.mesh.add_triangle(i, i + 1, i + 2);
        self.mesh.add_triangle(i, i + 2, i + 3);
    }

    pub fn rect(&mut self, rect: Rect, color: Color32) {
        self.mesh.add_colored_rect(rect, color);
    }

    /// A rectangle fading from `top` to `bottom`.
    pub fn vgradient(&mut self, rect: Rect, top: Color32, bottom: Color32) {
        self.quad([rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()], [top, top, bottom, bottom]);
    }

    /// A straight line `width` wide (at least one pixel, or it would flicker).
    pub fn line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32) {
        let width = width.max(1.0);
        let d = b - a;
        let len = d.length();
        if len < 1e-3 {
            return;
        }
        let dir = d / len;
        let n = Vec2::new(-dir.y, dir.x) * (width * 0.5);
        let ext = dir * (width * 0.35); // overlap a little so polylines have no gaps
        let (a, b) = (a - ext, b + ext);
        self.quad([a + n, b + n, b - n, a - n], [color; 4]);
    }

    /// A line with a neon halo.
    pub fn glow_line(&mut self, a: Pos2, b: Pos2, width: f32, color: Color32, strength: f32) {
        self.line(a, b, width * 4.0 + 2.0, glow(color, strength * 0.12));
        self.line(a, b, width * 2.0, glow(color, strength * 0.25));
        self.line(a, b, width, alpha(color, strength));
    }

    /// Connected line segments.
    pub fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32) {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], width, color);
        }
    }

    /// Connected line segments with a neon halo.
    pub fn glow_polyline(&mut self, points: &[Pos2], width: f32, color: Color32, strength: f32) {
        self.polyline(points, width * 3.5 + 2.0, glow(color, strength * 0.12));
        self.polyline(points, width, alpha(color, strength));
    }

    fn segments(radius: f32) -> usize {
        ((radius * 0.6) as usize).clamp(8, 96)
    }

    /// A filled circle whose colour goes from `inner` (centre) to `outer` (edge).
    /// Tiny circles are drawn one pixel wide, so sparks don't vanish.
    pub fn radial(&mut self, center: Pos2, radius: f32, inner: Color32, outer: Color32) {
        let radius = radius.max(0.9);
        let n = Self::segments(radius);
        let c = self.vertex(center, inner);
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            self.vertex(center + Vec2::angled(a) * radius, outer);
        }
        for k in 0..n as u32 {
            self.mesh.add_triangle(c, c + 1 + k, c + 1 + (k + 1) % n as u32);
        }
    }

    pub fn disc(&mut self, center: Pos2, radius: f32, color: Color32) {
        self.radial(center, radius, color, color);
    }

    /// A dot with a soft glow around it.
    pub fn glow_dot(&mut self, center: Pos2, radius: f32, color: Color32, strength: f32) {
        self.radial(center, radius * 3.0 + 2.0, glow(color, strength * 0.35), Color32::TRANSPARENT);
        self.disc(center, radius, alpha(color, strength));
    }

    /// A circle outline.
    pub fn ring(&mut self, center: Pos2, radius: f32, width: f32, color: Color32) {
        self.band(center, radius - width * 0.5, radius + width * 0.5, color, color);
    }

    /// The area between two circles, its colour going from `inner` to `outer`.
    pub fn band(&mut self, center: Pos2, r_in: f32, r_out: f32, inner: Color32, outer: Color32) {
        let (r0, r1) = (r_in.max(0.0), r_out.max(0.0));
        if r1 <= 0.1 {
            return;
        }
        let n = Self::segments(r1).max(24);
        let first = self.mesh.vertices.len() as u32;
        for k in 0..n {
            let dir = Vec2::angled(k as f32 / n as f32 * std::f32::consts::TAU);
            self.vertex(center + dir * r0, inner);
            self.vertex(center + dir * r1, outer);
        }
        for k in 0..n as u32 {
            let (a, b) = (first + 2 * k, first + 2 * ((k + 1) % n as u32));
            self.mesh.add_triangle(a, a + 1, b + 1);
            self.mesh.add_triangle(a, b + 1, b);
        }
    }

    /// A circle outline with a neon halo.
    pub fn glow_ring(&mut self, center: Pos2, radius: f32, width: f32, color: Color32, strength: f32) {
        let halo = width * 2.0 + 5.0;
        let g = glow(color, strength * 0.22);
        self.band(center, radius - halo, radius, Color32::TRANSPARENT, g);
        self.band(center, radius, radius + halo, g, Color32::TRANSPARENT);
        self.ring(center, radius, width, alpha(color, strength));
    }
}
