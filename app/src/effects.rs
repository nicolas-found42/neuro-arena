//! Bounded cosmetic light, driven by simulation time. No wall clock, and
//! nothing here touches the simulation's RNG: every event is an echo of a
//! change the World already made.
//!
//! Three languages, all of them light:
//! - an Impact flashes gold, rings outward and throws sparks;
//! - a cleared Wave pulses the four Arena edges, the seam the field wraps on;
//! - the Ship's death scatters embers where it died.
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

use crate::{
    painter::{Painter, Rgba},
    scene::ArenaView,
    theme::color,
    vector,
};
use sim::{config::arena, world::Impact, World};
use std::collections::VecDeque;

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

/// The logical Arena.
const W: f32 = arena::WIDTH as f32;
const H: f32 = arena::HEIGHT as f32;

/// How deep a Wave pulse reaches in from an edge, and how many flat bands
/// sample the quadratic falloff (the Painter's gradient polygon is not
/// additive, so the strip is banded light instead).
const WAVE_DEPTH: f32 = 10.0;
const WAVE_BANDS: usize = 4;

/// The longest spark or ember streak, in Arena units.
const STREAK_MAX: f32 = 16.0;
const STREAK_BASE: f32 = 1.4;
const STREAK_TIP: f32 = 0.4;

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

#[derive(Default)]
pub struct Effects {
    key: Option<(u32, usize)>,
    time: f64,
    echoes: VecDeque<(f64, Impact)>,
    /// The Wave count this Episode has already shown; only an increase pulses.
    wave: Option<u32>,
    waves: VecDeque<f64>,
    /// Whether the Ship was alive at the last observation; true→false scatters.
    alive: Option<bool>,
    bursts: VecDeque<(f64, [f64; 2])>,
    /// How hard the field has been hit, in `0..=1`, on the simulation clock.
    trauma: f32,
}
impl Effects {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// How hard the field has been hit: raised by impacts, falling by
    /// [`TRAUMA_DECAY`] a second of simulation time, bounded at one.
    pub fn trauma(&self) -> f32 {
        self.trauma
    }

    /// The Arena's displacement this frame, in Arena units.
    pub fn shake(&self) -> [f32; 2] {
        shake(self.trauma, self.time)
    }

    /// Called after watched steps; never steps or mutates the World itself.
    ///
    /// The frame's arena has already been drawn by the time an Episode is
    /// observed, so the app reads [`Effects::shake`] after this and draws the
    /// next frame with it; while a pause holds the clock, it is the same
    /// offset.
    pub fn observe(&mut self, key: (u32, usize), w: &World, enabled: bool) {
        self.advance(key, w, enabled);
    }

    fn advance(&mut self, key: (u32, usize), w: &World, enabled: bool) {
        if !enabled {
            self.clear();
            return;
        }
        if self.key != Some(key) || w.time < self.time || w.time - self.time > 0.25 {
            self.echoes.clear();
            self.waves.clear();
            self.bursts.clear();
            self.wave = None;
            self.alive = None;
            // A different Episode is a different field: the tremor starts still.
            self.trauma = 0.0;
            self.time = -1.0;
            self.key = Some(key);
        }
        if w.time == self.time {
            return;
        }
        // The tremor ages with the steps that were run, and the first look at
        // an Episode has no step behind it to age from.
        if self.time >= 0.0 {
            self.trauma = (self.trauma - TRAUMA_DECAY * (w.time - self.time) as f32).max(0.0);
        }
        self.time = w.time;
        while self
            .echoes
            .front()
            .is_some_and(|(t, _)| w.time - t > LIFETIME)
        {
            self.echoes.pop_front();
        }
        while self.waves.front().is_some_and(|t| w.time - t > WAVE_LIFE) {
            self.waves.pop_front();
        }
        while self
            .bursts
            .front()
            .is_some_and(|(t, _)| w.time - t > DEATH_LIFE)
        {
            self.bursts.pop_front();
        }
        self.detect_wave(w);
        self.detect_death(w);
        for impact in w.impacts() {
            self.trauma = (self.trauma + TRAUMA_PER_IMPACT).min(1.0);
            if self.echoes.len() == CAPACITY {
                self.echoes.pop_front();
            }
            self.echoes.push_back((w.time, *impact));
        }
    }

    /// A Wave counter that goes up means the field was cleared. The first look
    /// at an Episode only records the count, so arriving mid-Episode — a load,
    /// a replay, a change of watched member — never fires a spurious pulse.
    fn detect_wave(&mut self, w: &World) {
        let previous = self.wave.unwrap_or(w.wave);
        if w.wave > previous {
            if self.waves.len() == WAVE_CAP {
                self.waves.pop_front();
            }
            self.waves.push_back(w.time);
        }
        self.wave = Some(w.wave);
    }

