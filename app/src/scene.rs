//! The Arena scene: a [`World`] turned into filled polygons and lines.
//!
//! Everything here draws in the fixed logical 960×600 Arena space and lets the
//! Painter's transform carry it into window pixels (ADR 0006), so the field
//! keeps its shape at any window size and the geometry can be authored in the
//! units the simulation itself uses.
//!
//! Two rules shape the look. Plain filled polygons, no bevels or hachure — the
//! retired browser rendition is not the target (ADR 0004). And every mark is
//! authored with real thickness, because a hairline that survives at 1:1
//! disappears when the window is small.
//!
//! The field is toroidal, so an entity near an edge is drawn again across the
//! seam: one copy for each edge it laps, up to four in a corner. That is the
//! Seam Copy, and it is what stops the ship vanishing halfway off the right
//! edge before it appears on the left.

use std::cell::RefCell;
use std::sync::LazyLock;

use sim::config::asteroid::Size;
use sim::config::{
    arena, asteroid as asteroid_cfg, bullet as bullet_cfg, sensors, ship as ship_cfg,
};
use sim::world::{Bullet, Ship, World};
use sim::Asteroid;

use crate::painter::{Painter, Rgba, Transform};
use crate::theme;

/// The logical Arena, in the units every coordinate below is expressed in.
const W: f32 = arena::WIDTH as f32;
const H: f32 = arena::HEIGHT as f32;

/// Where the fixed 960×600 logical Arena sits in the window, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArenaView {
    pub origin: [f32; 2],
    pub scale: f32,
}

impl ArenaView {
    /// The transform the Painter needs to map logical Arena units into the window.
    pub fn transform(&self) -> Transform {
        Transform::new(self.scale, self.origin)
    }

    /// The Arena's outer rectangle in window pixels: `x, y, width, height`.
    pub fn rect(&self) -> (f32, f32, f32, f32) {
        (
            self.origin[0],
            self.origin[1],
            W * self.scale,
            H * self.scale,
        )
    }
}

// ---------------------------------------------------------------- starfield

/// One star, in logical Arena units, with its final colour resolved once.
#[derive(Clone, Copy, Debug)]
struct Star {
    x: f32,
    y: f32,
    size: f32,
    color: Rgba,
}

/// How many stars the field holds. Fixed, not density-scaled: the field is part
/// of the golden frame and must not depend on anything but this constant.
const STAR_COUNT: usize = 150;
/// The seed for the field's own generator. Changing it re-lays the sky.
const STAR_SEED: u32 = 0x5EED_2024;
/// Dim end of the brightness spread; the bright end is this plus the range.
const STAR_MIN_ALPHA: f32 = 0.12;
const STAR_ALPHA_RANGE: f32 = 0.48;

/// The field, laid out once per process and shared by every frame.
static STARS: LazyLock<Vec<Star>> = LazyLock::new(build_stars);

/// Lay out the field from [`STAR_SEED`].
///
/// The generator is written out here on purpose. `sim`'s `Rng` belongs to the
/// simulation stream — taking one number from it would move the Episode — and a
/// golden frame has to come out identical on every machine, so nothing on this
/// path may call a library RNG or a platform `sin`/`cos`. Integers and f32
/// multiply/divide are exactly reproducible; those are all that is used.
fn build_stars() -> Vec<Star> {
    let mut seed = STAR_SEED;
    let mut field = Vec::with_capacity(STAR_COUNT);
    for _ in 0..STAR_COUNT {
        let x = lcg_unit(&mut seed) * W;
        let y = lcg_unit(&mut seed) * H;
        // Three size classes and an independent brightness spread: a uniform
        // grid of equal dots reads as dirt on the glass rather than as sky.
        let class = lcg_unit(&mut seed);
        let size = if class < 0.62 {
            1.0
        } else if class < 0.90 {
            1.5
        } else {
            2.0
        };
        let alpha = STAR_MIN_ALPHA + lcg_unit(&mut seed) * STAR_ALPHA_RANGE;
        field.push(Star {
            x,
            y,
            size,
            color: theme::color::STAR.alpha(alpha),
        });
    }
    field
}

/// The next value in `0..1` from a Numerical-Recipes LCG, high bits first.
fn lcg_unit(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / 16_777_216.0
}

// ------------------------------------------------------------------- seam

