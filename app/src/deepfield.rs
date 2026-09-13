//! The deep field's density, baked once on the CPU.
//!
//! The Arena's sky is a place, and a place has weather: two coloured cloud
//! layers, a finer filament layer laid over them at a higher frequency, and a
//! soft envelope that leaves parts of the field genuinely empty. The backdrop
//! shader reads all four out of one texture.
//!
//! It is baked here rather than evaluated in the fragment shader for the same
//! reason the starfield is laid out from an integer LCG: a golden frame has to
//! come out the same on the machine that wrote it and on the machine that
//! checks it. `sin`-based hashes and `fract` of large arguments are the two
//! places shader noise drifts between GPUs; integer hashing on the CPU cannot.
//!
//! The lattice wraps at each octave's own period, so the texture tiles, which
//! is what lets the filament layer be sampled at three times the frequency
//! without a visible seam running down the Arena.

/// The baked texture's edge, in texels. Broad cloud is spread over the whole
/// 960×600 Arena from this, so it is deliberately small: what it carries is
/// shape, not detail.
pub const SIZE: u32 = 256;

/// One seed per channel, so re-baking one layer leaves the others alone.
const SEED_BROAD_A: u32 = 0x0DEE_9F1D;
const SEED_BROAD_B: u32 = 0x0DEE_5A17;
const SEED_FILAMENT: u32 = 0x0DEE_2024;
const SEED_ENVELOPE: u32 = 0x0DEE_B1E5;

/// Where each layer's density starts to register and where it saturates. The
/// broad layers open late so the field keeps real emptiness between clouds; the
/// second opens a little earlier, because its own noise runs colder and the two
/// clouds have to share the sky rather than one of them owning it.
const BROAD_FLOOR: f32 = 0.42;
const BROAD_B_FLOOR: f32 = 0.37;
const BROAD_CEIL: f32 = 0.92;
const FILAMENT_FLOOR: f32 = 0.54;
const FILAMENT_CEIL: f32 = 0.86;
const ENVELOPE_FLOOR: f32 = 0.30;
const ENVELOPE_CEIL: f32 = 0.78;

/// `SIZE × SIZE` RGBA8: broad cloud A, broad cloud B, filament detail, and the
/// envelope that decides where any of it is allowed to be.
pub fn bake() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / SIZE as f32;
            let v = y as f32 / SIZE as f32;
            let broad_a = window(fbm(u, v, 3, 5, SEED_BROAD_A), BROAD_FLOOR, BROAD_CEIL);
            let broad_b = window(fbm(u, v, 4, 5, SEED_BROAD_B), BROAD_B_FLOOR, BROAD_CEIL);
            let filament = window(
                fbm(u, v, 6, 4, SEED_FILAMENT),
                FILAMENT_FLOOR,
                FILAMENT_CEIL,
            );
            let envelope = window(
                fbm(u, v, 2, 3, SEED_ENVELOPE),
                ENVELOPE_FLOOR,
                ENVELOPE_CEIL,
            );
            for channel in [broad_a, broad_b, filament, envelope] {
                pixels.push((channel * 255.0 + 0.5) as u8);
            }
        }
    }
    pixels
}

/// Smoothstep between two edges: below `floor` there is nothing at all, above
/// `ceil` the layer is solid, and the curve between them is what gives a cloud
/// a core and an edge instead of a uniform fog.
fn window(value: f32, floor: f32, ceil: f32) -> f32 {
    let t = ((value - floor) / (ceil - floor)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Fractal value noise over `u, v` in `0..1`, every octave wrapping at its own
/// period so the whole field tiles.
fn fbm(u: f32, v: f32, base: i32, octaves: u32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut weight = 0.0;
    let mut amplitude = 0.5;
    let mut period = base;
    for octave in 0..octaves {
        sum += amplitude
            * value_noise(
                u * period as f32,
                v * period as f32,
                period,
                seed.wrapping_add(octave.wrapping_mul(0x9E37_79B9)),
            );
        weight += amplitude;
        amplitude *= 0.5;
        period *= 2;
    }
    sum / weight
}

/// One octave: the lattice hashed at four corners and smoothstep-interpolated.
fn value_noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (tx, ty) = (x - fx, y - fy);
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let wrap = |value: i32| value.rem_euclid(period);
    let (x0, y0) = (wrap(fx as i32), wrap(fy as i32));
    let (x1, y1) = (wrap(fx as i32 + 1), wrap(fy as i32 + 1));
    let top = mix(hash(x0, y0, seed), hash(x1, y0, seed), sx);
    let bottom = mix(hash(x0, y1, seed), hash(x1, y1, seed), sx);
    mix(top, bottom, sy)
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// An integer hash of a lattice point into `0..1`. Multiply, shift and xor
/// only: exactly reproducible everywhere, which is the whole point of baking.
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27D4_EB2D) ^ (y as u32).wrapping_mul(0x1656_67B1) ^ seed;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x2974_5B71);
    h ^= h >> 16;
    (h >> 8) as f32 / 16_777_216.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_is_baked_the_same_way_every_time() {
        let first = bake();
        let second = bake();
        assert_eq!(first.len(), (SIZE * SIZE * 4) as usize);
        assert_eq!(first, second);
    }

    #[test]
    fn every_layer_wraps_so_the_filaments_have_no_seam() {
        // The filament layer is sampled at three times the frequency with a
        // repeating address mode: the lattice has to close on itself, or a hard
        // line runs down the Arena.
        for octaves in 1..=5 {
            for base in [2, 3, 4, 6] {
                let left = fbm(0.0, 0.37, base, octaves, SEED_FILAMENT);
                let right = fbm(1.0, 0.37, base, octaves, SEED_FILAMENT);
                assert!((left - right).abs() < 1e-6, "u seam at base {base}");
                let top = fbm(0.63, 0.0, base, octaves, SEED_FILAMENT);
                let bottom = fbm(0.63, 1.0, base, octaves, SEED_FILAMENT);
                assert!((top - bottom).abs() < 1e-6, "v seam at base {base}");
            }
        }
    }

    #[test]
    fn the_sky_has_both_cloud_and_genuine_emptiness() {
        let pixels = bake();
        let texels = (SIZE * SIZE) as usize;
        for (channel, name) in [(0, "broad a"), (1, "broad b"), (2, "filament")] {
            let values: Vec<u8> = (0..texels).map(|i| pixels[i * 4 + channel]).collect();
            let empty = values.iter().filter(|v| **v < 8).count();
            let dense = values.iter().filter(|v| **v > 180).count();
            // A layer that is everywhere is fog, and a layer that is nowhere is
            // a wasted channel.
            assert!(
                empty > texels / 8,
                "{name} leaves no empty sky: {empty}/{texels}"
            );
            assert!(
                dense > texels / 200,
                "{name} has no cores: {dense}/{texels}"
            );
        }
    }
}
