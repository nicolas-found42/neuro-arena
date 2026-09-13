//! World: one toroidal Arena holding one Agent through one Episode.
//!
//! A World is a value, not a global: construct it from a seeded stream and step
//! it. Nothing here knows about windows, threads or the Population, which is
//! what makes an Episode reproducible on any core count (ADR 0005).
//!
//! The step order is a rule, not an implementation detail, because it decides
//! what a Ship can see and hit:
//! sense → think → act → move → die to a asteroid; then Bullets; then Asteroids;
//! then asteroid-asteroid collisions; then Wave escalation and Episode end.

use crate::asteroid::{collide_asteroids, Asteroid};
use crate::config::asteroid::{self as ast, Size};
use crate::config::{bullet, fitness, nn, ship as ship_cfg, world as world_cfg, DT};
use crate::evaluation::{behavior, entropy_bonus, Behavior};
use crate::math::{tdx, tdy, wrap_xy};
use crate::network::Network;
use crate::rng::Rng;
use crate::sensors::sense;

const DEG: f64 = std::f64::consts::PI / 180.0;
/// Horizontal bands of the arena-coverage grid (8 columns × 5 rows = 40 cells).
const COVERAGE_CELLS: f64 = 40.0;

/// The physical vessel: position, heading and velocity.
#[derive(Clone, Copy, Debug)]
pub struct Ship {
    pub x: f64,
    pub y: f64,
    pub heading: f64,
    pub vx: f64,
    pub vy: f64,
}

/// Per-Episode bookkeeping: what the Competence Gate and the novelty descriptor
/// read. Never affects physics.
#[derive(Clone, Debug)]
pub struct Stats {
    pub steps: u64,
    pub left: u64,
    pub right: u64,
    pub thrust: u64,
    pub fire: u64,
    pub speed_sum: f64,
    /// Bitset over the 8×5 arena-coverage grid.
    pub cells: u64,
    pub alive_time: f64,
    pub asteroid_points: f64,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            steps: 0,
            left: 0,
            right: 0,
            thrust: 0,
            fire: 0,
            speed_sum: 0.0,
            cells: 0,
            alive_time: 0.0,
            asteroid_points: 0.0,
        }
    }
}

impl Stats {
    pub fn covered_cells(&self) -> u32 {
        self.cells.count_ones()
    }
}

/// The Ship paired with the Network that controls it, for one Episode.
pub struct Agent {
    pub ship: Ship,
    pub alive: bool,
    pub fitness: f64,
    pub fire_cooldown: f64,
    pub bullets_out: u32,
    /// Last step's memory output, fed back as input 20.
    pub memory: f64,
    pub thrusting: bool,
    /// Last sensor frame, kept for the Sensor Ray overlay.
    pub inputs: [f64; nn::INPUTS],
    pub stats: Stats,
    network: Option<Network>,
}

impl Agent {
    pub fn network(&self) -> Option<&Network> {
        self.network.as_ref()
    }

    pub fn into_network(self) -> Option<Network> {
        self.network
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Bullet {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub life: f64,
}

/// A collision already resolved by physics. Presentation may read the last step's
/// bounded record; it cannot change the outcome or consume simulation randomness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub ship: bool,
}

/// One Episode: the Arena, one Agent, one asteroid field.
pub struct World {
    rng: Rng,
    pub time: f64,
    pub wave_time: f64,
    /// Waves cleared this Episode.
    pub wave: u32,
    pub done: bool,
    finalized: bool,
    pub agent: Agent,
    pub asteroids: Vec<Asteroid>,
    pub bullets: Vec<Bullet>,
    asteroid_count: usize,
    impacts: [Option<Impact>; bullet::MAX_ALIVE_PER_SHIP as usize + 1],
}

impl World {
    /// A fresh Episode. The `network` controls the Ship; `None` gives a
    /// Ship that never acts, which is what the sensors-only probes use.
    pub fn new(rng: Rng, network: Option<Network>) -> Self {
        let mut rng = rng;
        let ship = Ship {
            x: crate::config::arena::WIDTH / 2.0
                + rng.range(-ship_cfg::SPAWN_JITTER, ship_cfg::SPAWN_JITTER),
            y: crate::config::arena::HEIGHT / 2.0
                + rng.range(-ship_cfg::SPAWN_JITTER, ship_cfg::SPAWN_JITTER),
            heading: rng.range(0.0, std::f64::consts::TAU),
            vx: 0.0,
            vy: 0.0,
        };
        let mut world = Self {
            rng,
            time: 0.0,
            wave_time: 0.0,
            wave: 0,
            done: false,
            finalized: false,
            agent: Agent {
                ship,
                alive: true,
                fitness: 0.0,
                fire_cooldown: 0.0,
                bullets_out: 0,
                memory: 0.0,
                thrusting: false,
                inputs: [0.0; nn::INPUTS],
                stats: Stats::default(),
                network,
            },
            asteroids: Vec::new(),
            bullets: Vec::new(),
            asteroid_count: ast::INITIAL_COUNT,
            impacts: [None; bullet::MAX_ALIVE_PER_SHIP as usize + 1],
        };
        world.spawn_wave();
        world
    }