    /// A live Ship that stops being alive scatters embers where it died, and
    /// jolts the field harder than a rock does. The first look records the flag
    /// without firing, for the same reason.
    fn detect_death(&mut self, w: &World) {
        let was_alive = self.alive.unwrap_or(w.agent.alive);
        if was_alive && !w.agent.alive {
            self.trauma = (self.trauma + TRAUMA_PER_DEATH).min(1.0);
            if self.bursts.len() == DEATH_CAP {
                self.bursts.pop_front();
            }
            self.bursts
                .push_back((w.time, [w.agent.ship.x, w.agent.ship.y]));
        }
        self.alive = Some(w.agent.alive);
    }

    pub fn draw(&self, p: &mut Painter, view: ArenaView) {
        p.set_transform(view.transform());
        p.set_clip(Some([0.0, 0.0, W, H]));
        self.draw_waves(p);
        for (born, impact) in &self.echoes {
            self.draw_impact(p, *born, impact);
        }
        for (born, at) in &self.bursts {
            self.draw_burst(p, *born, *at);
        }
        p.set_clip(None);
    }

    /// A cleared Wave pulses the four Arena edges: additive light laid just
    /// inside the seam the field wraps on. No seam copies — the edges are it.
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

    /// One impact: a gold flash, its expanding ring, and its sparks.
    fn draw_impact(&self, p: &mut Painter, born: f64, impact: &Impact) {
        let age = self.age(born, LIFETIME);
        let fade = (1.0 - age).powi(2);
        let ring = 9.0 + age * 16.0;
        let ink = if impact.ship {
            color::WARN
        } else {
            color::SHIP_FLAME
        };
        let seed = spark_seed(impact.x, impact.y, born);
        let at = [impact.x as f32, impact.y as f32];
        seam_copies(p, at, ring + STREAK_MAX, |p, c| {
            p.set_gain(crate::theme::light::IMPACT);
            p.luminous_glow(c, 9.0 + age * 17.0, color::LIGHT_IMPACT.alpha(fade * 0.95));
            p.set_gain(1.0);
            p.path(
                &vector::arc(c, ring, 0.0, std::f32::consts::TAU),
                1.6,
                ink.alpha((1.0 - age) * 0.55),
            );
            p.set_gain(crate::theme::light::SPARK);
            // The same fan in every seam copy: one event, translated.
            let mut seed = seed;
            let count = 6 + (lcg_unit(&mut seed) * 5.0) as usize;
            for i in 0..count {
                let spread = std::f32::consts::TAU / count as f32;
                let angle = spread * i as f32 + lcg_unit(&mut seed) * spread;
                let length = 7.0 + lcg_unit(&mut seed) * 10.0;
                let streak = if lcg_unit(&mut seed) < 0.3 {
                    color::SHIP_FLAME
                } else {
                    color::LIGHT_IMPACT
                };
                streak_at(p, c, angle, length, streak.alpha((1.0 - age).powf(1.5)));
            }
            p.set_gain(1.0);
        });
    }

