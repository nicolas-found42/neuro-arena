//! The observatory's visual tokens: colour, light, type, spacing.
//!
//! One key light falls across the Arena from the upper left; the field behind
//! it is a cold volume of cloud and dust; everything the simulation *does* —
//! thrust, fire, impact, containment — is written as light. Colours are
//! authored in sRGB, the way a picker reads them, and converted to linear by
//! the Painter.
//!
//! The frame is high dynamic range (see `renderer`), so light carries a second
//! number besides its colour: a gain in [`light`] saying how far past white it
//! is written. That is the only thing the bloom threshold looks at, which is
//! why nothing in this file's interface palette can bloom.

use crate::painter::Rgba;

pub mod color {
    use super::Rgba;

    // ---- ground ---------------------------------------------------------
    /// The Arena: a window onto a cold volume, deeper and bluer than the
    /// instrument housing around it.
    pub const ARENA_BG: Rgba = Rgba::rgb(0.016, 0.027, 0.043);
    /// The application's own ground: near-black slate, a shade neutral, so the
    /// sidebar reads as housing rather than as more sky.
    pub const APP_BG: Rgba = Rgba::rgb(0.031, 0.039, 0.051);
    /// The shadow the field's edges and floor settle into.
    pub const VIGNETTE: Rgba = Rgba::rgb(0.004, 0.008, 0.016);

    // ---- deep field -----------------------------------------------------
    /// The two cloud layers behind the Arena, and the tint the upper air
    /// catches from them.
    pub const NEBULA_A: Rgba = Rgba::rgb(0.290, 0.247, 0.545);
    pub const NEBULA_B: Rgba = Rgba::rgb(0.078, 0.376, 0.435);
    pub const WASH: Rgba = Rgba::rgb(0.110, 0.204, 0.290);
    pub const STAR: Rgba = Rgba::rgba(0.878, 0.918, 1.0, 0.55);
    pub const STAR_BRIGHT: Rgba = Rgba::rgba(0.898, 0.957, 1.0, 0.95);

    // ---- instrument housing ---------------------------------------------
    /// The colour of the key light where it grazes an instrument surface.
    /// Every panel, key and field is lifted toward it along its top edge, which
    /// is what makes the sidebar read as one piece of hardware.
    pub const KEY: Rgba = Rgba::rgb(0.596, 0.729, 0.816);
    pub const PANEL_BG: Rgba = Rgba::rgb(0.043, 0.055, 0.071);
    pub const PANEL_BORDER: Rgba = Rgba::rgb(0.118, 0.157, 0.192);
    /// The hairline along a panel's lit edge, a step brighter than its border.
    pub const PANEL_EDGE: Rgba = Rgba::rgb(0.204, 0.275, 0.329);
    pub const PANEL_TITLE: Rgba = Rgba::rgb(0.451, 0.588, 0.651);

    // ---- type -----------------------------------------------------------
    pub const TEXT: Rgba = Rgba::rgb(0.898, 0.925, 0.949);
    pub const TEXT_DIM: Rgba = Rgba::rgb(0.541, 0.600, 0.659);
    /// The quietest ink the interface prints a *reading* in. It was authored
    /// at `rgb(0.361, 0.412, 0.467)`, which is about 3.5:1 on the panel — below
    /// WCAG AA for the 8–10 pt type it carries — so it was lifted until it
    /// clears 4.5:1. `the_reading_inks_clear_aa_contrast` pins it.
    pub const TEXT_FAINT: Rgba = Rgba::rgb(0.451, 0.502, 0.557);

    // ---- semantic -------------------------------------------------------
    /// Cyan is perception and the instrument's own voice.
    pub const ACCENT: Rgba = Rgba::rgb(0.290, 0.851, 0.980);
    /// Amber is energy: thrust, fire, heat, threat closing in.
    pub const ENERGY: Rgba = Rgba::rgb(1.0, 0.678, 0.247);
    pub const WARN: Rgba = Rgba::rgb(1.0, 0.478, 0.353);
    pub const OK: Rgba = Rgba::rgb(0.451, 0.859, 0.549);

