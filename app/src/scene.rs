//! The Arena scene: a [`World`] turned into filled polygons and lines.
//!
//! Everything here draws in the fixed logical 960×600 Arena space and lets the
//! Painter's transform carry it into window pixels (ADR 0006), so the field
//! keeps its shape at any window size and the geometry can be authored in the
//! units the simulation itself uses.
//!
//! Facets preserve the simulation polygons; all illumination is cosmetic.
//! Strokes have logical thickness and geometry is clipped at the toroidal seam.
//!
//! The presentation is a deep field, not a flat void. The field itself — the
//! wash, the nebulae and the frame's own falloff — is painted by the backdrop
//! shader from one baked density texture (`deepfield`), so what is drawn here
//! is only what the simulation put in it: the starfield laid over the field,
//! the measuring grid, the containment seam, and the entities.
//!
//! Asteroids and the Ship's hull are materials, lit by one key light from the
//! upper left. Everything that emits — the thrust plume, bullet tracers, the
//! seam hairline, the perception corona — goes into the Painter's additive
//! buffer at a gain from `theme::light`, which writes it past white so the
//! bloom chain turns it into an actual light source. Every colour and every
//! shade below is presentation: nothing here feeds the World.
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

/// A near star: a bright core with a four-point sparkle across it, which is
/// what reads as being in front of the dust rather than part of it.
#[derive(Clone, Copy, Debug)]
struct Sparkle {
    x: f32,
    y: f32,
    /// Half the arm span, in logical units, so the sparkle is 4–6 across.
    arm: f32,
    color: Rgba,
}

/// The sky is three depth layers: far dust, a mid field, and a handful of near
/// stars that sparkle. Counts are fixed, not density-scaled: the field is part
/// of the golden frame and must not depend on anything but these constants.
const STAR_FAR_COUNT: usize = 110;
const STAR_MID_COUNT: usize = 36;
const STAR_NEAR_COUNT: usize = 10;
/// One seed per layer, so re-laying one layer leaves the others where they are.
const STAR_FAR_SEED: u32 = 0x5EED_2024;
const STAR_MID_SEED: u32 = 0x5EED_5A17;
const STAR_NEAR_SEED: u32 = 0x5EED_9E37;
/// Dim end of each layer's brightness spread; the bright end is this plus that
/// layer's range. The nearer the layer, the brighter it runs.
const STAR_FAR_MIN_ALPHA: f32 = 0.12;
const STAR_FAR_ALPHA_RANGE: f32 = 0.15;
const STAR_MID_MIN_ALPHA: f32 = 0.22;
const STAR_MID_ALPHA_RANGE: f32 = 0.20;
/// Half the arm span of a near star's sparkle, in logical units.
const SPARKLE_MIN_ARM: f32 = 2.0;
const SPARKLE_ARM_RANGE: f32 = 1.0;

/// The whole sky, laid out once per process and shared by every frame.
#[derive(Debug)]
struct Sky {
    far: Vec<Star>,
    mid: Vec<Star>,
    near: Vec<Sparkle>,
}

static SKY: LazyLock<Sky> = LazyLock::new(build_sky);

/// Lay out the sky from the three layer seeds.
///
/// The generator is written out here on purpose. `sim`'s `Rng` belongs to the
/// simulation stream — taking one number from it would move the Episode — and a
/// golden frame has to come out identical on every machine, so nothing on this
/// path may call a library RNG or a platform `sin`/`cos`. Integers and f32
/// multiply/divide are exactly reproducible; those are all that is used.
fn build_sky() -> Sky {
    Sky {
        far: build_layer(
            STAR_FAR_COUNT,
            STAR_FAR_SEED,
            STAR_FAR_MIN_ALPHA,
            STAR_FAR_ALPHA_RANGE,
        ),
        mid: build_layer(
            STAR_MID_COUNT,
            STAR_MID_SEED,
            STAR_MID_MIN_ALPHA,
            STAR_MID_ALPHA_RANGE,
        ),
        near: build_sparkles(),
    }
}

/// One round-dot layer: spread over the whole Arena, three size classes and an
/// independent brightness spread, because a uniform grid of equal dots reads as
/// dirt on the glass rather than as sky.
fn build_layer(count: usize, seed: u32, min_alpha: f32, alpha_range: f32) -> Vec<Star> {
    let mut seed = seed;
    let mut layer = Vec::with_capacity(count);
    for _ in 0..count {
        let x = lcg_unit(&mut seed) * W;
        let y = lcg_unit(&mut seed) * H;
        let class = lcg_unit(&mut seed);
        let size = if class < 0.62 {
            1.0
        } else if class < 0.90 {
            1.5
        } else {
            2.0
        };
        let alpha = min_alpha + lcg_unit(&mut seed) * alpha_range;
        layer.push(Star {
            x,
            y,
            size,
            color: theme::color::STAR.alpha(alpha),
        });
    }
    layer
}

/// The near layer: [`theme::color::STAR_BRIGHT`], each with its own arm span.
fn build_sparkles() -> Vec<Sparkle> {
    let mut seed = STAR_NEAR_SEED;
    let mut layer = Vec::with_capacity(STAR_NEAR_COUNT);
    for _ in 0..STAR_NEAR_COUNT {
        layer.push(Sparkle {
            x: lcg_unit(&mut seed) * W,
            y: lcg_unit(&mut seed) * H,
            arm: SPARKLE_MIN_ARM + lcg_unit(&mut seed) * SPARKLE_ARM_RANGE,
            color: theme::color::STAR_BRIGHT,
        });
    }
    layer
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

pub fn draw_arena(
    painter: &mut Painter,
    world: &World,
    view: ArenaView,
    show_rays: bool,
    motion: bool,
) {
    painter.set_transform(view.transform());

    painter.set_clip(Some([0.0, 0.0, W, H]));
    // The wash, the nebulae and the frame's falloff are already on the target:
    // the backdrop shader painted them before any of this was submitted. What
    // is left is what sits *in* the field — its sky, its measure, its edge.
    draw_stars(painter);
    draw_grid(painter);
    draw_field_edge(painter);

    let agent = &world.agent;
    // The rays go down first: they are translucent, and reading them through
    // the asteroids is the point of the overlay.
    if show_rays && world.time > 0.0 {
        draw_rays(painter, &agent.ship, &agent.inputs);
    }
    draw_asteroids(painter, &world.asteroids);
    draw_bullets(painter, &world.bullets);
    draw_ship(
        painter,
        &agent.ship,
        agent.alive,
        agent.thrusting,
        world.time,
        motion,
    );
    // What the Agent senses and what it asks for, worn on the hull. Both come
    // from the step the World has already taken, so a paused frame shows the
    // reading the Network was actually given and the request it actually made.
    if agent.alive && world.time > 0.0 {
        draw_corona(painter, &agent.ship, &agent.inputs);
        if let Some(network) = agent.network() {
            draw_intent(painter, &agent.ship, network);
        }
    }
    draw_tracking(painter, world);
    painter.set_clip(None);
}

fn draw_grid(painter: &mut Painter) {
    // Major lines every 120 units on top of the minor 60s: the field reads as
    // a measured volume rather than a uniform mesh.
    let minor = theme::color::ACCENT.alpha(0.006);
    let major = theme::color::ACCENT.alpha(0.012);
    for x in (0..960).step_by(60) {
        painter.line(
            [x as f32, 0.0],
            [x as f32, H],
            if x % 120 == 0 { major } else { minor },
        );
    }
    for y in (0..600).step_by(60) {
        painter.line(
            [0.0, y as f32],
            [W, y as f32],
            if y % 120 == 0 { major } else { minor },
        );
    }
    for x in (60..960).step_by(120) {
        for y in (60..600).step_by(120) {
            painter.line(
                [x as f32 - 3.0, y as f32],
                [x as f32 + 3.0, y as f32],
                theme::color::ACCENT.alpha(0.055),
            );
            painter.line(
                [x as f32, y as f32 - 3.0],
                [x as f32, y as f32 + 3.0],
                theme::color::ACCENT.alpha(0.055),
            );
        }
    }
}

fn draw_stars(painter: &mut Painter) {
    for star in SKY.far.iter().chain(SKY.mid.iter()) {
        painter.rect(star.x, star.y, star.size, star.size, star.color);
    }
    // The near layer crosses its own dot, which is what makes it a sparkle.
    for star in &SKY.near {
        let ink = star.color.alpha(0.45);
        painter.stroke(
            [star.x - star.arm, star.y],
            [star.x + star.arm, star.y],
            0.7,
            ink,
        );
        painter.stroke(
            [star.x, star.y - star.arm],
            [star.x, star.y + star.arm],
            0.7,
            ink,
        );
        painter.rect(star.x - 1.0, star.y - 1.0, 2.0, 2.0, star.color);
    }
}

/// How far inside the seam the field's edge band sits, and how thick it is.
/// Both are logical units that scale with the window, so the band stays a band.
const EDGE_INSET: f32 = 2.0;
const EDGE_WIDTH: f32 = 2.0;
/// The seam's light: a one-unit hairline just inside the band, then two stacked
/// steps of inner glow, so the Arena settles into its containment.
const SEAM_HAIRLINE: f32 = 1.0;
const SEAM_HAIRLINE_ALPHA: f32 = 0.08;
const SEAM_GLOW_NEAR: f32 = 5.0;
const SEAM_GLOW_DEEP: f32 = 9.0;
const SEAM_GLOW_ALPHA: f32 = 0.008;

/// One luminous rectangle, as the two triangles the additive buffer takes: the
/// Painter's light primitives are triangles and radial glows, and a seam band
/// is a quad.
fn luminous_rect(painter: &mut Painter, x: f32, y: f32, width: f32, height: f32, color: Rgba) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    painter.luminous_triangle([x, y], [x + width, y], [x + width, y + height], color);
    painter.luminous_triangle([x, y], [x + width, y + height], [x, y + height], color);
}

