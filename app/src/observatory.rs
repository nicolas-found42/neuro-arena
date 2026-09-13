//! Observatory composition and read-only, bounded presentation history.
use crate::{
    painter::{Align, Painter, Rgba},
    scene::ArenaView,
    theme::{color, font},
    ui::{Controls, Rect},
};
use sim::{config::arena, World};
use std::collections::VecDeque;

/// The logical Arena, in the units the ribbon is stored and solved in.
const W: f32 = arena::WIDTH as f32;
const H: f32 = arena::HEIGHT as f32;

/// The fewest simulation seconds the ribbon spans, and the floor the window
/// grows from at speed.
///
/// Two seconds is set by what has to be legible around it, not by taste. A
/// Pilot early in a run covers about thirty Arena units a second, so the
/// three-quarter-second window this trace used to keep was twenty-odd units
/// long — entirely inside the hull's own halo and the corona's ring at three
/// and a half Ship radii. The trace was drawn every frame and could not be
/// seen. Two seconds puts the head of the ribbon clear of both at the speeds
/// the Population actually flies, and the fixed 48 samples keep the cost of
/// the longer window exactly what it was.
const TRAIL_WINDOW: f64 = 2.0;

/// The most samples in the ribbon. Fixed, because the window is not: the
/// spacing between samples stretches with the window, so a frame that runs a
/// second of simulation carries the same 48 points as one that runs a
/// sixtieth.
const TRAIL_SAMPLES: usize = 48;

/// The ribbon's half-width at its head, in Arena units, and the exponent of
/// its taper toward the tail. Below one holds the width further back before
/// falling away, which is what keeps the ribbon a ribbon behind the hull
/// rather than a spike at it.
const TRAIL_HALF_WIDTH: f32 = 2.0;
const TRAIL_TAPER: f32 = 0.7;

/// The ink at the head: the Ship's own colour at a trace's opacity.
const TRAIL_ALPHA: f32 = 0.5;

/// How the trace falls away along its length. Above one the head holds most of
/// the light and the tail drops quickly, which is what makes the ribbon a
/// comet's tail rather than a uniform line: the trace is additive, so a linear
/// fade across a two-second window reads as the same brightness the whole way
/// back and stops saying which way the Ship flew.
const TRAIL_FALLOFF: f32 = 1.6;

/// How far a joint's mitre may stand from the joint itself, in half-widths,
/// before that side is bevelled instead. Two and a half half-widths is a
/// direction change of about 130 degrees: past it the mitre is a spike
/// reaching off the path rather than a corner.
const MITRE_LIMIT: f32 = 2.5;

/// The sine below which two directions are treated as parallel, and the
/// `sides` the two edges of the ribbon are solved for.
const MITRE_PARALLEL: f32 = 1e-6;
const SIDES: [f32; 2] = [1.0, -1.0];

/// The slack a sample's age is compared with, one microsecond: at ×1 the step
/// is exactly the sample spacing and floating point must not make it skip.
const SAMPLE_SLACK: f64 = 1e-6;

/// The watched Ship's path, drawn as one continuous ribbon of light.
///
/// The ribbon is sampled on the simulation's own clock: [`Trail::observe`] is
/// called after every watched step, so what it holds is the path the Ship
/// actually flew rather than a line drawn between two rendered frames'
/// positions. Samples are stored *unwrapped* — a sample that crossed the seam
/// continues the path past the edge instead of jumping back to the other side
/// — and the ribbon is drawn once per Seam Copy, so a wrap reads as a ribbon
/// leaving one edge and arriving at the other.
///
/// An Episode identity prevents connecting different Ships after a transition.
#[derive(Default)]
pub struct Trail {
    key: Option<(u32, usize)>,
    time: f64,
    points: VecDeque<(f64, [f32; 2])>,
    /// The simulation time the last framed run covered: the window the ribbon
    /// keeps is never shorter than the frame it is being shown in, so a fast
    /// run's ribbon is neither a dot (a fixed short window over a long frame)
    /// nor a streak (every sample of it at once).
    span: f64,
}
impl Trail {
    pub fn clear(&mut self) {
        self.key = None;
        self.points.clear();
        self.time = 0.0;
    }

