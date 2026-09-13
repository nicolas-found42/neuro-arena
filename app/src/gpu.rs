//! The GPU: one adapter, one device, one queue, and the offscreen target the
//! golden-frame test reads back.
//!
//! Failures are values, not panics. A machine without a usable adapter or
//! device reports why, and a lost device or a validation error is recorded so
//! the window can show it: a long run must never be lost silently.

use std::sync::{Arc, Mutex};

use wgpu::util::DeviceExt;

/// The last error the GPU reported, for the window to display.
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Record a GPU-side failure without unwinding: validation errors raised inside
/// the window's draw callback would otherwise abort the process.
pub fn note_error(message: impl Into<String>) {
    let message = message.into();
    let mut slot = LAST_ERROR.lock().expect("no panic holds this lock");
    if slot.is_none() {
        log::error!("{message}");
        *slot = Some(message);
    }
}

/// Take the pending GPU error, if any.
pub fn take_error() -> Option<String> {
    LAST_ERROR.lock().expect("no panic holds this lock").take()
}

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
}

impl Gpu {
    /// The limits the device is opened with. Not the downlevel defaults: those
    /// cap `max_texture_dimension_2d` at 2048, and a Retina window configures
    /// its surface at physical pixels — 2880×1440 for a 1440×720 logical
    /// window — which wgpu-core then rejects with `TooLarge` through the
    /// uncaptured error handler, leaving the surface unconfigured so the first
    /// `get_current_texture` aborts inside the window's draw callback.
    pub fn limits() -> wgpu::Limits {
        wgpu::Limits::default()
    }
}

#[cfg(test)]
mod limits {
    use super::Gpu;

    /// The device must accept every mainstream display size: a 6K Pro Display
    /// XDR presents 6016×3384 physical pixels. No headless seam reaches a real
    /// window surface, so this pins the one CI-checkable fact the startup
    /// abort hinged on.
    #[test]
    fn the_device_accepts_mainstream_display_sizes() {
        assert!(Gpu::limits().max_texture_dimension_2d >= 6016);
    }
}

impl Gpu {
    /// Open the GPU on an existing instance. `compatible_surface` selects an
    /// adapter that can present to the window; pass `None` for an offscreen-only
    /// device.
    pub fn new(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|error| format!("no usable GPU adapter on this machine: {error}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("neuroarena-device"),
            required_features: wgpu::Features::empty(),
            required_limits: Self::limits(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
        }))
        .map_err(|error| format!("the GPU refused to open a device: {error}"))?;
        device.on_uncaptured_error(Arc::new(|error| {
            note_error(format!("GPU error: {error}"));
        }));
        let info = adapter.get_info();
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            info,
        })
    }

    /// One line naming the device, for the status bar.
    pub fn describe(&self) -> String {
        format!(
            "{} ({:?}, {:?})",
            self.info.name, self.info.backend, self.info.device_type
        )
    }

    /// A vertex buffer holding `bytes`, or `None` when there is nothing to
    /// draw. Recreates the buffer whenever the data does not fit.
    pub fn upload_vertices(
        buffer: &mut Option<wgpu::Buffer>,
        capacity: &mut usize,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        bytes: &[u8],
    ) {
        if bytes.is_empty() {
            return;
        }
        if *capacity < bytes.len() || buffer.is_none() {
            // Grow to at least double, so a slowly growing frame does not
            // reallocate every frame.
            let new_capacity = (bytes.len() * 2).next_power_of_two();
            *buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: new_capacity as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            *capacity = new_capacity;
        }
        if let Some(buffer) = buffer.as_ref() {
            queue.write_buffer(buffer, 0, bytes);
        }
    }
}

/// A colour target that lives off screen, so a frame can be rendered and read
/// back without a window — the golden-frame seam.
pub struct Offscreen {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Offscreen {
    pub fn new(gpu: &Gpu, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen-colour"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            width,
            height,
            format,
            texture,
            view,
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Copy the target back to the CPU as tightly packed RGBA8, stripping the
    /// 256-byte row alignment the copy requires.
    pub fn read_rgba(&self, gpu: &Gpu) -> Vec<u8> {
        let unpadded = self.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded) * u64::from(self.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback-encoder"),
            });
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            if let Err(error) = result {
                note_error(format!("could not map the readback buffer: {error}"));
            }
        });
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device is pollable");
        let mapped = slice
            .get_mapped_range()
            .expect("the readback buffer is mapped");
        let mut pixels = Vec::with_capacity((unpadded * self.height) as usize);
        for row in 0..self.height {
            let start = (row * padded) as usize;
            pixels.extend_from_slice(&mapped[start..start + unpadded as usize]);
        }
        drop(mapped);
        buffer.unmap();
        pixels
    }
}

/// A vertex buffer that grows on demand and remembers its capacity.
#[derive(Default)]
pub struct VertexBuffer {
    pub buffer: Option<wgpu::Buffer>,
    pub capacity: usize,
    pub vertices: u32,
}

impl VertexBuffer {
    pub fn upload(&mut self, gpu: &Gpu, label: &str, bytes: &[u8], vertex_size: usize) {
        self.vertices = bytes.len().checked_div(vertex_size).unwrap_or(0) as u32;
        Gpu::upload_vertices(
            &mut self.buffer,
            &mut self.capacity,
            &gpu.device,
            &gpu.queue,
            label,
            bytes,
        );
    }

    pub fn slice(&self) -> Option<wgpu::BufferSlice<'_>> {
        self.buffer.as_ref().map(|buffer| buffer.slice(..))
    }
}

/// A tiny helper so `create_buffer_init` is available alongside the raw path.
pub fn buffer_with_data(device: &wgpu::Device, label: &str, contents: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}
