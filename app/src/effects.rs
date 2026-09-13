//! Bounded cosmetic light, driven by simulation time. No wall clock, and
//! nothing here touches the simulation's RNG: every event is an echo of a
//! change the World already made.
//!
//! Three languages, all of them light:
//! - an Impact opens a shock front at the body's own recorded radius, spills
//!   its flash on the body and throws sparks that slow as they fly;
//! - a cleared Wave pulses the four Arena edges, the seam the field wraps on;
//! - the Ship's death scatters embers where it died.
//!
//! A shot is light too: the Ship's nose holds a muzzle flash for an instant.
//!
//! And one that is not light: an Impact also jolts the whole field. The Arena
//! carries a bounded trauma that decays on the simulation clock, squared into a
//! displacement no larger than [`SHAKE_AMPLITUDE`], which the app reads from
//! [`Effects::shake`] and carries on the Arena's transform.
//! The chrome is outside that transform, so the instruments never move.
//!
//! Events age on `World::time`, so they freeze with a pause and a replay draws
//! the frame it drew the first time. Reduced motion clears them entirely, the
//! tremor included.
//!
//! # The observation window
//!
//! A frame at ×100 runs about 1.7 simulation seconds and a frame at ×1 runs a
//! single step, so "observe every step" and "observe always" are the same thing
//! only at the slow end. Painting a fast frame's every event would stack a
//! whole history onto one picture, and the number of steps in a frame is the
//! machine's business, so the picture would stop being a function of the run.
//! The old `speed <= 16` gate avoided that by observing nothing at speed, which
//! made the language invisible on a default run.
//!
//! Instead each frame keeps only its own final [`OBSERVE_WINDOW`] seconds.
//! Every step is observed, but into a staging ring keyed by simulation time
//! that drops whatever has fallen a window behind the clock; what survives the
//! frame's last step is exactly the window that ends at the frame's end, and
//! [`Effects::settle`] commits it to the record the frame draws. At ×1 a frame
//! is a single step, entirely inside the window, so the record there is
//! unchanged: an Impact still lives its full [`LIFETIME`] across frames. At
//! speed the window is the difference between the field's most recent moment
//! and a palimpsest.

use crate::{
    painter::{Painter, Rgba},
    scene::ArenaView,
    theme::color,
};
use sim::{
    config::{arena, bullet},
    world::Impact,
    World,
};
use std::collections::VecDeque;

/// The slice of simulation time before each frame whose events the cosmetic
/// layer keeps, in seconds. A quarter second is long enough for a spark to
/// have flown somewhere and short enough that every light in it is still
/// young: the smallest window that still reads as "what just happened" rather
/// than as the instant of the frame.
const OBSERVE_WINDOW: f64 = 0.25;

/// How long an impact's light lives, in seconds.
const LIFETIME: f64 = 0.55;
/// The most impact lights kept; a storm drops the oldest first.
const CAPACITY: usize = 24;
/// How long the Arena's edges stay lit after a Wave is cleared.
const WAVE_LIFE: f64 = 0.9;
const WAVE_CAP: usize = 3;
/// How long the Ship's embers scatter.
const DEATH_LIFE: f64 = 0.7;
const DEATH_CAP: usize = 2;
/// How far the death's flash grows over its life, in Arena units — the reach
/// the seam copies cull the burst with.
const DEATH_GLOW: f32 = 26.0;

/// The logical Arena.
const W: f32 = arena::WIDTH as f32;
const H: f32 = arena::HEIGHT as f32;

/// How deep a Wave pulse reaches in from an edge, in Arena units. One gradient
/// strip per edge carries the quadratic falloff now; it was four flat bands for
/// as long as the Painter had no additive gradient to draw the falloff with.
const WAVE_DEPTH: f32 = 10.0;

/// How far the shock front travels past the body's recorded radius over the
/// echo's life, in Arena units: a Large rock's blast crosses its own body a
/// second time, and a Ship's death reaches about two hulls out.
const SHOCK_TRAVEL: f32 = 22.0;

/// How far the front's light reaches back behind its leading edge, in Arena
/// units: the front is an annulus with a real falloff across it, not a ring.
const SHOCK_WIDTH: f32 = 5.5;

/// How many segments the annulus is drawn in. Twenty keeps a circle round to
/// the eye at the largest radius an impact draws, and every segment is one
/// gradient quad rather than a stack of bands.
const SHOCK_SEGMENTS: usize = 20;

/// The spill: how far past the recorded radius the flash lights the body it
/// hit, the radius below which it is never drawn smaller, and how strongly.
/// One radial glow — a rock is not relit facet by facet, which would be a
/// lighting pass of its own.
const SPILL_SCALE: f32 = 1.9;
const SPILL_FLOOR: f32 = 7.0;
const SPILL_ALPHA: f32 = 0.5;

/// Sparks: how fast one leaves the impact, in Arena units a second. The speed
/// falls linearly to nothing over the echo's life, so a spark travels half of
/// `SPARK_SPEED × LIFETIME` — about 19 units, a little past the front's own
/// reach.
const SPARK_SPEED: f32 = 70.0;
const SPARK_TRAVEL: f32 = SPARK_SPEED * 0.5 * LIFETIME as f32;

/// How far either side of its nominal speed a spark's own draw may land, as a
/// fraction: an even fan of identical arcs reads as spokes, not as debris.
const SPARK_SPREAD: f32 = 0.35;

/// The part of the echo's life a spark's streak spans: the tail is the piece
/// of the ballistic arc the particle crossed in the last fifth of its flight.
const SPARK_TAIL: f32 = 0.22;

/// The spark's streak width at the particle and at its tail, in Arena units.
const SPARK_WIDTH: f32 = 1.3;
const SPARK_TIP: f32 = 0.35;

/// How long the muzzle flash lives, in seconds: five simulation steps, long
/// enough to read at ×1 and short enough to stay a flash rather than a lamp.
const MUZZLE_LIFE: f64 = 0.08;

/// The glow the flash throws around the nose, in Arena units.
const MUZZLE_RADIUS: f32 = 13.0;

/// The trauma one Impact leaves, and one Ship's death, and how fast it bleeds
/// away. The level is bounded at one by construction, and it is squared before
/// it becomes a displacement, so a light hit is a nudge — a rock is about a
/// pixel at 1×, the Ship's death about four — and sustained fire saturates at
/// the cap. A second of simulation time takes 0.8 off the level.
const TRAUMA_PER_IMPACT: f32 = 0.45;
const TRAUMA_PER_DEATH: f32 = 0.85;
const TRAUMA_DECAY: f32 = 0.8;