    /// Step the World by the fixed timestep.
    pub fn step_fixed(&mut self) {
        self.step(DT);
    }

    pub fn impacts(&self) -> impl Iterator<Item = &Impact> {
        self.impacts.iter().flatten()
    }

    fn record_impact(&mut self, impact: Impact) {
        if let Some(slot) = self.impacts.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(impact);
        }
    }

    /// Run to Episode end, or to the hard cap. Returns the number of steps run.
    /// The cap is a safety net for a Ship that cannot die; the World itself
    /// ends the Episode at [`world_cfg::EPISODE_HARD_CAP`].
    pub fn run_to_end(&mut self) -> u64 {
        let cap = (world_cfg::EPISODE_HARD_CAP / DT).ceil() as u64 + 10;
        let mut steps = 0;
        while !self.done && steps < cap {
            self.step_fixed();
            steps += 1;
        }
        steps
    }

    pub fn step(&mut self, dt: f64) {
        self.impacts.fill(None);
        self.time += dt;
        let hit_pad = ship_cfg::RADIUS * ship_cfg::HITBOX_FACTOR;

        if self.agent.alive {
            let mut fired = false;
            // The World always produces a sensor frame for its Agent: the rays
            // are what the overlay draws and what a probe reads, whether or not
            // a Network is steering.
            let inputs = sense(&self.agent, &self.asteroids);
            self.agent.inputs = inputs;
            if let Some(network) = &mut self.agent.network {
                let out = network.activate(&inputs);
                let left = out[0] > nn::ACTION_THRESHOLD;
                let right = out[1] > nn::ACTION_THRESHOLD;
                let turn = i32::from(right) - i32::from(left);
                let ship = &mut self.agent.ship;
                ship.heading += f64::from(turn) * ship_cfg::ROTATE_SPEED * dt;
                self.agent.thrusting = out[2] > nn::ACTION_THRESHOLD;
                if self.agent.thrusting {
                    ship.vx += ship.heading.cos() * ship_cfg::THRUST * dt;
                    ship.vy += ship.heading.sin() * ship_cfg::THRUST * dt;
                }
                if out[3] > nn::ACTION_THRESHOLD
                    && self.agent.fire_cooldown <= 0.0
                    && self.agent.bullets_out < bullet::MAX_ALIVE_PER_SHIP
                {
                    self.bullets.push(Bullet {
                        x: ship.x + ship.heading.cos() * bullet::NOSE_OFFSET,
                        y: ship.y + ship.heading.sin() * bullet::NOSE_OFFSET,
                        vx: ship.vx + ship.heading.cos() * bullet::SPEED,
                        vy: ship.vy + ship.heading.sin() * bullet::SPEED,
                        life: bullet::LIFE,
                    });
                    self.agent.fire_cooldown = bullet::COOLDOWN;
                    self.agent.bullets_out += 1;
                    self.agent.fitness -= fitness::BULLET_COST;
                    ship.vx -= ship.heading.cos() * bullet::RECOIL;
                    ship.vy -= ship.heading.sin() * bullet::RECOIL;
                    fired = true;
                }
                self.agent.memory = out[nn::MEMORY_OUTPUT];
                let stats = &mut self.agent.stats;
                stats.steps += 1;
                if left {
                    stats.left += 1;
                }
                if right {
                    stats.right += 1;
                }
                if self.agent.thrusting {
                    stats.thrust += 1;
                }
                if fired {
                    stats.fire += 1;
                }
            }

            self.agent.fire_cooldown -= dt;
            let ship = &mut self.agent.ship;
            let damp = (1.0 - ship_cfg::DAMPING * dt).max(0.0);
            ship.vx *= damp;
            ship.vy *= damp;
            let sp = ship.vx.hypot(ship.vy);
            if sp > ship_cfg::MAX_SPEED {
                let k = ship_cfg::MAX_SPEED / sp;
                ship.vx *= k;
                ship.vy *= k;
            }
            ship.x += ship.vx * dt;
            ship.y += ship.vy * dt;
            wrap_xy(&mut ship.x, &mut ship.y);
            self.agent.fitness += fitness::ALIVE_PER_SECOND * dt;
            self.agent.fitness += fitness::MOVE_RATE * (sp / ship_cfg::MAX_SPEED) * dt;
            let stats = &mut self.agent.stats;
            stats.alive_time += dt;
            stats.speed_sum += sp;
            stats.cells |= coverage_bit(ship.x, ship.y);

            for asteroid in &self.asteroids {
                let dx = tdx(asteroid.x, ship.x);
                let dy = tdy(asteroid.y, ship.y);
                let rr = asteroid.r + hit_pad;
                if dx * dx + dy * dy < rr * rr {
                    self.agent.alive = false;
                    self.impacts[0] = Some(Impact {
                        x: ship.x,
                        y: ship.y,
                        radius: ship_cfg::RADIUS,
                        ship: true,
                    });
                    break;
                }
            }
        }

        // Bullets: move, expire, and split the asteroids they hit.
        for index in (0..self.bullets.len()).rev() {
            let (bx, by) = {
                let b = &mut self.bullets[index];
                b.x += b.vx * dt;
                b.y += b.vy * dt;
                wrap_xy(&mut b.x, &mut b.y);
                b.life -= dt;
                (b.x, b.y)
            };
            if self.bullets[index].life <= 0.0 {
                self.bullets.remove(index);
                self.agent.bullets_out = self.agent.bullets_out.saturating_sub(1);
                continue;
            }
            for asteroid_index in (0..self.asteroids.len()).rev() {
                let asteroid = &self.asteroids[asteroid_index];
                let dx = tdx(asteroid.x, bx);
                let dy = tdy(asteroid.y, by);
                let rr = asteroid.r + bullet::RADIUS;
                if dx * dx + dy * dy < rr * rr {
                    self.bullets.remove(index);
                    self.agent.bullets_out = self.agent.bullets_out.saturating_sub(1);
                    self.agent.fitness += asteroid.points;
                    self.agent.stats.asteroid_points += asteroid.points;
                    self.record_impact(Impact {
                        x: asteroid.x,
                        y: asteroid.y,
                        radius: asteroid.r,
                        ship: false,
                    });
                    self.split_asteroid(asteroid_index);
                    break;
                }
            }
        }

        for asteroid in &mut self.asteroids {
            asteroid.advance(dt);
        }
        collide_asteroids(&mut self.asteroids);

        // Wave escalation: clearing the field grows the next Wave.
        if self.asteroids.is_empty() {
            self.wave += 1;
            self.asteroid_count = (self.asteroid_count as f64 * ast::WAVE_GROWTH).ceil() as usize;
            self.spawn_wave();
            self.wave_time = 0.0;
        }

        self.wave_time += dt;
        self.done = !self.agent.alive
            || self.wave_time >= world_cfg::WAVE_TIME_LIMIT
            || self.time >= world_cfg::EPISODE_HARD_CAP;
        if self.done && !self.finalized {
            self.finalized = true;
            self.agent.fitness += entropy_bonus(&self.agent);
        }
    }

