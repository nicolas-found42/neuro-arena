//! The glyph atlas: a shelf-packed texture of rasterized glyph bitmaps, plus
//! the sampler that reads it.
//!
//! Coverage is stored in a non-sRGB texture: an alpha mask must not be
//! gamma-decoded. Glyphs are cached by `CacheKey`, so a frame re-blits only
//! what it has never drawn before.

use std::collections::HashMap;

use cosmic_text::CacheKey;

use crate::gpu::Gpu;

/// Where a glyph's bitmap lives in the atlas, in texels, together with the
/// offset from the pen position to the bitmap's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}

pub struct Atlas {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: u32,
    cursor_x: u32,
    cursor_y: u32,
    shelf_height: u32,
}

impl Atlas {
    pub const SIZE: u32 = 1024;

    pub fn new(gpu: &Gpu, layout: &wgpu::BindGroupLayout) -> Self {
        let size = Self::SIZE;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph-atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Nearest filtering and integer-aligned quads: a glyph bitmap must not
        // be resampled, and a linear sampler would blur thin stems.
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph-atlas-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph-atlas-bind-group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            texture,
            bind_group,
            size,
            cursor_x: 0,
            cursor_y: 0,
            shelf_height: 0,
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Reserve a shelf slot for a bitmap of `width` × `height` texels.
    /// `None` means the atlas is full and the caller should reset its cache.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width == 0 || height == 0 || width > self.size || height > self.size {
            return None;
        }
        if self.cursor_x + width > self.size {
            // Next shelf.
            self.cursor_x = 0;
            self.cursor_y += self.shelf_height + 1;
            self.shelf_height = 0;
        }
        if self.cursor_y + height > self.size {
            return None;
        }
        let slot = (self.cursor_x, self.cursor_y);
        self.cursor_x += width + 1;
        self.shelf_height = self.shelf_height.max(height);
        Some(slot)
    }

    pub fn upload(&self, gpu: &Gpu, x: u32, y: u32, width: u32, height: u32, rgba: &[u8]) {
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Normalized texture coordinates for a slot.
    pub fn uvs(&self, x: u32, y: u32, width: u32, height: u32) -> ([f32; 2], [f32; 2]) {
        let size = self.size as f32;
        (
            [x as f32 / size, y as f32 / size],
            [(x + width) as f32 / size, (y + height) as f32 / size],
        )
    }

    /// Start over. Old texels are left in place: every new glyph writes into
    /// the region it is given, and nothing ever samples the gaps.
    pub fn reset(&mut self) {
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.shelf_height = 0;
    }
}

/// The glyph bitmap cache, keyed by what actually changes the pixels.
#[derive(Default)]
pub struct GlyphCache {
    slots: HashMap<CacheKey, Slot>,
}

impl GlyphCache {
    pub fn get(&self, key: &CacheKey) -> Option<Slot> {
        self.slots.get(key).copied()
    }

    pub fn insert(&mut self, key: CacheKey, slot: Slot) {
        self.slots.insert(key, slot);
    }

    pub fn clear(&mut self) {
        self.slots.clear();
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}