/// The most the tremor moves the field, in Arena units — six of the 960×600
/// field's units, which is six window pixels at 1× and the same fraction of the
/// field at any other scale, because the offset is carried by the Arena's
/// transform. It is a jolt, never a lurch.
const SHAKE_AMPLITUDE: f32 = 6.0;

/// How long the tremor holds one direction before the hash draws another, in
/// simulation steps: a held direction reads as the field being struck, where a
/// fresh one every frame reads as static. Three steps is a twentieth of a
/// second.
const SHAKE_STEPS: u64 = 3;

/// One over the square root of two: the tremor's two axes come from one hash
/// pair, and this is what bounds their magnitude to [`SHAKE_AMPLITUDE`] rather
/// than to its diagonal.
const SHAKE_DIAGONAL: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// One event, staged with the simulation time it happened at.
///
/// The staging ring is the observation window's memory: an event enters it when
/// its step is observed and leaves it when the clock has moved a window past,
/// which is exactly the rule "keep the frame's last quarter second".
enum Staged {
    Impact(Impact),
    Wave,
    Death([f64; 2]),
    Muzzle([f32; 2]),
}

#[derive(Default)]
pub struct Effects {
    key: Option<(u32, usize)>,
    /// The simulation time of the most recent observation.
    time: f64,
    /// The simulation time of the last commit; the tremor's decay runs from
    /// one commit to the next, so a decay step covers the simulation a whole
    /// frame ran rather than one step of it.
    settled: f64,
    echoes: VecDeque<(f64, Impact)>,
    /// The Wave count this Episode has already shown; only an increase pulses.
    wave: Option<u32>,
    waves: VecDeque<f64>,
    /// Whether the Ship was alive at the last observation; true→false scatters.
    alive: Option<bool>,
    /// The shot count (`Agent::stats.fire`) this Episode has already shown;
    /// only an increase lights a muzzle.
    fire: Option<u64>,
    bursts: VecDeque<(f64, [f64; 2])>,
    /// The most recent shot's flash. One is kept, not a ring: a muzzle is a
    /// moment, and a fast frame that contains several shots shows the last of
    /// them rather than a queue of lamps.
    muzzle: Option<(f64, [f32; 2])>,
    /// How hard the field has been hit, in `0..=1`, on the simulation clock.
    trauma: f32,
    /// The frame's staged events, oldest first, never more than one
    /// [`OBSERVE_WINDOW`] of observed time deep.
    staged: VecDeque<(f64, Staged)>,
}
impl Effects {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// How hard the field has been hit: raised by the window's impacts and
    /// deaths, falling by [`TRAUMA_DECAY`] a second of simulation time,
    /// bounded at one.
    pub fn trauma(&self) -> f32 {
        self.trauma
    }

    /// The Arena's displacement this frame, in Arena units.
    pub fn shake(&self) -> [f32; 2] {
        shake(self.trauma, self.time)
    }

    /// Called after each watched step; never steps or mutates the World itself.
    ///
    /// Everything the step produced is staged rather than drawn: the frame's
    /// whole history would be a palimpsest at speed, so [`Effects::settle`]
    /// commits only the window that ends at the frame's end.
    pub fn observe(&mut self, key: (u32, usize), w: &World, enabled: bool) {
        if !enabled {
            self.clear();
            return;
        }
        if self.key != Some(key) || w.time < self.time {
            // A different Episode is a different field, and so is this one
            // replayed from the top: the record starts empty and the tremor
            // still. The first look at any Episode only records what it finds,
            // so arriving mid-Episode — a load, a replay, a change of watched
            // member — never fires a spurious pulse, death or shot.
            self.clear();
            self.key = Some(key);
        }
        if w.time == self.time {
            return;
        }
        self.time = w.time;
        while self
            .staged
            .front()
            .is_some_and(|(t, _)| w.time - t > OBSERVE_WINDOW)
        {
            self.staged.pop_front();
        }
        self.detect_wave(w);
        self.detect_death(w);
        self.detect_fire(w);
        // The step's impacts. A single step can record at most
        // `bullet::MAX_ALIVE_PER_SHIP + 1` of them — the World's record is one
        // `Option<Impact>` slot per live bullet plus the Ship's own, filled
        // first-slot-first — so the per-observe input is capped by the
        // simulation itself and the ring cannot grow past a window of bounded
        // steps.
        for impact in w.impacts() {
            self.staged.push_back((w.time, Staged::Impact(*impact)));
        }
    }

    /// Close the frame's observation: commit the window's events to the record
    /// the frame draws, and let the tremor settle. Called once per frame after
    /// the watched steps, before anything is drawn.
    pub fn settle(&mut self, enabled: bool) {
        if !enabled {
            self.clear();
            return;
        }
        let time = self.time;
        // The tremor is the field's live state, not a windowed event: it decays
        // over every simulation second the frame ran and rises only by the hits
        // the window kept, so the Arena is never seen shaking without the flash
        // that caused it.
        let elapsed = (time - self.settled).max(0.0) as f32;
        let mut trauma = (self.trauma - TRAUMA_DECAY * elapsed).max(0.0);
        while self
            .echoes
            .front()
            .is_some_and(|(t, _)| time - t > LIFETIME)
        {
            self.echoes.pop_front();
        }
        while self.waves.front().is_some_and(|t| time - t > WAVE_LIFE) {
            self.waves.pop_front();
        }
        while self
            .bursts
            .front()
            .is_some_and(|(t, _)| time - t > DEATH_LIFE)
        {
            self.bursts.pop_front();
        }
        if self
            .muzzle
            .is_some_and(|(born, _)| time - born > MUZZLE_LIFE)
        {
            self.muzzle = None;
        }
        for (born, event) in self.staged.drain(..) {
            match event {
                Staged::Impact(impact) => {
                    trauma = (trauma + TRAUMA_PER_IMPACT).min(1.0);
                    if self.echoes.len() == CAPACITY {
                        self.echoes.pop_front();
                    }
                    self.echoes.push_back((born, impact));
                }
                Staged::Wave => {
                    if self.waves.len() == WAVE_CAP {
                        self.waves.pop_front();
                    }
                    self.waves.push_back(born);
                }
                Staged::Death(at) => {
                    trauma = (trauma + TRAUMA_PER_DEATH).min(1.0);
                    if self.bursts.len() == DEATH_CAP {
                        self.bursts.pop_front();
                    }
                    self.bursts.push_back((born, at));
                }
                // A later shot replaces an earlier one: the flash is coalesced
                // to the most recent, never drawn once per shot.
                Staged::Muzzle(at) => self.muzzle = Some((born, at)),
            }
        }
        self.trauma = trauma.min(1.0);
        self.settled = time;
    }

