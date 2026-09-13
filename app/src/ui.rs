//! Where the window's furniture sits, what a point hits, and the control strip's
//! state.
//!
//! The sidebar is laid out in window pixels at 1:1: the Arena scales with the
//! window (ADR 0006), the HUD does not, so panel text stays legible and crisp at
//! any window size. Panels are stacked top to bottom inside the docked sidebar
//! and give way in priority order when the window is too short for all of them,
//! never overlapping each other and never leaving the window.

use crate::theme::layout::{GAP, MARGIN, SIDEBAR_WIDTH};

/// The slider's top end: "as fast as the machine allows" rather than a number.
pub const SPEED_MAX: f64 = 1_000_000.0;

/// The seed field's width in digits; ten digits is more than a `u32` holds, so a
/// typed seed either parses or is visibly rejected.
const SEED_MAX_CHARS: usize = 10;

/// How much of the strip's width the slider track takes; the label takes the rest.
const SLIDER_WIDTH_FRACTION: f32 = 0.62;

/// The control strip's rows, top to bottom: two button rows, the seed field, the
/// speed slider.
const STRIP_BUTTON_H: f32 = 26.0;
const STRIP_FIELD_H: f32 = 22.0;
const STRIP_SLIDER_H: f32 = 22.0;

/// The panel body's inner padding, the heading line's height, and the space
/// under the heading — shared with `panels` so the control strip's rows begin
/// exactly where the heading ends.
pub(crate) const PANEL_PAD: f32 = 8.0;
pub(crate) const TITLE_H: f32 = 13.0;
pub(crate) const TITLE_GAP: f32 = 4.0;

/// A rectangle in window pixels. The left and top edges are inside and the right
/// and bottom edges are outside, so rects that share an edge never overlap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Is `p` inside? A rect with no area contains no point.
    pub fn contains(&self, p: [f32; 2]) -> bool {
        p[0] >= self.x && p[0] < self.right() && p[1] >= self.y && p[1] < self.bottom()
    }

    pub fn center(&self) -> [f32; 2] {
        [self.x + self.w * 0.5, self.y + self.h * 0.5]
    }

    /// Shrink on every side; never larger than the original, never negative.
    pub fn inset(&self, by: f32) -> Rect {
        Rect::new(
            self.x + by,
            self.y + by,
            (self.w - 2.0 * by).max(0.0),
            (self.h - 2.0 * by).max(0.0),
        )
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// Every interactive target in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Pause,
    Rays,
    Restart,
    NewSeed,
    Save,
    Load,
    Evolve,
    SpeedSlider,
    SeedField,
}

/// Keyboard traversal follows the visual reading order of the control strip.
pub const FOCUS_ORDER: [Hit; 9] = [
    Hit::Pause,
    Hit::Rays,
    Hit::Evolve,
    Hit::Restart,
    Hit::NewSeed,
    Hit::Save,
    Hit::Load,
    Hit::SeedField,
    Hit::SpeedSlider,
];

/// The five stacked sidebar panels, top to bottom: HUD, Chart, Network,
/// Controls, Status.
const PANELS: usize = 5;

/// Their natural heights. With the margins and gaps paid, the whole stack fits a
/// 700-pixel-tall window without shrinking anything.
const NATURAL: [f32; PANELS] = [216.0, 120.0, 180.0, 154.0, 40.0];

/// The height each panel gives way to before the next one shrinks.
const MINIMUM: [f32; PANELS] = [160.0, 56.0, 96.0, 140.0, 26.0];

/// Shrink order once the stack no longer fits: the Network gives way first, then
/// the Chart, then the furniture.
const SHRINK: [usize; PANELS] = [2, 1, 0, 3, 4];

/// Where every panel and every control sits for a window of a given size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// The fixed 960×600 logical playfield, scaled into the space left of the
    /// sidebar.
    pub arena: Rect,
    /// The docked column that holds every panel.
    pub sidebar: Rect,
    pub hud: Rect,
    pub chart: Rect,
    pub network: Rect,
    pub controls: Rect,
    pub status: Rect,
    pub pause: Rect,
    pub rays: Rect,
    pub restart: Rect,
    pub new_seed: Rect,
    pub save: Rect,
    pub load: Rect,
    pub evolve: Rect,
    /// The slider's track, the whole strip of it a click can land on.
    pub speed_slider: Rect,
    /// The seed box the caret lives in.
    pub seed_field: Rect,
}