/// The Arena's extent, drawn as a band of real thickness just inside the seam.
fn draw_field_edge(painter: &mut Painter) {
    for (x, y, dx, dy) in [
        (6.0, 6.0, 1.0, 1.0),
        (W - 6.0, 6.0, -1.0, 1.0),
        (6.0, H - 6.0, 1.0, -1.0),
        (W - 6.0, H - 6.0, -1.0, -1.0),
    ] {
        painter.stroke(
            [x, y],
            [x + dx * 26.0, y],
            2.0,
            theme::color::ACCENT.alpha(0.8),
        );
        painter.stroke(
            [x, y],
            [x, y + dy * 26.0],
            2.0,
            theme::color::ACCENT.alpha(0.8),
        );
    }
    let color = theme::color::ASTEROID_EDGE.alpha(0.16);
    let (left, top) = (EDGE_INSET, EDGE_INSET);
    let (right, bottom) = (W - EDGE_INSET, H - EDGE_INSET);
    painter.rect(left, top, right - left, EDGE_WIDTH, color);
    painter.rect(left, bottom - EDGE_WIDTH, right - left, EDGE_WIDTH, color);
    painter.rect(left, top, EDGE_WIDTH, bottom - top, color);
    painter.rect(right - EDGE_WIDTH, top, EDGE_WIDTH, bottom - top, color);

    // The seam's own light, inside the band: a hairline where the field stops,
    // and a shallow glow that settles the Arena into its containment. Static —
    // containment is a property of the field, not an event.
    painter.set_gain(theme::light::SEAM);
    let left = EDGE_INSET + EDGE_WIDTH;
    let top = EDGE_INSET + EDGE_WIDTH;
    let right = W - left;
    let bottom = H - top;
    let (span_x, span_y) = (right - left, bottom - top);
    let hair = theme::color::ACCENT.alpha(SEAM_HAIRLINE_ALPHA);
    luminous_rect(painter, left, top, span_x, SEAM_HAIRLINE, hair);
    luminous_rect(
        painter,
        left,
        bottom - SEAM_HAIRLINE,
        span_x,
        SEAM_HAIRLINE,
        hair,
    );
    luminous_rect(painter, left, top, SEAM_HAIRLINE, span_y, hair);
    luminous_rect(
        painter,
        right - SEAM_HAIRLINE,
        top,
        SEAM_HAIRLINE,
        span_y,
        hair,
    );

    // Two stacked steps: the deep band alone is the quiet outer falloff, and
    // the near band lands on top of it for the roughly 0.02 that touches the
    // hairline. Two luminous quads is the cheapest falloff the Painter offers.
    let deep = theme::color::ACCENT.alpha(SEAM_GLOW_ALPHA);
    let near = theme::color::ACCENT.alpha(SEAM_GLOW_ALPHA / 2.0);
    for (depth, ink) in [
        (SEAM_GLOW_NEAR, near),
        (SEAM_GLOW_NEAR + SEAM_GLOW_DEEP, deep),
    ] {
        luminous_rect(painter, left, top, span_x, depth, ink);
        luminous_rect(painter, left, bottom - depth, span_x, depth, ink);
        luminous_rect(painter, left, top, depth, span_y, ink);
        luminous_rect(painter, right - depth, top, depth, span_y, ink);
    }
    painter.set_gain(1.0);
}

// --------------------------------------------------------------- asteroids

thread_local! {
    /// One vertex buffer, reused by every asteroid in every frame:
    /// `Asteroid::vertices` insists on a `Vec`, and the field must not pay for
    /// a fresh one sixty times a second.
    static ASTEROID_VERTICES: RefCell<Vec<(f64, f64)>> = const { RefCell::new(Vec::new()) };
}

/// The key light: one direction, upper left, shared by every material in the
/// Arena. This is (-0.6, -0.6) normalized.
const KEY_LIGHT: [f32; 2] = [
    -std::f32::consts::FRAC_1_SQRT_2,
    -std::f32::consts::FRAC_1_SQRT_2,
];

/// How much of the key light a facet whose outward direction (a unit vector
/// from the asteroid's centre) faces, mapped from the dot product into
/// `ASTEROID_MIN_SHADE ..= 1.0`. The floor is nearly nothing: what keeps the
/// dark side readable is the ambient the deep field puts back, not a floor on
/// the key light.
const ASTEROID_MIN_SHADE: f32 = 0.04;

/// How sharply the terminator falls. Above one, the lit side holds its value
/// further around the body and then drops away quickly, which is what makes a
/// rock read as a sphere of mass rather than a disc with a gradient on it.
const ASTEROID_TERMINATOR: f32 = 1.8;

/// What the deep field itself puts back on the unlit side. Nothing in space is
/// lit from one direction only, and a rock that falls to black stops being an
/// object and becomes a hole.
const ASTEROID_AMBIENT: f32 = 0.16;

/// The shade at the rock's own middle, where every facet meets. Just past the
/// terminator's midpoint, so the body reads as turning away from the light
/// rather than as a flat disc with a bright edge.
const ASTEROID_CORE_SHADE: f32 = 0.55;

