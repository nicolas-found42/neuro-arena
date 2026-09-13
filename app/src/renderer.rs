//! The renderer: the frame graph the app owns, on top of `wgpu`.
//!
//! Three pipelines cover everything the app draws — filled triangles for the
//! Arena and the panels, a line list for outlines and connections, and a
//! textured pipeline for glyph quads. One uniform carries the viewport the
//! logical coordinates are measured against.
//!
//! There is no framework here on purpose (ADR 0004): this is the whole of the
//! drawing path, and it is small enough to read in one sitting.

use bytemuck::cast_slice;

use crate::gpu::{Gpu, VertexBuffer};
use crate::painter::{LineVertex, Painter, TriangleVertex};
use crate::text::{TextRenderer, TextVertex};

pub const SHADER: &str = r#"
struct Viewport {
    size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct ShapeIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct ShapeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

fn project(pos: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        pos.x / viewport.size.x * 2.0 - 1.0,
        1.0 - pos.y / viewport.size.y * 2.0,
    );
}

@vertex
fn vs_shape(in: ShapeIn) -> ShapeOut {
    var out: ShapeOut;
    out.clip = vec4<f32>(project(in.pos), 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_shape(in: ShapeOut) -> @location(0) vec4<f32> {
    return in.color;
}

struct TextIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct TextOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_text(in: TextIn) -> TextOut {
    var out: TextOut;
    out.clip = vec4<f32>(project(in.pos), 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_text(in: TextOut) -> @location(0) vec4<f32> {
    let mask = textureSample(atlas, atlas_sampler, in.uv);
    return vec4<f32>(in.color.rgb, in.color.a * mask.a);
}
"#;

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

pub struct Renderer {
    format: wgpu::TextureFormat,
    viewport_buffer: wgpu::Buffer,
    viewport_bind_group: wgpu::BindGroup,
    triangles: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    glyphs: wgpu::RenderPipeline,
    triangle_buffer: VertexBuffer,
    line_buffer: VertexBuffer,
    glyph_buffer: VertexBuffer,
    text: TextRenderer,
    /// Quads drawn in the last frame, for diagnostics and the status bar.
    pub last_glyphs: usize,
}

impl Renderer {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self {
        let viewport_buffer = crate::gpu::buffer_with_data(
            &gpu.device,
            "viewport",
            cast_slice(&[1.0f32, 1.0, 0.0, 0.0]),
        );
        let viewport_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("viewport-layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });
        let viewport_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport-bind-group"),
            layout: &viewport_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });
        let glyph_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("glyph-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("neuroarena-shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline-layout"),
                bind_group_layouts: &[Some(&viewport_layout), Some(&glyph_layout)],
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
                        topology: wgpu::PrimitiveTopology,
                        buffers: &[Option<wgpu::VertexBufferLayout>],
                        vs: &str,
                        fs: &str,
                        blend: bool| {
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some(vs),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers,
                    },
                    primitive: wgpu::PrimitiveState {
                        topology,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        unclipped_depth: false,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(fs),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            // Straight alpha: the fragment shaders emit
                            // unpremultiplied colour.
                            blend: if blend {
                                Some(wgpu::BlendState::ALPHA_BLENDING)
                            } else {
                                None
                            },
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        };

        let triangles = pipeline(
            "triangles",
            wgpu::PrimitiveTopology::TriangleList,
            &shape_buffers,
            "vs_shape",
            "fs_shape",
            false,
        );
        let lines = pipeline(
            "lines",
            wgpu::PrimitiveTopology::LineList,
            &shape_buffers,
            "vs_shape",
            "fs_shape",
            true,
        );
        let glyphs = pipeline(
            "glyphs",
            wgpu::PrimitiveTopology::TriangleList,
            &text_buffers,
            "vs_text",
            "fs_text",
            true,
        );

        let text = TextRenderer::new(gpu, &glyph_layout);
        Self {
            format,
            viewport_buffer,
            viewport_bind_group,
            triangles,
            lines,
            glyphs,
            triangle_buffer: VertexBuffer::default(),
            line_buffer: VertexBuffer::default(),
            glyph_buffer: VertexBuffer::default(),
            text,
            last_glyphs: 0,
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// The extent the coordinates of this frame are measured in, in the same
    /// pixels the Painter emitted (window physical pixels).
    pub fn set_viewport(&self, gpu: &Gpu, width: f32, height: f32) {
        let uniform: [f32; 4] = [width, height, 0.0, 0.0];
        gpu.queue
            .write_buffer(&self.viewport_buffer, 0, cast_slice(&uniform));
    }

    /// Draw one frame into `view`.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        view: &wgpu::TextureView,
        painter: &Painter,
        clear: wgpu::Color,
    ) {
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
        self.line_buffer.upload(
            gpu,
            "line-vertices",
            cast_slice(&painter.lines),
            std::mem::size_of::<LineVertex>(),
        );
        self.glyph_buffer.upload(
            gpu,
            "glyph-vertices",
            cast_slice(self.text.vertices()),
            std::mem::size_of::<TextVertex>(),
        );

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.viewport_bind_group, &[]);
            // Every group in the pipeline layout must be bound before any draw,
            // even the ones a given pipeline does not sample.
            pass.set_bind_group(1, self.text.atlas_bind_group(), &[]);

            if self.triangle_buffer.vertices > 0 {
                pass.set_pipeline(&self.triangles);
                if let Some(slice) = self.triangle_buffer.slice() {
                    pass.set_vertex_buffer(0, slice);
                    pass.draw(0..self.triangle_buffer.vertices, 0..1);
                }
            }
            if self.line_buffer.vertices > 0 {
                pass.set_pipeline(&self.lines);
                if let Some(slice) = self.line_buffer.slice() {
                    pass.set_vertex_buffer(0, slice);
                    pass.draw(0..self.line_buffer.vertices, 0..1);
                }
            }
            if glyph_count > 0 {
                pass.set_pipeline(&self.glyphs);
                if let Some(slice) = self.glyph_buffer.slice() {
                    pass.set_vertex_buffer(0, slice);
                    pass.draw(0..glyph_count, 0..1);
                }
            }
        }
        gpu.queue.submit(Some(encoder.finish()));
    }
}