impl Layout {
    /// Full-window layout for a window of `width` × `height` physical pixels.
    pub fn new(width: f32, height: f32) -> Layout {
        let width = width.max(0.0);
        let height = height.max(0.0);
        let sidebar_width = SIDEBAR_WIDTH.min(width);
        let sidebar = Rect::new(width - sidebar_width, 0.0, sidebar_width, height);
        let arena = Rect::new(0.0, 0.0, width - sidebar_width, height);

        let column = sidebar.inset(MARGIN);
        let heights = stack(column.h);

        let mut y = column.y;
        let mut place = |index: usize| {
            // The stack already fits; clamping the bottom is what makes "inside
            // the window" true of every size, down to a degenerate one.
            let h = heights[index].min((column.bottom() - y).max(0.0));
            let rect = Rect::new(column.x, y, column.w, h);
            y += h + GAP;
            rect
        };
        let hud = place(0);
        let chart = place(1);
        let network = place(2);
        let controls = place(3);
        let status = place(4);
        let strip = Strip::new(controls);

        Layout {
            arena,
            sidebar,
            hud,
            chart,
            network,
            controls,
            status,
            pause: strip.pause,
            rays: strip.rays,
            restart: strip.restart,
            new_seed: strip.new_seed,
            save: strip.save,
            load: strip.load,
            evolve: strip.evolve,
            speed_slider: strip.slider,
            seed_field: strip.seed_field,
        }
    }

    pub fn control_rect(&self, hit: Hit) -> Rect {
        match hit {
            Hit::Pause => self.pause,
            Hit::Rays => self.rays,
            Hit::Restart => self.restart,
            Hit::NewSeed => self.new_seed,
            Hit::Save => self.save,
            Hit::Load => self.load,
            Hit::Evolve => self.evolve,
            Hit::SpeedSlider => self.speed_slider,
            Hit::SeedField => self.seed_field,
        }
    }

    /// Which control a window-pixel point hits, if any.
    pub fn hit(&self, p: [f32; 2]) -> Option<Hit> {
        [
            (self.pause, Hit::Pause),
            (self.rays, Hit::Rays),
            (self.evolve, Hit::Evolve),
            (self.restart, Hit::Restart),
            (self.new_seed, Hit::NewSeed),
            (self.save, Hit::Save),
            (self.load, Hit::Load),
            (self.speed_slider, Hit::SpeedSlider),
            (self.seed_field, Hit::SeedField),
        ]
        .into_iter()
        .find(|(rect, _)| rect.contains(p))
        .map(|(_, hit)| hit)
    }
}

/// Where the five panels land inside the sidebar column, in the order they are
/// declared: the natural heights, shrunk by priority when the window is short.
fn stack(available: f32) -> [f32; PANELS] {
    let available = (available - GAP * (PANELS - 1) as f32).max(0.0);
    let mut heights = NATURAL;
    // Give spare height to learning inspection instead of leaving an empty dock.
    heights[2] += (available - NATURAL.iter().sum::<f32>()).max(0.0);
    let mut deficit = heights.iter().sum::<f32>() - available;
    if deficit > 0.0 {
        for index in SHRINK {
            let take = deficit.min(heights[index] - MINIMUM[index]);
            heights[index] -= take.max(0.0);
            deficit -= take.max(0.0);
            if deficit <= 0.0 {
                break;
            }
        }
    }
    if deficit > 0.0 {
        // Shorter than every minimum together: everything shrinks together
        // rather than spilling out of the window.
        let total: f32 = heights.iter().sum();
        let scale = if total > 0.0 {
            (available / total).clamp(0.0, 1.0)
        } else {
            0.0
        };
        for height in &mut heights {
            *height *= scale;
        }
    }
    heights
}

/// The control strip's internal geometry, inside its panel rect.
struct Strip {
    pause: Rect,
    rays: Rect,
    evolve: Rect,
    restart: Rect,
    new_seed: Rect,
    save: Rect,
    load: Rect,
    seed_field: Rect,
    slider: Rect,
}