/// The widest angle a facet may face the key light from and still catch its
/// rim, as a cosine. Ten facets are 36 degrees apart, so this lights one or two.
const RIM_CONE: f32 = 0.866;

/// And the opposite cone: the facets turned away from the key light catch the
/// field behind them instead. That cool back edge is what lifts a rock off the
/// deep field it is floating in.
const BACK_CONE: f32 = -0.5;

/// The rim's brightness against the facet edges that only draw the silhouette.
const RIM_ALPHA: f32 = 0.85;
const BACK_ALPHA: f32 = 0.16;
const FACET_EDGE_ALPHA: f32 = 0.10;

/// The interior cracks are brighter where the key light falls: dim on the dark
/// side, legible on the lit one.
const CRACK_MIN_ALPHA: f32 = 0.04;
const CRACK_ALPHA_RANGE: f32 = 0.14;
/// Three fractures per rock, each running from a little way out of the middle
/// toward one of the rock's own vertices: the first pair is the radius it
/// starts at in Arena units, the second the fraction of the vertex it reaches.
const CRACK_COUNT: usize = 3;
const CRACK_REACH: [[f32; 2]; CRACK_COUNT] = [[3.0, 0.72], [2.0, 0.55], [4.0, 0.80]];

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

            // The material of each vertex and of each facet's interior,
            // computed once per asteroid: the key light is fixed, so a Seam
            // Copy reuses all of it.
            let mut vertex_ink = [theme::color::ASTEROID_UNLIT; asteroid_cfg::VERTICES];
            for (slot, point) in vertex_ink.iter_mut().zip(relative.iter()).take(count) {
                *slot = asteroid_ink(asteroid.size, unit(point[0], point[1]));
            }
            // One colour at the rock's middle, shared by every facet. The fan
            // meets there, so a per-facet middle would make the centre a
            // pinwheel of ten hard wedges; one core turns the same fan into a
            // body shaded smoothly from the middle out to the lit and unlit
            // rims.
            let core = asteroid_shaded(asteroid.size, ASTEROID_CORE_SHADE);
            let mut facet_edge =
                [theme::color::ASTEROID_EDGE.alpha(FACET_EDGE_ALPHA); asteroid_cfg::VERTICES];
            for index in 0..count {
                let next = (index + 1) % count;
                let facing = unit(
                    relative[index][0] + relative[next][0],
                    relative[index][1] + relative[next][1],
                );
                // Three kinds of edge: the hot rim where the key light grazes
                // the silhouette, the cool back edge where the deep field does,
                // and the rest, which only has to describe the shape.
                let toward = facing[0] * KEY_LIGHT[0] + facing[1] * KEY_LIGHT[1];
                facet_edge[index] = if toward > RIM_CONE {
                    theme::color::ASTEROID_RIM.alpha(RIM_ALPHA)
                } else if toward < BACK_CONE {
                    // Desaturated on purpose: a back edge is the field seen
                    // past the rock, not a selection outline drawn on it.
                    theme::color::ACCENT
                        .mix(theme::color::STAR_BRIGHT, 0.55)
                        .alpha(BACK_ALPHA)
                } else {
                    theme::color::ASTEROID_EDGE.alpha(FACET_EDGE_ALPHA)
                };
            }

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
                    // The simulation polygon is preserved; light is a
                    // presentation-only facet. Each facet is a gradient from
                    // its own middle out to its two rim vertices: the facet's
                    // facing lights the wedge, and the rim carries the key
                    // light into the silhouette.
                    for index in 0..count {
                        let next = (index + 1) % count;
                        painter.gradient_polygon(
                            &[[cx, cy], points[index], points[next]],
                            &[core, vertex_ink[index], vertex_ink[next]],
                        );
                        // The facets that face the key light carry the rim.
                        painter.stroke(points[index], points[next], 1.2, facet_edge[index]);
                    }
                    // Fractures run outward from the middle along the rock's
                    // own vertices, so every rock's are its own, and each is
                    // inked by the light where it runs.
                    for step in 0..CRACK_COUNT {
                        let vertex = relative[(step * 3 + 2) % count];
                        let direction = unit(vertex[0], vertex[1]);
                        let reach = CRACK_REACH[step];
                        let lit = shade(direction, KEY_LIGHT);
                        painter.stroke(
                            [cx + direction[0] * reach[0], cy + direction[1] * reach[0]],
                            [cx + vertex[0] * reach[1], cy + vertex[1] * reach[1]],
                            0.7,
                            edge.alpha(CRACK_MIN_ALPHA + CRACK_ALPHA_RANGE * lit),
                        );
                    }
                },
            );
        }
    });
}

/// A unit direction, guarding the degenerate zero-length case so a shading
/// lookup can never produce NaN.
fn unit(x: f32, y: f32) -> [f32; 2] {
    let length = x.hypot(y);
    if length <= f32::EPSILON {
        [0.0, 1.0]
    } else {
        [x / length, y / length]
    }
}

/// How much of the key light a surface facing `direction` receives: 1.0 facing
/// the light, [`ASTEROID_MIN_SHADE`] facing away. Dot product remapped into
/// `0..=1`, then floored so the dark side stays material.
fn shade(direction: [f32; 2], light: [f32; 2]) -> f32 {
    let dot = direction[0] * light[0] + direction[1] * light[1];
    (dot * 0.5 + 0.5).clamp(ASTEROID_MIN_SHADE, 1.0)
}

/// The material at a shade: unlit rock toward lit rock, then the size lighten.
fn asteroid_ink(size: Size, direction: [f32; 2]) -> Rgba {
    asteroid_shaded(size, shade(direction, KEY_LIGHT))
}

/// The same material at an explicit shade: the deep field's ambient sets the
/// floor, the key light lifts it through a shaped terminator, and the size
/// lighten rides on top of both.
fn asteroid_shaded(size: Size, shade: f32) -> Rgba {
    theme::color::ASTEROID_UNLIT
        .mix(theme::color::WASH, ASTEROID_AMBIENT)
        .mix(theme::color::ASTEROID_LIT, shade.powf(ASTEROID_TERMINATOR))
        .mix(theme::color::ASTEROID_RIM, size_lighten(size))
}

/// Asteroids lighten as they shrink, so a Small reads apart from a Large at a
/// glance — the difference matters most in the moment an Asteroid splits. The
/// mix is toward the rim's warm white rather than the cool edge, so a fresh
/// fragment reads as hot stone and not as frosted glass.
fn size_lighten(size: Size) -> f32 {
    match size {
        Size::Large => 0.0,
        Size::Medium => 0.12,
        Size::Small => 0.28,
    }
}

// ---------------------------------------------------------------- entities

/// The Ship silhouette, in units of the sim's collision radius
/// (`ship::RADIUS`, 9 px): a nose at 1.7 r, tail corners at −1 r and ±0.8 r.
const SHIP_NOSE: f32 = 1.7;
const SHIP_TAIL: f32 = -1.0;
const SHIP_TAIL_HALF: f32 = 0.8;

/// The thrust flame, also in ship radii: a wide outer cone with a shorter,
/// brighter core inside it. Both are light, and both flicker only when motion
/// is on — see [`flame_length`].
const FLAME_LENGTH: f32 = 1.15;
const FLAME_HALF: f32 = 0.5;
const FLAME_CORE_LENGTH: f32 = 0.6;
const FLAME_CORE_HALF: f32 = 0.26;
/// How much of the hull's own colour the nose catches. The key light is a
/// material property here; the hull is the only thing in the Arena that also
/// catches it on a moving frame.
const HULL_NOSE_LIFT: f32 = 0.45;
/// The Ship's own light: a halo around the hull, and the glow the plume throws
/// on the space just behind it.
const SHIP_HALO_RADIUS: f32 = 19.0;
const SHIP_HALO_ALPHA: f32 = 0.17;
const FLAME_HALO_RADIUS: f32 = 10.0;
const FLAME_HALO_ALPHA: f32 = 0.25;

