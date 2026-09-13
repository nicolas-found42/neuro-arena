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
    pub const TEXT_FAINT: Rgba = Rgba::rgb(0.361, 0.412, 0.467);

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

    // ---- charts and the Network -----------------------------------------
    pub const BEST: Rgba = ACCENT;
    pub const MEAN: Rgba = ENERGY;
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
}

pub mod font {
    /// The panel body size.
    pub const BODY: f32 = 12.0;
    /// Section headings.
    pub const HEADING: f32 = 11.0;
    /// The dominant HUD numbers.
    pub const BIG: f32 = 20.0;
    pub const SMALL: f32 = 10.0;
    /// Captions under an instrument, and axis labels.
    pub const MICRO: f32 = 9.0;
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
        ] {
            assert!(gain > 1.0, "a light written at {gain} would never bloom");
        }
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
