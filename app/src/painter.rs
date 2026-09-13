//! Painter: the drawing contract between what a frame is made of and how it
//! reaches the GPU.
//!
//! Everything on screen is produced by pushing into a `Painter`: triangles,
//! strokes, and text anchors. Triangles are already in window pixels;
//! text carries an anchor and an alignment, and the text renderer resolves the
//! alignment once it has shaped the string, so no caller ever has to measure
//! text itself.
//!
//! A transform maps logical coordinates (the 960×600 Arena) into window pixels,
//! which is what keeps the Arena's shape fixed while the window resizes
//! (ADR 0006).

/// A straight-alpha colour, components in `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    // Keep the paired rgb/rgba color constructors explicit at drawing call sites.
    #[expect(clippy::self_named_constructors)]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// The same colour at a different opacity.
    pub const fn alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Blend toward another colour, `t` in `0..=1`.
    pub fn mix(self, other: Rgba, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Components as authored (sRGB), for tests and comparisons.
    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// The colour in linear space, which is what the GPU blends.
    ///
    /// Colours in this app are authored the way a designer reads them — the
    /// bytes a colour picker shows — and the render target is sRGB, so the
    /// value a shader writes is encoded on store. Converting here means the
    /// window shows the colour that was written down, and alpha blending
    /// happens where it belongs, in linear space.
    pub fn to_linear(self) -> [f32; 4] {
        [
            srgb_to_linear(self.r),
            srgb_to_linear(self.g),
            srgb_to_linear(self.b),
            self.a,
        ]
    }
}

/// The sRGB electro-optical transfer function.
#[inline]
pub fn srgb_to_linear(channel: f32) -> f32 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Where a text anchor sits relative to the string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Logical → window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub scale: f32,
    pub offset: [f32; 2],
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        offset: [0.0, 0.0],
    };

    pub const fn new(scale: f32, offset: [f32; 2]) -> Self {
        Self { scale, offset }
    }

    #[inline]
    pub fn apply(&self, point: [f32; 2]) -> [f32; 2] {
        [
            point[0] * self.scale + self.offset[0],
            point[1] * self.scale + self.offset[1],
        ]
    }

    /// A transform that first applies `self`, then `outer`.
    pub fn then(&self, outer: &Transform) -> Self {
        Self {
            scale: self.scale * outer.scale,
            offset: outer.apply(self.offset),
        }
    }
}

/// One triangle vertex.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TriangleVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typeface {
    Mono,
    Display,
}

/// A string to draw, anchored at a window-pixel position.
#[derive(Clone, Debug, PartialEq)]
pub struct TextItem {
    pub face: Typeface,
    pub pos: [f32; 2],
    pub size: f32,
    pub color: Rgba,
    pub align: Align,
    pub content: String,
}

/// The frame, as data.
#[derive(Default)]
pub struct Painter {
    pub triangles: Vec<TriangleVertex>,
    pub text: Vec<TextItem>,
    transform: Transform,
    clip: Option<[f32; 4]>,
    paths: crate::vector::Paths,
}