    fn spawn_wave(&mut self) {
        let ship = self.agent.ship;
        for _ in 0..self.asteroid_count {
            let (x, y) = loop {
                let x = self.rng.range(0.0, crate::config::arena::WIDTH);
                let y = self.rng.range(0.0, crate::config::arena::HEIGHT);
                if crate::math::tdist(x, y, ship.x, ship.y) >= ast::INITIAL_MIN_DIST_FROM_SHIP {
                    break (x, y);
                }
            };
            let speed_range = Size::Large.speed_range();
            let asteroid = Asteroid::new(
                Size::Large,
                x,
                y,
                self.rng.range(0.0, std::f64::consts::TAU),
                self.rng.range(speed_range.0, speed_range.1),
                &mut self.rng,
            );
            self.asteroids.push(asteroid);
        }
    }

    /// Split a asteroid into two children that inherit the parent's momentum plus a
    /// radial spread impulse. A small asteroid simply disappears.
    ///
    /// The field is order-sensitive (pairwise collisions and nearest-asteroid ties
    /// both read it in order), so removal keeps the list order.
    fn split_asteroid(&mut self, index: usize) {
        let parent = self.asteroids.remove(index);
        let Some(child_size) = parent.size.child() else {
            return;
        };
        let dir = parent.vy.atan2(parent.vx);
        let (min_speed, max_speed) = child_size.speed_range();
        for sign in [1.0, -1.0] {
            let d = dir + sign * self.rng.range(ast::SPLIT_ANGLE_MIN, ast::SPLIT_ANGLE_MAX) * DEG;
            let impulse = self.rng.range(min_speed, max_speed) * ast::SPLIT_IMPULSE_FACTOR;
            let mut child = Asteroid::new(child_size, parent.x, parent.y, d, 0.0, &mut self.rng);
            child.vx = parent.vx + d.cos() * impulse;
            child.vy = parent.vy + d.sin() * impulse;
            child.cap_speed();
            self.asteroids.push(child);
        }
    }

    /// The novelty descriptor for this Episode.
    pub fn behavior(&self) -> Behavior {
        behavior(&self.agent, self.wave)
    }
}

/// The coverage grid packs `(column << 3) | row` over 8 columns and 5 rows.
fn coverage_bit(x: f64, y: f64) -> u64 {
    let column = ((x / (crate::config::arena::WIDTH / 8.0)).floor() as i64).clamp(0, 7);
    let row = ((y / (crate::config::arena::HEIGHT / 5.0)).floor() as i64).clamp(0, 4);
    1u64 << ((column << 3) | row) as u32
}

/// Fraction of the coverage grid visited, for the behavior descriptor.
pub fn coverage_fraction(agent: &Agent) -> f64 {
    f64::from(agent.stats.covered_cells()) / COVERAGE_CELLS
}
