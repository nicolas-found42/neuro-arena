//! The Agent's 21 inputs: nine toroidal Sensor Rays plus threat telemetry.
//!
//! Sensing is pure — a function of the Agent and the rock field — so the whole
//! input vector is testable without stepping a World. The Arena is toroidal, so
//! rocks are seen across the seam and every distance here is a wrapped one.

use crate::asteroid::Asteroid;
use crate::config::asteroid::Size;
use crate::config::{bullet, nn, sensors as sens};
use crate::math::{clamp, tdx, tdy};
use crate::world::Agent;

const DEG: f64 = std::f64::consts::PI / 180.0;

/// Compute the input vector for one step. The caller stores it on the Agent for
/// the Ray overlay; the Network consumes the same array.
///
/// Input order is fixed (see [`crate::config::nn`]). An input is written in
/// exactly one place, and every value is normalized to a bounded range so a
/// weight means the same thing on every channel.
pub fn sense(agent: &Agent, asteroids: &[Asteroid]) -> [f64; nn::INPUTS] {
    let ship = &agent.ship;
    let mut inputs = [0.0; nn::INPUTS];
    inputs[nn::BIAS_INPUT] = 1.0;
    inputs[9] = clamp(ship.vx / sens::VEL_SCALE, -1.0, 1.0);
    inputs[10] = clamp(ship.vy / sens::VEL_SCALE, -1.0, 1.0);

    // Sensor Rays: nearest intersection with the rock field, toroidally.
    for (k, offset_deg) in sens::RAY_OFFSETS_DEG.iter().enumerate() {
        let ang = ship.heading + offset_deg * DEG;
        let dx = ang.cos();
        let dy = ang.sin();
        let mut nearest = f64::INFINITY;
        for rock in asteroids {
            let ox = tdx(rock.x, ship.x);
            let oy = tdy(rock.y, ship.y);
            let t = ox * dx + oy * dy;
            if t < 0.0 {
                continue;
            }
            let perp2 = ox * ox + oy * oy - t * t;
            let r2 = rock.r * rock.r;
            if perp2 > r2 {
                continue;
            }
            let d = (t - (r2 - perp2).sqrt()).max(0.0);
            if d < nearest {
                nearest = d;
            }
        }
        inputs[k] = if nearest.is_finite() {
            clamp(1.0 - nearest / sens::RANGE, 0.0, 1.0)
        } else {
            0.0
        };
    }

    // Threat radar: one toroidal pass. The "second nearest" is the previous
    // nearest when a closer rock displaces it — an approximation the browser
    // rendition pinned, kept so the input keeps its meaning.
    let mut nearest_d2 = f64::INFINITY;
    let mut second_d2 = f64::INFINITY;
    let (mut nx, mut ny) = (0.0, 0.0);
    let (mut nvx, mut nvy) = (0.0, 0.0);
    let mut nr = 1.0;
    let mut pressure = 0.0;
    for rock in asteroids {
        let dx = tdx(rock.x, ship.x);
        let dy = tdy(rock.y, ship.y);
        let d2 = dx * dx + dy * dy;
        if d2 < nearest_d2 {
            second_d2 = nearest_d2;
            nearest_d2 = d2;
            nx = dx;
            ny = dy;
            nvx = rock.vx;
            nvy = rock.vy;
            nr = rock.r;
        } else if d2 < second_d2 {
            second_d2 = d2;
        }
        pressure += 1.0 - d2.sqrt().min(sens::RANGE) / sens::RANGE;
    }

    if nearest_d2.is_finite() {
        let d = nearest_d2.sqrt();
        // The subtlety of atan2 differences: normalize to [-pi, pi] first, so
        // +1 is "hard right" and -1 is "hard left".
        inputs[12] = crate::math::wrap_angle(ny.atan2(nx) - ship.heading) / std::f64::consts::PI;
        inputs[13] = clamp(1.0 - d / sens::RANGE, 0.0, 1.0);
        // d(dist)/dt: negative means the rock is closing.
        let closing = (nx * (nvx - ship.vx) + ny * (nvy - ship.vy)) / d;
        inputs[14] = clamp(closing / sens::VEL_SCALE, -1.0, 1.0);
        // Tangential relative velocity: which way the threat crosses the nose.
        let lateral = (-ny * (nvx - ship.vx) + nx * (nvy - ship.vy)) / d;
        inputs[16] = clamp(lateral / sens::VEL_SCALE, -1.0, 1.0);
        inputs[18] = clamp(nr / Size::Large.radius(), 0.0, 1.0);
    }
    if second_d2.is_finite() {
        inputs[17] = clamp(1.0 - second_d2.sqrt() / sens::RANGE, 0.0, 1.0);
    }
    inputs[19] = clamp(pressure / sens::PRESSURE_NORM, 0.0, 1.0);
    inputs[15] = f64::from(agent.bullets_out) / f64::from(bullet::MAX_ALIVE_PER_SHIP);
    inputs[nn::MEMORY_INPUT] = agent.memory;
    inputs
}