    /// Close a frame: `span` is the simulation time its steps covered. A frame
    /// that ran no steps contributes no span and changes nothing, which is
    /// what keeps a held frame drawing what the last stepping frame drew.
    pub fn settle(&mut self, span: f64) {
        if span > 0.0 {
            self.span = span;
        }
    }

    /// Called after each watched step; never steps or mutates the World itself.
    ///
    /// Sampling per step and not per rendered frame is the point: at ×100 a
    /// frame gives a hundred samples, not one, and the ribbon has no straight
    /// lines across corners that never were.
    pub fn observe(&mut self, key: (u32, usize), world: &World, enabled: bool) {
        if !enabled {
            self.clear();
            return;
        }
        if self.key != Some(key) || world.time < self.time {
            // A different Episode, or this one replayed from the top, is a
            // different path: nothing before it connected to where the Ship is
            // now.
            self.clear();
            self.key = Some(key);
        }
        if world.time == self.time && !self.points.is_empty() {
            return;
        }
        self.time = world.time;
        if !world.agent.alive {
            // The trace a death leaves is part of the record: it stays where
            // it was, and only the sampling stops.
            return;
        }
        let window = self.window();
        let spacing = window / TRAIL_SAMPLES as f64;
        while self
            .points
            .front()
            .is_some_and(|(time, _)| world.time - time > window)
        {
            self.points.pop_front();
        }
        if self
            .points
            .back()
            .is_some_and(|(time, _)| world.time - time < spacing - SAMPLE_SLACK)
        {
            return;
        }
        let sample = match self.points.back() {
            Some((_, previous)) => unwrapped(*previous, world.agent.ship.x, world.agent.ship.y),
            None => [world.agent.ship.x as f32, world.agent.ship.y as f32],
        };
        if self.points.len() == TRAIL_SAMPLES {
            self.points.pop_front();
        }
        self.points.push_back((world.time, sample));
    }

    /// The window the ribbon spans: [`TRAIL_WINDOW`], or the frame's own span
    /// when that is longer. A frame at ×1 is a single step, so the window there
    /// is exactly the fixed one it always was.
    fn window(&self) -> f64 {
        self.span.max(TRAIL_WINDOW)
    }

    /// Draw the ribbon, restoring the Painter's transform, clip and gain: the
    /// trail is a guest in the Arena's space, not its owner.
    pub fn draw(&self, p: &mut Painter, view: ArenaView) {
        if self.points.len() < 2 {
            return;
        }
        let gain = p.gain();
        p.arena_scope(view.transform(), [0.0, 0.0, W, H], |p| {
            p.set_gain(crate::theme::light::TRAIL);
            self.draw_ribbon(p);
            p.set_gain(gain);
        });
    }

