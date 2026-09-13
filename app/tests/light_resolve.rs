//! The composite's resolve, on the real GPU: the two decisions the last pass
//! makes, and what a frame would look like without them.
//!
//! The first is that a light resolves with its hue. The bloom term used to be
//! added raw, so light that summed past white clipped channel by channel at the
//! brightest point — toward white, where its colour means the most. The
//! composite now resolves the bloom term through a hue-preserving curve before
//! it joins the scene, and touches nothing else: what the interface printed
//! resolves to exactly what was written.
//!
//! The second is the grain. It is interleaved gradient noise now, not a
//! white-noise hash: still a pure function of the pixel, so two captures of one
//! frame are identical, but low-discrepancy, so a patch of the field keeps a
//! steadier average than white noise could.
//!
//! Both are checked offscreen, in linear light. The renderer can be handed any
//! target format, and half float is the one the frame graph itself assembles
//! in; eight bits would quantize the grain — three percent of the value under
//! it — into two levels, and the pattern would be gone.

use neuroarena_app::gpu::{Gpu, Offscreen};
use neuroarena_app::painter::{srgb_to_linear, Painter, Rgba};
use neuroarena_app::renderer::{Frame, Renderer};
use neuroarena_app::theme;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 256;

/// The gain the Arena writes its impacts with: five times the brightest value
/// the interface can print.
const GAIN: f32 = theme::light::IMPACT;

/// How far the interface's white is sampled from, in pixels. Far enough that no
/// geometry reaches it, near enough that a six-level bloom chain does.
const BESIDE: u32 = 22;

fn gpu() -> Gpu {
    Gpu::new(
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        None,
    )
    .expect("these tests need a GPU adapter; they cannot run without one")
}

/// A half-float target and its readback. `Offscreen` reads back eight-bit RGBA,
/// which is the right seam for a golden frame and the wrong one here: the grain
/// moves a pixel by a couple of levels and nothing else in the frame does.
struct Linear {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl Linear {
    fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("linear-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            width,
            height,
        }
    }

    fn format(&self) -> wgpu::TextureFormat {
        wgpu::TextureFormat::Rgba16Float
    }

    fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The frame as linear RGBA, one `[f32; 4]` per pixel.
    fn read(&self, gpu: &Gpu) -> Vec<[f32; 4]> {
        let unpadded = self.width * 8;
        let padded = unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("linear-readback"),
            size: u64::from(padded) * u64::from(self.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("linear-readback-encoder"),
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
            assert!(result.is_ok(), "the readback buffer could not be mapped");
        });
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device is pollable");

        let mut frame = Vec::with_capacity((self.width * self.height) as usize);
        let mapped = slice
            .get_mapped_range()
            .expect("the readback buffer is mapped");
        for row in mapped.chunks(padded as usize).take(self.height as usize) {
            for x in 0..self.width as usize {
                let mut pixel = [0.0; 4];
                for (channel, value) in pixel.iter_mut().enumerate() {
                    let offset = (x * 4 + channel) * 2;
                    let bits = u16::from_le_bytes([row[offset], row[offset + 1]]);
                    *value = half(bits);
                }
                frame.push(pixel);
            }
        }
        drop(mapped);
        buffer.unmap();
        frame
    }
}

/// Half float to float. Everything read back here is a positive normal.
fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = f32::from(bits & 0x03ff);
    match exponent {
        0 => sign * mantissa * 2.0f32.powi(-24),
        0x1f => sign * f32::INFINITY,
        _ => sign * (1.0 + mantissa / 1024.0) * 2.0f32.powi(i32::from(exponent) - 15),
    }
}

/// Four overlapping flashes at the theme's impact gain, arranged around the
/// middle of the frame. A light's bloom is a blur of everything the scene wrote
/// past white, so light that overlaps sums: together they write a halo several
/// times white, which is the sum the composite has to resolve.
fn flashes(painter: &mut Painter, gain: f32) {
    let (cx, cy) = (WIDTH as f32 * 0.5, HEIGHT as f32 * 0.5);
    let half = 20.0;
    painter.set_gain(gain);
    for (dx, dy) in [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
        let (x, y) = (cx + dx * 26.0, cy + dy * 26.0);
        painter.luminous_triangle(
            [x - half, y - half],
            [x + half, y - half],
            [x + half, y + half],
            theme::color::BULLET,
        );
        painter.luminous_triangle(
            [x - half, y - half],
            [x + half, y + half],
            [x - half, y + half],
            theme::color::BULLET,
        );
    }
    painter.set_gain(1.0);
}