/// Draw an entity once, plus every copy the toroidal field calls for: one more
/// across each edge the entity laps, so up to four in a corner. `draw` is given
/// the centre of each copy and draws the entity around it.
fn seam_copies(
    painter: &mut Painter,
    x: f64,
    y: f64,
    r: f64,
    draw: impl Fn(&mut Painter, f32, f32),
) {
    let mut x_offsets = [0.0f64; 2];
    let mut x_count = 1;
    if x < r {
        x_offsets[1] = arena::WIDTH;
        x_count = 2;
    } else if x > arena::WIDTH - r {
        x_offsets[1] = -arena::WIDTH;
        x_count = 2;
    }

    let mut y_offsets = [0.0f64; 2];
    let mut y_count = 1;
    if y < r {
        y_offsets[1] = arena::HEIGHT;
        y_count = 2;
    } else if y > arena::HEIGHT - r {
        y_offsets[1] = -arena::HEIGHT;
        y_count = 2;
    }

    for y_offset in &y_offsets[..y_count] {
        for x_offset in &x_offsets[..x_count] {
            draw(painter, (x + x_offset) as f32, (y + y_offset) as f32);
        }
    }
}

// ------------------------------------------------------------------- frame

pub fn draw_arena(painter: &mut Painter, world: &World, view: ArenaView, show_rays: bool) {
    painter.set_transform(view.transform());

    painter.rect(0.0, 0.0, W, H, theme::color::ARENA_BG);
    draw_stars(painter);
    draw_field_edge(painter);

    let agent = &world.agent;
    // The rays go down first: they are translucent, and reading them through
    // the asteroids is the point of the overlay.
    if show_rays {
        draw_rays(painter, &agent.ship, &agent.inputs);
    }
    draw_asteroids(painter, &world.asteroids);
    draw_bullets(painter, &world.bullets);
    draw_ship(painter, &agent.ship, agent.alive, agent.thrusting);
}

fn draw_stars(painter: &mut Painter) {
    for star in STARS.iter() {
        painter.rect(star.x, star.y, star.size, star.size, star.color);
    }
}

/// How far inside the seam the field's edge band sits, and how thick it is.
/// Both are logical units that scale with the window, so the band stays a band.
const EDGE_INSET: f32 = 2.0;
const EDGE_WIDTH: f32 = 2.0;

/// The Arena's extent, drawn as a band of real thickness just inside the seam.
fn draw_field_edge(painter: &mut Painter) {
    let color = theme::color::ASTEROID_EDGE.alpha(0.20);
    let (left, top) = (EDGE_INSET, EDGE_INSET);
    let (right, bottom) = (W - EDGE_INSET, H - EDGE_INSET);
    painter.rect(left, top, right - left, EDGE_WIDTH, color);
    painter.rect(left, bottom - EDGE_WIDTH, right - left, EDGE_WIDTH, color);
    painter.rect(left, top, EDGE_WIDTH, bottom - top, color);
    painter.rect(right - EDGE_WIDTH, top, EDGE_WIDTH, bottom - top, color);
}

// --------------------------------------------------------------- asteroids

thread_local! {
    /// One vertex buffer, reused by every asteroid in every frame:
    /// `Asteroid::vertices` insists on a `Vec`, and the field must not pay for
    /// a fresh one sixty times a second.
    static ASTEROID_VERTICES: RefCell<Vec<(f64, f64)>> = const { RefCell::new(Vec::new()) };
}

fn draw_asteroids(painter: &mut Painter, asteroids: &[Asteroid]) {
    ASTEROID_VERTICES.with(|cell| {
        let mut scratch = cell.borrow_mut();
        for asteroid in asteroids {
            asteroid.vertices(&mut scratch);
            let count = scratch.len().min(asteroid_cfg::VERTICES);

            // Vertices relative to the asteroid's centre, so a Seam Copy is pure
            // translation and nothing has to be recomputed per copy.
            let mut relative = [[0.0f32; 2]; asteroid_cfg::VERTICES];
            for (slot, point) in relative.iter_mut().zip(scratch.iter()).take(count) {
                *slot = [(point.0 - asteroid.x) as f32, (point.1 - asteroid.y) as f32];
            }

            let fill = asteroid_fill(asteroid.size);
            let edge = theme::color::ASTEROID_EDGE;
            seam_copies(
                painter,
                asteroid.x,
                asteroid.y,
                asteroid.r,
                |painter, cx, cy| {
                    let mut points = [[0.0f32; 2]; asteroid_cfg::VERTICES];
                    for (slot, point) in points.iter_mut().zip(relative.iter()).take(count) {
                        *slot = [point[0] + cx, point[1] + cy];
                    }
                    painter.polygon(&points[..count], fill);
                    painter.polyline(&points[..count], edge, true);
                },
            );
        }
    });
}

/// Asteroids lighten as they shrink, so a Small reads apart from a Large at a
/// glance — the difference matters most in the moment an asteroid splits.
fn asteroid_fill(size: Size) -> Rgba {
    let edge = theme::color::ASTEROID_EDGE;
    match size {
        Size::Large => theme::color::ASTEROID,
        Size::Medium => theme::color::ASTEROID.mix(edge, 0.20),
        Size::Small => theme::color::ASTEROID.mix(edge, 0.45),
    }
}

// ---------------------------------------------------------------- entities