impl Strip {
    fn new(panel: Rect) -> Strip {
        let inner = panel.inset(PANEL_PAD);
        let top = inner.y + TITLE_H + TITLE_GAP;
        let available = (inner.bottom() - top).max(0.0);
        let needed = STRIP_BUTTON_H * 2.0 + STRIP_FIELD_H + STRIP_SLIDER_H + GAP * 3.0;
        // Row heights and gaps give way together, so rows can never overlap and
        // can never leave the panel however short the window gets.
        let scale = if needed > 0.0 {
            (available / needed).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let button_h = STRIP_BUTTON_H * scale;
        let field_h = STRIP_FIELD_H * scale;
        let slider_h = STRIP_SLIDER_H * scale;
        let gap = GAP * scale;

        let mut y = top;
        let buttons_a = Rect::new(inner.x, y, inner.w, button_h);
        y += button_h + gap;
        let buttons_b = Rect::new(inner.x, y, inner.w, button_h);
        y += button_h + gap;
        let field_row = Rect::new(inner.x, y, inner.w, field_h);
        y += field_h + gap;
        let slider_row = Rect::new(inner.x, y, inner.w, slider_h);

        Strip {
            pause: cell(buttons_a, 0, 3),
            rays: cell(buttons_a, 1, 3),
            evolve: cell(buttons_a, 2, 3),
            restart: cell(buttons_b, 0, 4),
            new_seed: cell(buttons_b, 1, 4),
            save: cell(buttons_b, 2, 4),
            load: cell(buttons_b, 3, 4),
            seed_field: field_row,
            slider: Rect::new(
                slider_row.x,
                slider_row.y,
                slider_row.w * SLIDER_WIDTH_FRACTION,
                slider_row.h,
            ),
        }
    }
}

/// One cell of a row split `count` ways, the `index`th from the left.
fn cell(row: Rect, index: usize, count: usize) -> Rect {
    let width = ((row.w - GAP * (count - 1) as f32) / count as f32).max(0.0);
    Rect::new(row.x + (width + GAP) * index as f32, row.y, width, row.h)
}

/// The control strip's state, owned by the app shell.
#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    pub paused: bool,
    pub rays: bool,
    /// Simulated seconds per wall second; `SPEED_MAX` means "as fast as the
    /// machine allows".
    pub speed: f64,
    /// What the seed field shows: the live run's seed while it is not being
    /// edited, and the digits the user is typing while it is.
    pub seed_text: String,
    pub seed_editing: bool,
    /// True while a loaded Genome is being replayed instead of evolving.
    pub watching: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            paused: false,
            rays: false,
            speed: 100.0,
            seed_text: String::new(),
            seed_editing: false,
            watching: false,
        }
    }
}

impl Controls {
    /// The speed a slider at `t` (0..=1) selects: logarithmic from ×1 to
    /// ×1,000,000, so the whole range is reachable with the mouse.
    pub fn speed_from_slider(t: f32) -> f64 {
        let t = f64::from(t.clamp(0.0, 1.0));
        10f64.powf(6.0 * t).clamp(1.0, SPEED_MAX)
    }

    /// Where the knob sits for a speed: the inverse of `speed_from_slider`,
    /// exact to within a pixel of the track.
    pub fn slider_from_speed(speed: f64) -> f32 {
        if !speed.is_finite() {
            return 1.0;
        }
        (speed.clamp(1.0, SPEED_MAX).log10() / 6.0).clamp(0.0, 1.0) as f32
    }

    /// Is the speed the unbounded end of the slider?
    pub fn is_unbounded(speed: f64) -> bool {
        speed >= SPEED_MAX * 0.999
    }

    /// How the speed reads: `×100`, `×10,000`, or `∞ unbounded`.
    pub fn speed_label(speed: f64) -> String {
        if !speed.is_finite() || Self::is_unbounded(speed) {
            return "∞ unbounded".to_string();
        }
        let value = speed.max(0.0).round().min(SPEED_MAX) as u32;
        format!("×{}", grouped(value))
    }

    /// Type a digit into the seed field; anything else is ignored.
    pub fn push_seed_char(&mut self, c: char) {
        if !c.is_ascii_digit() || self.seed_text.len() >= SEED_MAX_CHARS {
            return;
        }
        self.seed_text.push(c);
    }

    pub fn pop_seed_char(&mut self) {
        self.seed_text.pop();
    }

    /// The seed the field names, once it names a run at all.
    pub fn seed_value(&self) -> Option<u32> {
        if self.seed_text.is_empty() {
            return None;
        }
        self.seed_text.parse().ok()
    }
}