    /// One joined strip of gradient quads, its width tapering from full behind
    /// the Ship to nothing at the tail and its light fading with each sample's
    /// age.
    ///
    /// Joints are mitred, not overlapped: neighbouring segments share the
    /// corner where their two offset lines meet, so a bend has neither a gap
    /// nor a bright seam. The solve is Godot's `LineBuilder` joint (MIT,
    /// algorithm only): intersect `pos1 + normal0 * t` with `pos1 + normal1 *
    /// t`, and where the corner would stand further than [`MITRE_LIMIT`]
    /// half-widths from the joint — a hairpin — bevel it instead: each segment
    /// keeps its own end and a patch closes the wedge between them.
    fn draw_ribbon(&self, p: &mut Painter) {
        let count = self.points.len().min(TRAIL_SAMPLES);
        if count < 2 {
            return;
        }
        let window = self.window();
        // The samples are unwrapped, so the ribbon can run laps past an edge;
        // the whole strip is brought back into the canonical lap once, and the
        // seam copies are translations of that.
        let newest = self
            .points
            .back()
            .map(|(_, point)| *point)
            .unwrap_or([0.0, 0.0]);
        let shift = [
            newest[0].rem_euclid(W) - newest[0],
            newest[1].rem_euclid(H) - newest[1],
        ];
        let mut points = [[0.0_f32; 2]; TRAIL_SAMPLES];
        let mut half = [0.0_f32; TRAIL_SAMPLES];
        let mut ink = [color::SHIP.alpha(0.0); TRAIL_SAMPLES];
        for (index, (time, point)) in self
            .points
            .iter()
            .skip(self.points.len() - count)
            .enumerate()
        {
            points[index] = [point[0] + shift[0], point[1] + shift[1]];
            let along = index as f32 / (count - 1) as f32;
            half[index] = TRAIL_HALF_WIDTH * along.powf(TRAIL_TAPER);
            let age = (1.0 - (self.time - time) / window).clamp(0.0, 1.0) as f32;
            ink[index] = color::SHIP.alpha(TRAIL_ALPHA * age.powf(TRAIL_FALLOFF));
        }
        // One unit normal per segment, and one solved corner per interior
        // joint per side. A zero normal is a stretch where the Ship did not
        // move: there is no corner to solve and the quads collapse to nothing.
        let mut normals = [[0.0_f32; 2]; TRAIL_SAMPLES];
        for index in 1..count {
            normals[index] = normal(points[index - 1], points[index]);
        }
        let mut mitre = [[[0.0_f32; 2]; 2]; TRAIL_SAMPLES];
        let mut mitred = [[false; 2]; TRAIL_SAMPLES];
        for joint in 1..count - 1 {
            let (n0, n1) = (normals[joint], normals[joint + 1]);
            let half_width = half[joint];
            if half_width <= 0.0 || n0 == [0.0, 0.0] || n1 == [0.0, 0.0] {
                continue;
            }
            let dot = n0[0] * n1[0] + n0[1] * n1[1];
            let cross = n0[0] * n1[1] - n0[1] * n1[0];
            for (slot, side) in SIDES.into_iter().enumerate() {
                if cross.abs() < MITRE_PARALLEL {
                    // Parallel, or folded back on itself: the first is a corner
                    // where the two ends coincide, the second has none to find.
                    if dot > 0.0 {
                        mitre[joint][slot] = [
                            points[joint][0] + n0[0] * half_width * side,
                            points[joint][1] + n0[1] * half_width * side,
                        ];
                        mitred[joint][slot] = true;
                    }
                    continue;
                }
                // The intersection of the two offset lines, in the segment's
                // own frame: `t` along the incoming segment's direction.
                let t = side * half_width * (dot - 1.0) / cross;
                let corner = [
                    points[joint][0] + side * half_width * n0[0] + t * n0[1],
                    points[joint][1] + side * half_width * n0[1] - t * n0[0],
                ];
                if (corner[0] - points[joint][0]).hypot(corner[1] - points[joint][1])
                    <= MITRE_LIMIT * half_width
                {
                    mitre[joint][slot] = corner;
                    mitred[joint][slot] = true;
                }
            }
        }
        let mut bounds = [points[0], points[0]];
        for point in &points[..count] {
            bounds[0] = [bounds[0][0].min(point[0]), bounds[0][1].min(point[1])];
            bounds[1] = [bounds[1][0].max(point[0]), bounds[1][1].max(point[1])];
        }
        // A mitre corner stands outside its samples, so the copy test is run
        // against the samples grown by the furthest one can reach.
        let margin = MITRE_LIMIT * TRAIL_HALF_WIDTH;
        bounds[0] = [bounds[0][0] - margin, bounds[0][1] - margin];
        bounds[1] = [bounds[1][0] + margin, bounds[1][1] + margin];
        // One copy of the strip for each visible Seam Copy. The loop is
        // written out here rather than borrowed from `scene`, whose
        // `seam_copies` is private to that module and this pass does not own
        // its file; the ribbon culls on its own bounds because at speed it can
        // be much longer than an entity.
        for ox in [-W, 0.0, W] {
            for oy in [-H, 0.0, H] {
                let offset = [ox, oy];
                if bounds[0][0] + offset[0] > W
                    || bounds[1][0] + offset[0] < 0.0
                    || bounds[0][1] + offset[1] > H
                    || bounds[1][1] + offset[1] < 0.0
                {
                    continue;
                }
                for (segment, &normal) in normals.iter().enumerate().take(count).skip(1) {
                    let corner = |end: usize, slot: usize| {
                        let point = if mitred[end][slot] {
                            mitre[end][slot]
                        } else {
                            [
                                points[end][0] + normal[0] * half[end] * SIDES[slot],
                                points[end][1] + normal[1] * half[end] * SIDES[slot],
                            ]
                        };
                        [point[0] + offset[0], point[1] + offset[1]]
                    };
                    let (from, to) = (segment - 1, segment);
                    let quad = [
                        corner(from, 0),
                        corner(to, 0),
                        corner(to, 1),
                        corner(from, 1),
                    ];
                    p.luminous_gradient_polygon(&quad, &[ink[from], ink[to], ink[to], ink[from]]);
                }
                // A bevelled joint leaves a wedge between the two segments'
                // ends; one patch triangle per bevelled side covers it. At a
                // hairpin both sides are bevelled and both patches are drawn,
                // which is what a strip folded back on itself looks like.
                for joint in 1..count - 1 {
                    for (slot, side) in SIDES.into_iter().enumerate() {
                        if mitred[joint][slot] {
                            continue;
                        }
                        let other = 1 - slot;
                        let inner = if mitred[joint][other] {
                            mitre[joint][other]
                        } else {
                            points[joint]
                        };
                        let from = [
                            points[joint][0] + normals[joint][0] * half[joint] * side,
                            points[joint][1] + normals[joint][1] * half[joint] * side,
                        ];
                        let to = [
                            points[joint][0] + normals[joint + 1][0] * half[joint] * side,
                            points[joint][1] + normals[joint + 1][1] * half[joint] * side,
                        ];
                        let patch = [
                            [from[0] + offset[0], from[1] + offset[1]],
                            [to[0] + offset[0], to[1] + offset[1]],
                            [inner[0] + offset[0], inner[1] + offset[1]],
                        ];
                        p.luminous_gradient_polygon(&patch, &[ink[joint]; 3]);
                    }
                }
            }
        }
    }
}

