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
//! The sky is a volume the Ship moves through, not wallpaper behind it. Its
//! four layers — far dust, the mid field, the near stars that sparkle, and the
//! motes of dust closest to the hull — are laid out once from fixed seeds and
//! then displaced against the Ship's own velocity in proportion to their depth,
//! each one wrapped about the torus so the field never runs out of sky. The same
//! depth decides how much of the Arena's tremor a layer takes: the near ones
//! take nearly all of it and the far one nearly none, because a jolt that moves
//! a nebula as much as a rock three metres away is a camera trick, not depth.
//! The subject plane — rocks, hull, shots and the light over them — always takes
//! the whole jolt: it is the thing the shake is about.
//!
//! The backdrop the shader paints is deliberately still on both counts. A
//! nebula is at infinity, so no parallax and no tremor reaches it, and a still
//! backdrop under a shaken subject plane is exactly what parallax means.
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
    /// The displacement the Arena is drawn with, in Arena units, published by
    /// the effects pass when the field is hit. Zero until then — a golden
    /// frame, a probe and a paused frame are still.
    pub tremor: [f32; 2],
}

impl ArenaView {
    /// The transform the Painter needs to map logical Arena units into the window.
    ///
    /// The view's `tremor` is folded in here — an offset of a few Arena units,
    /// published by the effects pass when the field is hit — because this is
    /// the one place the *subject plane's* transform is built: the rocks, the
    /// hull, the shots, the trail, the light over them and the field's own
    /// measure take the shake together, and nothing outside it does. The sky
    /// deliberately does not: it is a volume with depth, so each of its layers
    /// takes its own share of the jolt instead (`sky_transform`). It is
    /// scaled like the rest of the Arena, so a jolt is the same fraction of
    /// the field at every window size.
    pub fn transform(&self) -> Transform {
        Transform::new(
            self.scale,
            [
                self.origin[0] + self.tremor[0] * self.scale,
                self.origin[1] + self.tremor[1] * self.scale,
            ],
        )
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

/// A mote of near dust: the layer closest to the Ship, and the smallest thing
/// in the field. It is drawn as light rather than as matter, because a mote has
/// no shape of its own — what is visible is the key light it catches, and only
/// where the field behind it is dark.
#[derive(Clone, Copy, Debug)]
struct Mote {
    x: f32,
    y: f32,
    size: f32,
    ink: Rgba,
}

/// The sky is four depth layers: far dust, a mid field, a handful of near stars
/// that sparkle, and the motes closest to the hull. Counts are fixed, not
/// density-scaled: the field is part of the golden frame and must not depend on
/// anything but these constants.
const STAR_FAR_COUNT: usize = 110;
const STAR_MID_COUNT: usize = 36;
const STAR_NEAR_COUNT: usize = 10;
/// Enough motes that the nearest layer reads as a volume, few enough that it
/// stays dust: at any moment only a handful sit where the field is dark enough
/// to show them.
const MOTE_COUNT: usize = 32;
/// One seed per layer, so re-laying one layer leaves the others where they are.
const STAR_FAR_SEED: u32 = 0x5EED_2024;
const STAR_MID_SEED: u32 = 0x5EED_5A17;
const STAR_NEAR_SEED: u32 = 0x5EED_9E37;
const MOTE_SEED: u32 = 0x5EED_D057;
/// Dim end of each layer's brightness spread; the bright end is this plus that
/// layer's range. The nearer the layer, the brighter it runs.
const STAR_FAR_MIN_ALPHA: f32 = 0.12;
const STAR_FAR_ALPHA_RANGE: f32 = 0.15;
const STAR_MID_MIN_ALPHA: f32 = 0.22;
const STAR_MID_ALPHA_RANGE: f32 = 0.20;
/// Half the arm span of a near star's sparkle, in logical units.
const SPARKLE_MIN_ARM: f32 = 2.0;
const SPARKLE_ARM_RANGE: f32 = 1.0;
/// A mote is 0.6–1.2 Arena units across: a speck of dust, not snow. Anything
/// larger stops reading as a thing the Ship passes and starts reading as a
/// second starfield in front of the first.
const MOTE_MIN_SIZE: f32 = 0.6;
const MOTE_SIZE_RANGE: f32 = 0.6;
/// How much of the key light a mote catches, at [`theme::light::DUST`]'s gain
/// over it. The brightest mote is still dimmer than the dimmest star behind it
/// (a far field star starts at [`STAR_FAR_MIN_ALPHA`]), which is the whole
/// point: dust registers as a speck over empty field and disappears over
/// anything bright, so it can say "there is air here" without becoming
/// confetti.
const MOTE_MIN_COVERAGE: f32 = 0.05;
const MOTE_COVERAGE_RANGE: f32 = 0.06;

// ------------------------------------------------------------ the depth

/// How much of its own motion the Ship's field lags by, in seconds: the sky is
/// displaced by the distance the Ship would cover in this time, at the 320
/// Arena units per second it is capped to (`ship::MAX_SPEED`). At full tilt the
/// nearest layer leans 41 units — about four per cent of the field — and a slow
/// drift leans a couple. That is the size at which the layers separate without
/// the field sliding out from under the rocks; a larger number turns a chase
/// into a starfield on rails.
const TRAIL_SECONDS: f32 = 0.15;

/// One depth per layer: the fraction of the Ship's own displacement it takes,
/// and at the same time the fraction of the Arena's tremor it takes. The two
/// are the same fact — how close the layer is — so they are one number, and the
/// gaps between them are wide enough that the four layers separate at a glance.
/// The subject plane is not in this table: it takes the whole of both.
const PARALLAX_FAR_DEPTH: f32 = 0.10;
const PARALLAX_MID_DEPTH: f32 = 0.25;
const PARALLAX_NEAR_DEPTH: f32 = 0.52;
const PARALLAX_DUST_DEPTH: f32 = 0.85;

/// The displacement one sky layer takes from the Ship's own motion, in Arena
/// units: the negative of the Ship's velocity over [`TRAIL_SECONDS`], scaled by
/// the layer's depth. The field gives way as the Ship moves through it, and it
/// answers the velocity itself — nothing is integrated and no history is kept,
/// so a Ship that stops puts the sky back exactly where it was, and a paused
/// frame is the frame before it. Reduced motion is a still field: the
/// displacement is exactly zero, which is what lets a golden frame be a pure
/// function of the World.
fn parallax(ship: &Ship, depth: f32, motion: bool) -> [f32; 2] {
    if !motion {
        return [0.0, 0.0];
    }
    [
        -(ship.vx as f32) * TRAIL_SECONDS * depth,
        -(ship.vy as f32) * TRAIL_SECONDS * depth,
    ]
}

/// Where one sky element lands once its layer has been displaced: its laid
/// position plus the layer's offset, brought back into the field.
///
/// The sky is laid over the torus, so an element pushed past an edge is already
/// arriving at the other one — at any displacement the field is as full as it
/// was laid, with no empty band where the sky slid out from under its own clip.
#[inline]
fn sky_point(x: f32, y: f32, offset: [f32; 2]) -> [f32; 2] {
    [(x + offset[0]).rem_euclid(W), (y + offset[1]).rem_euclid(H)]
}

/// The transform for one sky layer: the Arena's own placement, carrying the
/// layer's depth-weighted share of the tremor instead of the whole jolt.
///
/// Built from the view's public fields rather than by changing
/// [`ArenaView::transform`], which is the subject plane's and stays that. The
/// same `depth` that decides a layer's parallax decides its shake, because both
/// are the same question — how far away the layer is — and a nebula that shook
/// like a rock would flatten the field this pass exists to deepen.
fn sky_transform(view: &ArenaView, depth: f32) -> Transform {
    Transform::new(
        view.scale,
        [
            view.origin[0] + view.tremor[0] * depth * view.scale,
            view.origin[1] + view.tremor[1] * depth * view.scale,
        ],
    )
}

/// The field's rect in a sky layer's own space.
///
/// The Painter resolves a clip through whatever transform is current, and a
/// layer's transform carries less of the tremor than the subject plane's does.
/// Handing the field's rect over untranslated would therefore clip each layer
/// to its own, less-shaken copy of the field and let the sky hang a few units
/// outside the Arena the rocks are in. So the share the layer did *not* take is
/// added back here, and every layer — and the subject plane — is clipped to
/// exactly one rect: the field's.
fn sky_clip(view: &ArenaView, depth: f32) -> [f32; 4] {
    let x = view.tremor[0] * (1.0 - depth);
    let y = view.tremor[1] * (1.0 - depth);
    [x, y, W + x, H + y]
}

/// The whole sky, laid out once per process and shared by every frame.
#[derive(Debug)]
struct Sky {
    far: Vec<Star>,
    mid: Vec<Star>,
    near: Vec<Sparkle>,
    motes: Vec<Mote>,
}

static SKY: LazyLock<Sky> = LazyLock::new(build_sky);

/// Lay out the sky from the four layer seeds.
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
        motes: build_motes(),
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

/// The nearest layer: motes of dust, each with its own size and its own share
/// of the key light. The ink is resolved here rather than per frame because a
/// mote's colour is a constant of the layout, like a star's.
fn build_motes() -> Vec<Mote> {
    let mut seed = MOTE_SEED;
    let mut layer = Vec::with_capacity(MOTE_COUNT);
    for _ in 0..MOTE_COUNT {
        let coverage = MOTE_MIN_COVERAGE + lcg_unit(&mut seed) * MOTE_COVERAGE_RANGE;
        layer.push(Mote {
            x: lcg_unit(&mut seed) * W,
            y: lcg_unit(&mut seed) * H,
            size: MOTE_MIN_SIZE + lcg_unit(&mut seed) * MOTE_SIZE_RANGE,
            ink: theme::color::KEY.alpha(coverage),
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

/// Draw the field: its sky, its measure, its edge, the entities and the
/// tracking overlay, all in the Arena's own 960×600 units.
///
/// The Painter is expected to be in window units when this is called — that is
/// the scale the readings anchored in the field are measured against — and it
/// is left that way.
pub fn draw_arena(
    painter: &mut Painter,
    world: &World,
    view: ArenaView,
    show_rays: bool,
    motion: bool,
) {
    // The window's scale, which the Arena's transform is about to replace:
    // what an Arena unit is worth in window units is the ratio between them.
    let window_scale = painter.transform().scale;
    let fit = if window_scale > 0.0 {
        view.scale / window_scale
    } else {
        view.scale
    };
    // Everything below is in the field's own space, and the scope puts the
    // Painter back the way it found it: entering the Arena is one call, and so
    // is leaving it.
    painter.arena_scope(view.transform(), [0.0, 0.0, W, H], |painter| {
        // The wash, the nebulae and the frame's falloff are already on the target:
        // the backdrop shader painted them before any of this was submitted. What
        // is left is what sits *in* the field — its sky, its measure, its edge.
        // The sky is the one thing here that is not at the subject plane's
        // distance, so it takes the view and the Ship's own motion: its layers
        // displace and shake by their depth.
        draw_stars(painter, &view, &world.agent.ship, motion);
        draw_grid(painter);
        draw_field_edge(painter);

        let agent = &world.agent;
        // The near boundary goes down before the rays and under everything
        // solid, so the rocks it is measured against are drawn over it.
        if agent.alive && world.time > 0.0 {
            draw_envelope(painter, &agent.ship, &agent.inputs, fit);
        }
        // The rays follow: they are translucent, and reading them through the
        // asteroids is the point of the overlay.
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
        draw_tracking(painter, world, fit);
    });
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

/// Draw the sky, one layer at a time, each displaced by its own depth.
///
/// Each layer gets its own scope because each carries its own share of the
/// tremor, and its own offset because each leans a different amount against the
/// Ship's motion. The far layer is drawn first and the dust last, so the nearer
/// a layer is, the later it lands: the sparkle crosses the star behind it, and
/// the motes — light, so they composite over every material anyway — are the
/// closest thing to the hull.
fn draw_stars(painter: &mut Painter, view: &ArenaView, ship: &Ship, motion: bool) {
    let far = parallax(ship, PARALLAX_FAR_DEPTH, motion);
    painter.arena_scope(
        sky_transform(view, PARALLAX_FAR_DEPTH),
        sky_clip(view, PARALLAX_FAR_DEPTH),
        |painter| {
            for star in &SKY.far {
                let [x, y] = sky_point(star.x, star.y, far);
                painter.rect(x, y, star.size, star.size, star.color);
            }
        },
    );

    let mid = parallax(ship, PARALLAX_MID_DEPTH, motion);
    painter.arena_scope(
        sky_transform(view, PARALLAX_MID_DEPTH),
        sky_clip(view, PARALLAX_MID_DEPTH),
        |painter| {
            for star in &SKY.mid {
                let [x, y] = sky_point(star.x, star.y, mid);
                painter.rect(x, y, star.size, star.size, star.color);
            }
        },
    );

    // The near layer crosses its own dot, which is what makes it a sparkle.
    let near = parallax(ship, PARALLAX_NEAR_DEPTH, motion);
    painter.arena_scope(
        sky_transform(view, PARALLAX_NEAR_DEPTH),
        sky_clip(view, PARALLAX_NEAR_DEPTH),
        |painter| {
            for star in &SKY.near {
                let [x, y] = sky_point(star.x, star.y, near);
                let ink = star.color.alpha(0.45);
                painter.stroke([x - star.arm, y], [x + star.arm, y], 0.7, ink);
                painter.stroke([x, y - star.arm], [x, y + star.arm], 0.7, ink);
                painter.rect(x - 1.0, y - 1.0, 2.0, 2.0, star.color);
            }
        },
    );

    // The dust, and the only layer drawn as light: a mote has no shape of its
    // own, and what is visible is the key light it catches. The gain is lifted
    // for it and put back as it was found — the Painter outlives this call.
    let dust = parallax(ship, PARALLAX_DUST_DEPTH, motion);
    painter.arena_scope(
        sky_transform(view, PARALLAX_DUST_DEPTH),
        sky_clip(view, PARALLAX_DUST_DEPTH),
        |painter| {
            let gain = painter.gain();
            painter.set_gain(theme::light::DUST);
            for mote in &SKY.motes {
                let [x, y] = sky_point(mote.x, mote.y, dust);
                luminous_rect(
                    painter,
                    x - mote.size * 0.5,
                    y - mote.size * 0.5,
                    mote.size,
                    mote.size,
                    mote.ink,
                );
            }
            painter.set_gain(gain);
        },
    );
}

#[cfg(test)]
mod parallax_tests {
    use super::*;

    /// A Ship moving at a given velocity, in Arena units per second.
    fn moving(vx: f64, vy: f64) -> Ship {
        let mut world = World::new(sim::Rng::from_seed(3), None);
        world.agent.ship.vx = vx;
        world.agent.ship.vy = vy;
        world.agent.ship
    }

    fn reach(offset: [f32; 2]) -> f32 {
        offset[0].hypot(offset[1])
    }

    /// Whether two points are the same place. Adding a whole Arena to a
    /// coordinate and taking it back is not exact arithmetic in f32, but it is
    /// exact geometry — a hundredth of an Arena unit is a hundredth of a pixel
    /// at 1×, which is nowhere.
    fn same(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-2 && (a[1] - b[1]).abs() < 1e-2
    }

    #[test]
    fn a_still_frame_leaves_the_sky_where_it_was_laid() {
        // Reduced motion, and a Ship that has not moved: every layer sits at
        // exactly zero, so the sky a golden frame draws is the layout and
        // nothing else.
        let still = moving(0.0, 0.0);
        let flying = moving(180.0, -240.0);
        for depth in [
            PARALLAX_FAR_DEPTH,
            PARALLAX_MID_DEPTH,
            PARALLAX_NEAR_DEPTH,
            PARALLAX_DUST_DEPTH,
        ] {
            assert_eq!(parallax(&still, depth, true), [0.0, 0.0]);
            assert_eq!(parallax(&flying, depth, false), [0.0, 0.0]);
            // And the layer the backdrop lives on — no depth — never moves at
            // all, moving Ship or not.
            assert_eq!(parallax(&flying, 0.0, true), [0.0, 0.0]);
        }
        // A zero offset is the layout: every element lands where it was laid.
        assert_eq!(sky_point(12.5, 40.25, [0.0, 0.0]), [12.5, 40.25]);
    }

    #[test]
    fn the_layers_lean_against_the_ships_own_motion_by_depth() {
        let ship = moving(200.0, -100.0);
        let dust = parallax(&ship, PARALLAX_DUST_DEPTH, true);
        let near = parallax(&ship, PARALLAX_NEAR_DEPTH, true);
        let mid = parallax(&ship, PARALLAX_MID_DEPTH, true);
        let far = parallax(&ship, PARALLAX_FAR_DEPTH, true);

        // The field gives way backwards: a Ship moving +x and -y sees the sky
        // come toward it on x and fall behind on y.
        assert!(dust[0] < 0.0 && dust[1] > 0.0);
        // The whole effect is this ordering, and it is strict: closer layers
        // move more, and the farthest still moves.
        assert!(reach(dust) > reach(near));
        assert!(reach(near) > reach(mid));
        assert!(reach(mid) > reach(far));
        assert!(reach(far) > 0.0);
        // Half the speed, half the lean: the displacement is a function of the
        // velocity, not an accumulation of it.
        let slow = parallax(&moving(100.0, -50.0), PARALLAX_DUST_DEPTH, true);
        assert!((slow[0] - dust[0] * 0.5).abs() < 1e-4);
        assert!((slow[1] - dust[1] * 0.5).abs() < 1e-4);
        // At the Ship's own cap the nearest layer trails a few per cent of the
        // field: a volume, not a starfield on rails.
        let capped = parallax(&moving(ship_cfg::MAX_SPEED, 0.0), PARALLAX_DUST_DEPTH, true);
        assert!(reach(capped) < W * 0.05);
        assert!(reach(capped) > W * 0.02);

        // The same depth decides how much of the tremor a layer takes: the
        // nebula is nearly still under a jolt that moves the subject plane
        // whole, and nothing but the subject plane takes all of it.
        let view = ArenaView {
            origin: [40.0, 60.0],
            scale: 1.5,
            tremor: [4.0, -2.0],
        };
        let shift = |depth: f32| sky_transform(&view, depth).offset;
        assert_eq!(sky_transform(&view, PARALLAX_FAR_DEPTH).scale, view.scale);
        assert_eq!(shift(0.0), view.origin);
        let subject = view.transform().offset;
        let weights = [
            PARALLAX_FAR_DEPTH,
            PARALLAX_MID_DEPTH,
            PARALLAX_NEAR_DEPTH,
            PARALLAX_DUST_DEPTH,
        ];
        for pair in weights.windows(2) {
            assert!(shift(pair[0])[0] < shift(pair[1])[0]);
            assert!(shift(pair[0])[1] > shift(pair[1])[1]);
        }
        assert!(shift(PARALLAX_DUST_DEPTH)[0] < subject[0]);

        // The layers take different shares of the jolt but share one clip: a
        // layer's rect in its own space lands on exactly the field's rect in
        // the window, so the sky is clipped to the field and not to its own,
        // differently-shaken copy of it.
        let projected = |transform: Transform, rect: [f32; 4]| {
            let near = transform.apply([rect[0], rect[1]]);
            let far = transform.apply([rect[2], rect[3]]);
            [near[0], near[1], far[0], far[1]]
        };
        let field = projected(view.transform(), [0.0, 0.0, W, H]);
        for depth in weights {
            let layer = projected(sky_transform(&view, depth), sky_clip(&view, depth));
            for (side, corner) in layer.iter().zip(field.iter()) {
                assert!((side - corner).abs() < 1e-3, "{layer:?} is not {field:?}");
            }
        }
    }

    #[test]
    fn a_displaced_layer_wraps_about_the_torus() {
        // Moving the layer by a whole Arena is moving it not at all: this is
        // what makes a displacement of any size a shift on the torus rather
        // than a slide out of the frame.
        for (x, y) in [(0.0, 0.0), (W - 0.25, 12.0), (480.0, H - 0.25)] {
            assert!(same(sky_point(x, y, [W, H]), sky_point(x, y, [0.0, 0.0])));
            assert!(same(sky_point(x, y, [-W, -H]), sky_point(x, y, [0.0, 0.0])));
            // Which is the same statement as: an element pushed past an edge is
            // the element on the other side of the seam.
            assert!(same(
                sky_point(x - W, y, [0.0, 0.0]),
                sky_point(x, y, [0.0, 0.0])
            ));
        }

        // However hard the field is displaced, every element of every layer
        // lands inside the field: the displacement is a translation of the
        // torus, so a neighbour stays a neighbour and nothing is ever dropped.
        let ship = moving(ship_cfg::MAX_SPEED, -ship_cfg::MAX_SPEED);
        let sealed = |x: f32, y: f32, offset: [f32; 2]| {
            let point = sky_point(x, y, offset);
            assert!((0.0..W).contains(&point[0]) && (0.0..H).contains(&point[1]));
            // The element on the other side of the seam is the same element.
            assert!(same(sky_point(x + W, y + H, offset), point));
        };
        for star in SKY.far.iter().chain(SKY.mid.iter()) {
            sealed(star.x, star.y, parallax(&ship, PARALLAX_FAR_DEPTH, true));
        }
        for star in &SKY.near {
            sealed(star.x, star.y, parallax(&ship, PARALLAX_NEAR_DEPTH, true));
        }
        for mote in &SKY.motes {
            sealed(mote.x, mote.y, parallax(&ship, PARALLAX_DUST_DEPTH, true));
        }
    }

    #[test]
    fn the_newest_layer_is_the_nearest_and_the_ground_is_clear_of_it() {
        // The four layers are laid out in depth order and the motes come last:
        // they are the layer the Ship flies through.
        assert_eq!(SKY.motes.len(), MOTE_COUNT);
        let again = build_sky();
        assert_eq!(
            SKY.motes
                .iter()
                .map(|m| (m.x, m.y, m.size, m.ink.a))
                .collect::<Vec<_>>(),
            again
                .motes
                .iter()
                .map(|m| (m.x, m.y, m.size, m.ink.a))
                .collect::<Vec<_>>()
        );
        for mote in &SKY.motes {
            assert!((0.0..W).contains(&mote.x) && (0.0..H).contains(&mote.y));
            assert!((MOTE_MIN_SIZE..=MOTE_MIN_SIZE + MOTE_SIZE_RANGE).contains(&mote.size));
            assert!(
                (MOTE_MIN_COVERAGE..=MOTE_MIN_COVERAGE + MOTE_COVERAGE_RANGE).contains(&mote.ink.a)
            );
            // Dimmer than the dimmest star behind it, and nowhere near the
            // bloom threshold: dust is not a light source, it is a thing the
            // light falls on.
            assert!(mote.ink.a < STAR_FAR_MIN_ALPHA);
        }
        // The newest layer is also the nearest: it displaces past every star
        // layer, at any velocity.
        let ship = moving(300.0, 300.0);
        let dust = reach(parallax(&ship, PARALLAX_DUST_DEPTH, true));
        for star in [PARALLAX_FAR_DEPTH, PARALLAX_MID_DEPTH, PARALLAX_NEAR_DEPTH] {
            assert!(dust > reach(parallax(&ship, star, true)));
        }

        // The empty floor the golden image samples stays empty: the dust is the
        // one layer laid after that test was written, so it is checked here.
        const FLOOR: [f32; 2] = [475.0, 20.0];
        for mote in &SKY.motes {
            let half = mote.size * 0.5;
            assert!(
                (FLOOR[0] - mote.x).abs() > half || (FLOOR[1] - mote.y).abs() > half,
                "a mote sits on the empty floor at {FLOOR:?}"
            );
        }
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

// --------------------------------------------------------------- free space

/// The nine ray indices in bearing order. The Sensorium hands its readings over
/// in the order 0°, +40°, −40°, +80°, −80°, +120°, −120°, +160°, −160°, so
/// walking them in index order would visit the bearings out of order and cross
/// the boundary over itself. Walking them from −160° round to +160° is the order
/// the boundary actually has: one entry per ray, and the table is the whole of
/// the count.
///
/// Written out rather than sorted at run time, because `RAY_OFFSETS_DEG` is the
/// simulation's table and this is only its sort.
const ENVELOPE_ORDER: [usize; 9] = [8, 6, 4, 2, 0, 1, 3, 5, 7];

/// The chords between neighbouring samples, as the drawing's one luminous
/// accent: where the field is dark, the boundary of the near space is lit. The
/// gain is `theme::light::ENVELOPE`, below the Ray overlay's on purpose — this
/// is the shape *under* the readings, not another reading.
///
/// The width and the dots are sized to carry the instrument on their own. These
/// were first set for a closed polygon that had a fill doing most of the work;
/// when the fill went — the fill was asserting walls across bearings that had
/// found nothing — the line and the dots became the whole drawing and were left
/// too quiet to see. Measured on the rendered frame against the Ray overlay
/// beside it: at these numbers the boundary reads as its own instrument, and
/// below roughly two-thirds of them it reads as nothing at all.
const ENVELOPE_EDGE_WIDTH: f32 = 2.0;
const ENVELOPE_EDGE_ALPHA: f32 = 0.85;
/// A dot on every sample, in the reading's own ink. The bearings are nine
/// discrete measurements, and the dot is what says where the measurement is and
/// that the line beside it is the instrument's own join.
const ENVELOPE_VERTEX_RADIUS: f32 = 3.0;
const ENVELOPE_VERTEX_ALPHA: f32 = 0.95;
/// The caption, and the other half of the same truth: the boundary is drawn
/// from bearings that found something, and the dots are those bearings. It is
/// the field's reading size like the tracking overlay's, run through
/// [`arena_text`] so it holds its window size at any scale.
const ENVELOPE_CAPTION: &str = "NEAR BOUNDARY · SAMPLED BEARINGS";
/// How far below the hull the caption sits, in Arena units: clear of the intent
/// ring, which reaches 4.6 Ship radii, and of the velocity chevron at 62.
const ENVELOPE_CAPTION_DROP: f32 = 96.0;
/// How far in from the field's edge the caption is kept. It is centred on the
/// hull, and a hull at a seam would take half of it off the field — half a
/// caption is not a reading.
const ENVELOPE_CAPTION_MARGIN: f32 = 150.0;

/// What each of the nine bearings found, in bearing order: the point on that
/// bearing's measured edge, relative to the hull, or `None` when the bearing
/// found nothing inside the range.
///
/// `inputs[k]` is the proximity reading on ray `k`: 1.0 is a hit at zero
/// distance and 0.0 is nothing inside `sensors::RANGE`, so the distance is its
/// complement. A bearing that found nothing has no distance to report — all it
/// says is "not inside five hundred units" — and the honest drawing of that is
/// nothing at all. This is the whole difference between this instrument and a
/// sweep: nine bearings either found an edge or they did not, and a boundary
/// that is not there is not drawn.
///
/// It is the same reading, on the same bearings, from the same point on the
/// hull that the Ray overlay draws.
fn envelope_samples(ship: &Ship, inputs: &[f64]) -> [Option<[f32; 2]>; ENVELOPE_ORDER.len()] {
    let mut samples = [None; ENVELOPE_ORDER.len()];
    for (slot, index) in ENVELOPE_ORDER.iter().enumerate() {
        let value = inputs.get(*index).copied().unwrap_or(0.0).clamp(0.0, 1.0) as f32;
        if value <= 0.0 {
            continue;
        }
        let distance = (1.0 - value) * sensors::RANGE as f32;
        let angle = ship.heading + sensors::RAY_OFFSETS_DEG[*index].to_radians();
        let angle = angle as f32;
        samples[slot] = Some([angle.cos() * distance, angle.sin() * distance]);
    }
    samples
}

/// Whether the boundary joins the sample at `slot` to the one beside it.
///
/// Only neighbours, and only when both found something. A chord drawn across a
/// bearing that read nothing would run through space the Sensorium never
/// sampled, and the nine bearings are discrete measurements — the one thing
/// this drawing must not do is invent an edge between two of them.
fn boundary_joins(samples: &[Option<[f32; 2]>], slot: usize) -> bool {
    matches!(
        (samples.get(slot), samples.get(slot + 1)),
        (Some(Some(_)), Some(Some(_)))
    )
}

/// The near boundary: where each bearing that found something met it, and the
/// joins between neighbouring ones.
///
/// This is the shape a viewer wants and the corona cannot give — the corona
/// answers "how close is it on bearing k" one arc at a time, and the boundary
/// answers "where is the wall". It is drawn as open chains rather than a closed
/// loop on purpose: the bearings are samples, not a sweep, so a chord across
/// one that found nothing would be an edge that does not exist. What is left is
/// only ever a measured point, or a join between two measured neighbours.
///
/// A reading and not an event: `motion` does not touch it, and a paused frame
/// draws exactly the frame before it. It sits under the rocks and under the
/// hull, and its one lit line runs under the corona's arcs. With nothing found
/// on any bearing it draws nothing at all — not even the caption, because there
/// is no instrument to name.
fn draw_envelope(painter: &mut Painter, ship: &Ship, inputs: &[f64], fit: f32) {
    use crate::painter::Align;

    let samples = envelope_samples(ship, inputs);
    if samples.iter().all(Option::is_none) {
        return;
    }

    let edge = theme::color::ACCENT.alpha(ENVELOPE_EDGE_ALPHA);
    let mark = theme::color::ACCENT.alpha(ENVELOPE_VERTEX_ALPHA);

    // One gain for the whole drawing, put back as it was found: the gain is
    // frame state, and the Painter outlives this call.
    let gain = painter.gain();
    painter.set_gain(theme::light::ENVELOPE);
    seam_copies(
        painter,
        ship.x,
        ship.y,
        // The copy set comes from the range limit and not from the readings, so
        // a bearing closing never changes how many copies are drawn. Points off
        // the field are the clip's business, not this loop's.
        sensors::RANGE + 4.0,
        |painter, cx, cy| {
            let placed = |point: [f32; 2]| [point[0] + cx, point[1] + cy];
            for slot in 0..samples.len() {
                if boundary_joins(&samples, slot) {
                    crate::effects::light_stroke(
                        painter,
                        placed(samples[slot].unwrap()),
                        placed(samples[slot + 1].unwrap()),
                        ENVELOPE_EDGE_WIDTH,
                        edge,
                    );
                }
            }
            for point in samples.iter().flatten() {
                painter.circle(placed(*point), ENVELOPE_VERTEX_RADIUS, mark, 6);
            }
        },
    );
    painter.set_gain(gain);

    // The caption, under the hull — over it when the hull is near the floor,
    // because a caption off the field is not a caption.
    let x = (ship.x as f32).clamp(ENVELOPE_CAPTION_MARGIN, W - ENVELOPE_CAPTION_MARGIN);
    let y = ship.y as f32;
    let y = if y > H - ENVELOPE_CAPTION_DROP - 24.0 {
        y - ENVELOPE_CAPTION_DROP
    } else {
        y + ENVELOPE_CAPTION_DROP
    };
    painter.text_aligned(
        [x, y],
        arena_text(fit, READOUT_SIZE),
        theme::color::ACCENT.alpha(0.7),
        Align::Center,
        ENVELOPE_CAPTION,
    );
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

/// The sizes the tracking overlay's readings are set in — the ruler numbers and
/// the two readout lines.
const RULER_SIZE: f32 = 8.0;
const READOUT_SIZE: f32 = 9.0;

/// The smallest a reading is drawn at, in the window units the chrome is laid
/// out in: the type is anchored in the field but read in the window, and a
/// reading below this stops being a reading. Window units, not device pixels,
/// so the same window shows the same size at 1× and on a retina display.
const READOUT_MIN: f32 = 9.5;

/// How much larger than authored an Arena-space reading is allowed to grow as
/// the field is magnified: the type holds its size, and a big window does not
/// need its rulers to grow without limit.
const READOUT_HEADROOM: f32 = 2.0;

/// The size to hand the Painter for text anchored in Arena space, where `fit`
/// is how many window units one Arena unit covers.
///
/// The Painter multiplies a text size by the transform's scale, so a ruler
/// label set at eight would be four window pixels on a half-size field and
/// sixteen on a doubled one — neither of which is the size it was set at. The
/// fit is divided back out, which is what makes the reading hold its window
/// size: floored, so a shrunken field is still labelled legibly, and capped at
/// twice what was authored, so the rulers do not become headlines. Only the
/// type stops scaling; the anchor still travels with the field.
fn arena_text(fit: f32, size: f32) -> f32 {
    if fit <= 0.0 {
        return size;
    }
    (size * fit).max(READOUT_MIN).min(size * READOUT_HEADROOM) / fit
}

/// A geometric navigation overlay, independent of the Network's sampled inputs.
///
/// `fit` is how many window units an Arena unit covers — the Arena's scale with
/// the display's own scale taken back out — which is what the overlay's
/// readings are set in rather than the field's units.
fn draw_tracking(p: &mut Painter, world: &World, fit: f32) {
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
    let ruler = arena_text(fit, RULER_SIZE);
    for x in (120..960).step_by(120) {
        p.stroke(
            [x as f32, 3.0],
            [x as f32, 8.0],
            1.0,
            theme::color::PANEL_TITLE.alpha(0.5),
        );
        p.text_aligned(
            [x as f32, 12.0],
            ruler,
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
            arena_text(fit, READOUT_SIZE),
            ink,
            format!("NEAREST HULL  {clearance:05.1}u"),
        );
    }
    p.text_aligned(
        [W - 16.0, H - 24.0],
        arena_text(fit, READOUT_SIZE),
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
    fn the_sky_is_laid_out_once_in_reproducible_layers() {
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
    fn a_bearing_that_found_something_is_drawn_at_its_own_measured_range() {
        // A Sensorium with two hits and seven nothings, posed so the bearings
        // are the Ship's own. Ray 0 is dead ahead; ray 4 is 80 degrees to
        // starboard, which on a screen whose y runs down is below the nose.
        let world = posed(480.0, 300.0, &[(0, 0.25), (4, 0.9)]);
        let samples = envelope_samples(&world.agent.ship, &world.agent.inputs);
        for (slot, index) in ENVELOPE_ORDER.iter().enumerate() {
            let value = world.agent.inputs[*index] as f32;
            let bearing = sensors::RAY_OFFSETS_DEG[*index].to_radians() as f32;
            match samples[slot] {
                Some(point) => {
                    // The distance falls straight out of the proximity the
                    // simulation handed over: a reading of one is a hit at
                    // zero, and the range is what nothing inside it looks
                    // like.
                    let expected = (1.0 - value) * sensors::RANGE as f32;
                    let radius = point[0].hypot(point[1]);
                    assert!(
                        (radius - expected).abs() < 0.01,
                        "ray {index} reads {value} but its sample sits at {radius}, not {expected}"
                    );
                    let angle = point[1].atan2(point[0]);
                    assert!(
                        (angle - bearing).abs() < 1e-4,
                        "ray {index} is drawn at {angle}, not on its bearing {bearing}"
                    );
                }
                // And a bearing that found nothing is not drawn at all. Putting
                // it out at the range limit would draw an edge five hundred
                // units away that no reading put there.
                None => assert_eq!(value, 0.0, "ray {index} read {value} and drew nothing"),
            }
        }
        assert_eq!(samples.iter().flatten().count(), 2);
    }

    #[test]
    fn an_empty_sensorium_draws_nothing_at_all() {
        // Nine bearings with nothing inside the range is a reading, and the
        // reading is "no boundary within five hundred units". There is no shape
        // to draw, and naming an instrument that drew nothing would claim one.
        let world = posed(480.0, 300.0, &[]);
        let mut p = Painter::new();
        p.set_transform(Transform::new(1.0, [0.0, 0.0]));
        p.set_clip(Some([0.0, 0.0, W, H]));
        draw_envelope(&mut p, &world.agent.ship, &world.agent.inputs, 1.0);
        assert!(p.triangles.is_empty(), "the empty field drew geometry");
        assert!(p.luminous.is_empty(), "the empty field drew light");
        assert!(p.text.is_empty(), "the empty field named an instrument");
    }

    #[test]
    fn the_boundary_walks_its_samples_in_bearing_order_and_never_crosses_a_gap() {
        // Every bearing reading differently, so the chain is a proper zigzag
        // rather than a regular polygon: were the samples walked in the
        // Sensorium's own index order the joins would cross one another.
        let inputs: Vec<(usize, f64)> = (0..sensors::RAY_OFFSETS_DEG.len())
            .map(|ray| (ray, 0.15 + ray as f64 * 0.05))
            .collect();
        let world = posed(480.0, 300.0, &inputs);
        let samples = envelope_samples(&world.agent.ship, &world.agent.inputs);
        // No bearing reads one: a proximity of one is a hit at zero distance,
        // and its sample lands on the hull where it has no bearing left to be
        // checked against.
        assert!(world.agent.inputs[..9].iter().all(|value| *value < 1.0));
        let step = 40.0_f32.to_radians();
        let bearing = |point: [f32; 2]| point[1].atan2(point[0]);
        for pair in samples.windows(2) {
            let (a, b) = (pair[0].unwrap(), pair[1].unwrap());
            let turn = (bearing(b) - bearing(a)).rem_euclid(std::f32::consts::TAU);
            assert!(
                (turn - step).abs() < 1e-3,
                "the walk turns by {turn} at a step, not {step}"
            );
        }
        // Every bearing found something, so every neighbour is joined: eight
        // chords and nine dots.
        assert_eq!(
            (0..samples.len())
                .filter(|slot| boundary_joins(&samples, *slot))
                .count(),
            8
        );

        // A gap in the middle of the chain breaks it: a bearing that read
        // nothing is not crossed, and the runs either side are drawn as the
        // separate measurements they are. One bearing missing at slot 4 splits
        // the chain into slots 0–2 and 5–7, with nothing over the hole.
        let mut holed = samples;
        holed[4] = None;
        let joined: Vec<usize> = (0..holed.len())
            .filter(|slot| boundary_joins(&holed, *slot))
            .collect();
        assert_eq!(joined, vec![0, 1, 2, 5, 6, 7]);
        assert!(
            !boundary_joins(&holed, 3) && !boundary_joins(&holed, 4),
            "a chord was drawn across a bearing that found nothing"
        );
    }

    #[test]
    fn the_boundary_is_drawn_as_a_labelled_reading() {
        // A Sensorium shut in on every bearing: nine samples ten units out, and
        // a chain between each neighbouring pair.
        let inputs: Vec<(usize, f64)> = (0..sensors::RAY_OFFSETS_DEG.len())
            .map(|ray| (ray, 0.98))
            .collect();
        let world = posed(480.0, 300.0, &inputs);
        let mut p = Painter::new();
        p.set_transform(Transform::new(1.0, [0.0, 0.0]));
        p.set_clip(Some([0.0, 0.0, W, H]));
        // Whatever gain the Painter was left wearing, the drawing restores it.
        p.set_gain(3.0);
        draw_envelope(&mut p, &world.agent.ship, &world.agent.inputs, 1.0);
        assert_eq!(p.gain(), 3.0);

        // The joins are light and the samples are matter, so the boundary reads
        // where the field is dark and the dots sit over it.
        assert!(!p.triangles.is_empty() && !p.luminous.is_empty());
        // Nine dots ten units out and nothing nearer: the region between the
        // hull and the boundary is *not* filled — there is no measured edge
        // across the bearings in between to bound it with.
        let radius =
            |v: &crate::painter::TriangleVertex| (v.pos[0] - 480.0).hypot(v.pos[1] - 300.0);
        let expected = 0.02 * sensors::RANGE as f32;
        let nearest = p.triangles.iter().map(radius).fold(f32::MAX, f32::min);
        assert!(
            (nearest - (expected - ENVELOPE_VERTEX_RADIUS)).abs() < 0.1,
            "the nearest ink sits at {nearest}, not {expected}"
        );
        // The joins reach the dots and no further: the nearest lit ink is the
        // chord's own edge, a half-width inside the samples it joins.
        let lit = p.luminous.iter().map(radius).fold(f32::MAX, f32::min);
        assert!(
            (lit - expected).abs() < 2.0,
            "the nearest join ink sits at {lit}, not {expected}"
        );

        // The instrument is named, and the name says what the dots are.
        let caption = p
            .text
            .iter()
            .find(|item| item.content == ENVELOPE_CAPTION)
            .expect("the boundary's caption is drawn");
        // It sits clear of the hull's own instruments, holding the size a
        // reading in the field is set at.
        assert!(caption.pos[1] - 300.0 > CHEVRON_REACH);
        assert!(caption.size >= READOUT_MIN);
    }

    #[test]
    fn the_readings_stay_on_their_subjects_at_every_field_scale() {
        let world = posed(480.0, 300.0, &[]);
        for (scale, origin) in [(0.5_f32, [17.0_f32, 71.0_f32]), (2.0, [3.0, 5.0])] {
            let view = ArenaView {
                origin,
                scale,
                tremor: [0.0, 0.0],
            };
            let mut p = Painter::new();
            p.set_transform(Transform::new(1.0, [0.0, 0.0]));
            draw_arena(&mut p, &world, view, false, true);

            // The ruler number is anchored to its own tick, in Arena units, at
            // every scale: only the type stopped scaling.
            let transform = view.transform();
            let ruler = p
                .text
                .iter()
                .find(|item| item.content == "120")
                .expect("the 120 ruler is drawn");
            let expected = transform.apply([120.0, 12.0]);
            assert!((ruler.pos[0] - expected[0]).abs() < 1e-3);
            assert!((ruler.pos[1] - expected[1]).abs() < 1e-3);
            assert!(
                (ruler.size - RULER_SIZE * scale.clamp(READOUT_MIN / RULER_SIZE, 2.0)).abs() < 1e-3
            );

            // As is the V/T readout, in its own lane against the field's edge.
            let readout = p
                .text
                .iter()
                .find(|item| item.content.starts_with("V "))
                .expect("the velocity readout is drawn");
            let expected = transform.apply([W - 16.0, H - 24.0]);
            assert!((readout.pos[0] - expected[0]).abs() < 1e-3);
            assert!((readout.pos[1] - expected[1]).abs() < 1e-3);
        }
    }

    #[test]
    fn drawing_the_arena_leaves_the_painter_as_it_found_it() {
        let world = posed(480.0, 300.0, &[]);
        let mut p = Painter::new();
        let chrome = Transform::new(2.0, [31.0, 9.0]);
        p.set_transform(chrome);
        draw_arena(
            &mut p,
            &world,
            ArenaView {
                origin: [4.0, 6.0],
                scale: 1.5,
                tremor: [0.0, 0.0],
            },
            true,
            true,
        );
        // The chrome that follows is drawn in the window, not in the field: a
        // caller cannot walk away wearing the Arena's transform.
        assert_eq!(p.transform(), chrome);
        assert!(!p.triangles.is_empty());
        // And the field really was drawn in the field's space: every vertex is
        // inside the Arena's rect in the window.
        assert!(p
            .triangles
            .iter()
            .all(|v| (4.0..=4.0 + 960.0 * 1.5).contains(&v.pos[0])
                && (6.0..=6.0 + 600.0 * 1.5).contains(&v.pos[1])));
    }

    #[test]
    fn the_present_tremor_moves_the_field_and_nothing_else() {
        let view = ArenaView {
            origin: [40.0, 60.0],
            scale: 1.5,
            tremor: [0.0, 0.0],
        };
        let shaken = ArenaView {
            tremor: [2.0, -1.0],
            ..view
        }
        .transform();
        let still = view.transform();

        // The offset is in Arena units, so it arrives in the window scaled like
        // everything else the field is made of.
        assert_eq!(shaken.scale, still.scale);
        assert!((shaken.offset[0] - still.offset[0] - 3.0).abs() < 1e-6);
        assert!((shaken.offset[1] - still.offset[1] + 1.5).abs() < 1e-6);
        assert_eq!(shaken.offset, [43.0, 58.5]);

        // But the view itself does not: the field's rect is what the backdrop
        // and the instrument strip are placed from, and neither shakes.
        assert_eq!(view.rect(), (40.0, 60.0, W * 1.5, H * 1.5));
        assert_eq!(
            view,
            ArenaView {
                origin: [40.0, 60.0],
                scale: 1.5,
                tremor: [0.0, 0.0],
            }
        );
    }

    #[test]
    fn seam_copies_hold_while_the_field_is_shaken() {
        // The Ship on the left seam, and the field struck: the offset is on the
        // transform, so the copy across the seam is the same shape in the same
        // place relative to the field, and the wrap still reads.
        let tremor = [4.0, -3.0];
        let world = posed(4.0, 300.0, &[]);
        let view = ArenaView {
            origin: [10.0, 20.0],
            scale: 1.0,
            tremor,
        };
        let mut p = Painter::new();
        draw_arena(&mut p, &world, view, false, true);

        // The field is drawn where the shake put it, clipped to the shaken
        // rect: nothing hangs outside it.
        let left = view.origin[0] + tremor[0];
        let top = view.origin[1] + tremor[1];
        assert!(p
            .triangles
            .iter()
            .chain(p.luminous.iter())
            .all(|v| v.pos[0] >= left - 1e-3
                && v.pos[0] <= left + W + 1e-3
                && v.pos[1] >= top - 1e-3
                && v.pos[1] <= top + H + 1e-3));
        // And the hull is on both edges of the field at once, which is what a
        // Seam Copy is.
        assert!(p.triangles.iter().any(|v| v.pos[0] < left + 20.0));
        assert!(p.triangles.iter().any(|v| v.pos[0] > left + W - 8.0));
    }
}