    /// The Ship's death: one flash where it died, and its embers scattering.
    fn draw_burst(&self, p: &mut Painter, born: f64, at: [f64; 2]) {
        let age = self.age(born, DEATH_LIFE);
        let fade = (1.0 - age).powi(2);
        let ember = (1.0 - age).powf(1.5) * 0.7;
        let flash = color::SHIP_DEAD.mix(color::LIGHT_IMPACT, 0.5);
        let seed = spark_seed(at[0], at[1], born);
        let at = [at[0] as f32, at[1] as f32];
        seam_copies(p, at, 26.0 + STREAK_MAX, |p, c| {
            p.set_gain(crate::theme::light::IMPACT);
            p.luminous_glow(c, 8.0 + age * 18.0, flash.alpha(fade * 0.7));
            p.set_gain(crate::theme::light::SPARK);
            let mut seed = seed;
            let count = 12 + (lcg_unit(&mut seed) * 5.0) as usize;
            for i in 0..count {
                let spread = std::f32::consts::TAU / count as f32;
                let angle = spread * i as f32 + lcg_unit(&mut seed) * spread;
                let length = 6.0 + lcg_unit(&mut seed) * 10.0;
                let ink = color::SHIP_DEAD.mix(color::SHIP_FLAME, lcg_unit(&mut seed));
                streak_at(p, c, angle, length, ink.alpha(ember));
            }
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

/// A thin streak from the origin of an event, tapering to its tip.
fn streak_at(p: &mut Painter, at: [f32; 2], angle: f32, length: f32, ink: Rgba) {
    light_segment(
        p,
        at,
        [at[0] + angle.cos() * length, at[1] + angle.sin() * length],
        STREAK_BASE,
        STREAK_TIP,
        ink,
    )
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

/// A strip of light lying against an Arena edge, fading quadratically inward.
/// The falloff is sampled at each band's bright edge, so the strip starts at
/// exactly `strength` and decays in `WAVE_BANDS` steps.
fn edge_strip(
    p: &mut Painter,
    a: [f32; 2],
    b: [f32; 2],
    inward: [f32; 2],
    ink: Rgba,
    strength: f32,
) {
    let band = WAVE_DEPTH / WAVE_BANDS as f32;
    for i in 0..WAVE_BANDS {
        let outer = i as f32 * band;
        let inner = outer + band;
        let ink = ink.alpha(strength * (1.0 - outer / WAVE_DEPTH).powi(2));
        let p0 = [a[0] + inward[0] * outer, a[1] + inward[1] * outer];
        let p1 = [b[0] + inward[0] * outer, b[1] + inward[1] * outer];
        let p2 = [b[0] + inward[0] * inner, b[1] + inward[1] * inner];
        let p3 = [a[0] + inward[0] * inner, a[1] + inward[1] * inner];
        p.luminous_triangle(p0, p1, p2, ink);
        p.luminous_triangle(p0, p2, p3, ink);
    }
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
        assert!(p
            .triangles
            .iter()
            .all(|v| v.pos[0] >= 0.0 && v.pos[0] <= 960.0 && v.pos[1] >= 0.0 && v.pos[1] <= 600.0));
        assert!(p.triangles.iter().any(|v| v.pos[0] > 930.0));
        assert!(p
            .luminous
            .iter()
            .all(|v| v.pos[0] >= 0.0 && v.pos[0] <= 960.0 && v.pos[1] >= 0.0 && v.pos[1] <= 600.0));
        for _ in 0..4 {
            w.time += 0.2;
            e.observe((1, 0), &w, true);
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
        // The first look at an Episode records the count without pulsing.
        assert!(e.waves.is_empty());
        w.time = 1.1;
        w.wave = 5;
        e.observe((1, 0), &w, true);
        assert_eq!(e.waves.len(), 1);
        // The same instant is not a second pulse, a quiet count is not one
        // either, and a count that drops is not an increase.
        e.observe((1, 0), &w, true);
        assert_eq!(e.waves.len(), 1);
        w.time = 1.2;
        e.observe((1, 0), &w, true);
        assert_eq!(e.waves.len(), 1);
        w.wave = 2;
        w.time = 1.3;
        e.observe((1, 0), &w, true);
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
        }
        assert_eq!(e.waves.len(), WAVE_CAP);
        for _ in 0..4 {
            w.time += 0.23;
            e.observe((1, 0), &w, true);
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
        assert!(e.bursts.is_empty());
        w.agent.alive = true;
        w.time = 1.1;
        e.observe((1, 0), &w, true);
        assert!(e.bursts.is_empty());
        w.agent.ship.x = 240.0;
        w.agent.ship.y = 260.0;
        w.agent.alive = false;
        w.time = 1.2;
        e.observe((1, 0), &w, true);
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
        for _ in 0..4 {
            w.time += 0.05;
            w.agent.alive = false;
            e.observe((1, 0), &w, true);
            w.time += 0.05;
            w.agent.alive = true;
            e.observe((1, 0), &w, true);
        }
        assert_eq!(e.bursts.len(), DEATH_CAP);
        for _ in 0..4 {
            w.time += 0.2;
            e.observe((1, 0), &w, true);
        }
        assert!(e.bursts.is_empty());
    }

    #[test]
    fn reduced_motion_records_nothing_at_all() {
        let mut e = Effects::default();
        let mut w = World::new(sim::Rng::from_seed(1), None);
        w.time = 1.0;
        e.observe((1, 0), &w, true);
        w.time = 1.1;
        w.wave += 1;
        w.agent.alive = false;
        e.observe((1, 0), &w, true);
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
        let struck = TRAUMA_PER_IMPACT - TRAUMA_DECAY * 0.25;
        assert!((e.trauma() - struck).abs() < 1e-5, "{}", e.trauma());

        // And eventually the quiet takes it to nothing, never below.
        let mut left = struck;
        while left > 0.0 {
            clock += 0.25;
            w.time = clock;
            e.observe((1, 0), &w, true);
            left = (left - TRAUMA_DECAY * 0.25).max(0.0);
            assert!((e.trauma() - left).abs() < 1e-5, "{}", e.trauma());
        }
        clock += 0.5;
        w.time = clock;
        e.observe((1, 0), &w, true);
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
        let held = (e.trauma(), e.shake());
        for _ in 0..8 {
            e.observe((1, 0), &w, true);
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
