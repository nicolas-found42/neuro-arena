//! Painter: the drawing contract between what a frame is made of and how it
//! reaches the GPU.
//!
//! Everything on screen is produced by pushing into a `Painter`: triangles,
//! lines, and text anchors. Triangles and lines are already in window pixels;
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
        [srgb_to_linear(self.r), srgb_to_linear(self.g), srgb_to_linear(self.b), self.a]
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

/// One solid-colour vertex edge.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

/// One triangle vertex.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TriangleVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

/// A string to draw, anchored at a window-pixel position.
#[derive(Clone, Debug, PartialEq)]
pub struct TextItem {
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
    pub lines: Vec<LineVertex>,
    pub text: Vec<TextItem>,
    transform: Transform,
}

impl Painter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a frame: drop the previous one and reset the transform.
    pub fn clear(&mut self) {
        self.triangles.clear();
        self.lines.clear();
        self.text.clear();
        self.transform = Transform::IDENTITY;
    }

    pub fn set_transform(&mut self, transform: Transform) {
        self.transform = transform;
    }

    pub fn transform(&self) -> Transform {
        self.transform
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty() && self.lines.is_empty() && self.text.is_empty()
    }

    #[inline]
    fn point(&self, p: [f32; 2]) -> [f32; 2] {
        self.transform.apply(p)
    }

    pub fn triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: Rgba) {
        let color = color.to_linear();
        for p in [a, b, c] {
            self.triangles.push(TriangleVertex {
                pos: self.point(p),
                color,
            });
        }
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
        let color = color.to_linear();
        self.lines.push(LineVertex {
            pos: self.point(a),
            color,
        });
        self.lines.push(LineVertex {
            pos: self.point(b),
            color,
        });
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
    pub fn panel(&mut self, x: f32, y: f32, width: f32, height: f32, fill: Rgba, border: Option<Rgba>) {
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
            pos: self.point(pos),
            size: size * self.transform.scale,
            color,
            align,
            content: content.into(),
        });
    }
}