/// Bullets are drawn at the sim's own radius (2 px), which is already thick
/// enough to read as a ball rather than a pixel.
const BULLET_RADIUS: f32 = bullet_cfg::RADIUS as f32;
const BULLET_SEGMENTS: usize = 8;
/// The tracer is light, not a smear of the bullet's own colour: a short streak
/// of gradient quads ending in a halo around the head.
const TRACER_SEGMENTS: usize = 3;
const TRACER_HALF_WIDTH: f32 = 1.2;
const TRACER_HEAD_ALPHA: f32 = 0.7;
const BULLET_HALO_RADIUS: f32 = 8.0;
const BULLET_HALO_ALPHA: f32 = 0.4;

const RAY_TIP_RADIUS: f32 = 2.0;
const RAY_TIP_SEGMENTS: usize = 6;
const RAY_HALF_WIDTH: f32 = 0.9;
const RAY_SEGMENTS: usize = 6;

/// The plume's length scale at simulation time `time`. Flicker is motion: with
/// motion off the plume is its authored length, so a paused frame — or a
/// reduced-motion frame — is complete and perfectly still.
fn flame_length(base: f32, time: f64, motion: bool) -> f32 {
    if motion {
        base * (0.88 + 0.12 * (time * 21.0).sin() as f32)
    } else {
        base
    }
}

fn draw_ship(
    painter: &mut Painter,
    ship: &Ship,
    alive: bool,
    thrusting: bool,
    time: f64,
    motion: bool,
) {
    let radius = ship_cfg::RADIUS as f32;
    let (sin, cos) = (ship.heading.sin() as f32, ship.heading.cos() as f32);
    let body = if alive {
        theme::color::SHIP
    } else {
        theme::color::SHIP_DEAD
    };
    let nose = body.mix(Rgba::rgb(1.0, 1.0, 1.0), HULL_NOSE_LIFT);
    let flame_outer = theme::color::SHIP_FLAME.alpha(0.5);
    let flame_core = theme::color::LIGHT_FLAME_CORE.alpha(0.9);

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
        ship_cfg::RADIUS * 4.0,
        |painter, cx, cy| {
            if alive {
                painter.set_gain(theme::light::SHIP_HALO);
                painter.luminous_glow([cx, cy], SHIP_HALO_RADIUS, body.alpha(SHIP_HALO_ALPHA));
                painter.set_gain(1.0);
            }
            if alive && thrusting {
                // The plume's anchor is the tail; only its length flickers.
                let base = SHIP_TAIL * radius;
                let outer = flame_length(FLAME_LENGTH * radius, time, motion);
                let core = flame_length(FLAME_CORE_LENGTH * radius, time, motion);
                painter.set_gain(theme::light::PLUME);
                painter.luminous_glow(
                    place(base, 0.0, cx, cy),
                    FLAME_HALO_RADIUS,
                    theme::color::SHIP_FLAME.alpha(FLAME_HALO_ALPHA),
                );
                painter.luminous_triangle(
                    place(base, -FLAME_HALF * radius, cx, cy),
                    place(base, FLAME_HALF * radius, cx, cy),
                    place(base - outer, 0.0, cx, cy),
                    flame_outer,
                );
                painter.set_gain(theme::light::PLUME_CORE);
                painter.luminous_triangle(
                    place(base, -FLAME_CORE_HALF * radius, cx, cy),
                    place(base, FLAME_CORE_HALF * radius, cx, cy),
                    place(base - core, 0.0, cx, cy),
                    flame_core,
                );
                painter.set_gain(1.0);
            }
            // The hull is a material like the asteroids': the nose catches the
            // key light, the tail corners keep the hull's own colour.
            painter.gradient_polygon(
                &[
                    place(SHIP_NOSE * radius, 0.0, cx, cy),
                    place(SHIP_TAIL * radius, -SHIP_TAIL_HALF * radius, cx, cy),
                    place(SHIP_TAIL * radius, SHIP_TAIL_HALF * radius, cx, cy),
                ],
                &[nose, body, body],
            );
            painter.triangle(
                place(1.15 * radius, 0.0, cx, cy),
                place(-0.65 * radius, 0.0, cx, cy),
                place(-radius, 0.8 * radius, cx, cy),
                body.mix(theme::color::ASTEROID_UNLIT, 0.55),
            );
            painter.stroke(
                place(1.4 * radius, 0.0, cx, cy),
                place(-0.6 * radius, -0.4 * radius, cx, cy),
                1.0,
                Rgba::rgb(0.85, 1.0, 1.0),
            );
            painter.circle(place(0.0, 0.0, cx, cy), 2.0, Rgba::rgb(0.9, 1.0, 1.0), 12);
        },
    );
}

/// A streak of light from `tail` to `head`: a run of thin quads whose intensity
/// climbs toward the head, ending at nothing behind. Each quad is split along
/// the streak so its two halves carry the fade's two ends — a stepped gradient,
/// which is what the additive buffer offers without a luminous gradient.
fn luminous_streak(
    painter: &mut Painter,
    tail: [f32; 2],
    head: [f32; 2],
    half_width: f32,
    color: Rgba,
    segments: usize,
) {
    let (dx, dy) = (head[0] - tail[0], head[1] - tail[1]);
    let length = dx.hypot(dy);
    if length < 0.001 {
        return;
    }
    let (nx, ny) = (-dy / length * half_width, dx / length * half_width);
    for index in 0..segments {
        let near = index as f32 / segments as f32;
        let far = (index + 1) as f32 / segments as f32;
        let a = [tail[0] + dx * near, tail[1] + dy * near];
        let b = [tail[0] + dx * far, tail[1] + dy * far];
        painter.luminous_triangle(
            [a[0] + nx, a[1] + ny],
            [b[0] + nx, b[1] + ny],
            [b[0] - nx, b[1] - ny],
            color.alpha(color.a * far),
        );
        painter.luminous_triangle(
            [a[0] + nx, a[1] + ny],
            [b[0] - nx, b[1] - ny],
            [a[0] - nx, a[1] - ny],
            color.alpha(color.a * near),
        );
    }
}

fn draw_bullets(painter: &mut Painter, bullets: &[Bullet]) {
    let color = theme::color::BULLET;
    for bullet in bullets {
        seam_copies(painter, bullet.x, bullet.y, 24.0, |painter, cx, cy| {
            let dx = (bullet.vx as f32 * 0.025).clamp(-20.0, 20.0);
            let dy = (bullet.vy as f32 * 0.025).clamp(-20.0, 20.0);
            painter.set_gain(theme::light::TRACER);
            luminous_streak(
                painter,
                [cx - dx, cy - dy],
                [cx, cy],
                TRACER_HALF_WIDTH,
                color.alpha(TRACER_HEAD_ALPHA),
                TRACER_SEGMENTS,
            );
            painter.luminous_glow([cx, cy], BULLET_HALO_RADIUS, color.alpha(BULLET_HALO_ALPHA));
            // The round itself is the light, not a dot painted the colour of
            // one: it goes into the additive buffer, well past white.
            painter.set_gain(theme::light::BULLET);
            painter.luminous_circle([cx, cy], BULLET_RADIUS, color, BULLET_SEGMENTS);
            painter.set_gain(1.0);
        });
    }
}