/// A sample carried into the previous one's frame of reference: a step that
/// crossed the seam moved the Ship a little way in world terms, so the sample
/// that came out the far side continues the ribbon — the Arena's own width is
/// added or subtracted back. Without it a wrap would draw a streak across the
/// whole field; with it, the ribbon leaves one edge and arrives at the other,
/// because a Seam Copy puts the piece across the edge back where it belongs.
fn unwrapped(previous: [f32; 2], x: f64, y: f64) -> [f32; 2] {
    let mut point = [x as f32, y as f32];
    if (point[0] - previous[0]).abs() > W * 0.5 {
        point[0] += if point[0] < previous[0] { W } else { -W };
    }
    if (point[1] - previous[1]).abs() > H * 0.5 {
        point[1] += if point[1] < previous[1] { H } else { -H };
    }
    point
}

/// The unit normal of the segment from `a` to `b`, or zero when the two
/// samples coincide — a stationary Ship draws no ribbon rather than a NaN one.
fn normal(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy);
    if length < 1e-4 {
        return [0.0, 0.0];
    }
    [-dy / length, dx / length]
}

/// The instrument strip: the height it is set in, the gap it keeps under the
/// field, and the narrowest it is ever drawn. The floor exists because the
/// three sections have to fit at any window size, and a window short enough to
/// squeeze the field to a sliver still has to show its instruments.
const STRIP_HEIGHT: f32 = 156.0;
const STRIP_GAP: f32 = 16.0;
const STRIP_MIN_WIDTH: f32 = 460.0;