impl Painter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a frame: drop the previous one and reset the transform.
    pub fn clear(&mut self) {
        self.triangles.clear();
        self.text.clear();
        self.transform = Transform::IDENTITY;
        self.clip = None;
    }

    pub fn set_transform(&mut self, transform: Transform) {
        self.transform = transform;
    }

    pub fn transform(&self) -> Transform {
        self.transform
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty() && self.text.is_empty()
    }

    #[inline]
    fn point(&self, p: [f32; 2]) -> [f32; 2] {
        self.transform.apply(p)
    }

    pub fn triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: Rgba) {
        let points = [self.point(a), self.point(b), self.point(c)];
        let color = color.to_linear();
        if let Some(clip) = self.clip {
            if points
                .iter()
                .any(|p| p[0] < clip[0] || p[1] < clip[1] || p[0] > clip[2] || p[1] > clip[3])
            {
                let points = clip_polygon(&points, clip);
                for i in 1..points.len().saturating_sub(1) {
                    for pos in [points[0], points[i], points[i + 1]] {
                        self.triangles.push(TriangleVertex { pos, color });
                    }
                }
                return;
            }
        }
        for pos in points {
            self.triangles.push(TriangleVertex { pos, color });
        }
    }

    /// Clip subsequent geometry to a rectangle in the current logical space.
    pub fn set_clip(&mut self, rect: Option<[f32; 4]>) {
        self.clip = rect.map(|r| {
            let a = self.point([r[0], r[1]]);
            let b = self.point([r[2], r[3]]);
            [a[0], a[1], b[0], b[1]]
        });
    }

    /// A convex quad, as two triangles.
    pub fn quad(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2], color: Rgba) {
        self.triangle(a, b, c, color);
        self.triangle(a, c, d, color);
    }

    pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Rgba) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        self.quad(
            [x, y],
            [x + width, y],
            [x + width, y + height],
            [x, y + height],
            color,
        );
    }

    /// A filled convex polygon, fan-triangulated from its first vertex.
    pub fn polygon(&mut self, points: &[[f32; 2]], color: Rgba) {
        if points.len() < 3 {
            return;
        }
        for index in 1..points.len() - 1 {
            self.triangle(points[0], points[index], points[index + 1], color);
        }
    }

    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], color: Rgba) {
        self.stroke(a, b, 1.0, color);
    }

    /// A logical-width stroke in submission order with the filled geometry.
    pub fn stroke(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: Rgba) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let length = dx.hypot(dy);
        if length < 0.001 || width <= 0.0 {
            return;
        }
        let nx = -dy / length * width * 0.5;
        let ny = dx / length * width * 0.5;
        self.polygon(
            &[
                [a[0] + nx, a[1] + ny],
                [b[0] + nx, b[1] + ny],
                [b[0] - nx, b[1] - ny],
                [a[0] - nx, a[1] - ny],
            ],
            color,
        );
    }

    /// Smooth radial illumination: interpolated vertex alpha, no stacked-disc bands.
    pub fn glow(&mut self, center: [f32; 2], radius: f32, color: Rgba) {
        for ring in 0..5 {
            let inner = ring as f32 / 5.0;
            let outer = (ring + 1) as f32 / 5.0;
            let vertex = |r: f32, a: f32| {
                (
                    [
                        center[0] + radius * r * a.cos(),
                        center[1] + radius * r * a.sin(),
                    ],
                    color.alpha(color.a * 0.5 * (-4.0 * r * r).exp() * (1.0 - r)),
                )
            };
            for i in 0..32 {
                let a = i as f32 * std::f32::consts::TAU / 32.0;
                let b = (i + 1) as f32 * std::f32::consts::TAU / 32.0;
                self.gradient_triangle([vertex(inner, a), vertex(outer, a), vertex(outer, b)]);
                if ring > 0 {
                    self.gradient_triangle([vertex(inner, a), vertex(outer, b), vertex(inner, b)]);
                }
            }
        }
    }

    fn gradient_triangle(&mut self, points: [([f32; 2], Rgba); 3]) {
        let vertices = points.map(|(point, color)| TriangleVertex {
            pos: self.point(point),
            color: color.to_linear(),
        });
        let Some(clip) = self.clip.filter(|r| {
            vertices
                .iter()
                .any(|v| v.pos[0] < r[0] || v.pos[1] < r[1] || v.pos[0] > r[2] || v.pos[1] > r[3])
        }) else {
            self.triangles.extend(vertices);
            return;
        };
        let mut polygon = vertices.to_vec();
        for (axis, boundary, lower) in [
            (0, clip[0], true),
            (1, clip[1], true),
            (0, clip[2], false),
            (1, clip[3], false),
        ] {
            let input = std::mem::take(&mut polygon);
            let Some(mut previous) = input.last().copied() else {
                break;
            };
            let inside = |v: TriangleVertex| {
                if lower {
                    v.pos[axis] >= boundary
                } else {
                    v.pos[axis] <= boundary
                }
            };
            for current in input {
                if inside(current) != inside(previous) {
                    let t =
                        (boundary - previous.pos[axis]) / (current.pos[axis] - previous.pos[axis]);
                    let mut v = TriangleVertex {
                        pos: std::array::from_fn(|i| {
                            previous.pos[i] + t * (current.pos[i] - previous.pos[i])
                        }),
                        color: std::array::from_fn(|i| {
                            previous.color[i] + t * (current.color[i] - previous.color[i])
                        }),
                    };
                    v.pos[axis] = boundary;
                    polygon.push(v);
                }
                if inside(current) {
                    polygon.push(current);
                }
                previous = current;
            }
        }
        for i in 1..polygon.len().saturating_sub(1) {
            self.triangles
                .extend([polygon[0], polygon[i], polygon[i + 1]]);
        }
    }

    /// Smooth instrument arcs and connections, tessellated at display tolerance.
    pub fn path(&mut self, path: &lyon_tessellation::path::Path, width: f32, color: Rgba) {
        let mut paths = std::mem::take(&mut self.paths);
        paths.draw(self, path, width, color);
        self.paths = paths;
    }

    pub fn display_text(
        &mut self,
        pos: [f32; 2],
        size: f32,
        color: Rgba,
        content: impl Into<String>,
    ) {
        self.text(pos, size, color, content);
        self.text.last_mut().unwrap().face = Typeface::Display;
    }

    /// A connected run of segments; `closed` joins the last point to the first.
    pub fn polyline(&mut self, points: &[[f32; 2]], color: Rgba, closed: bool) {
        if points.len() < 2 {
            return;
        }
        for index in 0..points.len() - 1 {
            self.line(points[index], points[index + 1], color);
        }
        if closed {
            self.line(points[points.len() - 1], points[0], color);
        }
    }

    /// A rectangle outline drawn as four segments.
    pub fn rect_outline(&mut self, x: f32, y: f32, width: f32, height: f32, color: Rgba) {
        self.polyline(
            &[
                [x, y],
                [x + width, y],
                [x + width, y + height],
                [x, y + height],
            ],
            color,
            true,
        );
    }

    /// A filled circle, `segments` around the rim.
    pub fn circle(&mut self, center: [f32; 2], radius: f32, color: Rgba, segments: usize) {
        let segments = segments.max(3);
        let step = std::f32::consts::TAU / segments as f32;
        for index in 0..segments {
            let a = step * index as f32;
            let b = step * (index + 1) as f32;
            self.triangle(
                center,
                [center[0] + a.cos() * radius, center[1] + a.sin() * radius],
                [center[0] + b.cos() * radius, center[1] + b.sin() * radius],
                color,
            );
        }
    }

    /// A filled, optionally outlined block. The one helper every panel wants.
    pub fn panel(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill: Rgba,
        border: Option<Rgba>,
    ) {
        self.rect(x, y, width, height, fill);
        if let Some(border) = border {
            self.rect_outline(x, y, width, height, border);
        }
    }

    /// A text anchor. `pos` is transformed; alignment is resolved when the
    /// string is shaped, so a caller never measures text.
    pub fn text(&mut self, pos: [f32; 2], size: f32, color: Rgba, content: impl Into<String>) {
        self.text_aligned(pos, size, color, Align::Left, content);
    }

    pub fn text_aligned(
        &mut self,
        pos: [f32; 2],
        size: f32,
        color: Rgba,
        align: Align,
        content: impl Into<String>,
    ) {
        self.text.push(TextItem {
            face: Typeface::Mono,
            pos: self.point(pos),
            size: size * self.transform.scale,
            color,
            align,
            content: content.into(),
        });
    }
}