    // ---- the Ship -------------------------------------------------------
    pub const SHIP: Rgba = Rgba::rgb(0.455, 0.925, 1.0);
    pub const SHIP_FLAME: Rgba = ENERGY;
    pub const SHIP_DEAD: Rgba = Rgba::rgb(0.667, 0.302, 0.286);
    /// The white-hot centre of a flame or a muzzle.
    pub const LIGHT_FLAME_CORE: Rgba = Rgba::rgb(1.0, 0.965, 0.812);
    pub const LIGHT_IMPACT: Rgba = Rgba::rgb(1.0, 0.851, 0.545);
    pub const BULLET: Rgba = Rgba::rgb(1.0, 0.898, 0.514);
    pub const RAY: Rgba = Rgba::rgba(0.290, 0.851, 0.980, 0.30);

    // ---- Asteroids ------------------------------------------------------
    /// Rock is a material under one key light: warm stone where the light
    /// falls, near-black where it does not, and a cool rim picked out of the
    /// field. The range is wide on purpose — an Asteroid has to read as mass.
    pub const ASTEROID_LIT: Rgba = Rgba::rgb(0.643, 0.573, 0.478);
    pub const ASTEROID_UNLIT: Rgba = Rgba::rgb(0.055, 0.078, 0.122);
    pub const ASTEROID_EDGE: Rgba = Rgba::rgb(0.729, 0.792, 0.827);
    /// The hot edge on the facets that face the key light.
    pub const ASTEROID_RIM: Rgba = Rgba::rgb(1.0, 0.941, 0.851);
    /// A mid-tone kept for anything that needs "the colour of rock".
    pub const ASTEROID: Rgba = Rgba::rgb(0.322, 0.333, 0.337);

    // ---- the Record -----------------------------------------------------
    //
    // A third family, beside perception (cyan) and energy (amber): the run's
    // own history. It is deliberately not a shade of either, because a series
    // that is the same ink as the sensorium reads as another live reading
    // rather than as the record of one. Violet sits far enough from both in
    // hue and in value to hold two series plus a ghost without a legend doing
    // the work.
    /// The breeding score: the series that actually climbs (ADR 0003 keeps it
    /// named apart from Competence, and the Chart says so on its face).
    pub const RECORD: Rgba = Rgba::rgb(0.639, 0.588, 0.980);
    /// The Population's best member, a step above the mean and drawn thinner:
    /// never shown without the mean beside it.
    pub const RECORD_PEAK: Rgba = Rgba::rgb(0.910, 0.890, 1.0);
    /// The previous Generation, kept under the current one so the movement
    /// between them is the thing the eye reads.
    pub const RECORD_GHOST: Rgba = Rgba::rgb(0.639, 0.588, 0.980);
    /// The cleared-Wave share: Competence, so it stays on the perception hue.
    pub const RECORD_CLEAR: Rgba = ACCENT;

    // ---- charts and the Network -----------------------------------------
    pub const BEST: Rgba = ACCENT;
    pub const MEAN: Rgba = ENERGY;
    /// The Chart's spread band: the Population's median-to-p90 ribbon, drawn
    /// behind the mean. A deep cyan off the same family as ACCENT, at a wash's
    /// strength rather than a reading's, so a range can stand under the subject
    /// without competing with it or passing for one.
    pub const BAND: Rgba = Rgba::rgba(0.161, 0.472, 0.546, 0.22);
    pub const NODE_INPUT: Rgba = Rgba::rgb(0.443, 0.678, 0.592);
    pub const NODE_HIDDEN: Rgba = Rgba::rgb(0.839, 0.784, 0.478);
    pub const NODE_OUTPUT: Rgba = Rgba::rgb(0.898, 0.545, 0.412);
    pub const NODE_EDGE: Rgba = Rgba::rgb(0.325, 0.361, 0.408);

    // ---- controls -------------------------------------------------------
    pub const BUTTON: Rgba = Rgba::rgb(0.078, 0.098, 0.125);
    pub const BUTTON_HOT: Rgba = Rgba::rgb(0.149, 0.216, 0.259);
    pub const BUTTON_ON: Rgba = Rgba::rgb(0.086, 0.310, 0.361);
    pub const FIELD: Rgba = Rgba::rgb(0.024, 0.035, 0.047);
}

