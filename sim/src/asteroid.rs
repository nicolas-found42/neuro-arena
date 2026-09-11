//! Asteroid state, shape and rock-rock physics.
//!
//! Every rock is a jittered polygon: the vertex radii are drawn at creation and
//! carried for the rock's life, so a split inherits a fresh shape rather than a
//! scaled parent (a rule the old rendition also followed).

use crate::config::asteroid::{self as ast, Size};
use crate::math::{tdx, tdy, wrap_xy};
use crate::rng::Rng;

/// A toroidal polygon obstacle.
#[derive(Clone, Debug)]
pub struct Asteroid {
    pub size: Size,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    /// Collision radius: the nominal size radius, not the jittered silhouette.
    pub r: f64,
    /// Points banked to the Ship that destroys it.
    pub points: f64,
    /// Vertex radius multipliers, one per vertex.
    pub shape: [f64; ast::VERTICES],
    pub angle: f64,
    /// Spin, visual only: it never feeds physics.
    pub spin: f64,
}

impl Asteroid {
    /// A rock of `size` at `(x, y)` moving along `dir` at `speed`.
    ///
    /// Draw order is part of the stream contract: the shape radii, then the
    /// start angle, then the spin.
    pub fn new(size: Size, x: f64, y: f64, dir: f64, speed: f64, rng: &mut Rng) -> Self {
        let mut shape = [1.0; ast::VERTICES];
        for v in shape.iter_mut() {
            *v = rng.range(ast::JITTER_MIN, ast::JITTER_MAX);
        }
        Self {
            size,
            x,
            y,
            vx: dir.cos() * speed,
            vy: dir.sin() * speed,
            r: size.radius(),
            points: size.points(),
            shape,
            angle: rng.range(0.0, std::f64::consts::TAU),
            spin: rng.range(-ast::SPIN_MAX, ast::SPIN_MAX),
        }
    }

    /// Advance one fixed step and wrap across the seam.
    pub fn advance(&mut self, dt: f64) {
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        wrap_xy(&mut self.x, &mut self.y);
        self.angle += self.spin * dt;
    }

    /// Clamp to the global speed cap: momentum inheritance compounds through
    /// splits, so a chain of hits could otherwise accelerate a rock without bound.
    pub fn cap_speed(&mut self) {
        let sp = self.vx.hypot(self.vy);
        if sp > ast::SPEED_CAP {
            let k = ast::SPEED_CAP / sp;
            self.vx *= k;
            self.vy *= k;
        }
    }

    /// Vertex offsets in Arena space, for rendering.
    pub fn vertices(&self, out: &mut Vec<(f64, f64)>) {
        out.clear();
        let step = std::f64::consts::TAU / ast::VERTICES as f64;
        for (i, jitter) in self.shape.iter().enumerate() {
            let a = self.angle + step * i as f64;
            let r = self.r * jitter;
            out.push((self.x + a.cos() * r, self.y + a.sin() * r));
        }
    }
}

/// Positional de-overlap plus an elastic impulse for every approaching pair.
/// Mass goes as radius squared; separating pairs are pushed apart but not
/// re-impelled.
pub fn collide_asteroids(list: &mut [Asteroid]) {
    for i in 0..list.len() {
        for j in i + 1..list.len() {
            let (a, b) = two_mut(list, i, j);
            let dx = tdx(b.x, a.x);
            let dy = tdy(b.y, a.y);
            let rr = a.r + b.r;
            let d2 = dx * dx + dy * dy;
            if d2 >= rr * rr || d2 == 0.0 {
                continue;
            }
            let d = d2.sqrt();
            let nx = dx / d;
            let ny = dy / d;
            let ma = a.r * a.r;
            let mb = b.r * b.r;
            let overlap = rr - d;
            let total = ma + mb;
            a.x -= nx * overlap * (mb / total);
            a.y -= ny * overlap * (mb / total);
            b.x += nx * overlap * (ma / total);
            b.y += ny * overlap * (ma / total);
            let rvn = (b.vx - a.vx) * nx + (b.vy - a.vy) * ny;
            if rvn >= 0.0 {
                continue;
            }
            let impulse = (-(1.0 + ast::RESTITUTION) * rvn) / (1.0 / ma + 1.0 / mb);
            a.vx -= (impulse / ma) * nx;
            a.vy -= (impulse / ma) * ny;
            b.vx += (impulse / mb) * nx;
            b.vy += (impulse / mb) * ny;
            a.cap_speed();
            b.cap_speed();
        }
    }
}

/// Two distinct mutable references into one slice.
fn two_mut(list: &mut [Asteroid], i: usize, j: usize) -> (&mut Asteroid, &mut Asteroid) {
    debug_assert!(i != j);
    let (lo, hi) = list.split_at_mut(j.max(i));
    if i < j {
        (&mut lo[i], &mut hi[0])
    } else {
        (&mut hi[0], &mut lo[j])
    }
}