    /// A Wave counter that goes up means the field was cleared.
    fn detect_wave(&mut self, w: &World) {
        let previous = self.wave.unwrap_or(w.wave);
        if w.wave > previous {
            self.staged.push_back((w.time, Staged::Wave));
        }
        self.wave = Some(w.wave);
    }

    /// A live Ship that stops being alive scatters embers where it died.
    fn detect_death(&mut self, w: &World) {
        let was_alive = self.alive.unwrap_or(w.agent.alive);
        if was_alive && !w.agent.alive {
            self.staged
                .push_back((w.time, Staged::Death([w.agent.ship.x, w.agent.ship.y])));
        }
        self.alive = Some(w.agent.alive);
    }

    /// The shot count going up means the Ship fired: stage the muzzle flash
    /// where the nose was at that step. `Agent::stats::fire` is the one that
    /// only counts shots — `bullets_out` and the bullet list both fall again
    /// when a bullet expires or hits, so a shot fired in the same step as a
    /// bullet's death would show no rise in either.
    fn detect_fire(&mut self, w: &World) {
        let shots = w.agent.stats.fire;
        if self.fire.is_some_and(|previous| shots > previous) {
            let ship = &w.agent.ship;
            self.staged.push_back((
                w.time,
                Staged::Muzzle([
                    (ship.x + ship.heading.cos() * bullet::NOSE_OFFSET) as f32,
                    (ship.y + ship.heading.sin() * bullet::NOSE_OFFSET) as f32,
                ]),
            ));
        }
        self.fire = Some(shots);
    }

    pub fn draw(&self, p: &mut Painter, view: ArenaView) {
        // The scope returns the Painter's transform and clip as it found them,
        // and the gain is put back by hand: a drawing call never walks away
        // wearing the Arena's geometry.
        let gain = p.gain();
        p.arena_scope(view.transform(), [0.0, 0.0, W, H], |p| {
            self.draw_waves(p);
            for (born, impact) in &self.echoes {
                self.draw_impact(p, *born, impact);
            }
            for (born, at) in &self.bursts {
                self.draw_burst(p, *born, *at);
            }
            self.draw_muzzle(p);
            p.set_gain(gain);
        });
    }

    /// A cleared Wave pulses the four Arena edges: additive light laid just
    /// inside the seam the field wraps on, its falloff carried by one gradient
    /// strip per edge. No seam copies — the edges are it.
    fn draw_waves(&self, p: &mut Painter) {
        for born in &self.waves {
            let age = self.age(*born, WAVE_LIFE);
            let strength = (1.0 - age).powi(2) * 0.25;
            p.set_gain(crate::theme::light::WAVE);
            for (a, b, inward) in [
                ([0.0, 0.0], [W, 0.0], [0.0, 1.0]),
                ([0.0, H], [W, H], [0.0, -1.0]),
                ([0.0, 0.0], [0.0, H], [1.0, 0.0]),
                ([W, 0.0], [W, H], [-1.0, 0.0]),
            ] {
                edge_strip(p, a, b, inward, color::ACCENT, strength);
            }
            p.set_gain(1.0);
        }
    }

    /// One impact: a shock front opening from the body's own recorded radius,
    /// the flash it spills on the body, and its sparks.
    ///
    /// The radius is the one the simulation recorded — the rock's own size for
    /// a strike, the hull's for a Ship's death — so the front starts where the
    /// collision physically was, not at a fixed glow size.
    fn draw_impact(&self, p: &mut Painter, born: f64, impact: &Impact) {
        let age = self.age(born, LIFETIME);
        let fade = (1.0 - age).powi(2);
        let radius = impact.radius.max(0.0) as f32;
        let front = radius + age * SHOCK_TRAVEL;
        let ink = if impact.ship {
            color::WARN
        } else {
            color::SHIP_FLAME
        };
        let seed = spark_seed(impact.x, impact.y, born);
        let at = [impact.x as f32, impact.y as f32];
        let reach = radius + SHOCK_TRAVEL + SHOCK_WIDTH + SPARK_TRAVEL * (1.0 + SPARK_SPREAD);
        seam_copies(p, at, reach, |p, c| {
            p.set_gain(crate::theme::light::SHOCKWAVE);
            shock_front(p, c, front, fade, ink);
            p.set_gain(crate::theme::light::IMPACT);
            // The spill: the body that was hit catches the flash, one glow
            // scaled by the radius the collision recorded.
            p.luminous_glow(
                c,
                (radius * SPILL_SCALE).max(SPILL_FLOOR),
                color::LIGHT_IMPACT.alpha(fade * SPILL_ALPHA),
            );
            p.set_gain(crate::theme::light::SPARK);
            let mut seed = seed;
            sparks(p, c, &mut seed, age, 6, |seed| {
                if lcg_unit(seed) < 0.3 {
                    color::SHIP_FLAME
                } else {
                    color::LIGHT_IMPACT
                }
            });
            p.set_gain(1.0);
        });
    }

    /// The Ship's death: one flash where it died, and its embers scattering on
    /// the same ballistic arcs an impact's sparks fly.
    fn draw_burst(&self, p: &mut Painter, born: f64, at: [f64; 2]) {
        let age = self.age(born, DEATH_LIFE);
        let fade = (1.0 - age).powi(2);
        let flash = color::SHIP_DEAD.mix(color::LIGHT_IMPACT, 0.5);
        let seed = spark_seed(at[0], at[1], born);
        let at = [at[0] as f32, at[1] as f32];
        let reach = DEATH_GLOW + SPARK_TRAVEL * (1.0 + SPARK_SPREAD);
        seam_copies(p, at, reach, |p, c| {
            p.set_gain(crate::theme::light::IMPACT);
            p.luminous_glow(c, 8.0 + age * 18.0, flash.alpha(fade * 0.7));
            p.set_gain(crate::theme::light::SPARK);
            let mut seed = seed;
            sparks(p, c, &mut seed, age, 12, |seed| {
                color::SHIP_DEAD.mix(color::SHIP_FLAME, lcg_unit(seed))
            });
            p.set_gain(1.0);
        });
    }