/// The nine Sensor Rays, drawn at the distance they actually measured.
///
/// The frame is a proximity reading, not a distance: `inputs[k] == 1.0` is a
/// hit at zero distance and `0.0` is nothing inside `sensors::RANGE`, so the
/// length falls straight out of it. A ray that found nothing has no distance to
/// report and is drawn as a stub — the overlay is for reading what the Agent
/// found, and nine full-range lines from one point is a starburst, not a
/// reading.
fn draw_rays(painter: &mut Painter, ship: &Ship, inputs: &[f64]) {
    // Inputs were sensed at the Ship centre. Draw exactly that sampled distance.
    let origin = [ship.x as f32, ship.y as f32];
    for (index, offset_deg) in sensors::RAY_OFFSETS_DEG.iter().enumerate() {
        let input = inputs.get(index).copied().unwrap_or(0.0).clamp(0.0, 1.0) as f32;
        // A ray that found nothing has no distance to report; the corona on
        // the hull already marks its slot.
        if input <= 0.0 {
            continue;
        }
        let length = (1.0 - input) * sensors::RANGE as f32;
        let angle = ship.heading + offset_deg.to_radians();
        let far = [
            origin[0] + angle.cos() as f32 * length,
            origin[1] + angle.sin() as f32 * length,
        ];
        // Translate complete rays through the torus and clip, preserving seam
        // intersections.
        for ox in [-W, 0.0, W] {
            for oy in [-H, 0.0, H] {
                let a = [origin[0] + ox, origin[1] + oy];
                let b = [far[0] + ox, far[1] + oy];
                // Light that gathers toward what it found, so the eye goes to
                // the reading rather than to the line carrying it.
                painter.set_gain(theme::light::RAY);
                luminous_streak(
                    painter,
                    a,
                    b,
                    RAY_HALF_WIDTH,
                    theme::color::ENERGY.alpha(0.16 + input * 0.5),
                    RAY_SEGMENTS,
                );
                painter.luminous_circle(b, RAY_TIP_RADIUS, theme::color::ENERGY, RAY_TIP_SEGMENTS);
                painter.set_gain(1.0);
            }
        }
    }
}

// ------------------------------------------------------------------- mind
//
// The signature instrument: the Agent wears what it senses and what it asks
// for, on its own hull, where the eye already is.
//
// Both rings are read straight out of the World. The corona is the sampled
// Sensorium the Network was handed this step; the intent ring is the
// activations the Network actually produced. Nothing here is smoothed,
// extrapolated or invented — an empty corona means the Agent sensed nothing,
// and a dark intent arc means it asked for nothing.

/// The corona is a ring of nine arcs around the hull, one per Sensor Ray. An
/// empty reading holds its arc at [`CORONA_FAR`]; a closing one pulls it in
/// toward [`CORONA_NEAR`], so the ring dents inward where the Agent is under
/// pressure. Both are in Ship radii.
const CORONA_FAR: f32 = 3.4;
const CORONA_NEAR: f32 = 1.6;
/// Half the angular width of one ray's arc, in degrees. The nine rays are 40
/// degrees apart, so this leaves a hairline of field between neighbours.
const CORONA_HALF_ANGLE: f32 = 16.0;
const CORONA_STEPS: usize = 6;
const CORONA_WIDTH: f32 = 1.6;
/// How far a reading's arc carries a tick back toward the hull, pointing at
/// what it found — direction, without colour having to carry it.
const CORONA_TICK: f32 = 3.0;

/// The intent ring: four arcs outside the corona, one per motor request, each
/// sitting where the request acts — fire at the nose, thrust at the tail, the
/// turns to port and starboard.
const INTENT_RADIUS: f32 = 4.6;
const INTENT_HALF_ANGLE: f32 = 24.0;
const INTENT_WIDTH: f32 = 2.2;
const INTENT_STEPS: usize = 10;
/// Coverage of a lit intent arc. Below one so the gain lifts it past white
/// without burning its hue out of it.
const INTENT_LIT_ALPHA: f32 = 0.72;
/// Degrees from the heading, in `Network::output_ids` order: left, right,
/// thrust, fire. Screen y runs down, so starboard is the positive turn.
const INTENT_BEARINGS: [f32; 4] = [-90.0, 90.0, 180.0, 0.0];

/// What the Agent senses, as a ring of nine arcs around its hull: the arc for a
/// ray that found nothing sits out at the ring's rest radius, and one that
/// found something is pulled in toward the hull in proportion to how close it
/// is. It is the same mapping as the bench dial in the strip below the Arena,
/// so the two read as one instrument in two places.
fn draw_corona(painter: &mut Painter, ship: &Ship, inputs: &[f64]) {
    let radius = ship_cfg::RADIUS as f32;
    let reach = f64::from(CORONA_FAR * radius) + 2.0;
    seam_copies(painter, ship.x, ship.y, reach, |painter, cx, cy| {
        for (index, offset_deg) in sensors::RAY_OFFSETS_DEG.iter().enumerate() {
            let value = inputs.get(index).copied().unwrap_or(0.0).clamp(0.0, 1.0) as f32;
            let ring = (CORONA_FAR - (CORONA_FAR - CORONA_NEAR) * value) * radius;
            let middle = ship.heading as f32 + offset_deg.to_radians() as f32;
            let half = CORONA_HALF_ANGLE.to_radians();
            let at = |t: f32, r: f32| {
                let angle = middle - half + 2.0 * half * t;
                [cx + angle.cos() * r, cy + angle.sin() * r]
            };
            let arc: Vec<[f32; 2]> = (0..=CORONA_STEPS)
                .map(|step| at(step as f32 / CORONA_STEPS as f32, ring))
                .collect();
            if value <= 0.0 {
                // A slot that found nothing still says so, out at rest.
                for pair in arc.windows(2) {
                    painter.stroke(
                        pair[0],
                        pair[1],
                        CORONA_WIDTH * 0.7,
                        theme::color::ACCENT.alpha(0.16),
                    );
                }
                continue;
            }
            // Cyan is perception; the closer the reading, the further it runs
            // toward the amber the Arena uses for danger.
            let ink = crate::instruments::proximity_ink(value);
            painter.set_gain(theme::light::CORONA);
            for pair in arc.windows(2) {
                crate::effects::light_stroke(
                    painter,
                    pair[0],
                    pair[1],
                    CORONA_WIDTH,
                    ink.alpha(0.30 + value * 0.45),
                );
            }
            // A tick inward from the middle of the arc: the reading has a
            // direction, and it is readable without the colour.
            let inner = at(0.5, ring - CORONA_TICK);
            crate::effects::light_stroke(
                painter,
                at(0.5, ring),
                inner,
                CORONA_WIDTH * 0.8,
                ink.alpha(0.25 + value * 0.4),
            );
            painter.set_gain(1.0);
        }
    });
}