/// The region's own margin: the strip never runs to the edge of the window.
const REGION_MARGIN: f32 = 32.0;

/// Where the instrument strip sits: under the field it measures and as wide as
/// the field, rather than across the whole region.
///
/// At a wide window the Arena is letterboxed between two bands of empty ground,
/// and a strip laid out from the region stretches across both of them: the
/// instruments stop reading as instruments of the thing above them. Laid out
/// from the Arena's own rect, they attach to the field at every size — and the
/// tremor the field is drawn with does not move them, because the placement is
/// the field's, not the frame's.
fn strip_rect(region: Rect) -> Rect {
    // Taken at 1:1: the chrome is drawn in the region's own units, and the
    // Painter's device-pixel transform carries it from there.
    let (x, y, width, height) = arena_view(region, 1.0).rect();
    let room = (region.w - REGION_MARGIN).max(0.0);
    let strip = width.clamp(STRIP_MIN_WIDTH.min(room), room);
    Rect::new(
        x + (width - strip) * 0.5,
        y + height + STRIP_GAP,
        strip,
        STRIP_HEIGHT,
    )
}

pub fn arena_view(region: Rect, dpr: f32) -> ArenaView {
    let available = Rect::new(
        region.x + 16.0,
        region.y + 70.0,
        (region.w - 32.0).max(0.0),
        (region.h - 256.0).max(0.0),
    );
    let scale = (available.w / 960.0).min(available.h / 600.0).max(0.0);
    ArenaView {
        origin: [
            (available.x + (available.w - 960.0 * scale) * 0.5) * dpr,
            (available.y + (available.h - 600.0 * scale) * 0.5) * dpr,
        ],
        scale: scale * dpr,
        tremor: [0.0, 0.0],
    }
}

/// The wordmark: the size it is set at, where its baseline sits inside that line
/// box, and the square of accent that shares the baseline.
const WORDMARK_SIZE: f32 = 34.0;
const WORDMARK_BASELINE: f32 = 0.985;
const WORDMARK_MARK: f32 = 4.0;
const WORDMARK_OFFSET: f32 = 10.0;

/// The state lamp beside the status line, and the lane it keeps clear of the
/// words. The words are the monospace body face, so the lane is an advance away.
const STATUS_RADIUS: f32 = 2.5;
const STATUS_GAP: f32 = 12.0;
const MONO_ADVANCE: f32 = 0.6;

/// All measurements come from the current World's stored inputs and Network.
pub fn draw_chrome(
    p: &mut Painter,
    region: Rect,
    world: Option<&World>,
    seed: u32,
    controls: &Controls,
    trails: bool,
) {
    let x = region.x + 20.0;
    if region.w < 180.0 {
        return;
    }
    // The mark sits on the wordmark's baseline and the name clears it, so the
    // two read as one lockup rather than as a bullet in a string.
    let wordmark = region.y + 6.0;
    p.rect(
        x,
        wordmark + WORDMARK_SIZE * WORDMARK_BASELINE - WORDMARK_MARK,
        WORDMARK_MARK,
        WORDMARK_MARK,
        color::ACCENT,
    );
    p.display_text(
        [x + WORDMARK_OFFSET, wordmark],
        WORDMARK_SIZE,
        color::TEXT,
        "NEUROARENA",
    );
    p.text(
        [x, region.y + 44.0],
        font::SMALL,
        color::TEXT_DIM,
        "FLIGHT OBSERVATORY   /   EVOLVING INTELLIGENCE",
    );
    // What the run is doing is a lamp as well as a sentence: filled and lit while
    // it is evolving, hollow while it is not doing anything at all.
    let (state, ink, evolving) = if controls.paused {
        (
            if controls.watching {
                "PAUSED / REPLAY"
            } else {
                "PAUSED"
            },
            color::ENERGY,
            false,
        )
    } else if controls.watching {
        ("REPLAY", color::ACCENT, false)
    } else {
        ("EVOLVING", color::ACCENT, true)
    };
    let status = region.y + 17.0;
    let mark = [
        region.right()
            - 20.0
            - state.chars().count() as f32 * font::BODY * MONO_ADVANCE
            - STATUS_GAP,
        status + font::BODY * 0.5,
    ];
    if evolving {
        p.circle(mark, STATUS_RADIUS, color::ACCENT, 12);
        p.set_gain(crate::theme::light::LAMP);
        p.luminous_glow(mark, STATUS_RADIUS * 2.0, color::ACCENT.alpha(0.35));
        p.set_gain(1.0);
    } else {
        status_ring(p, mark, STATUS_RADIUS, color::SHIP_FLAME);
    }
    p.text_aligned(
        [region.right() - 20.0, status],
        font::BODY,
        ink,
        Align::Right,
        state,
    );
    if region.w > 660.0 {
        p.text_aligned(
            [region.right() - 20.0, region.y + 44.0],
            font::SMALL,
            color::TEXT_DIM,
            Align::Right,
            format!("SEED {seed} / 960 × 600 / TOROIDAL"),
        );
    }
    p.line(
        [x, region.y + 62.0],
        [region.right() - 20.0, region.y + 62.0],
        color::PANEL_BORDER,
    );
    let panel = strip_rect(region);
    crate::instruments::sensorium(p, panel, world, trails);
}

