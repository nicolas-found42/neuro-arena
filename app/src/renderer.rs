//! The renderer: the frame graph the app owns, on top of `wgpu`.
//!
//! The frame is built in high dynamic range and resolved into the window at the
//! end, which is what makes light behave like light:
//!
//! 1. **Scene**, into a 4× MSAA `Rgba16Float` target. The backdrop paints the
//!    deep field; matter is alpha-blended over it; light is added over that.
//!    Because the target is floating point, a lamp can be written past white.
//! 2. **Bloom**, a half-resolution mip chain over the resolved scene: a
//!    thresholded 13-tap downsample, then a 3×3 tent upsample that adds each
//!    level back into the one above it.
//! 3. **Composite**, into the window's sRGB surface: the scene plus the bloom,
//!    which is resolved through a hue-preserving curve before it is added, so a
//!    bright core keeps its colour instead of clipping toward white; then the
//!    glyphs, which are drawn last and therefore never bloom or blur.
//!
//! The threshold sits at white with a soft knee, so nothing the interface is
//! authored in can bloom: only what the Arena writes *past* white does. That is
//! why there is no mask and no second scene pass — the separation is carried by
//! the values themselves.
//!
//! There is no framework here on purpose (ADR 0004): this is the whole of the
//! drawing path, and it is small enough to read in one sitting.

use bytemuck::cast_slice;

use crate::gpu::{Gpu, VertexBuffer};
use crate::painter::{Painter, TriangleVertex};
use crate::text::{TextRenderer, TextVertex};

pub const SHADER: &str = include_str!("shader.wgsl");

/// Where the frame is assembled before it is resolved into the window. Half
/// float rather than sRGB: the scene has to be able to hold values past white.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Coverage samples on the scene target.
const SAMPLES: u32 = 4;

/// The most levels the bloom chain builds, and the smallest level it will make.
/// Six levels at half resolution reach roughly a sixteenth of the frame, which
/// is a wide enough halo for a bullet to read as a light source without the
/// whole Arena turning to haze.
const BLOOM_LEVELS: u32 = 6;
const BLOOM_MIN_EDGE: u32 = 8;

/// How much of the bloom chain reaches the window.
const BLOOM_INTENSITY: f32 = 0.215;

/// The Arena's grain, as a fraction of the value under it.
///
/// It is dither before it is character. The deep field is a wide, smooth
/// gradient over a very dark ground, and eight bits of sRGB cannot hold that
/// without contouring — without this the nebulae come out as stepped bands. A
/// multiplicative noise is roughly constant once the target encodes it, so this
/// works out at about one least-significant bit everywhere, which is exactly
/// enough to break the steps up. It is a pattern of the pixel and not of the
/// clock, so a paused frame is perfectly still and two captures of one frame
/// are identical.
const GRAIN: f32 = 0.06;

/// Everything the shader needs to know about the frame, in one upload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    viewport: [f32; 4],
    arena: [f32; 4],
    params: [f32; 4],
    app_bg: [f32; 4],
    arena_bg: [f32; 4],
    nebula_a: [f32; 4],
    nebula_b: [f32; 4],
    wash: [f32; 4],
    vignette: [f32; 4],
}

/// What the app tells the renderer about this frame's geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The window's extent in physical pixels.
    pub viewport: [f32; 2],
    /// The Arena's rectangle in physical pixels: `x, y, width, height`.
    pub arena: [f32; 4],
}

impl Frame {
    /// A frame whose Arena fills the whole viewport — what the offscreen
    /// captures and the golden frame draw.
    pub fn full(width: f32, height: f32) -> Self {
        Self {
            viewport: [width, height],
            arena: [0.0, 0.0, width, height],
        }
    }
}

/// Written out by hand on purpose: `wgpu::vertex_attr_array!` packs offsets
/// densely, silently mis-assigning any struct with padding — and these structs
/// are 24 and 32 bytes with no padding to spare, so a wrong offset would feed
/// one attribute into another with no validation error.
const SHAPE_ATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 8,
        shader_location: 1,
    },
];

const TEXT_ATTRS: [wgpu::VertexAttribute; 3] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 8,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 16,
        shader_location: 2,
    },
];