/// What the Agent is asking for, as four gauges on the hull. Each arc is a
/// track with a filled part: the fill is the raw output mapped from `-1..1`
/// onto the arc, and the tick is `nn::ACTION_THRESHOLD`, the value the
/// simulation actually compares against. An arc past its tick is lit.
fn draw_intent(painter: &mut Painter, ship: &Ship, network: &sim::Network) {
    let radius = ship_cfg::RADIUS as f32;
    let ring = INTENT_RADIUS * radius;
    let outputs = network.output_ids();
    seam_copies(
        painter,
        ship.x,
        ship.y,
        f64::from(ring) + 4.0,
        |painter, cx, cy| {
            for (slot, bearing) in INTENT_BEARINGS.iter().enumerate() {
                let value = network.activation(outputs[slot]) as f32;
                // tanh output onto the arc: -1 is the empty end, +1 the full one.
                let filled = ((value + 1.0) * 0.5).clamp(0.0, 1.0);
                let requested = value > sim::config::nn::ACTION_THRESHOLD as f32;
                let middle = ship.heading as f32 + bearing.to_radians();
                let half = INTENT_HALF_ANGLE.to_radians();
                let start = middle - half;
                let sweep = 2.0 * half;
                let at = |t: f32| {
                    let angle = start + sweep * t;
                    [cx + angle.cos() * ring, cy + angle.sin() * ring]
                };
                let track: Vec<[f32; 2]> = (0..=INTENT_STEPS)
                    .map(|step| at(step as f32 / INTENT_STEPS as f32))
                    .collect();
                for pair in track.windows(2) {
                    painter.stroke(
                        pair[0],
                        pair[1],
                        INTENT_WIDTH,
                        theme::color::ACCENT.alpha(0.13),
                    );
                }
                let lit = (filled * INTENT_STEPS as f32).ceil() as usize;
                let ink = if requested {
                    theme::color::ENERGY
                } else {
                    theme::color::ACCENT
                };
                painter.set_gain(if requested { theme::light::INTENT } else { 1.0 });
                for pair in track[..=lit.min(INTENT_STEPS)].windows(2) {
                    if requested {
                        // Coverage under one on purpose: at full coverage the
                        // gain clips the arc to white and it stops reading as
                        // energy at all.
                        crate::effects::light_stroke(
                            painter,
                            pair[0],
                            pair[1],
                            INTENT_WIDTH,
                            ink.alpha(INTENT_LIT_ALPHA),
                        );
                    } else {
                        painter.stroke(pair[0], pair[1], INTENT_WIDTH, ink.alpha(0.34));
                    }
                }
                painter.set_gain(1.0);
                // The threshold the simulation compares against, marked on the arc
                // so "lit" is never the only way to read that a request is live.
                let tick = (sim::config::nn::ACTION_THRESHOLD as f32 + 1.0) * 0.5;
                let mark = at(tick);
                let outward = [(mark[0] - cx) / ring, (mark[1] - cy) / ring];
                painter.stroke(
                    [
                        mark[0] - outward[0] * INTENT_WIDTH,
                        mark[1] - outward[1] * INTENT_WIDTH,
                    ],
                    [
                        mark[0] + outward[0] * INTENT_WIDTH * 1.6,
                        mark[1] + outward[1] * INTENT_WIDTH * 1.6,
                    ],
                    1.0,
                    theme::color::TEXT_DIM.alpha(0.65),
                );
            }
        },
    );
}

/// How far out the velocity chevron sits, in Arena units: clear of the intent
/// ring, which reaches `INTENT_RADIUS` Ship radii.
const CHEVRON_REACH: f32 = 62.0;