/// Hue in degrees: which primary the colour leans on, and how far the other two
/// lean with it. The bloom chain scales every channel of a lit pixel by the
/// same numbers — the threshold, the Karis weight, the tent — so hue is the one
/// property a resolve of that term must not disturb.
fn hue(colour: [f32; 3]) -> f32 {
    let high = colour[0].max(colour[1]).max(colour[2]);
    let low = colour[0].min(colour[1]).min(colour[2]);
    let chroma = high - low;
    if chroma <= f32::EPSILON {
        return 0.0;
    }
    let sector = if high == colour[0] {
        ((colour[1] - colour[2]) / chroma).rem_euclid(6.0)
    } else if high == colour[1] {
        (colour[2] - colour[0]) / chroma + 2.0
    } else {
        (colour[0] - colour[1]) / chroma + 4.0
    };
    sector * 60.0
}

/// The shorter way around the colour wheel from `a` to `b`, in degrees.
fn hue_gap(a: f32, b: f32) -> f32 {
    ((a - b + 180.0).rem_euclid(360.0) - 180.0).abs()
}

fn linear(colour: Rgba) -> [f32; 3] {
    let [r, g, b, _] = colour.to_linear();
    [r, g, b]
}

/// The frame read back as linear RGBA, at `(x, y)`.
fn at(frame: &[[f32; 4]], x: u32, y: u32) -> [f32; 3] {
    let pixel = frame[(y * WIDTH + x) as usize];
    [pixel[0], pixel[1], pixel[2]]
}

/// One eight-bit pixel of a display-encoded target, decoded back to linear.
fn shown(bytes: &[u8], x: u32, y: u32) -> [f32; 3] {
    let offset = ((y * WIDTH + x) * 4) as usize;
    std::array::from_fn(|channel| srgb_to_linear(f32::from(bytes[offset + channel]) / 255.0))
}

#[test]
fn a_bright_light_resolves_with_its_hue() {
    let gpu = gpu();
    let target = Linear::new(&gpu, WIDTH, HEIGHT);
    let mut renderer = Renderer::new(&gpu, target.format());
    let display = Offscreen::new(&gpu, WIDTH, HEIGHT, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut display_renderer = Renderer::new(&gpu, display.format);
    let frame = Frame::full(WIDTH as f32, HEIGHT as f32);

    // The same frame twice: once in the frame graph's own half float, where the
    // bloom term can be read without a display encoding in the way, and once
    // the way the window shows it.
    let mut painter = Painter::new();
    flashes(&mut painter, GAIN);
    renderer.set_frame(&gpu, frame);
    renderer.render(&gpu, target.view(), &painter);
    display_renderer.set_frame(&gpu, frame);
    display_renderer.render(&gpu, display.view(), &painter);
    let pixels = target.read(&gpu);
    let bytes = display.read_rgba(&gpu);

    let (cx, cy) = (WIDTH / 2, HEIGHT / 2);
    let emitter = linear(theme::color::BULLET);
    let colour = at(&pixels, cx, cy);

    // The halo is bright enough to be the thing under test, not the field...
    let peak = colour[0].max(colour[1]).max(colour[2]);
    assert!(
        peak > 0.5,
        "the flashes did not halo: {colour:?} at the centre"
    );
    // ...and it is *resolved*: the four halos sum to twice white, and there is
    // no clip in the frame graph, so a raw sum would read past the saturated
    // value the scene writes at. Resolving it holds the sum below white.
    assert!(
        peak < 0.99,
        "the halo was not resolved below white: {colour:?}"
    );

    // What the window shows keeps the emitter's hue: the red channel still
    // leads, and leads by the ratios the light was drawn in. Clipping the sum
    // instead would take the green channel to the ceiling with the red one and
    // wash the light toward white.
    let lit = shown(&bytes, cx, cy);
    assert!(
        hue_gap(hue(lit), hue(emitter)) < 6.0,
        "the halo lost its hue: {:?} is {:.1}° from {:.1}°",
        lit,
        hue(lit),
        hue(emitter)
    );
    let byte = |channel: usize| i32::from(bytes[(((cy * WIDTH) + cx) * 4) as usize + channel]);
    assert!(
        byte(0) - byte(1) >= 8 && byte(1) - byte(2) >= 8,
        "the halo washed out: red {} green {} blue {}",
        byte(0),
        byte(1),
        byte(2)
    );
}

/// The grain's pattern, on the CPU: interleaved gradient noise, and the
/// white-noise hash it replaced, for the test to measure the frame against.
fn interleaved_gradient(x: f32, y: f32) -> f32 {
    fract(52.982918 * fract(x * 0.06711056 + y * 0.00583715))
}

fn white_noise(x: f32, y: f32) -> f32 {
    let qx = fract(x * 0.1031);
    let qy = fract(y * 0.1030);
    let d = qx * (qy + 33.33) + qy * (qx + 33.33);
    fract((qx + d + qy + d) * (qx + d))
}

fn fract(value: f32) -> f32 {
    value - value.floor()
}

fn mean(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len() as f32
}

fn deviation(values: &[f32]) -> f32 {
    let mean = mean(values);
    (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / values.len() as f32).sqrt()
}

/// How closely two patterns run together, `-1..=1`.
fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let (ma, mb) = (mean(a), mean(b));
    let covariance: f32 = a
        .iter()
        .zip(b)
        .map(|(a, b)| (a - ma) * (b - mb))
        .sum::<f32>()
        / a.len() as f32;
    covariance / (deviation(a) * deviation(b))
}