/// Additive blending: light laid over the frame's matter. Straight-alpha
/// colours contribute `rgb × alpha`, so a light's intensity scales with its
/// coverage, and overlapping lights accumulate like light does.
const ADDITIVE: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    // Light accumulates onto the alpha channel too, so it can never punch a
    // transparent hole through the opaque frame beneath it.
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

/// The upsample pass adds a level into the one above it, so each level of the
/// chain keeps what the level below contributed.
const ACCUMULATE: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent::REPLACE,
};

/// One level of the bloom chain: the view it is drawn into and the bind group
/// that samples it.
struct Level {
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

/// Everything that has to be rebuilt when the window's size changes.
struct Targets {
    width: u32,
    height: u32,
    /// The multisampled scene target, resolved into `scene`.
    msaa: wgpu::TextureView,
    scene: wgpu::TextureView,
    /// Samples `scene` alone, for the bloom prefilter.
    scene_bind_group: wgpu::BindGroup,
    /// Samples `scene` and the bloom chain's top level, for the composite.
    composite_bind_group: wgpu::BindGroup,
    bloom: Vec<Level>,
}

pub struct Renderer {
    format: wgpu::TextureFormat,
    targets: Option<Targets>,
    frame: FrameUniform,
    frame_buffer: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    deep_field: wgpu::BindGroup,
    backdrop: wgpu::RenderPipeline,
    triangles: wgpu::RenderPipeline,
    luminous: wgpu::RenderPipeline,
    bloom_prefilter: wgpu::RenderPipeline,
    bloom_down: wgpu::RenderPipeline,
    bloom_up: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    glyphs: wgpu::RenderPipeline,
    triangle_buffer: VertexBuffer,
    luminous_buffer: VertexBuffer,
    glyph_buffer: VertexBuffer,
    text: TextRenderer,
    /// Quads drawn in the last frame, for diagnostics and the status bar.
    pub last_glyphs: usize,
}

impl Renderer {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self {
        let frame_buffer = crate::gpu::buffer_with_data(
            &gpu.device,
            "frame",
            cast_slice(&[FrameUniform::default()]),
        );
        let frame_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("frame-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let frame_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame-bind-group"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });

        let texture_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler_entry = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("texture-layout"),
                    entries: &[texture_entry(0), sampler_entry],
                });
        let composite_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("composite-layout"),
                    entries: &[texture_entry(0), sampler_entry, texture_entry(2)],
                });

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let deep_field = bake_deep_field(gpu, &texture_layout, &sampler);

        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("neuroarena-shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
        let scene_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("scene-pipeline-layout"),
                bind_group_layouts: &[Some(&frame_layout), Some(&texture_layout)],
                immediate_size: 0,
            });
        let composite_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("composite-pipeline-layout"),
                    bind_group_layouts: &[Some(&frame_layout), Some(&composite_layout)],
                    immediate_size: 0,
                });

        let shape_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TriangleVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &SHAPE_ATTRS,
        })];
        let text_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &TEXT_ATTRS,
        })];

        let pipeline = |label: &str,
                        layout: &wgpu::PipelineLayout,
                        buffers: &[Option<wgpu::VertexBufferLayout>],
                        vs: &str,
                        fs: &str,
                        target: wgpu::TextureFormat,
                        blend: Option<wgpu::BlendState>,
                        samples: u32| {
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some(vs),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers,
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        unclipped_depth: false,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: samples,
                        ..Default::default()
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(fs),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: target,
                            // Straight alpha: the fragment shaders emit
                            // unpremultiplied colour.
                            blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        };

        let backdrop = pipeline(
            "backdrop",
            &scene_layout,
            &[],
            "vs_fullscreen",
            "fs_backdrop",
            HDR_FORMAT,
            None,
            SAMPLES,
        );
        let triangles = pipeline(
            "triangles",
            &scene_layout,
            &shape_buffers,
            "vs_shape",
            "fs_shape",
            HDR_FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            SAMPLES,
        );
        let luminous = pipeline(
            "luminous",
            &scene_layout,
            &shape_buffers,
            "vs_shape",
            "fs_shape",
            HDR_FORMAT,
            Some(ADDITIVE),
            SAMPLES,
        );
        let bloom_prefilter = pipeline(
            "bloom-prefilter",
            &scene_layout,
            &[],
            "vs_fullscreen",
            "fs_bloom_prefilter",
            HDR_FORMAT,
            None,
            1,
        );
        let bloom_down = pipeline(
            "bloom-down",
            &scene_layout,
            &[],
            "vs_fullscreen",
            "fs_bloom_down",
            HDR_FORMAT,
            None,
            1,
        );
        let bloom_up = pipeline(
            "bloom-up",
            &scene_layout,
            &[],
            "vs_fullscreen",
            "fs_bloom_up",
            HDR_FORMAT,
            Some(ACCUMULATE),
            1,
        );
        let composite = pipeline(
            "composite",
            &composite_pipeline_layout,
            &[],
            "vs_fullscreen",
            "fs_composite",
            format,
            None,
            1,
        );
        let glyphs = pipeline(
            "glyphs",
            &scene_layout,
            &text_buffers,
            "vs_text",
            "fs_text",
            format,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            1,
        );

        let text = TextRenderer::new(gpu, &texture_layout);
        Self {
            format,
            targets: None,
            frame: FrameUniform::default(),
            frame_buffer,
            frame_bind_group,
            texture_layout,
            composite_layout,
            sampler,
            deep_field,
            backdrop,
            triangles,
            luminous,
            bloom_prefilter,
            bloom_down,
            bloom_up,
            composite,
            glyphs,
            triangle_buffer: VertexBuffer::default(),
            luminous_buffer: VertexBuffer::default(),
            glyph_buffer: VertexBuffer::default(),
            text,
            last_glyphs: 0,
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// The extent this frame's coordinates are measured in — the same pixels
    /// the Painter emitted — and where the Arena sits inside it.
    pub fn set_frame(&mut self, gpu: &Gpu, frame: Frame) {
        let (width, height) = (frame.viewport[0].max(1.0), frame.viewport[1].max(1.0));
        let (w, h) = (width as u32, height as u32);
        if self
            .targets
            .as_ref()
            .is_none_or(|targets| (targets.width, targets.height) != (w, h))
        {
            self.targets = Some(self.build_targets(gpu, w, h));
        }
        self.frame = FrameUniform {
            viewport: [width, height, 1.0 / width, 1.0 / height],
            arena: frame.arena,
            params: [0.0, BLOOM_INTENSITY, GRAIN, 0.0],
            app_bg: crate::theme::color::APP_BG.to_linear(),
            arena_bg: crate::theme::color::ARENA_BG.to_linear(),
            nebula_a: crate::theme::color::NEBULA_A.to_linear(),
            nebula_b: crate::theme::color::NEBULA_B.to_linear(),
            wash: crate::theme::color::WASH.to_linear(),
            vignette: crate::theme::color::VIGNETTE.to_linear(),
        };
        gpu.queue
            .write_buffer(&self.frame_buffer, 0, cast_slice(&[self.frame]));
    }

    /// The size-dependent half of the frame graph: the scene targets and the
    /// bloom chain, all at `HDR_FORMAT`.
    fn build_targets(&self, gpu: &Gpu, width: u32, height: u32) -> Targets {
        let color_target = |label: &str, w: u32, h: u32, samples: u32, mips: u32| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w.max(1),
                    height: h.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: mips,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };

        let msaa =
            color_target("scene-msaa", width, height, SAMPLES, 1).create_view(&Default::default());
        let scene_texture = color_target("scene", width, height, 1, 1);
        let scene = scene_texture.create_view(&Default::default());

        let sizes = bloom_sizes(width, height);
        let bloom_texture =
            color_target("bloom-chain", sizes[0].0, sizes[0].1, 1, sizes.len() as u32);
        let bloom = (0..sizes.len())
            .map(|level| {
                let view = bloom_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("bloom-level"),
                    base_mip_level: level as u32,
                    mip_level_count: Some(1),
                    ..Default::default()
                });
                let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("bloom-level-bind-group"),
                    layout: &self.texture_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                });
                Level { view, bind_group }
            })
            .collect::<Vec<_>>();

        let scene_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene-bind-group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let composite_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composite-bind-group"),
            layout: &self.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&bloom[0].view),
                },
            ],
        });

        Targets {
            width,
            height,
            msaa,
            scene,
            scene_bind_group,
            composite_bind_group,
            bloom,
        }
    }

    /// Draw one frame into `view`.
    pub fn render(&mut self, gpu: &Gpu, view: &wgpu::TextureView, painter: &Painter) {
        // The atlas must exist before the pass binds it, and the borrow of the
        // text renderer ends before the vertex buffer is filled from it.
        self.text.build(gpu, &painter.text);
        let glyph_count = self.text.vertices().len() as u32;
        self.last_glyphs = self.text.glyphs;

        self.triangle_buffer.upload(
            gpu,
            "triangle-vertices",
            cast_slice(&painter.triangles),
            std::mem::size_of::<TriangleVertex>(),
        );
        self.luminous_buffer.upload(
            gpu,
            "luminous-vertices",
            cast_slice(&painter.luminous),
            std::mem::size_of::<TriangleVertex>(),
        );
        self.glyph_buffer.upload(
            gpu,
            "glyph-vertices",
            cast_slice(self.text.vertices()),
            std::mem::size_of::<TextVertex>(),
        );

        let Some(targets) = self.targets.as_ref() else {
            crate::gpu::note_error("set the frame before rendering");
            return;
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-encoder"),
            });
        self.draw_scene(&mut encoder, targets);
        self.draw_bloom(&mut encoder, targets);
        self.draw_composite(&mut encoder, targets, view, glyph_count);
        gpu.queue.submit(Some(encoder.finish()));
    }

    /// The scene: the deep field, the frame's matter, then its light.
    fn draw_scene(&self, encoder: &mut wgpu::CommandEncoder, targets: &Targets) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.msaa,
                resolve_target: Some(&targets.scene),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &self.deep_field, &[]);
        pass.set_pipeline(&self.backdrop);
        pass.draw(0..3, 0..1);

        if self.triangle_buffer.vertices > 0 {
            pass.set_pipeline(&self.triangles);
            if let Some(slice) = self.triangle_buffer.slice() {
                pass.set_vertex_buffer(0, slice);
                pass.draw(0..self.triangle_buffer.vertices, 0..1);
            }
        }
        if self.luminous_buffer.vertices > 0 {
            pass.set_pipeline(&self.luminous);
            if let Some(slice) = self.luminous_buffer.slice() {
                pass.set_vertex_buffer(0, slice);
                pass.draw(0..self.luminous_buffer.vertices, 0..1);
            }
        }
    }

    /// The bloom chain: threshold into level 0, downsample to the bottom, then
    /// tent-upsample back up, each level adding into the one above it.
    fn draw_bloom(&self, encoder: &mut wgpu::CommandEncoder, targets: &Targets) {
        let mut level = |label: &str,
                         target: &wgpu::TextureView,
                         source: &wgpu::BindGroup,
                         pipeline: &wgpu::RenderPipeline,
                         load: wgpu::LoadOp<wgpu::Color>| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, source, &[]);
            pass.set_pipeline(pipeline);
            pass.draw(0..3, 0..1);
        };

        level(
            "bloom-prefilter",
            &targets.bloom[0].view,
            &targets.scene_bind_group,
            &self.bloom_prefilter,
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );
        for index in 1..targets.bloom.len() {
            level(
                "bloom-down",
                &targets.bloom[index].view,
                &targets.bloom[index - 1].bind_group,
                &self.bloom_down,
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            );
        }
        for index in (1..targets.bloom.len()).rev() {
            level(
                "bloom-up",
                &targets.bloom[index - 1].view,
                &targets.bloom[index].bind_group,
                &self.bloom_up,
                wgpu::LoadOp::Load,
            );
        }
    }

    /// The window: the scene plus its bloom, and the glyphs over the top.
    fn draw_composite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &Targets,
        view: &wgpu::TextureView,
        glyph_count: u32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &targets.composite_bind_group, &[]);
        pass.set_pipeline(&self.composite);
        pass.draw(0..3, 0..1);

        if glyph_count > 0 {
            pass.set_bind_group(1, self.text.atlas_bind_group(), &[]);
            pass.set_pipeline(&self.glyphs);
            if let Some(slice) = self.glyph_buffer.slice() {
                pass.set_vertex_buffer(0, slice);
                pass.draw(0..glyph_count, 0..1);
            }
        }
    }
}