    /// The muzzle: the Ship's nose lit for the instant after a shot. A shot is
    /// something the World really did, and this is the light of it.
    fn draw_muzzle(&self, p: &mut Painter) {
        let Some((born, at)) = self.muzzle else {
            return;
        };
        let fade = (1.0 - self.age(born, MUZZLE_LIFE)).powi(2);
        seam_copies(p, at, MUZZLE_RADIUS, |p, c| {
            p.set_gain(crate::theme::light::MUZZLE);
            p.luminous_glow(c, MUZZLE_RADIUS, color::LIGHT_FLAME_CORE.alpha(fade * 0.8));
            p.set_gain(1.0);
        });
    }

    /// Age of an event in `0..=1`, on the simulation clock the events share.
    fn age(&self, born: f64, lifetime: f64) -> f32 {
        ((self.time - born) / lifetime).clamp(0.0, 1.0) as f32
    }
}

/// A luminous segment between two points: the additive buffer has no stroke of
/// its own, so this is the light-drawn twin of `Painter::stroke`.
pub(crate) fn light_stroke(p: &mut Painter, a: [f32; 2], b: [f32; 2], width: f32, ink: Rgba) {
    light_segment(p, a, b, width, width, ink);
}

/// One event's sparks: each has its own bearing and speed, drawn from the
/// event's own seed, and each slows over the echo's life — a ballistic arc in
/// Arena units, `v·t·(1 − t/2T)`, so a spark is thrown and comes to rest rather
/// than sliding at a constant rate. Every position is a closed form of the age,
/// so nothing is integrated frame to frame and the same event draws the same
/// arcs on every machine and in every seam copy.
fn sparks(
    p: &mut Painter,
    at: [f32; 2],
    seed: &mut u32,
    age: f32,
    base: usize,
    ink: impl Fn(&mut u32) -> Rgba,
) {
    let life = LIFETIME as f32;
    let t = age * life;
    let tail_t = (t - SPARK_TAIL * life).max(0.0);
    let (head, tail) = (spark_travel(t), spark_travel(tail_t));
    let fade = (1.0 - age).powf(1.5);
    let count = base + (lcg_unit(seed) * 5.0) as usize;
    for i in 0..count {
        let spread = std::f32::consts::TAU / count as f32;
        let angle = spread * i as f32 + lcg_unit(seed) * spread;
        let speed = 1.0 + (lcg_unit(seed) - 0.5) * 2.0 * SPARK_SPREAD;
        let ink = ink(seed).alpha(fade);
        let (sin, cos) = (angle.sin(), angle.cos());
        let (head, tail) = (head * speed, tail * speed);
        light_segment(
            p,
            [at[0] + cos * tail, at[1] + sin * tail],
            [at[0] + cos * head, at[1] + sin * head],
            SPARK_WIDTH,
            SPARK_TIP,
            ink,
        );
    }
}

/// Where a spark is `t` seconds into a life that slows linearly from
/// [`SPARK_SPEED`] to rest at [`LIFETIME`]: the integral of that speed,
/// `v·t·(1 − t/2T)`, which is what carries it [`SPARK_TRAVEL`] units in a whole
/// life.
fn spark_travel(t: f32) -> f32 {
    let life = LIFETIME as f32;
    let t = t.clamp(0.0, life);
    SPARK_SPEED * t * (1.0 - t / (2.0 * life))
}

/// The shock front: an annulus of additive gradient quads opening outward from
/// the body's recorded radius. Its leading edge carries the echo's brightness
/// and the light falls to nothing across [`SHOCK_WIDTH`] behind it, so the
/// front is a front and not a ring — one gradient polygon per segment, which is
/// what the Painter's additive fill was added for.
fn shock_front(p: &mut Painter, centre: [f32; 2], front: f32, fade: f32, ink: Rgba) {
    let inner = (front - SHOCK_WIDTH).max(0.0);
    let bright = ink.alpha(fade);
    let dark = ink.alpha(0.0);
    for segment in 0..SHOCK_SEGMENTS {
        let a0 = segment as f32 * std::f32::consts::TAU / SHOCK_SEGMENTS as f32;
        let a1 = (segment + 1) as f32 * std::f32::consts::TAU / SHOCK_SEGMENTS as f32;
        let (sin0, cos0) = (a0.sin(), a0.cos());
        let (sin1, cos1) = (a1.sin(), a1.cos());
        let points = [
            [centre[0] + cos0 * inner, centre[1] + sin0 * inner],
            [centre[0] + cos0 * front, centre[1] + sin0 * front],
            [centre[0] + cos1 * front, centre[1] + sin1 * front],
            [centre[0] + cos1 * inner, centre[1] + sin1 * inner],
        ];
        p.luminous_gradient_polygon(&points, &[dark, bright, bright, dark]);
    }
}

/// Two triangles between `a` and `b`, their half-widths `width_a`/`width_b`.
fn light_segment(p: &mut Painter, a: [f32; 2], b: [f32; 2], width_a: f32, width_b: f32, ink: Rgba) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx.hypot(dy);
    if length < 0.001 || (width_a <= 0.0 && width_b <= 0.0) {
        return;
    }
    let (nx, ny) = (-dy / length, dx / length);
    let (ha, hb) = (width_a * 0.5, width_b * 0.5);
    let p0 = [a[0] + nx * ha, a[1] + ny * ha];
    let p1 = [b[0] + nx * hb, b[1] + ny * hb];
    let p2 = [b[0] - nx * hb, b[1] - ny * hb];
    let p3 = [a[0] - nx * ha, a[1] - ny * ha];
    p.luminous_triangle(p0, p1, p2, ink);
    p.luminous_triangle(p0, p2, p3, ink);
}