/// Digits with a grouping comma, so a long speed stays readable.
fn grouped(value: u32) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: (f32, f32) = (600.0, 400.0);
    const LARGE: (f32, f32) = (3840.0, 2160.0);

    fn panels(layout: &Layout) -> [(&'static str, Rect); 7] {
        [
            ("arena", layout.arena),
            ("sidebar", layout.sidebar),
            ("hud", layout.hud),
            ("chart", layout.chart),
            ("network", layout.network),
            ("controls", layout.controls),
            ("status", layout.status),
        ]
    }

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }

    fn sizes() -> Vec<(f32, f32)> {
        let mut sizes = vec![SMALL, LARGE];
        for width in [600.0, 960.0, 1280.0, 1728.0, 2560.0, 3840.0] {
            for height in [400.0, 640.0, 700.0, 1080.0, 1440.0, 2160.0] {
                sizes.push((width, height));
            }
        }
        sizes
    }

    #[test]
    fn every_panel_stays_inside_the_window() {
        for (width, height) in sizes() {
            let layout = Layout::new(width, height);
            for (name, rect) in panels(&layout) {
                assert!(
                    rect.x >= -0.5
                        && rect.y >= -0.5
                        && rect.right() <= width + 0.5
                        && rect.bottom() <= height + 0.5,
                    "{name} escapes a {width}×{height} window: {rect:?}"
                );
            }
        }
    }

    #[test]
    fn panels_never_overlap_each_other() {
        for (width, height) in sizes() {
            let layout = Layout::new(width, height);
            // The sidebar is the docked column that holds the stack, so the Arena
            // is its only peer; every stacked panel sits inside it.
            let siblings = [
                ("arena", layout.arena),
                ("hud", layout.hud),
                ("chart", layout.chart),
                ("network", layout.network),
                ("controls", layout.controls),
                ("status", layout.status),
            ];
            for (index, (name, rect)) in siblings.iter().enumerate() {
                for (other_name, other) in &siblings[index + 1..] {
                    assert!(
                        !overlaps(*rect, *other),
                        "{name} and {other_name} overlap in a {width}×{height} window: {rect:?} {other:?}"
                    );
                }
            }
            for (name, rect) in &siblings[1..] {
                assert!(
                    rect.x >= layout.sidebar.x
                        && rect.right() <= layout.sidebar.right() + 0.5
                        && rect.y >= layout.sidebar.y
                        && rect.bottom() <= layout.sidebar.bottom() + 0.5,
                    "{name} is not inside the sidebar in a {width}×{height} window: {rect:?}"
                );
            }
        }
    }

    #[test]
    fn hit_resolves_every_control() {
        for (width, height) in [SMALL, (1280.0, 800.0), (1728.0, 1117.0), LARGE] {
            let layout = Layout::new(width, height);
            let targets = [
                (Hit::Pause, layout.pause),
                (Hit::Rays, layout.rays),
                (Hit::Restart, layout.restart),
                (Hit::NewSeed, layout.new_seed),
                (Hit::Save, layout.save),
                (Hit::Load, layout.load),
                (Hit::Evolve, layout.evolve),
                (Hit::SpeedSlider, layout.speed_slider),
                (Hit::SeedField, layout.seed_field),
            ];
            for (hit, rect) in targets {
                assert!(
                    rect.w > 0.0 && rect.h > 0.0,
                    "{hit:?} has no area in a {width}×{height} window: {rect:?}"
                );
                assert_eq!(
                    layout.hit(rect.center()),
                    Some(hit),
                    "{hit:?} did not resolve its own centre in a {width}×{height} window: {rect:?}"
                );
            }
            // The Arena and the sidebar are not controls.
            assert_eq!(layout.hit(layout.arena.center()), None);
            assert_eq!(layout.hit([width + 40.0, height + 40.0]), None);
        }
    }

    #[test]
    fn the_sidebar_never_scales_with_the_window() {
        let normal = Layout::new(1200.0, 900.0);
        let huge = Layout::new(3840.0, 2160.0);
        // Docked right at a fixed width, full height, and the panels inside it
        // keep both their size and their place relative to it.
        assert_eq!(normal.sidebar.w, SIDEBAR_WIDTH);
        assert_eq!(huge.sidebar.w, SIDEBAR_WIDTH);
        for (name, rect, other) in [
            ("hud", normal.hud, huge.hud),
            ("chart", normal.chart, huge.chart),
            ("network", normal.network, huge.network),
            ("controls", normal.controls, huge.controls),
            ("pause", normal.pause, huge.pause),
            ("speed slider", normal.speed_slider, huge.speed_slider),
        ] {
            assert_eq!(rect.w, other.w, "{name} width follows the window");
            if name != "network" {
                assert_eq!(rect.h, other.h, "{name} height follows the window");
            }
            assert_eq!(
                rect.x - normal.sidebar.x,
                other.x - huge.sidebar.x,
                "{name} moved in the sidebar"
            );
        }
        // The Arena is what the window adds to.
        assert!(huge.arena.w > normal.arena.w);
        assert_eq!(huge.arena.right(), huge.sidebar.x);
    }

    #[test]
    fn a_tall_window_gives_spare_height_to_network_inspection() {
        let layout = Layout::new(1200.0, 900.0);
        assert_eq!(layout.hud.h, NATURAL[0]);
        assert_eq!(layout.chart.h, NATURAL[1]);
        assert!(layout.network.h > NATURAL[2]);
        assert!((layout.status.bottom() - 888.0).abs() < 0.01);
        assert_eq!(layout.controls.h, NATURAL[3]);
        assert_eq!(layout.status.h, NATURAL[4]);
        // The stack is top-anchored in the docked column and fits with slack to
        // spare; a taller window leaves that slack below it rather than scaling.
        assert!(layout.status.bottom() <= 900.0 - MARGIN);
    }

    #[test]
    fn a_short_window_shrinks_the_network_first_then_the_chart() {
        let middling = Layout::new(1200.0, 740.0);
        assert!(
            middling.network.h < NATURAL[2],
            "the Network gives way first"
        );
        assert_eq!(middling.chart.h, NATURAL[1], "the Chart holds its height");
        assert_eq!(middling.hud.h, NATURAL[0], "the HUD holds its height");

        let cramped = Layout::new(1200.0, 500.0);
        assert!(cramped.network.h < middling.network.h);
        assert!(cramped.chart.h < NATURAL[1], "then the Chart gives way too");
    }

    #[test]
    fn the_slider_round_trips_within_a_pixel() {
        let track = Layout::new(1280.0, 800.0).speed_slider.w;
        assert!(track > 50.0);
        let pixel = 1.0 / track;
        for step in 0..=100 {
            let t = step as f32 / 100.0;
            let back = Controls::slider_from_speed(Controls::speed_from_slider(t));
            assert!(
                (back - t).abs() <= pixel,
                "t = {t} came back as {back}, more than a pixel ({pixel}) away"
            );
        }
        for speed in [1.0, 2.0, 100.0, 1234.5, 999_999.0, SPEED_MAX] {
            let back = Controls::speed_from_slider(Controls::slider_from_speed(speed));
            assert!(
                (back / speed - 1.0).abs() < 0.08,
                "×{speed} came back as ×{back}, further than a slider pixel"
            );
        }
    }

    #[test]
    fn speeds_read_as_promised() {
        assert_eq!(Controls::speed_label(1.0), "×1");
        assert_eq!(Controls::speed_label(100.0), "×100");
        assert_eq!(Controls::speed_label(999.0), "×999");
        assert_eq!(Controls::speed_label(1000.0), "×1,000");
        assert_eq!(Controls::speed_label(10_000.0), "×10,000");
        assert_eq!(Controls::speed_label(250_000.0), "×250,000");
        assert_eq!(Controls::speed_label(SPEED_MAX), "∞ unbounded");
        assert_eq!(Controls::speed_label(SPEED_MAX * 0.999), "∞ unbounded");
        assert!(Controls::is_unbounded(SPEED_MAX));
        assert!(!Controls::is_unbounded(SPEED_MAX * 0.9));
    }

    #[test]
    fn the_seed_field_takes_digits_only() {
        let mut controls = Controls::default();
        assert_eq!(controls.seed_value(), None);
        for c in "12a3 4".chars() {
            controls.push_seed_char(c);
        }
        assert_eq!(controls.seed_text, "1234");
        assert_eq!(controls.seed_value(), Some(1234));
        controls.pop_seed_char();
        assert_eq!(controls.seed_value(), Some(123));

        controls.seed_text.clear();
        for _ in 0..20 {
            controls.push_seed_char('7');
        }
        assert_eq!(controls.seed_text, "7777777777");
        // Ten digits is more than a u32 holds: the field says so rather than
        // silently wrapping.
        assert_eq!(controls.seed_value(), None);

        controls.seed_text = u32::MAX.to_string();
        assert_eq!(controls.seed_value(), Some(u32::MAX));
    }
}
