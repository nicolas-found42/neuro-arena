//! The high-dynamic-range frame graph, checked on the real GPU.
//!
//! The whole design rests on one property: the frame holds values past white,
//! the bloom threshold sits at white, and therefore a light halos while
//! anything the interface is authored in does not — with no mask, no second
//! scene pass and no per-draw flag. That property is worth a test of its own,
//! because nothing about a frame that merely *renders* would reveal its loss.

use neuroarena_app::gpu::{Gpu, Offscreen};
use neuroarena_app::painter::{Painter, Rgba};
use neuroarena_app::renderer::{Frame, Renderer};
use neuroarena_app::theme;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 256;
/// How far from the emitter the halo is sampled, in pixels. Far enough that no
/// geometry reaches it, near enough that a six-level chain does.
const HALO_AT: u32 = 22;

fn gpu() -> Gpu {
    Gpu::new(
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        None,
    )
    .expect("these tests need a GPU adapter; they cannot run without one")
}

/// Draw one 6-pixel square in the middle of the frame and read the pixel
/// `HALO_AT` to its right. `gain` of `None` draws it as matter instead of light.
fn halo_beside(
    gpu: &Gpu,
    renderer: &mut Renderer,
    target: &Offscreen,
    gain: Option<f32>,
) -> [u8; 4] {
    let mut painter = Painter::new();
    let (cx, cy) = (WIDTH as f32 * 0.5, HEIGHT as f32 * 0.5);
    let white = Rgba::rgb(1.0, 1.0, 1.0);
    match gain {
        Some(gain) => {
            painter.set_gain(gain);
            painter.luminous_triangle(
                [cx - 3.0, cy - 3.0],
                [cx + 3.0, cy - 3.0],
                [cx + 3.0, cy + 3.0],
                white,
            );
            painter.luminous_triangle(
                [cx - 3.0, cy - 3.0],
                [cx + 3.0, cy + 3.0],
                [cx - 3.0, cy + 3.0],
                white,
            );
            painter.set_gain(1.0);
        }
        None => painter.rect(cx - 3.0, cy - 3.0, 6.0, 6.0, white),
    }
    renderer.set_frame(gpu, Frame::full(WIDTH as f32, HEIGHT as f32));
    renderer.render(gpu, target.view(), &painter);
    let pixels = target.read_rgba(gpu);
    let x = (cx as u32) + HALO_AT;
    let y = cy as u32;
    let offset = ((y * WIDTH + x) * 4) as usize;
    [
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
        pixels[offset + 3],
    ]
}

#[test]
fn light_halos_and_the_interface_does_not() {
    let gpu = gpu();
    let target = Offscreen::new(&gpu, WIDTH, HEIGHT, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut renderer = Renderer::new(&gpu, target.format);

    // A light written past white spreads into the space around it.
    let lit = halo_beside(&gpu, &mut renderer, &target, Some(theme::light::BULLET));
    // The same square as matter — the brightest value the interface can print —
    // does not, because it never crosses the threshold.
    let printed = halo_beside(&gpu, &mut renderer, &target, None);

    assert!(
        lit[0] > printed[0] + 8,
        "a light did not halo: {lit:?} beside printed white {printed:?}"
    );
    // And white matter is genuinely quiet out there, not merely quieter: the
    // backdrop beside it is the deep field, within a couple of bits.
    assert!(
        printed.iter().take(3).all(|channel| *channel < 40),
        "printed white bled into the field: {printed:?}"
    );
    // Nothing punches a hole in the frame: every pixel stays opaque, which is
    // what every capture path depends on.
    assert_eq!(lit[3], 255);
    assert_eq!(printed[3], 255);
}

#[test]
fn the_frame_graph_survives_every_size_the_window_can_take() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, wgpu::TextureFormat::Rgba8UnormSrgb);
    // One renderer, resized through the shapes a window actually takes: a
    // minimum window, a default one, a Retina one, and back down. The targets
    // and the whole bloom chain are rebuilt each time, and a level that came
    // out with no area would fail validation rather than merely look wrong.
    for (width, height) in [
        (900u32, 560u32),
        (1440, 900),
        (2880, 1800),
        (1200, 64),
        (16, 16),
        (1440, 900),
    ] {
        let target = Offscreen::new(&gpu, width, height, wgpu::TextureFormat::Rgba8UnormSrgb);
        let mut painter = Painter::new();
        painter.rect(
            0.0,
            0.0,
            width as f32,
            height as f32 * 0.5,
            theme::color::PANEL_BG,
        );
        painter.set_gain(theme::light::IMPACT);
        painter.luminous_glow(
            [width as f32 * 0.5, height as f32 * 0.5],
            12.0,
            theme::color::ENERGY,
        );
        painter.set_gain(1.0);
        renderer.set_frame(
            &gpu,
            Frame {
                viewport: [width as f32, height as f32],
                arena: [0.0, 0.0, width as f32, height as f32 * 0.8],
            },
        );
        renderer.render(&gpu, target.view(), &painter);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device is pollable");
        assert!(
            neuroarena_app::gpu::take_error().is_none(),
            "the frame graph failed at {width}×{height}"
        );
        let pixels = target.read_rgba(&gpu);
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255),
            "the frame is not opaque at {width}×{height}"
        );
    }
}

#[test]
fn the_deep_field_is_painted_inside_the_arena_and_nowhere_else() {
    let gpu = gpu();
    let target = Offscreen::new(&gpu, WIDTH, HEIGHT, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut renderer = Renderer::new(&gpu, target.format);
    // Half the frame is Arena, half is housing. The backdrop pass owns both,
    // and the two have to come out as the two grounds the theme names.
    renderer.set_frame(
        &gpu,
        Frame {
            viewport: [WIDTH as f32, HEIGHT as f32],
            arena: [0.0, 0.0, WIDTH as f32 * 0.5, HEIGHT as f32],
        },
    );
    renderer.render(&gpu, target.view(), &Painter::new());
    let pixels = target.read_rgba(&gpu);
    let at = |x: u32, y: u32| {
        let offset = ((y * WIDTH + x) * 4) as usize;
        [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
    };

    let field = at(WIDTH / 4, HEIGHT / 2);
    let housing = at(WIDTH * 3 / 4, HEIGHT / 2);
    let expected = theme::color::APP_BG
        .to_array()
        .map(|channel| (channel * 255.0).round() as i32);
    for channel in 0..3 {
        assert!(
            (i32::from(housing[channel]) - expected[channel]).abs() <= 2,
            "outside the Arena is not the application's ground: {housing:?} vs {expected:?}"
        );
    }
    // Inside it, the field is cool and lit by the wash and the clouds, so it is
    // never simply the housing repeated.
    assert!(
        field != housing,
        "the Arena and the housing are painted the same: {field:?}"
    );
    assert!(
        field[2] >= field[1] && field[1] >= field[0],
        "the field runs cool: {field:?}"
    );
}