/// The Ship silhouette, in units of the sim's collision radius
/// (`ship::RADIUS`, 9 px): a nose at 1.7 r, tail corners at −1 r and ±0.8 r.
const SHIP_NOSE: f32 = 1.7;
const SHIP_TAIL: f32 = -1.0;
const SHIP_TAIL_HALF: f32 = 0.8;

/// The thrust flame, also in ship radii: a wide translucent outer cone with a
/// shorter, brighter core inside it.
const FLAME_LENGTH: f32 = 1.15;
const FLAME_HALF: f32 = 0.5;
const FLAME_CORE_LENGTH: f32 = 0.6;
const FLAME_CORE_HALF: f32 = 0.26;

/// Bullets are drawn at the sim's own radius (2 px), which is already thick
/// enough to read as a ball rather than a pixel.
const BULLET_RADIUS: f32 = bullet_cfg::RADIUS as f32;
const BULLET_SEGMENTS: usize = 8;

const RAY_TIP_RADIUS: f32 = 2.0;
const RAY_TIP_SEGMENTS: usize = 6;

fn draw_ship(painter: &mut Painter, ship: &Ship, alive: bool, thrusting: bool) {
    let radius = ship_cfg::RADIUS as f32;
    let (sin, cos) = (ship.heading.sin() as f32, ship.heading.cos() as f32);
    let body = if alive {
        theme::color::SHIP
    } else {
        theme::color::SHIP_DEAD
    };
    let flame_outer = theme::color::SHIP_FLAME.alpha(0.55);
    let flame_core = theme::color::SHIP_FLAME.mix(Rgba::rgb(1.0, 1.0, 1.0), 0.45);

    // Forward and lateral offsets in the ship's own frame, rotated into Arena
    // space around the centre of whichever copy is being drawn.
    let place = |forward: f32, lateral: f32, cx: f32, cy: f32| {
        [
            cx + forward * cos - lateral * sin,
            cy + forward * sin + lateral * cos,
        ]
    };

    seam_copies(
        painter,
        ship.x,
        ship.y,
        ship_cfg::RADIUS,
        |painter, cx, cy| {
            if alive && thrusting {
                let base = SHIP_TAIL * radius;
                painter.triangle(
                    place(base, -FLAME_HALF * radius, cx, cy),
                    place(base, FLAME_HALF * radius, cx, cy),
                    place(base - FLAME_LENGTH * radius, 0.0, cx, cy),
                    flame_outer,
                );
                painter.triangle(
                    place(base, -FLAME_CORE_HALF * radius, cx, cy),
                    place(base, FLAME_CORE_HALF * radius, cx, cy),
                    place(base - FLAME_CORE_LENGTH * radius, 0.0, cx, cy),
                    flame_core,
                );
            }
            painter.triangle(
                place(SHIP_NOSE * radius, 0.0, cx, cy),
                place(SHIP_TAIL * radius, -SHIP_TAIL_HALF * radius, cx, cy),
                place(SHIP_TAIL * radius, SHIP_TAIL_HALF * radius, cx, cy),
                body,
            );
        },
    );
}

fn draw_bullets(painter: &mut Painter, bullets: &[Bullet]) {
    let color = theme::color::BULLET;
    for bullet in bullets {
        seam_copies(
            painter,
            bullet.x,
            bullet.y,
            bullet_cfg::RADIUS,
            |painter, cx, cy| {
                painter.circle([cx, cy], BULLET_RADIUS, color, BULLET_SEGMENTS);
            },
        );
    }
}

/// The nine Sensor Rays, from the Ship's nose outward.
///
/// The frame is a proximity reading, not a distance: `inputs[k] == 1.0` is a
/// hit at zero distance and `0.0` is nothing inside `sensors::RANGE`, so the
/// length falls straight out of it. A ray that reaches full length has found
/// nothing, which is why its far end carries a dot — otherwise a long ray and
/// a ray stopped just short of the range limit would look alike.
fn draw_rays(painter: &mut Painter, ship: &Ship, inputs: &[f64]) {
    let radius = ship_cfg::RADIUS as f32;
    let color = theme::color::RAY;
    let nose = [
        ship.x as f32 + ship.heading.cos() as f32 * radius,
        ship.y as f32 + ship.heading.sin() as f32 * radius,
    ];

    for (index, offset_deg) in sensors::RAY_OFFSETS_DEG.iter().enumerate() {
        let input = inputs.get(index).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let length = if input > 0.0 {
            (1.0 - input) * sensors::RANGE
        } else {
            sensors::RANGE
        } as f32;

        let angle = ship.heading + offset_deg.to_radians();
        let (dir_x, dir_y) = (angle.cos() as f32, angle.sin() as f32);
        let far = [nose[0] + dir_x * length, nose[1] + dir_y * length];

        painter.line(nose, far, color);
        painter.circle(far, RAY_TIP_RADIUS, color, RAY_TIP_SEGMENTS);
    }
}
