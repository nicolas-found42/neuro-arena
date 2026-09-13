//! Offscreen frames: the only test that touches rendering.
//!
//! It exists because the visual layer is otherwise unverifiable without a human
//! looking at a window. A fixed World is drawn into an offscreen target at the
//! fixed logical Arena size and compared against a golden image — a coarse grid
//! of cell averages, which is stable across GPUs while still catching any
//! change in layout, colour or geometry.
//!
//! Regenerate the golden with `NEUROARENA_WRITE_GOLDEN=1 cargo test -p
//! neuroarena-app --test golden_frame`.

use std::path::PathBuf;

use neuroarena_app::gpu::{Gpu, Offscreen};
use neuroarena_app::painter::Painter;
use neuroarena_app::renderer::Renderer;
use neuroarena_app::scene::{self, ArenaView};
use neuroarena_app::theme;
use sim::config::asteroid::Size;
use sim::{Asteroid, Bullet, Rng, World};

const WIDTH: u32 = 960;
const HEIGHT: u32 = 600;
const COLS: u32 = 16;
const ROWS: u32 = 10;
const COLORS: u32 = COLS * ROWS;
/// Per-channel slack: two GPUs may rasterize an edge one bit differently.
const TOLERANCE: i32 = 8;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/arena-960x600.txt")
}

/// A World with nothing left to chance: fixed Ship, fixed asteroids, fixed bullets.
fn fixed_world() -> World {
    let mut world = World::new(Rng::from_seed(20260911), None);
    world.agent.ship.x = 300.0;
    world.agent.ship.y = 320.0;
    world.agent.ship.heading = 0.6;
    world.agent.ship.vx = 0.0;
    world.agent.ship.vy = 0.0;
    world.agent.thrusting = true;
    world.asteroids.clear();
    let asteroids: [(Size, f64, f64, f64, f64); 6] = [
        (Size::Large, 700.0, 160.0, 20.0, 30.0),
        (Size::Medium, 820.0, 420.0, 35.0, 55.0),
        (Size::Small, 180.0, 140.0, 60.0, 70.0),
        (Size::Large, 500.0, 500.0, 10.0, 25.0),
        (Size::Small, 40.0, 560.0, 90.0, 80.0),
        (Size::Medium, 940.0, 300.0, 45.0, 40.0),
    ];
    for (index, (size, x, y, dir_deg, speed)) in asteroids.iter().enumerate() {
        let mut rng = Rng::from_seed(1000 + index as u32);
        let mut asteroid = Asteroid::new(*size, *x, *y, dir_deg.to_radians(), *speed, &mut rng);
        asteroid.spin = 0.0;
        world.asteroids.push(asteroid);
    }
    world.bullets.push(Bullet {
        x: 420.0,
        y: 300.0,
        vx: 0.0,
        vy: 0.0,
        life: 1.0,
    });
    world.bullets.push(Bullet {
        x: 800.0,
        y: 120.0,
        vx: 0.0,
        vy: 0.0,
        life: 1.0,
    });
    world.time = sim::DT;
    world.agent.inputs = sim::sensors::sense(&world.agent, &world.asteroids);
    world
}

struct Frame {
    pixels: Vec<u8>,
}

impl Frame {
    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * WIDTH + x) * 4) as usize;
        [
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ]
    }

    /// The average colour of every cell of a `COLS` × `ROWS` grid.
    fn grid(&self) -> Vec<[u8; 3]> {
        let cell_w = WIDTH / COLS;
        let cell_h = HEIGHT / ROWS;
        let mut cells = Vec::with_capacity(COLORS as usize);
        for row in 0..ROWS {
            for col in 0..COLS {
                let mut sums = [0u64; 3];
                let mut count = 0u64;
                for y in row * cell_h..(row + 1) * cell_h {
                    for x in col * cell_w..(col + 1) * cell_w {
                        let pixel = self.pixel(x, y);
                        for channel in 0..3 {
                            sums[channel] += u64::from(pixel[channel]);
                        }
                        count += 1;
                    }
                }
                cells.push([
                    (sums[0] / count) as u8,
                    (sums[1] / count) as u8,
                    (sums[2] / count) as u8,
                ]);
            }
        }
        cells
    }

    fn differing_pixels(&self, other: &Frame) -> usize {
        self.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(other.pixels.as_chunks::<4>().0.iter())
            .filter(|(a, b)| a != b)
            .count()
    }
}

fn render(world: &World, show_rays: bool) -> Frame {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let gpu = Gpu::new(instance, None)
        .expect("rendering a frame needs a GPU adapter; this test cannot run without one");
    let target = Offscreen::new(&gpu, WIDTH, HEIGHT, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut renderer = Renderer::new(&gpu, target.format);
    renderer.set_frame(
        &gpu,
        neuroarena_app::renderer::Frame::full(WIDTH as f32, HEIGHT as f32),
    );

    let mut painter = Painter::new();
    scene::draw_arena(
        &mut painter,
        world,
        ArenaView {
            origin: [0.0, 0.0],
            scale: 1.0,
        },
        show_rays,
        show_rays,
    );
    renderer.render(&gpu, target.view(), &painter);
    Frame {
        pixels: target.read_rgba(&gpu),
    }
}

fn write_golden(cells: &[[u8; 3]]) {
    let mut text = String::from(
        "# NeuroArena arena golden — the fixed test World, 960×600, one line per\n\
         # cell of a 16×10 grid: average sRGB of the cell, RRGGBB, row-major.\n",
    );
    for cell in cells {
        text.push_str(&format!("{:02x}{:02x}{:02x}\n", cell[0], cell[1], cell[2]));
    }
    std::fs::create_dir_all(
        golden_path()
            .parent()
            .expect("the golden path has a parent"),
    )
    .expect("the golden directory is creatable");
    std::fs::write(golden_path(), text).expect("the golden file is writable");
}

fn read_golden() -> Vec<[u8; 3]> {
    let text = std::fs::read_to_string(golden_path())
        .expect("the golden image is missing; regenerate it with NEUROARENA_WRITE_GOLDEN=1");
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let bytes = u32::from_str_radix(line.trim(), 16).expect("a hex colour");
            [(bytes >> 16) as u8, (bytes >> 8) as u8, bytes as u8]
        })
        .collect()
}

