//! The look: a clear palette on a dark field, plain filled polygons, system
//! fonts. Nothing of the retired browser chrome (ADR 0004) — no Win98 bevels,
//! no hachure. Colours are authored for an sRGB target: what you read here is
//! what the window shows.

use crate::painter::Rgba;

pub mod color {
    use super::Rgba;

    /// The Arena's background, a shade darker than the app's.
    pub const ARENA_BG: Rgba = Rgba::rgb(0.043, 0.055, 0.078);
    pub const APP_BG: Rgba = Rgba::rgb(0.086, 0.094, 0.110);
    pub const PANEL_BG: Rgba = Rgba::rgb(0.129, 0.141, 0.161);
    pub const PANEL_BORDER: Rgba = Rgba::rgb(0.216, 0.235, 0.263);
    pub const PANEL_TITLE: Rgba = Rgba::rgb(0.557, 0.596, 0.647);

    pub const TEXT: Rgba = Rgba::rgb(0.902, 0.914, 0.929);
    pub const TEXT_DIM: Rgba = Rgba::rgb(0.596, 0.639, 0.690);

    pub const SHIP: Rgba = Rgba::rgb(0.376, 0.816, 1.0);
    pub const SHIP_FLAME: Rgba = Rgba::rgb(1.0, 0.702, 0.278);
    pub const SHIP_DEAD: Rgba = Rgba::rgb(0.545, 0.235, 0.235);

    pub const ROCK: Rgba = Rgba::rgb(0.616, 0.643, 0.678);
    pub const ROCK_EDGE: Rgba = Rgba::rgb(0.804, 0.827, 0.855);

    pub const BULLET: Rgba = Rgba::rgb(1.0, 0.878, 0.400);
    pub const RAY: Rgba = Rgba::rgba(0.376, 0.816, 1.0, 0.30);
    pub const STAR: Rgba = Rgba::rgba(1.0, 1.0, 1.0, 0.55);

    pub const BEST: Rgba = Rgba::rgb(0.224, 0.816, 1.0);
    pub const MEAN: Rgba = Rgba::rgb(1.0, 0.702, 0.278);

    pub const NODE_INPUT: Rgba = Rgba::rgb(0.482, 0.639, 0.478);
    pub const NODE_HIDDEN: Rgba = Rgba::rgb(0.855, 0.804, 0.478);
    pub const NODE_OUTPUT: Rgba = Rgba::rgb(0.855, 0.545, 0.416);
    pub const NODE_EDGE: Rgba = Rgba::rgb(0.353, 0.400, 0.451);

    pub const ACCENT: Rgba = Rgba::rgb(0.376, 0.816, 1.0);
    pub const WARN: Rgba = Rgba::rgb(1.0, 0.541, 0.400);
    pub const OK: Rgba = Rgba::rgb(0.545, 0.855, 0.545);

    pub const BUTTON: Rgba = Rgba::rgb(0.196, 0.216, 0.243);
    pub const BUTTON_HOT: Rgba = Rgba::rgb(0.278, 0.310, 0.353);
    pub const BUTTON_ON: Rgba = Rgba::rgb(0.145, 0.400, 0.549);
    pub const FIELD: Rgba = Rgba::rgb(0.086, 0.098, 0.118);
}

pub mod font {
    /// The panel body size.
    pub const BODY: f32 = 12.0;
    /// Section headings.
    pub const HEADING: f32 = 11.0;
    /// The dominant HUD numbers.
    pub const BIG: f32 = 20.0;
    pub const SMALL: f32 = 10.0;
    pub const LINE_STEP: f32 = 15.0;
}

pub mod layout {
    pub const SIDEBAR_WIDTH: f32 = 340.0;
    pub const MARGIN: f32 = 12.0;
    pub const GAP: f32 = 8.0;
    pub const ARENA_WIDTH: f32 = 960.0;
    pub const ARENA_HEIGHT: f32 = 600.0;
    pub const BUTTON_HEIGHT: f32 = 26.0;
}