/// A strip of light lying against an Arena edge, fading inward.
///
/// The falloff is the same quadratic it always sampled, but sampled twice —
/// bright at the seam, a quarter at half depth, nothing at [`WAVE_DEPTH`] in —
/// and carried across one gradient strip instead of four flat bands, so the
/// pulse reads as light rather than as a row of them.
fn edge_strip(
    p: &mut Painter,
    a: [f32; 2],
    b: [f32; 2],
    inward: [f32; 2],
    ink: Rgba,
    strength: f32,
) {
    let outer = ink.alpha(strength);
    let half = ink.alpha(strength * 0.25);
    let dark = ink.alpha(0.0);
    let far = |point: [f32; 2]| {
        [
            point[0] + inward[0] * WAVE_DEPTH,
            point[1] + inward[1] * WAVE_DEPTH,
        ]
    };
    let mid = |point: [f32; 2]| {
        [
            point[0] + inward[0] * WAVE_DEPTH * 0.5,
            point[1] + inward[1] * WAVE_DEPTH * 0.5,
        ]
    };
    // The boundary in order, so the fan tiles the strip exactly: along the
    // seam, down the halfway mark of the far edge, on to its corner, across
    // the back, and up the other side. The mid points are collinear, which is
    // what lets one quad carry the quadratic's three samples.
    let points = [a, b, mid(b), far(b), far(a), mid(a)];
    p.luminous_gradient_polygon(&points, &[outer, outer, half, dark, dark, half]);
}

/// Draw an event once for each toroidal copy that can reach the Arena: its
/// centre, plus a copy for each edge it laps. Copies the clip would discard
/// anyway are dropped by `reach`, the event's radius in Arena units.
fn seam_copies(p: &mut Painter, at: [f32; 2], reach: f32, draw: impl Fn(&mut Painter, [f32; 2])) {
    for ox in [-W, 0.0, W] {
        for oy in [-H, 0.0, H] {
            let c = [at[0] + ox, at[1] + oy];
            if c[0] + reach < 0.0 || c[1] + reach < 0.0 || c[0] - reach > W || c[1] - reach > H {
                continue;
            }
            draw(p, c);
        }
    }
}

/// The Arena's displacement, in Arena units, for a trauma level and a moment in
/// simulation time.
///
/// Trauma is squared, so the level reads as severity rather than as an amount;
/// the direction comes from the event's own discipline — integer arithmetic on
/// the clock quantized to simulation steps, no library RNG, no `sin`/`cos` —
/// held for [`SHAKE_STEPS`] steps at a time so the field moves as one piece
/// instead of buzzing. A frame that observes nothing publishes the same offset,
/// which is what makes a paused frame pixel-identical.
fn shake(level: f32, time: f64) -> [f32; 2] {
    let level = level.clamp(0.0, 1.0);
    if level <= 0.0 || !time.is_finite() || time < 0.0 {
        return [0.0, 0.0];
    }
    let amplitude = SHAKE_AMPLITUDE * level * level;
    let step = (time / sim::DT).round() as u64 / SHAKE_STEPS;
    let mut seed = (step ^ (step >> 32)) as u32;
    let x = lcg_unit(&mut seed) * 2.0 - 1.0;
    let y = lcg_unit(&mut seed) * 2.0 - 1.0;
    [
        x * SHAKE_DIAGONAL * amplitude,
        y * SHAKE_DIAGONAL * amplitude,
    ]
}

/// The seed for one event's radiating pattern: its identity quantized into
/// integers — position to 1/64 Arena unit, birth to a simulation step — and
/// folded together. Integer arithmetic only, so the same event draws the same
/// light on every machine, the same discipline as the starfield.
fn spark_seed(x: f64, y: f64, born: f64) -> u32 {
    let quantize = |v: f64| (v * 64.0).round() as i64 as u64;
    let steps = (born / sim::DT).round() as i64 as u64;
    // Fold the whole 64-bit mix down, so a rotated low quantity is never lost.
    let mixed = quantize(x) ^ quantize(y).rotate_left(21) ^ steps.rotate_left(42);
    let mut seed = (mixed ^ (mixed >> 32)) as u32;
    // One warm-up step, so neighbouring identities start far apart.
    lcg_unit(&mut seed);
    seed
}

