//! Text: cosmic-text shapes a string with a system font, swash rasterizes each
//! glyph, and this module emits one quad per glyph, sampling an atlas.
//!
//! Glyphs are shaped at their final on-screen size, so the window shows real
//! pixels rather than a scaled bitmap. Alignment is resolved here rather than
//! by the caller: the text renderer is the only thing that knows how wide a
//! shaped string is.

use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
};

use crate::atlas::{Atlas, GlyphCache, Slot};
use crate::gpu::Gpu;
use crate::painter::{Align, TextItem};

/// One glyph quad.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: Atlas,
    cache: GlyphCache,
    buffer: Buffer,
    vertices: Vec<TextVertex>,
    /// Glyphs emitted and glyphs the atlas could not hold, for diagnostics.
    pub glyphs: usize,
    pub skipped: usize,
}

impl TextRenderer {
    pub fn new(gpu: &Gpu, layout: &wgpu::BindGroupLayout) -> Self {
        let mut font_system = FontSystem::new();
        let buffer = Buffer::new(&mut font_system, Metrics::new(12.0, 16.0));
        Self {
            font_system,
            swash_cache: SwashCache::new(),
            atlas: Atlas::new(gpu, layout),
            cache: GlyphCache::default(),
            buffer,
            vertices: Vec::new(),
            glyphs: 0,
            skipped: 0,
        }
    }

    pub fn atlas_bind_group(&self) -> &wgpu::BindGroup {
        self.atlas.bind_group()
    }

    pub fn cached_glyphs(&self) -> usize {
        self.cache.len()
    }

    /// The quads produced by the last [`TextRenderer::build`].
    pub fn vertices(&self) -> &[TextVertex] {
        &self.vertices
    }

    /// Shape and rasterize every item into the vertex list. The list is
    /// replaced by the next call.
    pub fn build(&mut self, gpu: &Gpu, items: &[TextItem]) {
        self.vertices.clear();
        self.glyphs = 0;
        self.skipped = 0;
        for item in items {
            if item.content.is_empty() || item.size <= 0.0 {
                continue;
            }
            self.build_item(gpu, item);
        }
    }

    fn build_item(&mut self, gpu: &Gpu, item: &TextItem) {
        // Monospace by family, and a line height that keeps the descenders of
        // one row clear of the caps of the next.
        let attrs = Attrs::new().family(Family::Monospace);
        self.buffer
            .set_metrics(Metrics::new(item.size, item.size * 1.3));
        self.buffer.set_size(Some(8192.0), None);
        self.buffer
            .set_text(&item.content, &attrs, Shaping::Advanced, None);
        self.buffer.shape_until_scroll(&mut self.font_system, false);

        // Split the borrows up front: the layout runs borrow the buffer, while
        // rasterizing a glyph needs the font system, the swash cache and the
        // atlas at the same time.
        let Self {
            buffer,
            font_system,
            swash_cache,
            atlas,
            cache,
            vertices,
            ..
        } = self;
        let color = item.color.to_linear();
        let mut drawn = 0usize;
        let mut missed = 0usize;
        for run in buffer.layout_runs() {
            let shift = match item.align {
                Align::Left => 0.0,
                Align::Center => -run.line_w * 0.5,
                Align::Right => -run.line_w,
            };
            let x0 = item.pos[0] + shift;
            for glyph in run.glyphs {
                // `line_y` is the baseline of the run.
                let physical = glyph.physical((x0, run.line_y + item.pos[1]), 1.0);
                let key = physical.cache_key;
                let Some(slot) = place(gpu, font_system, swash_cache, atlas, cache, key) else {
                    missed += 1;
                    continue;
                };
                let Slot {
                    x,
                    y,
                    width,
                    height,
                    left,
                    top,
                } = slot;
                let (uv0, uv1) = atlas.uvs(x, y, width, height);
                let quad_x0 = (physical.x + left) as f32;
                let quad_y0 = (physical.y - top) as f32;
                let quad_x1 = quad_x0 + width as f32;
                let quad_y1 = quad_y0 + height as f32;
                let corner = |pos: [f32; 2], uv: [f32; 2]| TextVertex { pos, uv, color };
                vertices.extend_from_slice(&[
                    corner([quad_x0, quad_y0], [uv0[0], uv0[1]]),
                    corner([quad_x1, quad_y0], [uv1[0], uv0[1]]),
                    corner([quad_x1, quad_y1], [uv1[0], uv1[1]]),
                    corner([quad_x0, quad_y0], [uv0[0], uv0[1]]),
                    corner([quad_x1, quad_y1], [uv1[0], uv1[1]]),
                    corner([quad_x0, quad_y1], [uv0[0], uv1[1]]),
                ]);
                drawn += 1;
            }
        }
        self.glyphs += drawn;
        self.skipped += missed;
    }
}

/// Find a glyph's bitmap in the atlas, rasterizing and uploading it on first
/// use. The cached slot carries the pen offset, so a hit and a miss return the
/// same shape. Returns `None` when the glyph has no ink.
fn place(
    gpu: &Gpu,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    atlas: &mut Atlas,
    cache: &mut GlyphCache,
    key: CacheKey,
) -> Option<Slot> {
    if let Some(slot) = cache.get(&key) {
        // A cached glyph with no bitmap was checked before; report it as no ink.
        return (slot.width > 0 && slot.height > 0).then_some(slot);
    }
    let image = swash_cache.get_image_uncached(font_system, key)?;
    let width = image.placement.width;
    let height = image.placement.height;
    let left = image.placement.left;
    let top = image.placement.top;
    let empty = Slot {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        left,
        top,
    };
    if width == 0 || height == 0 {
        // A glyph with no ink still advances the pen; nothing to draw.
        cache.insert(key, empty);
        return None;
    }
    let rgba: Vec<u8> = match image.content {
        // A coverage mask becomes white with the coverage in alpha.
        SwashContent::Mask => image
            .data
            .iter()
            .flat_map(|alpha| [255u8, 255, 255, *alpha])
            .collect(),
        // Colour bitmaps (emoji) arrive as RGBA already.
        SwashContent::Color | SwashContent::SubpixelMask => image
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
            .collect(),
    };
    let (x, y) = match atlas.allocate(width, height) {
        Some(position) => position,
        None => {
            // The atlas filled up: drop the cache and start a fresh shelf run.
            // Stale texels are never sampled — every glyph gets its own slot.
            cache.clear();
            atlas.reset();
            atlas.allocate(width, height)?
        }
    };
    atlas.upload(gpu, x, y, width, height, &rgba);
    let slot = Slot {
        x,
        y,
        width,
        height,
        left,
        top,
    };
    cache.insert(key, slot);
    Some(slot)
}