/// How far past white each emitter is written. The frame holds values above
/// 1.0 and the bloom threshold sits at 1.0, so these numbers are exactly the
/// difference between "a bright shape" and "a light source".
///
/// They are ratios of intensity, not opacities: a gain of 4 means the emitter
/// is four times as bright as the brightest thing the interface can print.
pub mod light {
    /// The halo a living hull throws on the space around it.
    pub const SHIP_HALO: f32 = 1.6;
    /// The thrust plume, and the white centre inside it.
    pub const PLUME: f32 = 2.6;
    pub const PLUME_CORE: f32 = 6.0;
    /// A bullet: a hot ball with a tracer behind it.
    pub const BULLET: f32 = 7.0;
    pub const TRACER: f32 = 3.0;
    /// An Asteroid coming apart, and the Ship doing the same.
    pub const IMPACT: f32 = 5.0;
    pub const SPARK: f32 = 3.4;
    /// The containment seam, and a Wave pulsing along it.
    pub const SEAM: f32 = 1.5;
    pub const WAVE: f32 = 4.0;
    /// The watched Ship's trace, and the perception corona on its hull.
    pub const TRAIL: f32 = 1.8;
    pub const CORONA: f32 = 1.9;
    pub const INTENT: f32 = 2.0;
    /// The Sensor Ray overlay, where a ray has actually found something.
    pub const RAY: f32 = 1.8;
    /// A Network node firing, and the weighted signal path carrying it.
    pub const SIGNAL: f32 = 1.9;
    /// Instrument lamps in the chrome: bright enough to read as lit, not
    /// bright enough to smear the text beside them.
    pub const LAMP: f32 = 1.7;
    /// The shock front an Impact throws: brighter than the flash's own glow
    /// because the ring is thin, so it has to carry the same energy through
    /// far fewer fragments.
    pub const SHOCKWAVE: f32 = 6.5;
    /// The muzzle light when the Ship fires.
    pub const MUZZLE: f32 = 5.2;
    /// The Near Boundary the nine Sensor Rays bound, where a bearing found
    /// something. Below [`RAY`] on purpose: it is the shape *under* the
    /// readings, not another reading.
    pub const ENVELOPE: f32 = 1.7;
    /// A mote of near dust catching the key light. The dimmest light in the
    /// Arena: dust is only visible because it moves.
    pub const DUST: f32 = 1.2;
}

/// Timing tokens for the whole interface. One place to look up how long
/// something takes and what shape the curve is.
///
/// The durations and the four easing curves are IBM Carbon's motion tokens
/// (`packages/motion/src/tokens.ts`, Apache-2.0, © IBM), taken as published
/// values: a *productive* curve leaves and arrives quickly, an *expressive*
/// one lingers. Use `productive` for anything a person is waiting on and
/// `expressive` for anything the run is announcing.
pub mod motion {
    /// Seconds. A state change the pointer caused.
    pub const FAST_01: f32 = 0.070;
    /// Seconds. A small panel rearranging.
    pub const FAST_02: f32 = 0.110;
    /// Seconds. A message arriving.
    pub const MODERATE_01: f32 = 0.150;
    /// Seconds. A generation handover.
    pub const MODERATE_02: f32 = 0.240;
    /// Seconds. A run restarting.
    pub const SLOW_01: f32 = 0.400;
    /// Seconds. The longest thing the interface does.
    pub const SLOW_02: f32 = 0.700;

    /// Control points for a cubic Bézier, `[x1, y1, x2, y2]`, as CSS writes
    /// them. Every curve starts at (0,0) and ends at (1,1).
    pub type Curve = [f32; 4];

    pub const STANDARD_PRODUCTIVE: Curve = [0.2, 0.0, 0.38, 0.9];
    pub const STANDARD_EXPRESSIVE: Curve = [0.4, 0.14, 0.3, 1.0];
    pub const ENTRANCE_PRODUCTIVE: Curve = [0.0, 0.0, 0.38, 0.9];
    pub const ENTRANCE_EXPRESSIVE: Curve = [0.0, 0.0, 0.3, 1.0];
    pub const EXIT_PRODUCTIVE: Curve = [0.2, 0.0, 1.0, 0.9];
    pub const EXIT_EXPRESSIVE: Curve = [0.4, 0.14, 1.0, 1.0];

    /// One component of a cubic Bézier whose endpoints are 0 and 1.
    fn bezier(control_a: f32, control_b: f32, t: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * t * control_a + 3.0 * u * t * t * control_b + t * t * t
    }

