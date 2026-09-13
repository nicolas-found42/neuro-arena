//! Bounded collision echoes, driven by simulation time. No wall clock or RNG.
use crate::{painter::Painter, scene::ArenaView, theme::color, vector};
use sim::{config::arena, world::Impact, World};
use std::collections::VecDeque;

const LIFETIME: f64 = 0.55;
const CAPACITY: usize = 24;

#[derive(Default)]
pub struct Effects {
    key: Option<(u32, usize)>,
    time: f64,
    echoes: VecDeque<(f64, Impact)>,
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
        for impact in w.impacts() {
            if self.echoes.len() == CAPACITY {
                self.echoes.pop_front();
            }
            self.echoes.push_back((w.time, *impact));
        }
    }

    pub fn draw(&self, p: &mut Painter, view: ArenaView) {
        p.set_transform(view.transform());
        p.set_clip(Some([0.0, 0.0, arena::WIDTH as f32, arena::HEIGHT as f32]));
        for (time, impact) in &self.echoes {
            let age = ((self.time - time) / LIFETIME).clamp(0.0, 1.0) as f32;
            let fade = (1.0 - age).powi(2);
            let radius = impact.radius as f32 * (0.4 + age) + 8.0;
            let ink = if impact.ship {
                color::WARN
            } else {
                color::SHIP_FLAME
            };
            for ox in [-arena::WIDTH as f32, 0.0, arena::WIDTH as f32] {
                for oy in [-arena::HEIGHT as f32, 0.0, arena::HEIGHT as f32] {
                    let c = [impact.x as f32 + ox, impact.y as f32 + oy];
                    if c[0] + radius + 12.0 < 0.0
                        || c[1] + radius + 12.0 < 0.0
                        || c[0] - radius - 12.0 > arena::WIDTH as f32
                        || c[1] - radius - 12.0 > arena::HEIGHT as f32
                    {
                        continue;
                    }
                    p.path(
                        &vector::arc(c, radius, 0.0, std::f32::consts::TAU),
                        1.1,
                        ink.alpha(fade * 0.45),
                    );
                    for i in 0..8 {
                        let a = i as f32 * std::f32::consts::TAU / 8.0 + impact.x as f32 * 0.01;
                        let at = |r| [c[0] + a.cos() * r, c[1] + a.sin() * r];
                        p.stroke(
                            at(radius),
                            at(radius + 6.0 * (1.0 - age)),
                            1.3,
                            ink.alpha(fade * 0.8),
                        );
                    }
                }
            }
        }
        p.set_clip(None);
    }
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
}