/// A hollow circle: a closed run of segments, never a filled disc, so "paused"
/// cannot be misread as "lit".
fn status_ring(p: &mut Painter, center: [f32; 2], radius: f32, ink: Rgba) {
    const SEGMENTS: usize = 16;
    let mut points = [[0.0_f32; 2]; SEGMENTS];
    for (index, point) in points.iter_mut().enumerate() {
        let angle = index as f32 * std::f32::consts::TAU / SEGMENTS as f32;
        *point = [
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ];
    }
    p.polyline(&points, ink, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::Rng;
    #[test]
    fn history_freezes_caps_and_resets_on_discontinuities() {
        let mut w = World::new(Rng::from_seed(2), None);
        let mut t = Trail::default();
        // Long enough to fill the window several times over: the samples are
        // spaced across it rather than taken one per step, so what the ribbon
        // settles at is the window divided by the spacing, and the cap is a
        // ceiling it may never reach rather than a target it grows to.
        for i in 0..1200 {
            w.time = i as f64 / 60.0;
            t.observe((1, 0), &w, true);
        }
        let settled = t.points.len();
        assert!(
            settled > 1 && settled <= TRAIL_SAMPLES,
            "the ribbon settled at {settled} samples"
        );
        // And it holds there: the oldest falls out as the newest arrives.
        for i in 1200..1260 {
            w.time = i as f64 / 60.0;
            t.observe((1, 0), &w, true);
        }
        assert_eq!(t.points.len(), settled, "the ribbon is still growing");
        t.observe((2, 0), &w, true);
        assert_eq!(t.points.len(), 1);
        // A gap longer than the window is a gap in the record, not a stitch
        // across it: the old sample falls out of the window and the ribbon
        // restarts where the Ship is. (Before the window the rule was `dt >
        // 0.25`; the window is the same idea with the frame's own span in it.)
        w.time += TRAIL_WINDOW + 0.1;
        t.observe((2, 0), &w, true);
        assert_eq!(t.points.len(), 1);
        t.observe((2, 0), &w, false);
        assert!(t.points.is_empty());
    }
    #[test]
    fn trail_duration_is_bounded_at_different_observation_rates() {
        for hz in [30, 60, 120, 600] {
            let mut w = World::new(Rng::from_seed(2), None);
            let mut trail = Trail::default();
            for i in 0..hz * 4 {
                w.time = f64::from(i) / f64::from(hz);
                trail.observe((1, 0), &w, true);
            }
            assert!(trail.points.len() <= TRAIL_SAMPLES);
            assert!(
                trail.time - trail.points.front().unwrap().0 <= TRAIL_WINDOW + 1e-9,
                "the ribbon kept more than its window at {hz} Hz"
            );
        }
        // And the window is at least the frame the ribbon was sampled in: at
        // ×100 a frame runs 1.67 s and the ribbon covers it, where a fixed
        // short window would have shown the last half of the frame and the old
        // `dt > 0.25` clear would have shown nothing at all.
        let mut w = World::new(Rng::from_seed(2), None);
        let mut trail = Trail::default();
        trail.settle(1.67);
        for step in 0..100 {
            w.time = step as f64 * sim::DT;
            trail.observe((1, 0), &w, true);
        }
        let span = trail.time - trail.points.front().unwrap().0;
        assert!(
            span > 1.4,
            "the ribbon spans the frame it was sampled in: {span}"
        );
        assert!(trail.points.len() <= 48);
    }

    #[test]
    fn samples_stay_continuous_across_the_seam() {
        let mut w = World::new(Rng::from_seed(2), None);
        let mut trail = Trail::default();
        w.agent.ship.x = 950.0;
        w.agent.ship.y = 300.0;
        w.time = 1.0;
        trail.observe((1, 0), &w, true);
        assert_eq!(trail.points.back().unwrap().1, [950.0, 300.0]);
        // The step that carried the Ship out of the right edge arrives at the
        // left one; the stored sample continues the path past the edge rather
        // than jumping back across the field.
        w.agent.ship.x = 10.0;
        // Far enough apart in simulation time for the ribbon to take a second
        // sample: the samples are spaced across the window, not one per step.
        w.time += TRAIL_WINDOW / TRAIL_SAMPLES as f64 + 1e-6;
        trail.observe((1, 0), &w, true);
        let crossed = trail.points.back().unwrap().1;
        assert_eq!(crossed, [970.0, 300.0], "the sample is unwrapped");
        assert!((crossed[0] - 950.0).abs() < W * 0.5);
        // And the Seam Copy puts the piece back on the other edge, so the wrap
        // reads as one ribbon leaving and arriving rather than a streak across
        // the field.
        let mut p = Painter::new();
        trail.draw(
            &mut p,
            ArenaView {
                origin: [0.0, 0.0],
                scale: 1.0,
                tremor: [0.0, 0.0],
            },
        );
        assert!(!p.luminous.is_empty());
        assert!(p.luminous.iter().any(|v| v.pos[0] < 20.0));
        assert!(p.luminous.iter().any(|v| v.pos[0] > 940.0));
        assert!(p
            .luminous
            .iter()
            .all(|v| v.pos[0] >= 0.0 && v.pos[0] <= W && v.pos[1] >= 0.0 && v.pos[1] <= H));
    }

    #[test]
    fn a_hairpin_is_bevelled_rather_than_spiked() {
        // Samples doubling back on themselves: the mitre solve has no corner
        // to find, and the fallback must keep the geometry where the path is —
        // an exploding mitre would throw a wedge across the field.
        let mut w = World::new(Rng::from_seed(2), None);
        let mut trail = Trail::default();
        let path = [
            (300.0, 300.0, 1.0),
            (340.0, 300.0, 1.1),
            (301.0, 302.0, 1.2),
        ];
        for (x, y, time) in path {
            w.agent.ship.x = x;
            w.agent.ship.y = y;
            w.time = time;
            trail.observe((1, 0), &w, true);
        }
        assert_eq!(trail.points.len(), 3);
        let mut p = Painter::new();
        trail.draw(
            &mut p,
            ArenaView {
                origin: [0.0, 0.0],
                scale: 1.0,
                tremor: [0.0, 0.0],
            },
        );
        assert!(!p.luminous.is_empty());
        let slack = MITRE_LIMIT * TRAIL_HALF_WIDTH + 1.0;
        assert!(
            p.luminous.iter().all(|v| v.pos[0] >= 300.0 - slack
                && v.pos[0] <= 340.0 + slack
                && v.pos[1] >= 300.0 - slack
                && v.pos[1] <= 302.0 + slack),
            "a corner left the path"
        );
    }

    #[test]
    fn the_same_ribbon_draws_the_same_light_twice() {
        // The draw path reads the samples and writes the Painter; a held frame
        // re-draws the frame before it, so the same samples must produce the
        // same vertices, mitres and all.
        let mut w = World::new(Rng::from_seed(2), None);
        let mut trail = Trail::default();
        for (index, (x, y)) in [
            (200.0, 200.0),
            (240.0, 220.0),
            (280.0, 200.0),
            (320.0, 240.0),
        ]
        .into_iter()
        .enumerate()
        {
            w.agent.ship.x = x;
            w.agent.ship.y = y;
            w.time = 1.0 + index as f64 * sim::DT;
            trail.observe((1, 0), &w, true);
        }
        let view = ArenaView {
            origin: [0.0, 0.0],
            scale: 1.0,
            tremor: [0.0, 0.0],
        };
        let (mut a, mut b) = (Painter::new(), Painter::new());
        trail.draw(&mut a, view);
        trail.draw(&mut b, view);
        assert!(!a.luminous.is_empty());
        assert_eq!(a.luminous, b.luminous);
        assert_eq!(a.triangles, b.triangles);
    }

    #[test]
    fn arena_fit_preserves_ratio_at_small_and_retina_sizes() {
        for (w, h, dpr) in [(530.0, 560.0, 1.0), (1070.0, 900.0, 2.0)] {
            let v = arena_view(Rect::new(0.0, 0.0, w, h), dpr);
            let (x, y, aw, ah) = v.rect();
            assert!((aw / ah - 1.6).abs() < 0.001);
            assert!(x >= 0.0 && y >= 70.0 * dpr);
            assert!(x + aw <= w * dpr && y + ah <= (h - 186.0) * dpr + 0.01);
        }
    }

    #[test]
    fn the_strip_measures_the_field_at_a_wide_and_a_minimum_window() {
        // 2560×1080 leaves wide letterbox bands beside the field; 900×560 is
        // the smallest window the app opens, where the field fills the region.
        for (width, height) in [(2560.0, 1080.0), (900.0, 560.0)] {
            let region = crate::ui::Layout::new(width, height).arena;
            let (field_x, field_y, field_w, field_h) = arena_view(region, 1.0).rect();
            let strip = strip_rect(region);

            // The strip is the field's width and sits under it: at a wide
            // window the bands beside the field are outside it, which is the
            // whole point of measuring from the field rather than the region.
            assert!(
                (strip.w - field_w).abs() < 1e-3,
                "at {width}×{height} the strip is {} wide over a {field_w} field",
                strip.w
            );
            assert!(strip.x >= field_x - 1e-3 && strip.right() <= field_x + field_w + 1e-3);
            assert!(
                strip.y >= field_y + field_h,
                "the strip rides over the field"
            );
            assert!(strip.bottom() <= region.bottom() && strip.right() <= region.right());

            // And all three sections are in it, at both sizes.
            let mut p = Painter::new();
            let world = World::new(sim::Rng::from_seed(3), None);
            draw_chrome(
                &mut p,
                region,
                Some(&world),
                7,
                &crate::ui::Controls::default(),
                true,
            );
            let texts: Vec<&str> = p.text.iter().map(|item| item.content.as_str()).collect();
            for section in ["01 / PERCEPTION", "03 / MOTOR REQUESTS"] {
                assert!(
                    texts.contains(&section),
                    "{section} is missing at {width}×{height}"
                );
            }
            assert!(
                texts.iter().any(|text| text.starts_with("02 /")),
                "the Threat Telemetry section is missing at {width}×{height}"
            );
            for meter in ["PROXIMITY", "CLOSING - / + AWAY", "PRESSURE"] {
                assert!(
                    texts.contains(&meter),
                    "the {meter} meter is missing at {width}×{height}"
                );
            }
        }
    }
}