#[test]
fn the_grain_is_interleaved_gradient_noise() {
    let gpu = gpu();
    let target = Linear::new(&gpu, WIDTH, HEIGHT);
    let mut renderer = Renderer::new(&gpu, target.format());
    let frame = Frame::full(WIDTH as f32, HEIGHT as f32);

    // A flat field with nothing lit in it: one opaque panel grey over the whole
    // frame, so the scene reaches the composite as a constant, the bloom term
    // is exactly zero, and everything the frame carries away from the grey is
    // the grain.
    let grey = Rgba::rgb(0.6, 0.6, 0.6);
    let mut painter = Painter::new();
    painter.rect(0.0, 0.0, WIDTH as f32, HEIGHT as f32, grey);
    renderer.set_frame(&gpu, frame);
    renderer.render(&gpu, target.view(), &painter);
    let first = target.read(&gpu);
    let second = {
        renderer.render(&gpu, target.view(), &painter);
        target.read(&gpu)
    };
    // Nothing in the composite reads the clock or the frame's number, so the
    // same frame twice is the same pixels — bit for bit, not merely close.
    assert_eq!(first, second, "two renders of one frame differ");

    // The grain is a fraction of the value under it, so the deviation from the
    // grey recovers the noise itself, up to one constant the grain is scaled
    // by. Every measurement below is a shape, not an amplitude.
    let level = srgb_to_linear(grey.r);
    let mut measured = Vec::new();
    let mut expected = Vec::new();
    let mut replaced = Vec::new();
    for y in 8..HEIGHT - 8 {
        for x in 8..WIDTH - 8 {
            let pixel = at(&first, x, y);
            measured.push(pixel[0] / level - 1.0);
            expected.push((interleaved_gradient(x as f32, y as f32) - 0.5) * 0.06);
            replaced.push((white_noise(x as f32, y as f32) - 0.5) * 0.06);
        }
    }

    // The frame carries the pattern the shader's noise makes, pixel for pixel.
    assert!(
        correlation(&measured, &expected) > 0.99,
        "the grain is not interleaved gradient noise: {:.3}",
        correlation(&measured, &expected)
    );
    assert!(
        correlation(&measured, &replaced) < 0.3,
        "the grain is still the white-noise hash: {:.3}",
        correlation(&measured, &replaced)
    );

    // And its weight is the one the renderer documents: three percent either
    // way, which is a standard deviation of 0.06 / sqrt(12) ≈ 0.017 against the
    // value under it. This is the one place the grain's *amount* is pinned; if
    // it moves in the renderer, this is the test that should notice.
    let amount = deviation(&measured);
    assert!(
        (0.014..=0.021).contains(&amount),
        "the grain is {amount:.4}, not the documented ±3%"
    );

    // A patch of the field holds a steadier average than white noise could:
    // blocking the same pattern into 8×8 cells and weighing the cell averages
    // against the pixels, the interleaved gradient's cells are three times
    // tighter than the hash's. (The scale is the pattern's own, so the number
    // is a shape — what the eye reads as grain rather than as static.)
    let spread = |pattern: &[f32]| block_spread(pattern) / deviation(pattern);
    assert!(
        spread(&measured) < 0.5 * spread(&replaced),
        "the grain is not low-discrepancy: {:.4} against white noise's {:.4}",
        spread(&measured),
        spread(&replaced)
    );
    assert!(
        (spread(&measured) - spread(&expected)).abs()
            < (spread(&measured) - spread(&replaced)).abs(),
        "the grain's patchiness is neither pattern's"
    );

    // The grain stays inside the Arena: the same flat field with the Arena
    // covering half the frame. Outside it the housing is painted flat and the
    // grain is masked off, so those pixels agree exactly — a mask that leaked
    // would leave the housing as noisy as the field.
    let mut painter = Painter::new();
    painter.rect(0.0, 0.0, WIDTH as f32, HEIGHT as f32, grey);
    renderer.set_frame(
        &gpu,
        Frame {
            viewport: [WIDTH as f32, HEIGHT as f32],
            arena: [0.0, 0.0, WIDTH as f32 * 0.5, HEIGHT as f32],
        },
    );
    renderer.render(&gpu, target.view(), &painter);
    let split = target.read(&gpu);
    let housing: Vec<f32> = (2..(HEIGHT - 2))
        .map(|y| at(&split, WIDTH - 3, y)[1])
        .collect();
    assert!(
        deviation(&housing) == 0.0,
        "the grain leaked outside the Arena"
    );
    let field: Vec<f32> = (2..(HEIGHT - 2)).map(|y| at(&split, 3, y)[1]).collect();
    assert!(deviation(&field) > 0.0, "the Arena has no grain in it");
}