    /// The curve's progress at time `x`, both in `0..=1`.
    ///
    /// Solved by bisection rather than Newton iteration: it always converges
    /// here, it is a fixed number of steps however steep the curve is, and it
    /// is exact arithmetic a test can rely on. Sixty-four steps is far past
    /// what a colour ramp can show.
    pub fn ease(curve: Curve, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        // The ends are exact, not approached: a fade that ends at 0.9999 is a
        // frame of visible ink, and a test can hold these two.
        if x == 0.0 || x == 1.0 {
            return x;
        }
        let [x1, y1, x2, y2] = curve;
        let (mut low, mut high) = (0.0f32, 1.0f32);
        for _ in 0..24 {
            let mid = 0.5 * (low + high);
            if bezier(x1, x2, mid) < x {
                low = mid;
            } else {
                high = mid;
            }
        }
        bezier(y1, y2, 0.5 * (low + high))
    }
}

pub mod font {
    /// The panel body size.
    pub const BODY: f32 = 12.0;
    /// Section headings.
    pub const HEADING: f32 = 11.0;
    /// The dominant HUD numbers.
    pub const BIG: f32 = 20.0;
    /// The run's headline numerals: the Generation and the Shaped Fitness at
    /// the top of the HUD, the largest type in the application.
    pub const HEADLINE: f32 = 28.0;
    pub const SMALL: f32 = 10.0;
    /// Captions under an instrument, and axis labels.
    pub const MICRO: f32 = 9.0;
    /// The fine print: legends and labels inside an instrument too crowded for
    /// MICRO, where a name has to stand beside a reading rather than under it.
    pub const FINE: f32 = 8.0;
    pub const LINE_STEP: f32 = 15.0;
}

pub mod layout {
    pub const SIDEBAR_WIDTH: f32 = 370.0;
    pub const MARGIN: f32 = 12.0;
    pub const GAP: f32 = 8.0;
    pub const ARENA_WIDTH: f32 = 960.0;
    pub const ARENA_HEIGHT: f32 = 600.0;
    pub const BUTTON_HEIGHT: f32 = 26.0;
}

/// How far a surface is lifted toward the key light along its top edge.
const KEY_LIFT: f32 = 0.11;