/// Sutherland–Hodgman clipping, only used by triangles crossing a clip edge.
fn clip_polygon(points: &[[f32; 2]], rect: [f32; 4]) -> Vec<[f32; 2]> {
    let mut polygon = points.to_vec();
    for (axis, boundary, lower) in [
        (0, rect[0], true),
        (1, rect[1], true),
        (0, rect[2], false),
        (1, rect[3], false),
    ] {
        let input = std::mem::take(&mut polygon);
        if input.is_empty() {
            break;
        }
        let inside = |p: [f32; 2]| {
            if lower {
                p[axis] >= boundary
            } else {
                p[axis] <= boundary
            }
        };
        let mut previous = input[input.len() - 1];
        for current in input {
            if inside(current) != inside(previous) {
                let t = (boundary - previous[axis]) / (current[axis] - previous[axis]);
                let mut intersection = [
                    previous[0] + t * (current[0] - previous[0]),
                    previous[1] + t * (current[1] - previous[1]),
                ];
                intersection[axis] = boundary;
                polygon.push(intersection);
            }
            if inside(current) {
                polygon.push(current);
            }
            previous = current;
        }
    }
    polygon
}

#[cfg(test)]
mod clipping_tests {
    use super::*;
    #[test]
    fn transformed_strokes_and_triangles_stay_inside_clip() {
        let mut p = Painter::new();
        p.set_transform(Transform::new(2.0, [10.0, 20.0]));
        p.set_clip(Some([0.0, 0.0, 100.0, 60.0]));
        p.triangle(
            [-20.0, 30.0],
            [50.0, -100.0],
            [150.0, 100.0],
            Rgba::rgb(1.0, 1.0, 1.0),
        );
        p.stroke([-100.0, 30.0], [200.0, 30.0], 4.0, Rgba::rgb(1.0, 1.0, 1.0));
        assert!(!p.triangles.is_empty());
        assert!(p.triangles.iter().all(|v| v.pos[0] >= 10.0
            && v.pos[0] <= 210.0
            && v.pos[1] >= 20.0
            && v.pos[1] <= 140.0));
        p.clear();
        assert!(p.clip.is_none());
    }
}

#[cfg(test)]
mod typography_tests {
    use super::*;
    #[test]
    fn typeface_is_independent_of_display_scale() {
        for scale in [1.0, 2.0, 3.0] {
            let mut p = Painter::new();
            p.set_transform(Transform::new(scale, [0.0, 0.0]));
            p.text([0.0, 0.0], 12.0, Rgba::rgb(1.0, 1.0, 1.0), "001.23");
            p.display_text([0.0, 20.0], 32.0, Rgba::rgb(1.0, 1.0, 1.0), "NEUROARENA");
            assert_eq!(p.text[0].face, Typeface::Mono);
            assert_eq!(p.text[1].face, Typeface::Display);
            assert_eq!(p.text[0].size, 12.0 * scale);
        }
    }
}