/// The height of an 8×8 cell's average, across a region's cells: the number
/// that separates a low-discrepancy pattern from white noise.
fn block_spread(values: &[f32]) -> f32 {
    let cells = (WIDTH - 16) / 8;
    let rows = (HEIGHT - 16) / 8;
    let mut means = Vec::new();
    for cell_y in 0..rows {
        for cell_x in 0..cells {
            let mut sum = 0.0;
            for y in 0..8 {
                for x in 0..8 {
                    let index = ((cell_y * 8 + y) * (WIDTH - 16) + cell_x * 8 + x) as usize;
                    sum += values[index];
                }
            }
            means.push(sum / 64.0);
        }
    }
    deviation(&means)
}

#[test]
fn printed_white_stays_quiet_a_short_distance_away() {
    let gpu = gpu();
    let target = Offscreen::new(&gpu, WIDTH, HEIGHT, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut renderer = Renderer::new(&gpu, target.format);
    let (cx, cy) = (WIDTH / 2, HEIGHT / 2);

    // The brightest thing the interface can print: white matter, no gain, drawn
    // where the flashes are drawn. The resolve touches the bloom term only, so
    // what the interface printed is untouched — and white writes nothing over
    // the threshold, so there is no bloom term to resolve.
    let mut painter = Painter::new();
    painter.rect(
        cx as f32 - 3.0,
        cy as f32 - 3.0,
        6.0,
        6.0,
        Rgba::rgb(1.0, 1.0, 1.0),
    );
    renderer.set_frame(&gpu, Frame::full(WIDTH as f32, HEIGHT as f32));
    renderer.render(&gpu, target.view(), &painter);
    let printed = target.read_rgba(&gpu);

    let pixel = |bytes: &[u8], x: u32| {
        let offset = (((cy * WIDTH) + x) * 4) as usize;
        [
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]
    };
    let beside = pixel(&printed, cx + BESIDE);
    assert!(
        beside.iter().take(3).all(|channel| *channel < 40),
        "printed white bled into the field: {beside:?}"
    );

    // And the sample point is where a light *would* show: the same square as a
    // light at the theme's gain does reach it. Without this the test above
    // could pass by measuring nothing.
    let mut painter = Painter::new();
    painter.set_gain(GAIN);
    painter.luminous_triangle(
        [cx as f32 - 3.0, cy as f32 - 3.0],
        [cx as f32 + 3.0, cy as f32 - 3.0],
        [cx as f32 + 3.0, cy as f32 + 3.0],
        Rgba::rgb(1.0, 1.0, 1.0),
    );
    painter.luminous_triangle(
        [cx as f32 - 3.0, cy as f32 - 3.0],
        [cx as f32 + 3.0, cy as f32 + 3.0],
        [cx as f32 - 3.0, cy as f32 + 3.0],
        Rgba::rgb(1.0, 1.0, 1.0),
    );
    painter.set_gain(1.0);
    renderer.render(&gpu, target.view(), &painter);
    let lit = pixel(&target.read_rgba(&gpu), cx + BESIDE);
    assert!(
        i32::from(lit[0]) > i32::from(beside[0]) + 8,
        "a light at the same size and distance did not halo: {lit:?} beside {beside:?}"
    );
    assert_eq!(lit[3], 255, "the frame is not opaque: {lit:?}");
}