/// A surface's lit edge: its own colour, lifted toward the key light. One rule
/// for every panel, key and field in the application, so they are all lit from
/// the same place as the Arena is.
pub fn lit(base: Rgba) -> Rgba {
    base.mix(color::KEY, KEY_LIFT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luma(color: Rgba) -> f32 {
        0.299 * color.r + 0.587 * color.g + 0.114 * color.b
    }

    #[test]
    fn nothing_the_interface_prints_can_bloom() {
        // The bloom threshold is white. Every interface colour is authored at
        // or below it, which is what keeps panels, text and charts out of the
        // halo without a mask.
        for color in [
            color::TEXT,
            color::TEXT_DIM,
            color::PANEL_BG,
            lit(color::PANEL_BG),
            color::PANEL_EDGE,
            color::ACCENT,
            color::ENERGY,
            color::BUTTON_ON,
            color::ASTEROID_LIT,
            color::ASTEROID_RIM,
            // The Chart's own inks: the mean's line and the band behind it are
            // both authored at or below white, like everything else the
            // interface prints.
            color::BEST,
            color::BAND,
            // The Record's family, on the same budget as every other ink.
            color::RECORD,
            color::RECORD_PEAK,
            color::RECORD_CLEAR,
        ] {
            for channel in [color.r, color.g, color.b] {
                assert!((0.0..=1.0).contains(&channel), "{color:?} leaves the gamut");
            }
        }
        // And every light is authored past it, or it is not a light.
        for gain in [
            light::SHIP_HALO,
            light::PLUME,
            light::BULLET,
            light::IMPACT,
            light::SEAM,
            light::TRAIL,
            light::CORONA,
            light::RAY,
            light::LAMP,
            light::SHOCKWAVE,
            light::MUZZLE,
            light::ENVELOPE,
            light::DUST,
        ] {
            assert!(gain > 1.0, "a light written at {gain} would never bloom");
        }
    }

    /// WCAG 2.2 relative luminance for an sRGB triple.
    fn relative_luminance(color: Rgba) -> f32 {
        let channel = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }

    /// WCAG 2.2 contrast ratio, 1:1 through 21:1.
    fn contrast(a: Rgba, b: Rgba) -> f32 {
        let (la, lb) = (relative_luminance(a), relative_luminance(b));
        let (high, low) = if la > lb { (la, lb) } else { (lb, la) };
        (high + 0.05) / (low + 0.05)
    }

    #[test]
    fn the_reading_inks_clear_aa_contrast() {
        // Every ink the interface prints a *reading* in has to clear WCAG AA
        // (4.5:1) on the surface it is printed on, at the 8–12 pt sizes the
        // panels are set in. TEXT_FAINT used to fail this at about 3.5:1.
        for (ink, name) in [
            (color::TEXT, "TEXT"),
            (color::TEXT_DIM, "TEXT_DIM"),
            (color::TEXT_FAINT, "TEXT_FAINT"),
            (color::ACCENT, "ACCENT"),
            (color::RECORD, "RECORD"),
            (color::RECORD_PEAK, "RECORD_PEAK"),
        ] {
            let ratio = contrast(ink, color::PANEL_BG);
            assert!(ratio >= 4.5, "{name} on PANEL_BG is only {ratio:.2}:1");
        }
        // And the three are still three: the quietest ink must be visibly
        // quieter than the loudest, or the hierarchy collapses.
        assert!(relative_luminance(color::TEXT_FAINT) < relative_luminance(color::TEXT_DIM));
        assert!(relative_luminance(color::TEXT_DIM) < relative_luminance(color::TEXT));
    }

    #[test]
    fn an_easing_curve_is_monotonic_and_pins_its_ends() {
        // A curve that ever went backwards would make an animation reverse.
        for curve in [
            motion::STANDARD_PRODUCTIVE,
            motion::STANDARD_EXPRESSIVE,
            motion::ENTRANCE_PRODUCTIVE,
            motion::EXIT_PRODUCTIVE,
        ] {
            assert_eq!(motion::ease(curve, 0.0), 0.0);
            assert_eq!(motion::ease(curve, 1.0), 1.0);
            let mut previous = -1.0;
            for step in 0..=100 {
                let value = motion::ease(curve, step as f32 / 100.0);
                assert!(value >= previous, "{curve:?} reverses at {step}");
                previous = value;
            }
        }
        // An entrance curve is a fast-then-settling rise: by the midpoint it
        // has covered more than half its distance.
        assert!(motion::ease(motion::ENTRANCE_PRODUCTIVE, 0.5) > 0.5);
        // An exit curve is the opposite: it holds, then falls away late.
        assert!(motion::ease(motion::EXIT_PRODUCTIVE, 0.5) < 0.5);
    }

    #[test]
    fn the_arena_is_the_darkest_ground_in_the_window() {
        // The field has to sit below the housing, or the Arena reads as a
        // panel rather than as a window onto something.
        assert!(luma(color::ARENA_BG) < luma(color::APP_BG));
        assert!(luma(color::APP_BG) < luma(color::PANEL_BG));
        assert!(luma(color::PANEL_BG) < luma(lit(color::PANEL_BG)));
        // A lit top edge is a graze on the face, still below the border that
        // closes the panel and below the hairline that marks where it catches.
        assert!(luma(lit(color::PANEL_BG)) < luma(color::PANEL_BORDER));
        assert!(luma(color::PANEL_BORDER) < luma(color::PANEL_EDGE));
    }

    #[test]
    fn rock_has_a_wide_enough_range_to_read_as_mass() {
        let lit = luma(color::ASTEROID_LIT);
        let unlit = luma(color::ASTEROID_UNLIT);
        assert!(lit - unlit > 0.4, "rock is flat: {lit} vs {unlit}");
        // The unlit side still sits above the field, so a rock is a body in
        // the dark rather than a hole cut out of it.
        assert!(unlit > luma(color::ARENA_BG));
    }

    #[test]
    fn every_surface_is_lifted_toward_the_same_key_light() {
        for base in [
            color::PANEL_BG,
            color::BUTTON,
            color::FIELD,
            color::BUTTON_ON,
        ] {
            let edge = lit(base);
            assert!(luma(edge) > luma(base), "{base:?} is not lit at all");
            // The lift is a graze, not a repaint: the surface keeps its identity.
            assert!(luma(edge) - luma(base) < 0.12, "{base:?} is washed out");
        }
        // Two surfaces that differ still differ once they are lit, so the key
        // light can never flatten the hierarchy it is drawn on top of.
        assert!(luma(lit(color::BUTTON_ON)) > luma(lit(color::BUTTON)));
    }
}