/// The bloom chain's levels for a frame of this size: half resolution, then
/// halving, stopping at [`BLOOM_LEVELS`] or once a level would be smaller than
/// [`BLOOM_MIN_EDGE`]. A window too small for even one level still has to
/// render, so the chain keeps a single level the size of the frame and the
/// bloom simply has nothing to spread over.
fn bloom_sizes(width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut sizes = Vec::new();
    let (mut w, mut h) = (width / 2, height / 2);
    while sizes.len() < BLOOM_LEVELS as usize && w >= BLOOM_MIN_EDGE && h >= BLOOM_MIN_EDGE {
        sizes.push((w, h));
        w /= 2;
        h /= 2;
    }
    if sizes.is_empty() {
        sizes.push((width.max(1), height.max(1)));
    }
    sizes
}

/// Upload the baked deep field and bind it for the backdrop pass.
fn bake_deep_field(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let size = crate::deepfield::SIZE;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("deep-field"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // The channels are densities, not colours: no transfer function
        // belongs on them.
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        texture.as_image_copy(),
        &crate::deepfield::bake(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size * 4),
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&Default::default());
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("deep-field-bind-group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bloom_chain_halves_down_and_never_reaches_nothing() {
        // A normal window: six levels, each half the one above, the first at
        // half the frame.
        let levels = bloom_sizes(1440, 900);
        assert_eq!(levels.len(), BLOOM_LEVELS as usize);
        assert_eq!(levels[0], (720, 450));
        for pair in levels.windows(2) {
            assert_eq!(pair[1], (pair[0].0 / 2, pair[0].1 / 2));
        }
        assert!(levels
            .iter()
            .all(|(w, h)| *w >= BLOOM_MIN_EDGE && *h >= BLOOM_MIN_EDGE));

        // A short window stops early rather than making a level with no area.
        let short = bloom_sizes(1200, 40);
        assert_eq!(short, vec![(600, 20), (300, 10)]);

        // And a frame too small for any level still gets one to render into:
        // an empty chain would leave the composite sampling nothing.
        for (width, height) in [(1, 1), (8, 4), (0, 0)] {
            let tiny = bloom_sizes(width, height);
            assert_eq!(tiny.len(), 1);
            assert!(tiny[0].0 >= 1 && tiny[0].1 >= 1, "{tiny:?} has no area");
        }
    }

    #[test]
    fn a_frame_carries_the_arena_and_the_palette_the_backdrop_paints_from() {
        let frame = Frame {
            viewport: [1440.0, 900.0],
            arena: [16.0, 70.0, 1030.0, 644.0],
        };
        assert_eq!(Frame::full(960.0, 600.0).arena, [0.0, 0.0, 960.0, 600.0]);
        // The uniform's reciprocals are what the vertex shader projects with,
        // so a wrong pair would put every triangle in the wrong place.
        let viewport = [
            frame.viewport[0],
            frame.viewport[1],
            1.0 / frame.viewport[0],
            1.0 / frame.viewport[1],
        ];
        assert!((viewport[0] * viewport[2] - 1.0).abs() < 1e-6);
        assert!((viewport[1] * viewport[3] - 1.0).abs() < 1e-6);
        // The Arena sits inside the viewport it is measured against.
        assert!(frame.arena[0] + frame.arena[2] <= frame.viewport[0]);
        assert!(frame.arena[1] + frame.arena[3] <= frame.viewport[1]);
    }

    #[test]
    fn the_scene_is_assembled_where_light_can_exceed_white() {
        // The whole separation between "a bright shape" and "a light" rests on
        // the scene target holding values past 1.0, and on the threshold being
        // at exactly white.
        assert_eq!(HDR_FORMAT, wgpu::TextureFormat::Rgba16Float);
        assert!(SHADER.contains("const BLOOM_THRESHOLD: f32 = 1.0;"));
        const { assert!(BLOOM_INTENSITY > 0.0 && BLOOM_INTENSITY < 1.0) };
    }
}