#[test]
fn the_arena_frame_matches_its_golden_image() {
    let frame = render(&fixed_world(), false);
    if let Ok(path) = std::env::var("NEUROARENA_CAPTURE_GOLDEN") {
        let mut png = png::Encoder::new(std::fs::File::create(path).unwrap(), WIDTH, HEIGHT);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .unwrap()
            .write_image_data(&frame.pixels)
            .unwrap();
    }
    let cells = frame.grid();
    if std::env::var("NEUROARENA_WRITE_GOLDEN").is_ok() {
        write_golden(&cells);
        return;
    }
    let golden = read_golden();
    assert_eq!(
        golden.len(),
        COLORS as usize,
        "the golden image is complete"
    );
    let mut worst = 0i32;
    let mut worst_cell = 0usize;
    for (index, (cell, expected)) in cells.iter().zip(golden.iter()).enumerate() {
        for channel in 0..3 {
            let delta = i32::from(cell[channel]) - i32::from(expected[channel]);
            if delta.abs() > worst {
                worst = delta.abs();
                worst_cell = index;
            }
        }
    }
    assert!(
        worst <= TOLERANCE,
        "cell {worst_cell} (column {}, row {}) differs by {worst} from the golden image",
        worst_cell as u32 % COLS,
        worst_cell as u32 / COLS
    );
}

#[test]
fn the_same_world_renders_the_same_pixels_twice() {
    let first = render(&fixed_world(), true);
    let second = render(&fixed_world(), true);
    assert_eq!(
        first.differing_pixels(&second),
        0,
        "rendering is a pure function of the World"
    );
}

#[test]
fn the_ship_asteroids_and_bullets_land_where_the_simulation_puts_them() {
    let world = fixed_world();
    let frame = render(&world, false);

    // The Ship: near its centre, some pixel is dominated by the Ship colour.
    let ship = [world.agent.ship.x as u32, world.agent.ship.y as u32];
    let mut ship_pixels = 0;
    for y in ship[1].saturating_sub(12)..(ship[1] + 12).min(HEIGHT) {
        for x in ship[0].saturating_sub(12)..(ship[0] + 12).min(WIDTH) {
            let pixel = frame.pixel(x, y);
            if i32::from(pixel[2]) > i32::from(pixel[0]) + 30 && pixel[2] > 120 {
                ship_pixels += 1;
            }
        }
    }
    assert!(ship_pixels > 20, "the Ship is drawn: {ship_pixels} pixels");

    // An Asteroid: the Large asteroid at (700, 160) is drawn in the asteroid grey, which
    // is brighter than the empty floor everywhere around it.
    let floor = frame.pixel(475, 20);
    let asteroid = frame.pixel(700, 160);
    assert!(
        asteroid[0] > floor[0] + 20 && asteroid[1] > floor[1] + 20,
        "the asteroid reads brighter than the floor: {asteroid:?} vs {floor:?}"
    );

    // A Bullet: the one at (420, 300) is bright and warm.
    let bullet = frame.pixel(420, 300);
    assert!(
        bullet[0] > 200 && bullet[1] > 180,
        "the bullet is drawn: {bullet:?}"
    );

    // The floor is the deep field the backdrop shader paints, not an entity:
    // cool, dark, and at least as deep as the Arena's own ground, which the
    // wash and the nebulae are laid over rather than replacing.
    let background = theme::color::ARENA_BG;
    let ground = background.to_array().map(|channel| (channel * 255.0) as u8);
    assert!(
        floor[2] >= floor[1] && floor[1] >= floor[0],
        "the field runs cool: {floor:?}"
    );
    assert!(
        floor.iter().take(3).all(|channel| *channel < 90),
        "the field stays a dark ground: {floor:?}"
    );
    assert!(
        floor[2] >= ground[2],
        "the field is never deeper than the Arena's own ground: {floor:?} vs {ground:?}"
    );
    // And it is nowhere near the value of rock, which is what the Asteroid
    // check above leans on.
    assert!(
        i32::from(asteroid[0]) > i32::from(floor[0]) + 20,
        "rock and field are not told apart: {asteroid:?} vs {floor:?}"
    );
}

#[test]
fn sensor_rays_are_drawn_only_when_asked_for() {
    let world = fixed_world();
    let without = render(&world, false);
    let with = render(&world, true);
    let changed = without.differing_pixels(&with);
    assert!(
        changed > 500,
        "the Sensor Rays change the frame: {changed} pixels"
    );
}

#[test]
fn seam_copies_draw_entities_that_straddle_the_edge() {
    let mut world = fixed_world();
    world.asteroids.clear();
    world.asteroids.push({
        let mut rng = Rng::from_seed(7);
        let mut asteroid = Asteroid::new(Size::Large, 4.0, 300.0, 0.0, 30.0, &mut rng);
        asteroid.spin = 0.0;
        asteroid
    });
    let frame = render(&world, false);
    // The asteroid straddles the left edge, so it is also drawn on the right one.
    let left = frame.pixel(4, 300);
    let right = frame.pixel(WIDTH - 5, 300);
    assert!(
        left[0] > 60 && right[0] > 60,
        "both the asteroid and its Seam Copy are drawn: {left:?} {right:?}"
    );
}
