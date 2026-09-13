// NeuroArena's whole shading path, in one module.
//
// The frame is built in high dynamic range: matter is written at the values the
// theme authors, and light is written *past* 1.0, which is what lets the bloom
// chain tell a lamp from a pale panel without a mask. Five kinds of draw share
// this module:
//
//   vs_shape / fs_shape          triangles and strokes, alpha or additive
//   vs_fullscreen / fs_backdrop  the deep field behind the Arena
//   fs_bloom_*                   the threshold, downsample and upsample chain
//   fs_composite                 HDR + bloom, resolved into the window
//   vs_text / fs_text            glyph quads, drawn last and never bloomed

struct Frame {
    /// x, y = physical pixels; z, w = their reciprocals.
    viewport: vec4<f32>,
    /// The Arena's rectangle in physical pixels: x, y, width, height.
    arena: vec4<f32>,
    /// time (seconds), bloom intensity, grain amount, reserved.
    params: vec4<f32>,
    /// The palette the backdrop is painted from, in linear light.
    app_bg: vec4<f32>,
    arena_bg: vec4<f32>,
    nebula_a: vec4<f32>,
    nebula_b: vec4<f32>,
    wash: vec4<f32>,
    vignette: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var source: texture_2d<f32>;
@group(1) @binding(1) var source_sampler: sampler;
// Only the composite binds a second texture; every other pass's layout stops
// at binding 1, and naga prunes what an entry point does not reach.
@group(1) @binding(2) var bloom_source: texture_2d<f32>;

// ------------------------------------------------------------------ shapes

struct ShapeIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct ShapeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

fn project(pos: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        pos.x * frame.viewport.z * 2.0 - 1.0,
        1.0 - pos.y * frame.viewport.w * 2.0,
    );
}

