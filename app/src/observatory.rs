//! Observatory composition and read-only, bounded presentation history.
use crate::{
    painter::{Align, Painter, Rgba},
    scene::ArenaView,
    theme::{color, font},
    ui::{Controls, Rect},
};
use sim::{config::arena, World};
use std::collections::VecDeque;

/// An Episode identity prevents connecting different Ships after a transition.
#[derive(Default)]
pub struct Trail {
    key: Option<(u32, usize)>,
    time: f64,
    points: VecDeque<(f64, [f32; 2])>,
}
impl Trail {
    pub fn clear(&mut self) {
        self.key = None;
        self.points.clear();
        self.time = 0.0;
    }
    pub fn observe(&mut self, key: (u32, usize), world: &World, enabled: bool) {
        if !enabled {
            self.clear();
            return;
        }
        let dt = world.time - self.time;
        if self.key != Some(key) || dt < 0.0 || dt > 0.25 {
            self.clear();
            self.key = Some(key);
        }
        if world.time == self.time && !self.points.is_empty() {
            return;
        }
        self.time = world.time;
        if !world.agent.alive {
            return;
        }
        while self
            .points
            .front()
            .is_some_and(|(time, _)| world.time - time > 0.8)
        {
            self.points.pop_front();
        }
        if self
            .points
            .back()
            .is_some_and(|(time, _)| world.time - time < 1.0 / 60.0 - 1e-6)
        {
            return;
        }
        if self.points.len() == 48 {
            self.points.pop_front();
        }
        self.points.push_back((
            world.time,
            [world.agent.ship.x as f32, world.agent.ship.y as f32],
        ));
    }
    pub fn draw(&self, p: &mut Painter, view: ArenaView) {
        p.set_transform(view.transform());
        p.set_clip(Some([0.0, 0.0, arena::WIDTH as f32, arena::HEIGHT as f32]));
        p.set_gain(crate::theme::light::TRAIL);
        for i in 1..self.points.len() {
            let a = self.points[i - 1].1;
            let b = self.points[i].1;
            // A seam jump is never a physical trajectory across the Arena.
            if (a[0] - b[0]).abs() > arena::WIDTH as f32 * 0.5
                || (a[1] - b[1]).abs() > arena::HEIGHT as f32 * 0.5
            {
                continue;
            }
            let age = (1.0 - (self.time - self.points[i].0) as f32 / 0.8).clamp(0.0, 1.0);
            // Light, not matter: the trace tapers and brightens toward the Ship.
            crate::effects::light_stroke(p, a, b, 0.5 + age * 0.9, color::SHIP.alpha(age * 0.5));
        }
        p.set_gain(1.0);
        p.set_clip(None);
    }
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
        for i in 0..100 {
            w.time = i as f64 / 60.0;
            t.observe((1, 0), &w, true);
        }
        assert_eq!(t.points.len(), 48);
        t.observe((1, 0), &w, true);
        assert_eq!(t.points.len(), 48);
        t.observe((2, 0), &w, true);
        assert_eq!(t.points.len(), 1);
        w.time += 1.0;
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
            for i in 0..hz * 3 {
                w.time = f64::from(i) / f64::from(hz);
                trail.observe((1, 0), &w, true);
            }
            assert!(trail.points.len() <= 48);
            assert!(trail.time - trail.points.front().unwrap().0 <= 0.8 + 1e-9);
        }
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
