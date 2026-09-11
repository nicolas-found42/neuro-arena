//! Toroidal geometry for the Arena. Every distance in the simulation is taken
//! across the seam, so these helpers are the only place the wrap lives.

use crate::config::arena::{HEIGHT, WIDTH};

/// Wrapped delta on the x axis, in `[-W/2, W/2]`.
#[inline]
pub fn tdx(ax: f64, sx: f64) -> f64 {
    let mut d = ax - sx;
    if d > WIDTH / 2.0 {
        d -= WIDTH;
    } else if d < -WIDTH / 2.0 {
        d += WIDTH;
    }
    d
}

/// Wrapped delta on the y axis, in `[-H/2, H/2]`.
#[inline]
pub fn tdy(ay: f64, sy: f64) -> f64 {
    let mut d = ay - sy;
    if d > HEIGHT / 2.0 {
        d -= HEIGHT;
    } else if d < -HEIGHT / 2.0 {
        d += HEIGHT;
    }
    d
}

/// Wrapped distance between two positions.
#[inline]
pub fn tdist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = tdx(ax, bx);
    let dy = tdy(ay, by);
    (dx * dx + dy * dy).sqrt()
}

/// Bring a position back inside the Arena.
#[inline]
pub fn wrap_xy(x: &mut f64, y: &mut f64) {
    if *x < 0.0 {
        *x += WIDTH;
    } else if *x >= WIDTH {
        *x -= WIDTH;
    }
    if *y < 0.0 {
        *y += HEIGHT;
    } else if *y >= HEIGHT {
        *y -= HEIGHT;
    }
}

/// Normalize an angle to `[-PI, PI]`.
#[inline]
pub fn wrap_angle(a: f64) -> f64 {
    let mut r = a % std::f64::consts::TAU;
    if r > std::f64::consts::PI {
        r -= std::f64::consts::TAU;
    } else if r < -std::f64::consts::PI {
        r += std::f64::consts::TAU;
    }
    r
}

#[inline]
pub fn hypot(x: f64, y: f64) -> f64 {
    x.hypot(y)
}

#[inline]
pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}
