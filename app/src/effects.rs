//! Bounded cosmetic light, driven by simulation time. No wall clock, and
//! nothing here touches the simulation's RNG: every event is an echo of a
//! change the World already made.
//!
//! Three languages, all of them light:
//! - an Impact flashes gold, rings outward and throws sparks;
//! - a cleared Wave pulses the four Arena edges, the seam the field wraps on;
//! - the Ship's death scatters embers where it died.
//!
//! Events age on `World::time`, so they freeze with a pause and a replay draws
//! the frame it drew the first time. Reduced motion clears them entirely.

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
}
impl Effects {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Called after watched steps; never steps or mutates the World itself.
    pub fn observe(&mut self, key: (u32, usize), w: &World, enabled: bool) {
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
            self.time = -1.0;
            self.key = Some(key);
        }
        if w.time == self.time {
            return;
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

    /// A live Ship that stops being alive scatters embers where it died. The
    /// first look records the flag without firing, for the same reason.
    fn detect_death(&mut self, w: &World) {
        let was_alive = self.alive.unwrap_or(w.agent.alive);
        if was_alive && !w.agent.alive {
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
}