/// The next value in `0..1` from a Numerical-Recipes LCG, high bits first.
fn lcg_unit(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / 16_777_216.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_impact_storm_has_a_fixed_cosmetic_capacity() {
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.agent.ship.x = 100.0;
        w.agent.ship.y = 100.0;
        let mut e = Effects::default();
        let mut rng = sim::Rng::from_seed(2);
        for _ in 0..60 {
            w.asteroids.clear();
            w.asteroids.push(sim::Asteroid::new(
                sim::config::asteroid::Size::Large,
                500.0,
                500.0,
                0.0,
                0.0,
                &mut rng,
            ));
            w.bullets.push(sim::Bullet {
                x: 500.0,
                y: 500.0,
                vx: 0.0,
                vy: 0.0,
                life: 1.0,
            });
            w.step_fixed();
            e.observe((1, 0), &w, true);
            e.settle(true);
        }
        assert_eq!(e.echoes.len(), CAPACITY);
        let time = e.time;
        e.observe((1, 0), &w, true);
        assert_eq!(e.time, time);
        assert_eq!(e.echoes.len(), CAPACITY);
    }

    #[test]
    fn echoes_expire_freeze_and_reset_without_crossing_the_seam() {
        let mut e = Effects::default();
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.time = 1.0;
        e.observe((1, 0), &w, true);
        e.echoes.push_back((
            1.0,
            Impact {
                x: 2.0,
                y: 300.0,
                radius: 20.0,
                ship: false,
            },
        ));
        e.observe((1, 0), &w, true);
        assert_eq!(e.echoes.len(), 1);
        let mut p = Painter::new();
        e.draw(
            &mut p,
            ArenaView {
                origin: [0.0, 0.0],
                scale: 1.0,
                tremor: [0.0, 0.0],
            },
        );
        // The impact vocabulary is all light now — the matter ring it used to
        // draw is a shock front in the additive buffer — so the copy across the
        // seam is checked where the light is, and nothing crosses the clip.
        assert!(p.triangles.is_empty());
        assert!(!p.luminous.is_empty());
        assert!(p
            .luminous
            .iter()
            .all(|v| v.pos[0] >= 0.0 && v.pos[0] <= 960.0 && v.pos[1] >= 0.0 && v.pos[1] <= 600.0));
        assert!(p.luminous.iter().any(|v| v.pos[0] > 930.0));
        for _ in 0..4 {
            w.time += 0.2;
            e.observe((1, 0), &w, true);
            e.settle(true);
        }
        assert!(e.echoes.is_empty());
        e.echoes.push_back((
            w.time,
            Impact {
                x: 2.0,
                y: 300.0,
                radius: 20.0,
                ship: false,
            },
        ));
        e.observe((2, 0), &w, true);
        assert!(e.echoes.is_empty());
        e.observe((2, 0), &w, false);
        assert_eq!(e.key, None);
    }

    #[test]
    fn a_cleared_wave_pulses_the_seam_once_and_not_on_identity_change() {
        let mut e = Effects::default();
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.time = 1.0;
        w.wave = 4;
        e.observe((1, 0), &w, true);
        e.settle(true);
        // The first look at an Episode records the count without pulsing.
        assert!(e.waves.is_empty());
        w.time = 1.1;
        w.wave = 5;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert_eq!(e.waves.len(), 1);
        // The same instant is not a second pulse, a quiet count is not one
        // either, and a count that drops is not an increase.
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert_eq!(e.waves.len(), 1);
        w.time = 1.2;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert_eq!(e.waves.len(), 1);
        w.wave = 2;
        w.time = 1.3;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert_eq!(e.waves.len(), 1);
        // A new Episode identity clears the history, count included.
        w.wave = 7;
        e.observe((2, 0), &w, true);
        assert!(e.waves.is_empty());
    }

    #[test]
    fn wave_pulses_are_bounded_and_expire_on_simulation_time() {
        let mut e = Effects::default();
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.time = 1.0;
        e.observe((1, 0), &w, true);
        for _ in 0..5 {
            w.time += 0.01;
            w.wave += 1;
            e.observe((1, 0), &w, true);
            e.settle(true);
        }
        assert_eq!(e.waves.len(), WAVE_CAP);
        for _ in 0..4 {
            w.time += 0.23;
            e.observe((1, 0), &w, true);
            e.settle(true);
        }
        assert!(e.waves.is_empty());
    }

    #[test]
    fn death_scatters_embers_only_on_a_live_to_dead_transition() {
        let mut e = Effects::default();
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.time = 1.0;
        w.agent.alive = false;
        // Watching after the fact is not a death.
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert!(e.bursts.is_empty());
        w.agent.alive = true;
        w.time = 1.1;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert!(e.bursts.is_empty());
        w.agent.ship.x = 240.0;
        w.agent.ship.y = 260.0;
        w.agent.alive = false;
        w.time = 1.2;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert_eq!(e.bursts.len(), 1);
        assert_eq!(e.bursts[0].1, [240.0, 260.0]);
        let mut p = Painter::new();
        e.draw(
            &mut p,
            ArenaView {
                origin: [0.0, 0.0],
                scale: 1.0,
                tremor: [0.0, 0.0],
            },
        );
        assert!(p.triangles.is_empty());
        assert!(!p.luminous.is_empty());
        assert!(p
            .luminous
            .iter()
            .all(|v| v.pos[0] >= 0.0 && v.pos[0] <= 960.0 && v.pos[1] >= 0.0 && v.pos[1] <= 600.0));
    }

    #[test]
    fn death_bursts_are_bounded_and_expire() {
        let mut e = Effects::default();
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.time = 1.0;
        e.observe((1, 0), &w, true);
        e.settle(true);
        for _ in 0..4 {
            w.time += 0.05;
            w.agent.alive = false;
            e.observe((1, 0), &w, true);
            e.settle(true);
            w.time += 0.05;
            w.agent.alive = true;
            e.observe((1, 0), &w, true);
            e.settle(true);
        }
        assert_eq!(e.bursts.len(), DEATH_CAP);
        for _ in 0..4 {
            w.time += 0.2;
            e.observe((1, 0), &w, true);
            e.settle(true);
        }
        assert!(e.bursts.is_empty());
    }

    #[test]
    fn reduced_motion_records_nothing_at_all() {
        let mut e = Effects::default();
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.time = 1.0;
        e.observe((1, 0), &w, true);
        e.settle(true);
        w.time = 1.1;
        w.wave += 1;
        w.agent.alive = false;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert_eq!(e.waves.len(), 1);
        e.observe((1, 0), &w, false);
        assert!(e.echoes.is_empty());
        assert!(e.waves.is_empty());
        assert!(e.bursts.is_empty());
        assert!(e.wave.is_none());
        assert!(e.alive.is_none());
        assert_eq!(e.key, None);
    }

    #[test]
    fn reduced_motion_records_nothing_at_any_speed() {
        // The same field at a ×100 frame's cadence: many steps per frame, hits
        // and a Wave clear in among them, the switch off from the first step.
        // The record stays empty, the tremor stays still, and the frame draws
        // nothing — the window cannot smuggle any of it past the switch.
        let mut w = World::new(sim::Rng::from_seed(1), None);
        let mut rng = sim::Rng::from_seed(2);
        let mut e = Effects::default();
        let mut clock = 1.0;
        w.time = clock;
        for step in 0..100 {
            // Real steps with real hits in them; the field is not cleared, so
            // the World's own Wave escalation does not multiply the rock count
            // out from under the test.
            if step % 25 == 0 {
                strike(&mut w, &mut rng);
            } else {
                w.step_fixed();
            }
            if step == 50 {
                w.wave += 1;
            }
            clock += sim::DT;
            w.time = clock;
            e.observe((1, 0), &w, false);
        }
        e.settle(false);
        assert!(e.echoes.is_empty());
        assert!(e.waves.is_empty());
        assert!(e.bursts.is_empty());
        assert!(e.muzzle.is_none());
        assert_eq!(e.trauma(), 0.0);
        assert_eq!(e.shake(), [0.0, 0.0]);
        let mut p = Painter::new();
        e.draw(
            &mut p,
            ArenaView {
                origin: [0.0, 0.0],
                scale: 1.0,
                tremor: [0.0, 0.0],
            },
        );
        assert!(p.is_empty());
    }

    #[test]
    fn the_window_keeps_only_the_frames_own_tail() {
        // One frame's stepping, with hits far apart in simulation time inside
        // it: the old ones are outside the window that ends at the frame's end
        // and every one of them is dropped, so a fast frame cannot paint its
        // whole history at once.
        let mut w = World::new(sim::Rng::from_seed(1), None);
        let mut rng = sim::Rng::from_seed(2);
        let mut e = Effects::default();
        let mut clock = 1.0;
        w.time = clock;
        e.observe((1, 0), &w, true);

        strike(&mut w, &mut rng);
        clock = 1.01;
        w.time = clock;
        e.observe((1, 0), &w, true);
        assert_eq!(e.staged.len(), 1, "the first hit is observed");

        // Two and a half seconds later in the same frame: the first hit has
        // fallen out of the window behind the clock.
        strike(&mut w, &mut rng);
        clock = 3.51;
        w.time = clock;
        e.observe((1, 0), &w, true);
        assert_eq!(e.staged.len(), 1, "the stale hit did not survive");

        e.settle(true);
        assert_eq!(e.echoes.len(), 1);
        assert_eq!(e.echoes[0].0, clock, "only the window's hit is drawn");

        // And the window ends at the frame's own end: the committed events are
        // drained out of the staging ring, so nothing is drawn twice.
        assert!(e.staged.is_empty());
    }

    #[test]
    fn a_shot_lights_one_muzzle_flash_for_the_frame() {
        let mut e = Effects::default();
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.time = 1.0;
        w.agent.stats.fire = 3;
        // Arriving mid-Episode records the count without firing.
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert!(e.muzzle.is_none());

        w.agent.ship.x = 240.0;
        w.agent.ship.y = 260.0;
        w.agent.ship.heading = 0.0;
        w.agent.stats.fire = 4;
        w.time = 1.1;
        e.observe((1, 0), &w, true);
        e.settle(true);
        let (born, at) = e.muzzle.expect("a shot lights the nose");
        assert_eq!(born, 1.1);
        assert!((at[0] - (240.0 + bullet::NOSE_OFFSET as f32)).abs() < 1e-3);
        assert!((at[1] - 260.0).abs() < 1e-3);

        // Two shots inside one frame are one flash, the most recent one.
        w.time += 0.05;
        w.agent.stats.fire = 5;
        e.observe((1, 0), &w, true);
        w.time += 0.01;
        w.agent.stats.fire = 6;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert_eq!(e.muzzle.map(|(born, _)| born), Some(w.time));

        // And it is a flash: a moment later, nothing.
        w.time += MUZZLE_LIFE;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert!(e.muzzle.is_none());
    }

    #[test]
    fn the_shock_front_opens_from_the_recorded_radius() {
        // The front's leading edge is what the brightest vertices carry, so
        // their distance from the centre is the front's own radius. Measured
        // on the front alone: an impact's sparks fly further than the front
        // and are equally bright, and the point here is that the *front* is
        // sized by the collision rather than by a fixed glow.
        let apex = |front: f32| {
            let mut p = Painter::new();
            shock_front(&mut p, [300.0, 300.0], front, 1.0, color::SHIP_FLAME);
            assert!(!p.luminous.is_empty());
            let brightest = p
                .luminous
                .iter()
                .max_by(|a, b| a.color[3].total_cmp(&b.color[3]))
                .expect("the front drew nothing");
            (brightest.pos[0] - 300.0).hypot(brightest.pos[1] - 300.0)
        };
        // At birth the front is exactly the rock's own recorded radius.
        let born = apex(20.0);
        assert!((born - 20.0).abs() < 0.6, "{born}");
        // And it opens outward from it over the echo's life.
        let later = apex(20.0 + SHOCK_TRAVEL * 0.5);
        let expected = 20.0 + SHOCK_TRAVEL * 0.5;
        assert!((later - expected).abs() < 0.6, "{later} vs {expected}");
    }

    #[test]
    fn a_quiet_frame_draws_no_light() {
        let mut p = Painter::new();
        Effects::default().draw(
            &mut p,
            ArenaView {
                origin: [0.0, 0.0],
                scale: 1.0,
                tremor: [0.0, 0.0],
            },
        );
        assert!(p.is_empty());
    }

    #[test]
    fn a_wave_pulse_is_light_confined_to_the_arena() {
        let e = Effects {
            waves: VecDeque::from([0.0]),
            time: 0.1,
            ..Effects::default()
        };
        let mut p = Painter::new();
        e.draw(
            &mut p,
            ArenaView {
                origin: [0.0, 0.0],
                scale: 1.0,
                tremor: [0.0, 0.0],
            },
        );
        assert!(p.triangles.is_empty());
        assert!(!p.luminous.is_empty());
        assert!(p
            .luminous
            .iter()
            .all(|v| v.pos[0] >= 0.0 && v.pos[0] <= 960.0 && v.pos[1] >= 0.0 && v.pos[1] <= 600.0));
        // All four edges are lit.
        assert!(p.luminous.iter().any(|v| v.pos[1] < 10.0));
        assert!(p.luminous.iter().any(|v| v.pos[1] > 590.0));
        assert!(p.luminous.iter().any(|v| v.pos[0] < 10.0));
        assert!(p.luminous.iter().any(|v| v.pos[0] > 950.0));
    }

    #[test]
    fn the_same_event_draws_the_same_light_every_time() {
        let e = Effects {
            echoes: VecDeque::from([(
                1.0,
                Impact {
                    x: 321.5,
                    y: 200.25,
                    radius: 20.0,
                    ship: false,
                },
            )]),
            bursts: VecDeque::from([(1.2, [40.0, 560.0])]),
            waves: VecDeque::from([0.9]),
            time: 1.25,
            ..Effects::default()
        };
        let view = ArenaView {
            origin: [0.0, 0.0],
            scale: 1.0,
            tremor: [0.0, 0.0],
        };
        let (mut a, mut b) = (Painter::new(), Painter::new());
        e.draw(&mut a, view);
        e.draw(&mut b, view);
        assert!(!a.luminous.is_empty());
        assert_eq!(a.luminous, b.luminous);
        assert_eq!(a.triangles, b.triangles);
    }

    #[test]
    fn spark_seeds_come_from_the_event_identity() {
        let a = spark_seed(321.5, 200.25, 3.0);
        assert_eq!(a, spark_seed(321.5, 200.25, 3.0));
        assert_ne!(a, spark_seed(321.5, 200.25, 3.0 + sim::DT));
        assert_ne!(a, spark_seed(321.51, 200.25, 3.0));
        assert_ne!(a, spark_seed(321.5, 200.26, 3.0));
    }

    /// A World with one bullet about to strike one rock, well clear of the
    /// Ship, so a step produces exactly one Impact.
    fn strike(w: &mut World, rng: &mut sim::Rng) {
        w.agent.ship.x = 100.0;
        w.agent.ship.y = 100.0;
        w.asteroids.clear();
        w.asteroids.push(sim::Asteroid::new(
            sim::config::asteroid::Size::Large,
            500.0,
            500.0,
            0.0,
            0.0,
            rng,
        ));
        w.bullets.clear();
        w.bullets.push(sim::Bullet {
            x: 500.0,
            y: 500.0,
            vx: 0.0,
            vy: 0.0,
            life: 1.0,
        });
        w.step_fixed();
    }

    /// One step with nothing to hit, so the last step's Impacts are cleared.
    fn quiet_step(w: &mut World) {
        w.asteroids.clear();
        w.bullets.clear();
        w.step_fixed();
    }

    #[test]
    fn trauma_rises_by_an_impact_and_falls_at_the_rate_it_is_given() {
        let mut w = World::new(sim::Rng::from_seed(1), None);
        let mut rng = sim::Rng::from_seed(2);
        let mut e = Effects::default();
        let mut clock = 1.0;
        w.time = clock;
        // The first look at an Episode is not a hit.
        e.observe((1, 0), &w, true);
        assert_eq!(e.trauma(), 0.0);

        // One struck rock: one Impact's worth. The level was at nothing, so
        // there was nothing for the step to take off it.
        strike(&mut w, &mut rng);
        clock += sim::DT;
        w.time = clock;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert!(
            (e.trauma() - TRAUMA_PER_IMPACT).abs() < 1e-6,
            "{}",
            e.trauma()
        );

        // Then nothing: the level bleeds away by TRAUMA_DECAY a simulation
        // second. This is a rate, not a per-frame easing, so a quarter of a
        // second takes a quarter of a second's worth off — and the clock it
        // reads is the simulation's, which is what makes a pause hold.
        quiet_step(&mut w);
        clock += 0.25;
        w.time = clock;
        e.observe((1, 0), &w, true);
        e.settle(true);
        let struck = TRAUMA_PER_IMPACT - TRAUMA_DECAY * 0.25;
        assert!((e.trauma() - struck).abs() < 1e-5, "{}", e.trauma());

        // And eventually the quiet takes it to nothing, never below.
        let mut left = struck;
        while left > 0.0 {
            clock += 0.25;
            w.time = clock;
            e.observe((1, 0), &w, true);
            e.settle(true);
            left = (left - TRAUMA_DECAY * 0.25).max(0.0);
            assert!((e.trauma() - left).abs() < 1e-5, "{}", e.trauma());
        }
        clock += 0.5;
        w.time = clock;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert_eq!(e.trauma(), 0.0);
    }

    #[test]
    fn trauma_saturates_at_one_and_a_new_episode_starts_the_field_still() {
        let mut w = World::new(sim::Rng::from_seed(1), None);
        let mut rng = sim::Rng::from_seed(2);
        let mut e = Effects::default();
        let mut time = 1.0;
        w.time = time;
        e.observe((1, 0), &w, true);
        for _ in 0..8 {
            strike(&mut w, &mut rng);
            time += sim::DT;
            w.time = time;
            e.observe((1, 0), &w, true);
            e.settle(true);
            assert!(e.trauma() <= 1.0);
        }
        assert_eq!(e.trauma(), 1.0, "eight strikes do not saturate");

        // A different Episode is a different field, and a restart is a different
        // Episode: no tremor crosses it.
        quiet_step(&mut w);
        w.time += 0.25;
        e.observe((2, 0), &w, true);
        assert_eq!(e.trauma(), 0.0);
        assert_eq!(e.shake(), [0.0, 0.0]);

        // As does clearing the effects outright, which is what the app does on
        // a restart, a new seed and a load.
        w.time += 0.25;
        e.observe((2, 0), &w, true);
        e.clear();
        assert_eq!(e.trauma(), 0.0);
        assert_eq!(e.shake(), [0.0, 0.0]);
    }

    #[test]
    fn reduced_motion_leaves_the_field_exactly_where_it_was() {
        let mut w = World::new(sim::Rng::from_seed(1), None);
        let mut rng = sim::Rng::from_seed(2);
        let mut e = Effects::default();
        w.time = 1.0;
        e.observe((1, 0), &w, true);
        strike(&mut w, &mut rng);
        w.time += sim::DT;
        e.observe((1, 0), &w, true);
        e.settle(true);
        assert!(e.trauma() > 0.0);
        assert_ne!(e.shake(), [0.0, 0.0]);

        // Motion off clears the tremor with the echoes, and the drawing
        // contract is told to shake by nothing.
        e.observe((1, 0), &w, false);
        assert_eq!(e.trauma(), 0.0);
        assert_eq!(e.shake(), [0.0, 0.0]);
    }

    #[test]
    fn a_held_frame_draws_the_offset_it_was_given() {
        // A pause runs no steps, so it observes the same instant again: the
        // level cannot decay and the direction cannot change, which is what
        // makes a paused frame pixel-identical.
        let mut w = World::new(sim::Rng::from_seed(1), None);
        let mut rng = sim::Rng::from_seed(2);
        let mut e = Effects::default();
        w.time = 1.0;
        e.observe((1, 0), &w, true);
        strike(&mut w, &mut rng);
        w.time += sim::DT;
        e.observe((1, 0), &w, true);
        e.settle(true);
        let held = (e.trauma(), e.shake());
        for _ in 0..8 {
            e.observe((1, 0), &w, true);
            e.settle(true);
        }
        assert_eq!((e.trauma(), e.shake()), held);
        e.clear();
    }

    #[test]
    fn the_displacement_is_trauma_squared_bounded_and_held() {
        // Nothing hit, nothing moves — and a clock that never started is not a
        // clock the hash can read.
        assert_eq!(shake(0.0, 1.0), [0.0, 0.0]);
        assert_eq!(shake(0.5, -1.0), [0.0, 0.0]);

        // Half the trauma is a quarter of the displacement: severity, squared.
        let half = shake(0.5, 1.0);
        let full = shake(1.0, 1.0);
        for axis in 0..2 {
            assert!((half[axis] * 4.0 - full[axis]).abs() < 1e-3);
        }

        // Bounded by the amplitude at the level's ceiling, however the hash
        // falls — the cap is a magnitude, not two independent bounds.
        let mut moved = false;
        for step in 0..400 {
            let offset = shake(1.0, step as f64 * sim::DT);
            assert!(offset[0].hypot(offset[1]) <= SHAKE_AMPLITUDE + 1e-4);
            moved |= offset != [0.0, 0.0];
        }
        assert!(moved, "a saturating hit moved nothing at all");

        // The same moment draws the same offset, and a direction is held for
        // its steps rather than redrawn every frame.
        assert_eq!(shake(0.8, 2.0), shake(0.8, 2.0));
        assert_eq!(shake(0.8, 2.0), shake(0.8, 2.0 + sim::DT));
        assert_ne!(
            shake(0.8, 2.0),
            shake(0.8, 2.0 + SHAKE_STEPS as f64 * sim::DT)
        );
    }
}