/// A geometric navigation overlay, independent of the Network's sampled inputs.
fn draw_tracking(p: &mut Painter, world: &World) {
    use crate::painter::Align;
    let ship = &world.agent.ship;
    let speed = ship.vx.hypot(ship.vy) as f32;
    if world.agent.alive && speed > 8.0 {
        let direction = [ship.vx as f32 / speed, ship.vy as f32 / speed];
        // This short chevron shows actual velocity, which can differ from heading.
        // Outside the intent ring, so the two never overlap: the chevron is
        // where the Ship is going, the ring is what it is asking for.
        seam_copies(p, ship.x, ship.y, 80.0, |p, cx, cy| {
            let tip = [
                cx + direction[0] * CHEVRON_REACH,
                cy + direction[1] * CHEVRON_REACH,
            ];
            for sign in [-1.0, 1.0] {
                p.stroke(
                    tip,
                    [
                        tip[0] - direction[0] * 6.0 - direction[1] * sign * 4.0,
                        tip[1] - direction[1] * 6.0 + direction[0] * sign * 4.0,
                    ],
                    1.1,
                    theme::color::SHIP.alpha(0.5),
                );
            }
        });
    }
    // Edge rulers make the fixed coordinate system and seam readable.
    for x in (120..960).step_by(120) {
        p.stroke(
            [x as f32, 3.0],
            [x as f32, 8.0],
            1.0,
            theme::color::PANEL_TITLE.alpha(0.5),
        );
        p.text_aligned(
            [x as f32, 12.0],
            8.0,
            theme::color::TEXT_DIM.alpha(0.55),
            Align::Center,
            format!("{x:03}"),
        );
    }
    // Nearest geometry is explicitly labeled; this is not a selected neural target.
    let nearest = world.asteroids.iter().min_by(|a, b| {
        let distance =
            |a: &Asteroid| sim::math::tdx(a.x, ship.x).hypot(sim::math::tdy(a.y, ship.y)) - a.r;
        distance(a).total_cmp(&distance(b))
    });
    if let Some(a) = nearest.filter(|_| world.agent.alive) {
        let dx = sim::math::tdx(a.x, ship.x) as f32;
        let dy = sim::math::tdy(a.y, ship.y) as f32;
        let clearance = (dx.hypot(dy) - a.r as f32 - ship_cfg::RADIUS as f32).max(0.0);
        let ink = if clearance < 100.0 {
            theme::color::WARN
        } else {
            theme::color::SHIP_FLAME
        };
        let r = a.r as f32 + 8.0;
        seam_copies(p, a.x, a.y, a.r + 28.0, |p, cx, cy| {
            for (start, sweep) in [(0.2, 0.55), (1.77, 0.55), (3.34, 0.55), (4.91, 0.55)] {
                p.path(
                    &crate::vector::arc([cx, cy], r, start, sweep),
                    1.25,
                    ink.alpha(0.6),
                );
            }
        });
        // Keep labels in a fixed HUD lane, never over moving Asteroids.
        p.text(
            [16.0, H - 24.0],
            9.0,
            ink,
            format!("NEAREST HULL  {clearance:05.1}u"),
        );
    }
    p.text_aligned(
        [W - 16.0, H - 24.0],
        9.0,
        theme::color::TEXT_DIM,
        Align::Right,
        format!("V {speed:05.1}u/s   T {:05.1}s", world.time),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Perceived brightness, good enough to compare two inks.
    fn luma(color: Rgba) -> f32 {
        0.299 * color.r + 0.587 * color.g + 0.114 * color.b
    }

    #[test]
    fn the_sky_is_laid_out_once_in_three_reproducible_layers() {
        let sky = &*SKY;
        assert_eq!(sky.far.len(), STAR_FAR_COUNT);
        assert_eq!(sky.mid.len(), STAR_MID_COUNT);
        assert_eq!(sky.near.len(), STAR_NEAR_COUNT);

        // Laid out from fixed seeds and integer arithmetic alone: building it
        // twice cannot move a star. The golden frame depends on that.
        let again = build_sky();
        let layout = |layer: &[Star]| layer.iter().map(|s| (s.x, s.y, s.size)).collect::<Vec<_>>();
        assert_eq!(layout(&sky.far), layout(&again.far));
        assert_eq!(layout(&sky.mid), layout(&again.mid));

        for star in sky.far.iter().chain(sky.mid.iter()) {
            assert!((0.0..W).contains(&star.x) && (0.0..H).contains(&star.y));
            assert!((1.0..=2.0).contains(&star.size));
        }
        let band = |min: f32, range: f32| (0.0..=1.0).contains(&min) && range > 0.0;
        assert!(band(STAR_FAR_MIN_ALPHA, STAR_FAR_ALPHA_RANGE));
        assert!(sky.far.iter().all(|s| {
            (STAR_FAR_MIN_ALPHA..=STAR_FAR_MIN_ALPHA + STAR_FAR_ALPHA_RANGE).contains(&s.color.a)
        }));
        assert!(sky.mid.iter().all(|s| {
            (STAR_MID_MIN_ALPHA..=STAR_MID_MIN_ALPHA + STAR_MID_ALPHA_RANGE).contains(&s.color.a)
        }));
        // Near stars are brighter than anything behind them and carry a
        // sparkle 4–6 units across.
        for star in &sky.near {
            assert!(star.color.a > STAR_MID_MIN_ALPHA + STAR_MID_ALPHA_RANGE);
            assert!((SPARKLE_MIN_ARM..=SPARKLE_MIN_ARM + SPARKLE_ARM_RANGE).contains(&star.arm));
        }
    }

    #[test]
    fn the_key_light_lifts_the_lit_side_and_keeps_the_dark_side_readable() {
        let facing_the_light = shade(unit(-0.6, -0.6), KEY_LIGHT);
        let facing_away = shade(unit(0.6, 0.6), KEY_LIGHT);
        let across = shade(unit(0.6, -0.6), KEY_LIGHT);
        assert!((facing_the_light - 1.0).abs() < 1e-6);
        assert!((facing_away - ASTEROID_MIN_SHADE).abs() < 1e-6);
        assert!(across > facing_away && across < facing_the_light);
        // A degenerate direction shades rather than producing NaN.
        assert!(shade(unit(0.0, 0.0), KEY_LIGHT) > 0.0);

        // A Large is a solid now: its lit side is brighter than the flat fill it
        // replaced and its dark side is darker, so contrast with the field can
        // only have gone up.
        let flat = luma(theme::color::ASTEROID);
        let lit = luma(asteroid_ink(Size::Large, unit(-0.6, -0.6)));
        let unlit = luma(asteroid_ink(Size::Large, unit(0.6, 0.6)));
        assert!(lit > flat && unlit < flat);
        assert!(lit - unlit > 0.2);

        // And a Small still reads lighter than a Large at the same facing.
        let small = luma(asteroid_ink(Size::Small, unit(0.6, 0.6)));
        assert!(small > unlit);

        // The rock's middle is one colour, shared by every facet, and it sits
        // between the two sides: the fan meets there, and a per-facet middle
        // would turn the centre into a pinwheel of ten hard wedges.
        let core = luma(asteroid_shaded(Size::Large, ASTEROID_CORE_SHADE));
        assert!(unlit < core && core < lit);
        // It is also the colour anything sampling the centre of a rock finds,
        // so it has to be plainly rock rather than the field behind it.
        assert!(core > luma(theme::color::ARENA_BG) + 0.15);

        // The terminator is shaped, so the lit half holds its value and the
        // fall happens over the back: a facing halfway round is nearer the dark
        // end than a straight ramp would put it.
        let across_ink = luma(asteroid_ink(Size::Large, unit(0.6, -0.6)));
        assert!(across_ink < (unlit + lit) * 0.5);
    }

    #[test]
    fn the_rim_light_catches_one_or_two_facets_at_every_rotation() {
        // Ten facets are 36° apart, so a 30° cone around the key light can hold
        // one facet or two — never none, never the whole silhouette.
        for turn in 0..72 {
            let angle = turn as f64 * std::f64::consts::TAU / 72.0;
            let mut rim = 0;
            let points: [[f32; 2]; asteroid_cfg::VERTICES] = std::array::from_fn(|index| {
                let a =
                    angle + std::f64::consts::TAU / asteroid_cfg::VERTICES as f64 * index as f64;
                [(a.cos()) as f32 * 20.0, (a.sin()) as f32 * 20.0]
            });
            for index in 0..asteroid_cfg::VERTICES {
                let next = (index + 1) % asteroid_cfg::VERTICES;
                let facing = unit(
                    points[index][0] + points[next][0],
                    points[index][1] + points[next][1],
                );
                if facing[0] * KEY_LIGHT[0] + facing[1] * KEY_LIGHT[1] > RIM_CONE {
                    rim += 1;
                }
            }
            assert!(
                (1..=2).contains(&rim),
                "turn {turn} catches the rim on {rim} facets"
            );
        }
    }

    #[test]
    fn the_plume_holds_still_when_motion_is_off() {
        for time in [0.0, 0.25, 3.5, 91.0] {
            assert_eq!(flame_length(10.0, time, false), 10.0);
        }
        // With motion on it breathes, and stays inside its authored band.
        let lengths: Vec<f32> = (0..32)
            .map(|step| flame_length(10.0, step as f64 * 0.05, true))
            .collect();
        assert!(lengths.iter().all(|l| (7.6..=10.0).contains(l)));
        assert!(lengths.iter().any(|l| *l < 8.0));
        // Same time, same length: the flicker is a function of simulation time.
        assert_eq!(flame_length(10.0, 1.5, true), flame_length(10.0, 1.5, true));
    }

    #[test]
    fn the_tracer_fades_into_nothing_behind_the_bullet() {
        let mut p = Painter::new();
        luminous_streak(
            &mut p,
            [0.0, 0.0],
            [20.0, 0.0],
            TRACER_HALF_WIDTH,
            theme::color::BULLET.alpha(TRACER_HEAD_ALPHA),
            TRACER_SEGMENTS,
        );
        assert_eq!(p.luminous.len(), TRACER_SEGMENTS * 6);
        assert!(p.triangles.is_empty());
        let alphas: Vec<f32> = p.luminous.iter().map(|v| v.color[3]).collect();
        let head = alphas.iter().cloned().fold(f32::MIN, f32::max);
        let tail = alphas.iter().cloned().fold(f32::MAX, f32::min);
        assert!((head - TRACER_HEAD_ALPHA).abs() < 1e-6);
        assert_eq!(tail, 0.0);

        // A bullet at rest drags nothing behind it.
        p.clear();
        luminous_streak(
            &mut p,
            [4.0, 4.0],
            [4.0, 4.0],
            TRACER_HALF_WIDTH,
            theme::color::BULLET.alpha(TRACER_HEAD_ALPHA),
            TRACER_SEGMENTS,
        );
        assert!(p.luminous.is_empty());
    }

    /// A World with the Ship at a known place, facing along +x, with a chosen
    /// Sensorium written into it. Nothing here steps the World.
    fn posed(x: f64, y: f64, inputs: &[(usize, f64)]) -> World {
        let mut world = World::new(sim::Rng::from_seed(11), None);
        world.agent.ship.x = x;
        world.agent.ship.y = y;
        world.agent.ship.heading = 0.0;
        world.time = 1.0;
        for (index, value) in inputs {
            world.agent.inputs[*index] = *value;
        }
        world
    }

    #[test]
    fn the_corona_shows_what_the_sensorium_holds_and_nothing_else() {
        let view = ArenaView {
            origin: [0.0, 0.0],
            scale: 1.0,
        };

        // Nothing sensed: nine ticks mark the slots, and no wedge claims a
        // reading that was never taken.
        let quiet = posed(480.0, 300.0, &[]);
        let mut p = Painter::new();
        p.set_transform(view.transform());
        draw_corona(&mut p, &quiet.agent.ship, &quiet.agent.inputs);
        assert!(!p.triangles.is_empty(), "the slots are marked");
        assert!(p.luminous.is_empty(), "an empty Sensorium lit a wedge");

        // One ray closed: exactly one wedge, and it points where that ray does.
        // Ray 3 is +80 degrees from the heading, which with the Ship facing +x
        // and screen y running down is below and ahead of it.
        let sensing = posed(480.0, 300.0, &[(3, 0.9)]);
        p.clear();
        p.set_transform(view.transform());
        draw_corona(&mut p, &sensing.agent.ship, &sensing.agent.inputs);
        assert!(!p.luminous.is_empty());
        let bearing = 80.0_f32.to_radians();
        for vertex in &p.luminous {
            let angle = (vertex.pos[1] - 300.0).atan2(vertex.pos[0] - 480.0);
            let off = (angle - bearing).abs();
            assert!(
                off <= (CORONA_HALF_ANGLE + 1.0).to_radians(),
                "a wedge at {angle} is not on the ray at {bearing}"
            );
        }
        // And the ring is dented inward where the reading is: a closing threat
        // pulls its arc toward the hull, away from the rest radius the eight
        // quiet slots are still holding.
        let radius = ship_cfg::RADIUS as f32;
        let lit_ring = p
            .luminous
            .iter()
            .map(|v| (v.pos[0] - 480.0).hypot(v.pos[1] - 300.0))
            .fold(f32::MIN, f32::max);
        let quiet_ring = p
            .triangles
            .iter()
            .map(|v| (v.pos[0] - 480.0).hypot(v.pos[1] - 300.0))
            .fold(f32::MIN, f32::max);
        assert!(
            lit_ring < quiet_ring,
            "a reading of 0.9 did not dent the ring: {lit_ring} vs {quiet_ring}"
        );
        assert!(lit_ring >= (CORONA_NEAR * radius) - CORONA_TICK - 1.0);
        assert!(quiet_ring <= CORONA_FAR * radius + 1.0);

        // Halfway in is halfway between the two radii, so the ring is a
        // reading and not merely an alarm.
        let halfway = posed(480.0, 300.0, &[(3, 0.5)]);
        p.clear();
        p.set_transform(view.transform());
        draw_corona(&mut p, &halfway.agent.ship, &halfway.agent.inputs);
        let middle = p
            .luminous
            .iter()
            .map(|v| (v.pos[0] - 480.0).hypot(v.pos[1] - 300.0))
            .fold(f32::MIN, f32::max);
        let expected = (CORONA_FAR + CORONA_NEAR) * 0.5 * radius;
        assert!(
            (middle - expected).abs() < 1.5,
            "a reading of 0.5 sat at {middle}, not {expected}"
        );
    }

    #[test]
    fn the_corona_crosses_the_seam_with_the_ship() {
        let view = ArenaView {
            origin: [0.0, 0.0],
            scale: 1.0,
        };
        // A Ship on the right edge wears its corona on both sides of the seam,
        // the same way its hull is drawn on both sides.
        let world = posed(W as f64 - 4.0, 300.0, &[(0, 0.8)]);
        let mut p = Painter::new();
        p.set_transform(view.transform());
        p.set_clip(Some([0.0, 0.0, W, H]));
        draw_corona(&mut p, &world.agent.ship, &world.agent.inputs);
        assert!(p.triangles.iter().any(|v| v.pos[0] > W - 10.0));
        assert!(p.triangles.iter().any(|v| v.pos[0] < 40.0));
        assert!(p
            .triangles
            .iter()
            .chain(p.luminous.iter())
            .all(|v| v.pos[0] >= 0.0 && v.pos[0] <= W));
    }

    #[test]
    fn the_intent_ring_reads_the_activations_the_network_produced() {
        let mut rng = sim::Rng::from_seed(7);
        let genome = sim::Genome::new(&mut rng, &mut sim::InnovationTracker::new());
        let mut network = sim::Network::from_genome(&genome);
        let mut inputs = [0.0; sim::config::nn::INPUTS];
        inputs[sim::config::nn::BIAS_INPUT] = 1.0;
        let outputs = network.activate(&inputs);
        let world = posed(480.0, 300.0, &[]);

        let view = ArenaView {
            origin: [0.0, 0.0],
            scale: 1.0,
        };
        let mut p = Painter::new();
        p.set_transform(view.transform());
        draw_intent(&mut p, &world.agent.ship, &network);

        // Every arc is drawn, so a shut request reads as shut rather than as
        // absent; only the ones over the threshold carry light.
        assert!(!p.triangles.is_empty());
        let requested = outputs
            .iter()
            .take(4)
            .filter(|value| **value > sim::config::nn::ACTION_THRESHOLD)
            .count();
        assert_eq!(
            p.luminous.is_empty(),
            requested == 0,
            "light and request disagree: {outputs:?}"
        );
        // The ring sits off the hull, clear of the corona inside it.
        let radius = ship_cfg::RADIUS as f32;
        let nearest = p
            .triangles
            .iter()
            .map(|v| (v.pos[0] - 480.0).hypot(v.pos[1] - 300.0))
            .fold(f32::MAX, f32::min);
        assert!(nearest > CORONA_FAR * radius);
    }

    #[test]
    fn a_ray_is_drawn_only_where_it_measured_something() {
        let view = ArenaView {
            origin: [0.0, 0.0],
            scale: 1.0,
        };
        let quiet = posed(480.0, 300.0, &[]);
        let mut p = Painter::new();
        p.set_transform(view.transform());
        draw_rays(&mut p, &quiet.agent.ship, &quiet.agent.inputs);
        assert!(p.is_empty(), "nine 'nothing there's were drawn as lines");

        // A reading of 0.5 is half the sensor range away, and the streak ends
        // there rather than running on to the range limit.
        let sensing = posed(480.0, 300.0, &[(0, 0.5)]);
        p.clear();
        p.set_transform(view.transform());
        p.set_clip(Some([0.0, 0.0, W, H]));
        draw_rays(&mut p, &sensing.agent.ship, &sensing.agent.inputs);
        let reach = p
            .luminous
            .iter()
            .map(|v| v.pos[0] - 480.0)
            .fold(f32::MIN, f32::max);
        let expected = 0.5 * sensors::RANGE as f32;
        assert!(
            (reach - expected).abs() < RAY_TIP_RADIUS + 1.0,
            "the ray ended at {reach}, not the {expected} it measured"
        );
    }

    #[test]
    fn the_sky_stays_clear_of_the_frame_the_golden_image_samples() {
        // The golden frame samples this pixel as empty field. The backdrop
        // shader owns what is under it now, but nothing drawn here may sit on
        // it, or the sample stops meaning "empty field".
        const FLOOR: [f32; 2] = [475.0, 20.0];
        for star in SKY.far.iter().chain(SKY.mid.iter()) {
            let covered = FLOOR[0] >= star.x
                && FLOOR[0] <= star.x + star.size
                && FLOOR[1] >= star.y
                && FLOOR[1] <= star.y + star.size;
            assert!(!covered, "a star sits on the empty floor at {FLOOR:?}");
        }
        for star in &SKY.near {
            let reach = star.arm + 1.0;
            assert!(
                (FLOOR[0] - star.x).abs() > reach || (FLOOR[1] - star.y).abs() > reach,
                "a sparkle crosses the empty floor at {FLOOR:?}"
            );
        }
    }
}