@vertex
fn vs_shape(in: ShapeIn) -> ShapeOut {
    var out: ShapeOut;
    out.clip = vec4<f32>(project(in.pos), 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_shape(in: ShapeOut) -> @location(0) vec4<f32> {
    return in.color;
}

// -------------------------------------------------------------- fullscreen

struct FullscreenOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

/// One oversized triangle covering the viewport; `uv` runs 0..1 across it,
/// with 0 at the top-left, the same way the Painter measures pixels.
@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullscreenOut {
    var out: FullscreenOut;
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    out.uv = vec2<f32>(x, y);
    out.clip = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

// ---------------------------------------------------------------- backdrop

/// How far the upper wash lifts the field, and how deep the floor settles.
const WASH_TOP: f32 = 0.55;
const WASH_FLOOR: f32 = 0.40;
/// Where the wash turns over, as a fraction of the Arena's height.
const WASH_HORIZON: f32 = 0.58;
/// How much of each nebula layer reaches the field. The density is squared
/// first, so the clouds have cores and edges rather than a uniform fog.
const NEBULA_GAIN: f32 = 1.35;
const FILAMENT_GAIN: f32 = 0.30;
/// The frame's own falloff, as a fraction of each axis.
const VIGNETTE_X: f32 = 0.20;
const VIGNETTE_Y: f32 = 0.30;
const VIGNETTE_DEPTH: f32 = 0.55;

@fragment
fn fs_backdrop(in: FullscreenOut) -> @location(0) vec4<f32> {
    let pixel = in.uv * frame.viewport.xy;
    let local = (pixel - frame.arena.xy) / max(frame.arena.zw, vec2<f32>(1.0, 1.0));
    if local.x < 0.0 || local.y < 0.0 || local.x > 1.0 || local.y > 1.0 {
        return vec4<f32>(frame.app_bg.rgb, 1.0);
    }

    // The field is a lit volume: the upper air catches the deep field's colour
    // and the floor settles toward the frame's own shadow.
    var color = frame.arena_bg.rgb;
    let lift = smoothstep(WASH_HORIZON, 0.0, local.y);
    let sink = smoothstep(WASH_HORIZON, 1.0, local.y);
    color = mix(color, frame.wash.rgb, lift * WASH_TOP);
    color = mix(color, frame.vignette.rgb, sink * WASH_FLOOR);

    // Two coloured density fields, baked once on the CPU so the sky is the same
    // on every GPU: broad cloud across the whole Arena, filaments at a higher
    // frequency over it.
    let broad = textureSample(source, source_sampler, local);
    let fine = textureSample(source, source_sampler, local * vec2<f32>(3.0, 1.9) + vec2<f32>(0.31, 0.17));
    let mask = broad.a;
    let a = broad.r * mask;
    let b = broad.g * mask;
    color += frame.nebula_a.rgb * a * a * NEBULA_GAIN;
    color += frame.nebula_b.rgb * b * b * NEBULA_GAIN;
    color += mix(frame.nebula_a.rgb, frame.nebula_b.rgb, 0.5) * fine.b * mask * FILAMENT_GAIN;

    // The frame's own falloff: the Arena's edges sit deeper than its middle.
    let fx = smoothstep(0.0, VIGNETTE_X, min(local.x, 1.0 - local.x));
    let fy = smoothstep(0.0, VIGNETTE_Y, min(local.y, 1.0 - local.y));
    color = mix(color, frame.vignette.rgb, (1.0 - fx * fy) * VIGNETTE_DEPTH);
    return vec4<f32>(color, 1.0);
}

// ------------------------------------------------------------------- bloom
//
// The chain is the one described in "Next Generation Post Processing in Call of
// Duty: Advanced Warfare" (Jimenez, SIGGRAPH 2014) and written up as
// "Physically Based Bloom" on LearnOpenGL: a 13-tap downsample with a Karis
// average on the first level to stop fireflies smearing, then a 3×3 tent
// upsample that adds each level back into the one above it.
//
// The threshold sits at 1.0 with a soft knee, and the frame is HDR, so only
// what the scene writes *past* white blooms. Panel fills, text and chart lines
// are authored at or below white and never reach it.

const BLOOM_THRESHOLD: f32 = 1.0;
const BLOOM_KNEE: f32 = 0.55;

fn soft_threshold(color: vec3<f32>) -> vec3<f32> {
    let brightness = max(color.r, max(color.g, color.b));
    var soft = brightness - (BLOOM_THRESHOLD - BLOOM_KNEE);
    soft = clamp(soft, 0.0, 2.0 * BLOOM_KNEE);
    soft = soft * soft * (0.25 / BLOOM_KNEE);
    let contribution = max(soft, brightness - BLOOM_THRESHOLD) / max(brightness, 0.0001);
    return color * contribution;
}

/// Rec. 709 luminance, which is what the firefly weighting is measured in.
fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

/// Brian Karis' weighted average: a very bright single pixel is pulled back
/// toward its neighbours instead of smearing across the whole level.
fn karis_weight(color: vec3<f32>) -> f32 {
    return 1.0 / (1.0 + luminance(color) * 0.25);
}

/// The 13 taps of the downsample kernel, in the order the groupings below
/// expect: the outer ring of nine at ±2 texels, then the inner four at ±1.
fn taps(uv: vec2<f32>, texel: vec2<f32>) -> array<vec3<f32>, 13> {
    let big = texel * 2.0;
    return array<vec3<f32>, 13>(
        textureSample(source, source_sampler, uv + vec2<f32>(-big.x, big.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(0.0, big.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(big.x, big.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(-big.x, 0.0)).rgb,
        textureSample(source, source_sampler, uv).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(big.x, 0.0)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(-big.x, -big.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(0.0, -big.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(big.x, -big.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(-texel.x, texel.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(texel.x, texel.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(-texel.x, -texel.y)).rgb,
        textureSample(source, source_sampler, uv + vec2<f32>(texel.x, -texel.y)).rgb,
    );
}

fn source_texel() -> vec2<f32> {
    return 1.0 / vec2<f32>(textureDimensions(source));
}

/// The first level: the same kernel, but each 2×2 group is weighted down by its
/// own luminance before the groups are summed, and only what is over the
/// threshold survives.
@fragment
fn fs_bloom_prefilter(in: FullscreenOut) -> @location(0) vec4<f32> {
    let t = taps(in.uv, source_texel());
    var g0 = (t[0] + t[1] + t[3] + t[4]) * (0.125 / 4.0);
    var g1 = (t[1] + t[2] + t[4] + t[5]) * (0.125 / 4.0);
    var g2 = (t[3] + t[4] + t[6] + t[7]) * (0.125 / 4.0);
    var g3 = (t[4] + t[5] + t[7] + t[8]) * (0.125 / 4.0);
    var g4 = (t[9] + t[10] + t[11] + t[12]) * (0.5 / 4.0);
    g0 *= karis_weight(g0);
    g1 *= karis_weight(g1);
    g2 *= karis_weight(g2);
    g3 *= karis_weight(g3);
    g4 *= karis_weight(g4);
    let total = clamp(g0 + g1 + g2 + g3 + g4, vec3<f32>(0.0), vec3<f32>(65000.0));
    return vec4<f32>(soft_threshold(total), 1.0);
}

@fragment
fn fs_bloom_down(in: FullscreenOut) -> @location(0) vec4<f32> {
    let t = taps(in.uv, source_texel());
    var sample = (t[0] + t[2] + t[6] + t[8]) * 0.03125;
    sample += (t[1] + t[3] + t[5] + t[7]) * 0.0625;
    sample += (t[4] + t[9] + t[10] + t[11] + t[12]) * 0.125;
    return vec4<f32>(sample, 1.0);
}

@fragment
fn fs_bloom_up(in: FullscreenOut) -> @location(0) vec4<f32> {
    let texel = source_texel();
    let x = texel.x;
    let y = texel.y;
    var sample = textureSample(source, source_sampler, in.uv).rgb * 0.25;
    sample += textureSample(source, source_sampler, in.uv + vec2<f32>(0.0, y)).rgb * 0.125;
    sample += textureSample(source, source_sampler, in.uv + vec2<f32>(0.0, -y)).rgb * 0.125;
    sample += textureSample(source, source_sampler, in.uv + vec2<f32>(x, 0.0)).rgb * 0.125;
    sample += textureSample(source, source_sampler, in.uv + vec2<f32>(-x, 0.0)).rgb * 0.125;
    sample += textureSample(source, source_sampler, in.uv + vec2<f32>(x, y)).rgb * 0.0625;
    sample += textureSample(source, source_sampler, in.uv + vec2<f32>(-x, y)).rgb * 0.0625;
    sample += textureSample(source, source_sampler, in.uv + vec2<f32>(x, -y)).rgb * 0.0625;
    sample += textureSample(source, source_sampler, in.uv + vec2<f32>(-x, -y)).rgb * 0.0625;
    return vec4<f32>(sample, 1.0);
}

// --------------------------------------------------------------- composite

// The bloom term is resolved before it reaches the scene, not merely added to
// it. Raw addition is wrong at the top of the range: the brightest channels
// clip first, so the core of a bright light — the one place its colour carries
// the most meaning — washes out toward white while the halo around it keeps
// the hue. The curve below compresses the top end and leaves the hue angle
// alone: PBR Neutral, the Khronos Group's fit (Apache-2.0,
// https://github.com/KhronosGroup/ToneMapping), as adapted in Bevy v0.19.1
// `crates/bevy_core_pipeline/src/tonemapping/tonemapping_shared.wgsl` (MIT OR
// Apache-2.0).
//
// The scene is *not* tone-mapped, and neither is anything the interface
// printed: panel fills, text and chart lines are authored at or below white
// and must resolve to exactly what was written (and they cannot bloom — the
// threshold sits at white). Only the light the chain gathered is resolved.
//
// `K_s` and `K_d` in the specification; the only input is the colour, so the
// composite stays a pure function of the frame.
const RESOLVE_START_COMPRESSION: f32 = 0.8 - 0.04;
const RESOLVE_DESATURATION: f32 = 0.15;

/// PBR Neutral's highlight compression, and only that: below
/// [`RESOLVE_START_COMPRESSION`] the colour passes through untouched, above it
/// the peak asymptotes toward `1.0` and every channel is scaled by the same
/// factor — so the ratios between them, and with them the hue, survive the
/// compression. Only the very top desaturates toward white.
///
/// The fit's low-end toe is deliberately not here. It takes back up to 0.04 per
/// channel wherever the darkest channel is under 0.08, which is the right trade
/// for a whole image — near-black scene values must not be lifted — but wrong
/// for a light term measured against a dark field: it swallows the halo's tail
/// (measured: the halo of a bullet — a light at seven times white — no longer
/// cleared the field's own grain with it in place) and leans the tail's colour
/// red, which is the opposite of what this pass is for. What remains is exactly
/// what the composite needs: identity through the halo, compression in the core.
fn resolve_bloom(color: vec3<f32>) -> vec3<f32> {
    let peak = max(color.r, max(color.g, color.b));
    if peak < RESOLVE_START_COMPRESSION {
        return color;
    }

    // `p_n` and `g` in the specification, with `1 - K_s` named `range`.
    let range = 1.0 - RESOLVE_START_COMPRESSION;
    let compressed = 1.0 - range * range / (peak + range - RESOLVE_START_COMPRESSION);
    let desaturate = 1.0 - 1.0 / (RESOLVE_DESATURATION * (peak - compressed) + 1.0);
    return mix(color * (compressed / peak), vec3<f32>(compressed), desaturate);
}

/// Interleaved gradient noise in `0..1` from a pixel coordinate — the pattern
/// Jorge Jimenez described in "Next Generation Post Processing in Call of Duty:
/// Advanced Warfare" (SIGGRAPH 2014) and that Filament (Apache-2.0) and
/// PlayCanvas (MIT) both ship. It sits flatter than white noise over a small
/// patch — a low-discrepancy sequence, so the field keeps a steadier average —
/// which is what makes it read as grain on glass rather than as static.
///
/// It is a function of the pixel and nothing else: no clock, no frame index,
/// so a paused frame is perfectly still and two captures of one frame are
/// identical.
fn interleaved_gradient(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

@fragment
fn fs_composite(in: FullscreenOut) -> @location(0) vec4<f32> {
    let scene = textureSample(source, source_sampler, in.uv).rgb;
    let bloom = textureSample(bloom_source, source_sampler, in.uv).rgb;
    var color = scene + resolve_bloom(bloom * frame.params.y);

    // Sensor grain, inside the Arena only: the panels are printed matter and
    // stay clean, while the field reads as something being *looked at*.
    let pixel = in.uv * frame.viewport.xy;
    let local = (pixel - frame.arena.xy) / max(frame.arena.zw, vec2<f32>(1.0, 1.0));
    let inside = step(0.0, local.x) * step(0.0, local.y)
        * step(local.x, 1.0) * step(local.y, 1.0);
    // Multiplicative, not additive: a constant offset in linear light is
    // enormous against a field this dark, and would read as dither rather than
    // as the instrument's own noise.
    let grain = 1.0 + (interleaved_gradient(floor(pixel)) - 0.5) * frame.params.z * inside;
    color *= grain;

    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}

// -------------------------------------------------------------------- text

struct TextIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct TextOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_text(in: TextIn) -> TextOut {
    var out: TextOut;
    out.clip = vec4<f32>(project(in.pos), 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_text(in: TextOut) -> @location(0) vec4<f32> {
    let mask = textureSample(source, source_sampler, in.uv);
    return vec4<f32>(in.color.rgb, in.color.a * mask.a);
}
