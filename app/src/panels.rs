//! The sidebar's panels and the Arena's banner.
//!
//! Every panel is drawn in window pixels at 1:1 (ADR 0006): the Arena scales
//! with the window, the sidebar never does, so HUD text stays crisp and legible
//! at any window size. Nothing here measures text — the renderer shapes it and
//! resolves alignment — so panels size boxes from the monospace advance and let
//! the Painter place the glyphs.
//!
//! The chrome is a language rather than decoration: an accent tick marks a
//! section's title, a hairline fading to nothing separates one reading from the
//! next, and brackets in the corners say "instrument" without words. Cyan stays
//! perception and positive, amber stays energy and warning, white stays numbers;
//! chrome never names a measurement, so none of it can be read as telemetry.

use sim::config::gate::STAGNATION_LIMIT;
use sim::config::nn;
use sim::{Competence, CompetenceGate, GenerationStats, Genome, Network, NodeType};

use crate::cohort::{Cohort, Member};
use crate::painter::{Align, Painter, Rgba, Typeface};
use crate::theme::{self, color, font, layout, motion};
use crate::ui::{Controls, Hit, Layout, Rect, PANEL_PAD, TITLE_GAP, TITLE_H};

/// A character's width in the monospace face the text renderer shapes with.
const CHAR_ADVANCE: f32 = 0.6;

/// The white a Network node climbs toward as it fires.
const LIT: Rgba = Rgba::rgb(1.0, 1.0, 1.0);

/// The gap between the Network panel's three columns.
const COLUMN_GAP: f32 = 10.0;

/// How strong a node's own activation has to be before it carries light as well
/// as colour. Magnitude of the node's last activation, which is a different
/// reading from the bands' `activation × weight` and is labelled as such.
const SIGNAL_LIT: f32 = 0.35;

/// The width every signal band is drawn at, whatever the term it carries. The
/// band's magnitude rides opacity and nothing else: a hairline's apparent weight
/// is quantised by rasterisation into a handful of steps, while opacity has the
/// whole range, so width here belongs to legibility rather than to measurement.
///
/// This is the BertViz discipline for attention (Vig, 2019 — constant-width
/// links, opacity carrying the weight). The TensorFlow Playground instead rides
/// width; the two cannot be mixed without the reader decoding which channel
/// they are in, so this panel picks one and says so here.
const SIGNAL_WIDTH: f32 = 1.7;

/// The opacity at a band's source end, as a fraction of its opacity at the
/// target end. The ramp is one shape for every band — it says which way the term
/// runs, not how strong it is — so the magnitude still rides opacity alone.
const SIGNAL_TAIL: f32 = 0.18;

/// The opacity a band's target end takes at magnitude zero and at magnitude one,
/// `|activation × weight|` clamped. A floor rather than zero, so a connection
/// that exists is drawn faintly and "no signal" cannot be read as "no wire"; a
/// ceiling below one, so even the strongest term leaves the ink beside it
/// legible.
const SIGNAL_FLOOR: f32 = 0.10;
const SIGNAL_CEIL: f32 = 0.85;

/// The wire under every band: [`color::NODE_EDGE`] at a fixed ink, so the
/// graph's own shape is readable whoever is firing and the wire is never a
/// channel the magnitude rides.
const WIRE_ALPHA: f32 = 0.55;

/// How many quads the cubic between two nodes is sampled into for a band. Eight
/// follows the curve to well inside a pixel at the widths the columns span.
const SIGNAL_SEGMENTS: usize = 8;

/// The lane under the Network's columns: the column captions, then the legend's
/// two lines of fine print, measured from the body's bottom edge. The panel's
/// lane and the legend's own offsets are derived from each other, so the two
/// cannot overlap at any height the panel draws at.
const CAPTION_LANE: f32 = 35.0;

/// The air the columns keep above that lane, so the last dot of a packed column
/// cannot touch the captions printed under it.
const COLUMN_AIR: f32 = 3.0;

/// The pitch between the legend's two lines of fine print, the air it keeps
/// above the panel's bottom edge, and the air the column captions keep above it.
const LEGEND_STEP: f32 = 10.0;
const LEGEND_AIR: f32 = 3.0;
const CAPTION_GAP: f32 = 4.0;

/// The key chip a control wears when it answers to a key: the fine print in a
/// hairline box, with its padding and the inset that keeps it off the control's
/// own corner.
const KEY_CHIP_PAD: f32 = 2.0;
const KEY_CHIP_INSET: f32 = 3.0;

/// Where each input sits in the Arena's own colour language: the nine Sensor
/// Rays are perception, the threat telemetry is energy, and the Agent's own
/// internal channels — bias, guns, memory — are neither.
const INPUT_ROLES: [InputRole; 21] = {
    use InputRole::{Internal, Sense, Threat};
    [
        Sense, Sense, Sense, Sense, Sense, Sense, Sense, Sense, Sense, Threat, Threat, Internal,
        Threat, Threat, Threat, Internal, Threat, Threat, Threat, Threat, Internal,
    ]
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputRole {
    Sense,
    Threat,
    Internal,
}

impl InputRole {
    fn ink(self) -> Rgba {
        match self {
            InputRole::Sense => color::ACCENT,
            InputRole::Threat => color::ENERGY,
            InputRole::Internal => color::NODE_INPUT,
        }
    }
}

/// The pitch each column draws at when the box gives it the height, and never
/// above: an input's dot and its name need 11 px, a hidden node's dot 14 — the
/// narrowest either reads as a distinct node at — and an output's row is a name,
/// a bar, and the air that separates it from the next, 32.
///
/// These are pitches, not caps on what is drawn: the panel shows every hidden
/// node it has the height for, and past that it says in words how many it left
/// out.
const INPUT_MAX_STEP: f32 = 11.0;
const HIDDEN_MAX_STEP: f32 = 14.0;
const OUTPUT_MAX_STEP: f32 = 32.0;

/// The tightest a hidden node's dot may be packed before it stops reading as one.
const HIDDEN_MIN_STEP: f32 = 8.0;

/// Room kept under the hidden dots for the "+N more" summary.
const HIDDEN_SUMMARY_H: f32 = 12.0;

/// The five outputs, in `Network::output_ids` order — left, right, thrust, fire,
/// memory (`config::nn::OUTPUT_IDS`).
const OUTPUT_NAMES: [&str; 5] = ["left", "right", "thrust", "fire", "memory"];

/// The section tick every panel heading wears, and how far the heading moves
/// right to clear it — within the same pad, so the panel's own geometry holds.
const TICK_W: f32 = 3.0;
const TICK_H: f32 = 10.0;
const TITLE_INSET: f32 = 8.0;

/// The corner brackets: 8-pixel arms, a hairline and a half thick, set just
/// inside the panel's border so the border stays the panel's edge.
const BRACKET_ARM: f32 = 8.0;
const BRACKET_WEIGHT: f32 = 1.5;
const BRACKET_INSET: f32 = 4.0;

/// The heading hairline: full strength at the left, nothing at the right.
const RULE_ALPHA: f32 = 0.9;

/// The HUD's headline bar, and the lane the Generation number keeps clear for it.
const HUD_BAR_W: f32 = 3.0;
const HUD_BAR_H: f32 = 28.0;
const HUD_NUMBER_INSET: f32 = 8.0;

/// The Record's area fill: the ink's alpha at the plot's crest and at its floor.
/// The ramp between the two is a function of absolute height, one for the whole
/// plot, which is what makes the fill a single sheet of light.
const AREA_TOP: f32 = 0.28;
const AREA_FLOOR: f32 = 0.02;

/// The most gridlines a plot's y axis will print, the explicit zero included. A
/// shorter plot gets fewer, never a pile of labels.
const GRID_MAX: usize = 4;

/// How many rungs the axis asks the 1-2-5 ladder to divide its peak into,
/// before the fit check steps the ladder up.
const GRID_INTERVALS: usize = 3;

/// The smallest gap two gridline labels may keep on a short plot.
const GRID_GAP: f32 = 4.0;

/// A labelled row inside a register: a line of `FINE` type and the air under it.
/// Every row the Record stacks — the settled count, the keys under the cloud,
/// the projection, the share's label — is this one height, so the registers give
/// way as whole rows rather than as text at loose coordinates.
const RECORD_ROW_H: f32 = font::FINE + 3.0;

/// The cleared-Wave share's bar: thin enough to read as a proportion rather than
/// a gauge, thick enough to survive the panel's ground.
const SHARE_H: f32 = 4.0;

/// Air between the share's groove and the label row above it.
const SHARE_GAP: f32 = 2.0;

/// Air between the run's register and the cloud above it: enough that the two
/// plots read as two instruments in one housing rather than as one tall plot.
const REGISTER_GAP: f32 = 6.0;

/// The share of the two plots' room the cloud takes when there is room to
/// choose. Just under half, because the run carries the axis the whole panel is
/// measured against and the cloud reads as a plane rather than as a letterbox.
const CLOUD_SHARE: f32 = 0.42;

/// The shortest cloud plot still worth calling a plane: a hundred points in less
/// than this stop being a cloud and start being a line of dots.
const CLOUD_MIN_H: f32 = 38.0;

/// The shortest run plot the curve is read against. Its floor is what the cloud
/// has to leave behind when the panel runs out of depth.
const RUN_MIN_H: f32 = 44.0;

/// The shortest cloud that also carries the previous Generation. Below it the
/// ghost is the first thing given up: it is context, the live cloud is the state.
const GHOST_MIN_H: f32 = 48.0;

/// A settled member's dot, and the ghost's: small enough that a hundred of them
/// stay points rather than becoming a mass, and drawn as matter in the Record's
/// own hue.
const DOT_R: f32 = 1.8;
const DOT_SEGMENTS: usize = 8;
const GHOST_R: f32 = 1.3;
const GHOST_SEGMENTS: usize = 6;

/// The ghost's opacity: the same hue as the live cloud, far enough under it that
/// the eye reads the live Generation as the subject and the previous one as the
/// ground it stands on — and high enough that it is read at all, which 0.35 was
/// not: on the panel's ground that faded the previous Generation into a
/// shimmer, and the movement between the two clouds is the whole instrument.
const GHOST_ALPHA: f32 = 0.5;

/// The novelty ramp: the opacity the least and the most novel member are drawn
/// at, over one fixed hue. The floor is where a dot still clears the plot's
/// ground; full is the brightest matter the panel writes.
const NOVELTY_FLOOR: f32 = 0.45;
const NOVELTY_TOP: f32 = 1.0;

/// The Run's series weights: the mean is the subject and carries the fill, the
/// best is the Population's envelope and is drawn thinner over it.
const MEAN_WEIGHT: f32 = 1.6;
const BEST_WEIGHT: f32 = 1.1;

/// The shortest run plot that still wears its legend: any less and the key would
/// stand over the curve rather than beside it.
const RUN_LEGEND_MIN_H: f32 = 24.0;

/// The novelty ramp's key, and the air either side of it: a swatch of the ramp
/// itself, because a key to a tint has to be the tint.
const SWATCH_W: f32 = 14.0;
const SWATCH_H: f32 = 5.0;

/// The watched member's mark: four arms at this gap and length, and the lamp
/// under it. Sized to clear a dot's own radius, so the mark reads as around the
/// member rather than as part of it.
const WATCH_GAP: f32 = 3.5;
const WATCH_ARM: f32 = 3.0;
const WATCH_GLOW: f32 = 4.0;

/// The run's x-axis row: a line of `SMALL` type under the share's bar.
const RUN_AXIS_H: f32 = font::SMALL + 3.0;

/// The least air a panel's heading leaves between its title and a readout set
/// beside it: below this the readout is dropped rather than crowded onto the
/// title.
const TITLE_READOUT_GAP: f32 = 12.0;

/// The speed slider's fill: full at the track's base, easing back to the fader,
/// so the groove reads as lit travel rather than a progress bar.
const FILL_AT_BASE: f32 = 0.55;
const FILL_AT_THUMB: f32 = 0.25;

/// The speed slider's thumb: a fader bar, not a knob, with a light of its own.
const THUMB_W: f32 = 2.0;
const THUMB_H: f32 = 10.0;
const THUMB_GLOW: f32 = 4.0;

/// Everything the HUD reads, gathered by the app shell each frame.
pub struct HudInfo<'a> {
    pub seed: u32,
    pub generation: u32,
    pub member: usize,
    pub population: usize,
    pub episode_time: f64,
    pub wave: u32,
    /// Shaped Fitness of the member being watched, live.
    pub fitness: f64,
    /// Raw Competence Gate numbers of the member being watched, live.
    pub competence: Competence,
    /// The run's banked best Competence Gate numbers.
    pub gate: &'a CompetenceGate,
    pub species: usize,
    pub speed: f64,
    pub measured_rate: f64,
    pub watching: bool,
}

/// The HUD: what is running on the left, what it is scoring on the right.
///
/// The two headline numbers are the Generation and Fitness. Fitness is shaped
/// and steers search; the Competence Gate beside it is the raw skill, never
/// folded together, so "the score went up" and "the ship got better" cannot be
/// mistaken for one another (ADR 0003).
///
/// Wave counts are shown one-based, as the retired browser HUD did: the
/// simulation counts *cleared* Waves, and nobody watching wants to hear that a
/// ship fighting its first Wave is on Wave 0.
pub fn draw_hud(painter: &mut Painter, rect: Rect, info: &HudInfo) {
    let body = frame(painter, rect, "01 / EVOLUTION");
    if body.w < 80.0 || body.h < 40.0 {
        return;
    }
    let mid = body.x + body.w * 0.5;
    painter.text([body.x, body.y], font::SMALL, color::TEXT_DIM, "GENERATION");
    painter.text(
        [mid, body.y],
        font::SMALL,
        color::TEXT_DIM,
        "SHAPED FITNESS",
    );
    // The run's headline number wears the same section bar the panel titles do,
    // at the scale of what it marks.
    painter.rect(body.x, body.y + 13.0, HUD_BAR_W, HUD_BAR_H, color::ACCENT);
    painter.text(
        [body.x + HUD_NUMBER_INSET, body.y + 13.0],
        font::HEADLINE,
        color::TEXT,
        format!("{:03}", info.generation),
    );
    painter.text(
        [mid, body.y + 13.0],
        font::HEADLINE,
        color::BEST,
        number(info.fitness),
    );
    let rows = [
        (
            "Agent / species",
            format!(
                "{:03}/{}  /  {}",
                info.member + 1,
                info.population,
                info.species
            ),
        ),
        (
            "Episode / wave",
            format!("{:.1}s  /  {:02}", info.episode_time, info.wave + 1),
        ),
        ("Measured rate", Controls::speed_label(info.measured_rate)),
    ];
    let mut y = body.y + 48.0;
    // The headline block above, the detail rows below: the spacing already said
    // so, and the hairline says it in ink.
    if y <= body.bottom() {
        painter.rect(body.x, y - 4.0, body.w, 1.0, color::PANEL_BORDER.alpha(0.5));
    }
    for (label, value) in rows {
        if y + 13.0 > body.bottom() {
            return;
        }
        painter.text([body.x, y], font::SMALL, color::TEXT_DIM, label);
        painter.text_aligned(
            [body.right(), y],
            font::BODY,
            color::TEXT,
            Align::Right,
            value,
        );
        y += 17.0;
    }
    if y + 18.0 > body.bottom() {
        return;
    }
    if y + 63.0 > body.bottom() {
        painter.text(
            [body.x, y],
            font::SMALL,
            if info.gate.tripped {
                color::WARN
            } else {
                color::OK
            },
            format!(
                "GATE / {}   {} / {}",
                if info.gate.tripped {
                    "STAGNANT"
                } else {
                    "OBSERVING"
                },
                info.gate.run_of_stagnant,
                STAGNATION_LIMIT
            ),
        );
        return;
    }
    painter.rect(body.x, y, body.w, 1.0, color::PANEL_BORDER.alpha(0.5));
    y += 5.0;
    painter.text([body.x, y], font::SMALL, color::TEXT_DIM, "COMPETENCE");
    painter.text_aligned(
        [body.right() - 72.0, y],
        font::SMALL,
        color::PANEL_TITLE,
        Align::Right,
        "LIVE",
    );
    painter.text_aligned(
        [body.right(), y],
        font::SMALL,
        color::PANEL_TITLE,
        Align::Right,
        "BEST",
    );
    y += 15.0;
    for (label, live, best) in [
        (
            "Alive seconds",
            format!("{:.1}", info.competence.alive_time),
            format!("{:.1}", info.gate.best_alive_time),
        ),
        (
            "Asteroid points",
            format!("{:.0}", info.competence.asteroids),
            format!("{:.0}", info.gate.best_asteroids),
        ),
    ] {
        if y + 12.0 > body.bottom() {
            return;
        }
        painter.text([body.x, y], font::SMALL, color::TEXT_DIM, label);
        painter.text_aligned(
            [body.right() - 72.0, y],
            font::SMALL,
            color::TEXT,
            Align::Right,
            live,
        );
        painter.text_aligned(
            [body.right(), y],
            font::SMALL,
            color::TEXT,
            Align::Right,
            best,
        );
        y += 15.0;
    }
    if y + 12.0 <= body.bottom() {
        let state = if info.gate.tripped {
            "STAGNANT"
        } else {
            "OBSERVING"
        };
        painter.text(
            [body.x, y],
            font::SMALL,
            if info.gate.tripped {
                color::WARN
            } else {
                color::OK
            },
            format!("GATE / {state}"),
        );
        painter.text_aligned(
            [body.right(), y],
            font::SMALL,
            color::TEXT_DIM,
            Align::Right,
            format!("{} / {}", info.gate.run_of_stagnant, STAGNATION_LIMIT),
        );
    }
}

/// The Record: the run's own history, and the Generation filling in under it.
///
/// One instrument in two registers, because the run and the Generation are two
/// different things and the panel used to average them into one number.
///
/// The **lower register is the run**: the shaped Fitness the Population breeds
/// on, as `GenerationStats` banks it — `best` and `mean` drawn together, never
/// one without the other — with the cleared-Wave share as a bar across the
/// plot's own width. The upper register is **this Generation**: every member as
/// a point in (coverage, alive time), tinted by its own novelty, with the
/// previous Generation kept behind it as a ghost the live cloud is moving away
/// from.
///
/// Fitness is a breeding score, not a skill reading (ADR 0003): the shaping in
/// it is the search's, not the operator's, so it rising says nothing about
/// whether the ships fly better. The title says "breeding score" for exactly
/// that reason, and Competence itself — the share of the Population that
/// cleared the first Wave — is what the bar under the plot reads.
///
/// The cloud's slots are `None` until an Episode lands: `Cohort::settled()` of
/// `Cohort::size()` have landed at any moment, the heading prints that pair, and
/// only settled members are drawn. Nothing here interpolates, defaults or fakes
/// an unsettled member — the cloud filling in as the pool reports is the point —
/// and nothing steps, samples or mutates the World.
///
/// The x axis is the run's own history, never a fixed window: at most one
/// sample per pixel column, the newest Generation always drawn and the axis
/// peak taken from a sample that is drawn, so decimation can never skip the
/// crest. The y axis starts at an explicit zero and steps in 1-2-5 numbers, so
/// two Generations and five hundred read the same way and every gridline is a
/// round value.
///
/// The panel gives way from the top down. The cloud is the register that needs a
/// plane, so it is the first thing given up: below [`GHOST_MIN_H`] of cloud it
/// loses the previous Generation, below [`CLOUD_MIN_H`] it is dropped whole
/// rather than squeezed — half a plane is not a reading — and the rows it was
/// using go to the run. The run's curve is never the one named away: the panel's
/// floor is where its axis still reads. Nothing the panel prints is ever
/// measured outside it, because `Painter` clips triangles but not text.
pub fn draw_record(
    painter: &mut Painter,
    rect: Rect,
    history: &[GenerationStats],
    cohort: &Cohort,
) {
    let title = "02 / FITNESS · THE BREEDING SCORE";
    let body = frame(painter, rect, title);
    if body.w < 40.0 || body.h < 20.0 {
        return;
    }
    // The newest mean, as the panel's own readout, beside the title: printed in
    // the curve's own ink over the curve, it was a number nobody could read.
    if let Some(latest) = history.last() {
        let heading = title_row(rect);
        let readout = format!("mean {}", number(latest.mean));
        let spare = heading.w
            - TITLE_INSET
            - advance(title.chars().count(), font::HEADING)
            - advance(readout.chars().count(), font::BODY);
        if spare >= TITLE_READOUT_GAP {
            painter.text_aligned(
                [heading.right(), heading.y + (TITLE_H - font::BODY) * 0.5],
                font::BODY,
                color::TEXT,
                Align::Right,
                readout,
            );
        }
    }
    let registers = Registers::of(body);
    if let Some(cloud) = registers.cloud {
        draw_cloud(painter, body, cloud, registers.ghost, cohort);
    }
    draw_run(painter, body, &registers, history);
}

/// Where the Record's two registers sit inside a panel body: the Generation's
/// cloud above, the run's own curve below.
///
/// The panel gives way from the top down. The cloud needs a plane — a hundred
/// points read as a cloud or they read as nothing — so it is the first thing
/// given up: at [`GHOST_MIN_H`] of plot it loses the previous Generation, and
/// when the room left for it falls under [`CLOUD_MIN_H`] it is dropped whole
/// and the run takes its rows. The run's curve is never the one named away.
struct Registers {
    /// The cloud's plot, when the panel is deep enough for one.
    cloud: Option<Rect>,
    /// Whether the ghost stands behind the cloud: the first thing given up.
    ghost: bool,
    /// The run's plot: the shaped Fitness series, best over mean.
    run: Rect,
    /// The share bar's lane, spanning the run plot's width.
    share: Rect,
}

impl Registers {
    /// The layout a panel body of `body` supports.
    fn of(body: Rect) -> Registers {
        // The same gutter both registers measure their y labels in, so the two
        // plots stand over one another down the panel.
        let gutter = advance(6, font::SMALL).min(body.w * 0.25);
        let plot_x = body.x + gutter;
        let plot_w = (body.w - gutter).max(0.0);
        // What the two registers pay for whatever the panel's depth: the share's
        // label and bar and the run's x axis below, the cloud's heading, keys
        // and projection above.
        let run_fixed = RECORD_ROW_H + SHARE_GAP + SHARE_H + RUN_AXIS_H;
        let cloud_fixed = RECORD_ROW_H * 3.0 + REGISTER_GAP;
        let room = body.h - run_fixed - cloud_fixed;
        let (cloud_h, ghost) = if room >= CLOUD_MIN_H + RUN_MIN_H {
            let height = (room * CLOUD_SHARE).clamp(CLOUD_MIN_H, room - RUN_MIN_H);
            (Some(height), height >= GHOST_MIN_H)
        } else {
            (None, false)
        };
        let run_y = match cloud_h {
            Some(height) => body.y + cloud_fixed + height,
            None => body.y,
        };
        let run = Rect::new(
            plot_x,
            run_y,
            plot_w,
            (body.bottom() - run_fixed - run_y).max(0.0),
        );
        let share = Rect::new(
            plot_x,
            run.bottom() + RECORD_ROW_H + SHARE_GAP,
            plot_w,
            SHARE_H,
        );
        Registers {
            cloud: cloud_h.map(|height| Rect::new(plot_x, body.y + RECORD_ROW_H, plot_w, height)),
            ghost,
            run,
            share,
        }
    }
}

/// The upper register: this Generation, as the Episodes that have landed.
///
/// The horizontal axis is behaviour descriptor 4 — the fifth of the seven, the
/// fraction of the Arena the member visited — and the vertical axis is the
/// member's alive time, read straight off its Competence. Both are the
/// simulation's own numbers, and the plot's face says which two it is: the
/// descriptors are seven-dimensional and this is the two the Population
/// separates on.
///
/// Every dot is a real Episode: one per settled slot, at that member's own
/// (coverage, alive time), tinted by its own novelty. The ghost behind is the
/// previous Generation, drawn first and dimmer, so the eye reads the movement
/// between Generations rather than a snapshot. The median of what has landed is
/// the crosshair the cloud is read against, and the watched member — the one the
/// Arena is flying — wears a mark.
///
/// Nothing here is smoothed, fitted or interpolated: alive time is bimodal (a
/// camping mode at the Wave clock, a minority past it) and a hundred real points
/// show that better than any curve over them would.
fn draw_cloud(painter: &mut Painter, body: Rect, plot: Rect, ghost: bool, cohort: &Cohort) {
    let head = Rect::new(body.x, plot.y - RECORD_ROW_H, body.w, RECORD_ROW_H);
    let legend = Rect::new(body.x, plot.bottom(), body.w, RECORD_ROW_H);
    let caption = Rect::new(body.x, legend.bottom(), body.w, RECORD_ROW_H);

    if plot.w > 0.0 && plot.h > 0.0 {
        painter.rect(plot.x, plot.y, plot.w, plot.h, color::FIELD);
        // The same inset lip the run's plot wears: the two registers are cut
        // into one housing rather than printed on it.
        painter.rect(
            plot.x + 1.0,
            plot.y + 1.0,
            (plot.w - 2.0).max(0.0),
            1.0,
            color::VIGNETTE.alpha(0.7),
        );
        painter.rect_outline(
            plot.x,
            plot.y,
            plot.w,
            plot.h,
            color::PANEL_BORDER.alpha(0.6),
        );
    }

    // The alive-time axis: zero to `alive_ceiling()`, which is floored at half
    // the Wave clock and spans the ghost as well as the cloud, so it does not
    // jump when the previous Generation is taller than the one in front of it.
    let axis = Axis::for_peak(f64::from(cohort.alive_ceiling()), gridline_room(plot));
    let scale = Scale {
        peak: axis.peak,
        zero: plot.bottom(),
        height: plot.h,
    };
    axis_grid(painter, plot, &axis, &scale);

    // The crosshair: the middle of what has landed, over the members that have
    // landed, which is what the heading's `settled / size` is there to say.
    if let Some(median) = cohort.median() {
        let at = project(&median, plot, &scale);
        let ink = color::TEXT_FAINT.alpha(0.5);
        painter.rect(plot.x, at[1], plot.w, 1.0, ink);
        painter.rect(at[0], plot.y, 1.0, plot.h, ink);
        if at[0] + 4.0 + advance(6, font::FINE) <= plot.right()
            && at[1] - font::FINE - 2.0 >= plot.y
        {
            painter.text(
                [at[0] + 4.0, at[1] - font::FINE - 2.0],
                font::FINE,
                color::TEXT_FAINT,
                "median",
            );
        }
    }

    // The ghost first, then the live cloud over it: the previous Generation is
    // context, the one that landed is the state.
    if ghost {
        for member in cohort.ghost() {
            painter.circle(
                project(member, plot, &scale),
                GHOST_R,
                color::RECORD_GHOST.alpha(GHOST_ALPHA),
                GHOST_SEGMENTS,
            );
        }
    }
    let top = novelty_top(cohort);
    let mut watched = None;
    for (index, member) in cohort.slots().iter().enumerate() {
        let Some(member) = member else {
            continue;
        };
        let at = project(member, plot, &scale);
        painter.circle(at, DOT_R, novelty_ink(member.novelty, top), DOT_SEGMENTS);
        if index == cohort.watched() {
            watched = Some(at);
        }
    }
    if let Some(at) = watched {
        watched_mark(painter, at);
    }

    if cohort.settled() == 0 {
        let awaiting = "Awaiting first Episode";
        if advance(awaiting.chars().count(), font::SMALL) <= plot.w {
            painter.text_aligned(
                [plot.center()[0], line_y(plot.y, plot.h, font::SMALL)],
                font::SMALL,
                color::TEXT_DIM.alpha(0.7),
                Align::Center,
                awaiting,
            );
        }
    }

    // The heading: how much of the Population has reported, and the key to the
    // ghost standing behind the cloud when there is one.
    let counted = if cohort.size() == 0 {
        "THIS GENERATION · NONE RUNNING".to_string()
    } else {
        format!(
            "THIS GENERATION · {} / {} SETTLED",
            cohort.settled(),
            cohort.size()
        )
    };
    let counted_w = advance(counted.chars().count(), font::FINE);
    if counted_w <= head.w {
        painter.text([head.x, head.y], font::FINE, color::TEXT_DIM, counted);
    }
    let ghost_key = "ghost = previous Generation";
    let ghost_key_w = advance(ghost_key.chars().count(), font::FINE) + 10.0;
    if ghost && counted_w + ghost_key_w + TITLE_READOUT_GAP <= head.w {
        let right = head.right() - advance(ghost_key.chars().count(), font::FINE);
        painter.text([right, head.y], font::FINE, color::TEXT_FAINT, ghost_key);
        painter.circle(
            [right - 6.0, head.y + font::FINE * 0.5],
            2.5,
            color::RECORD_GHOST.alpha(GHOST_ALPHA),
            6,
        );
    }

    // The keys the cloud needs to be read: the ramp its dots are tinted on, and
    // the mark the watched member wears.
    let glyph_y = legend.y + (RECORD_ROW_H - SWATCH_H) * 0.5;
    let mut x = plot.x;
    let ramp_w = advance(7, font::FINE);
    if x + SWATCH_W + 4.0 + ramp_w <= plot.right() {
        painter.gradient_polygon(
            &[
                [x, glyph_y],
                [x + SWATCH_W, glyph_y],
                [x + SWATCH_W, glyph_y + SWATCH_H],
                [x, glyph_y + SWATCH_H],
            ],
            &[
                color::RECORD.alpha(NOVELTY_FLOOR),
                color::RECORD.alpha(NOVELTY_TOP),
                color::RECORD.alpha(NOVELTY_TOP),
                color::RECORD.alpha(NOVELTY_FLOOR),
            ],
        );
        painter.text(
            [x + SWATCH_W + 4.0, legend.y],
            font::FINE,
            color::TEXT_FAINT,
            "novelty",
        );
        x += SWATCH_W + 4.0 + ramp_w + 12.0;
    }
    let watched_key = "watched Ship";
    let watched_w = advance(watched_key.chars().count(), font::FINE);
    if watched.is_some() && x + 15.0 + watched_w <= plot.right() {
        reticle_arms(
            painter,
            [x + 6.5, glyph_y + SWATCH_H * 0.5],
            2.5,
            2.0,
            color::RECORD_CLEAR,
        );
        painter.text(
            [x + 15.0, legend.y],
            font::FINE,
            color::RECORD_CLEAR,
            watched_key,
        );
    }

    // The projection, named on the instrument's face: which two of the
    // simulation's own numbers these axes are.
    let projected = "x coverage (behaviour descriptor 5 of 7) · y alive (s)";
    if advance(projected.chars().count(), font::FINE) <= plot.w {
        painter.text(
            [plot.x, caption.y],
            font::FINE,
            color::TEXT_FAINT,
            projected,
        );
    }
}

/// The lower register: the run itself.
///
/// The subject is the shaped Fitness the Population breeds on, as
/// `GenerationStats` banks it: `best` and `mean` drawn together, never one
/// without the other, because a best with no Population under it is one lucky
/// Episode and a mean with no best hides the tail the search is chasing. The
/// fill under the mean is the subject's own light; the best is a thin envelope
/// over it.
///
/// The cleared-Wave share stands under the plot as a bar across it: the exact
/// fraction of the Population that cleared the first Wave, which is the
/// Competence reading that belongs on this panel. It is the newest Generation's
/// share — the same Generation its rightmost sample is.
fn draw_run(painter: &mut Painter, body: Rect, registers: &Registers, history: &[GenerationStats]) {
    let plot = registers.run;
    if plot.w > 0.0 && plot.h > 0.0 {
        painter.rect(plot.x, plot.y, plot.w, plot.h, color::FIELD);
        painter.rect(
            plot.x + 1.0,
            plot.y + 1.0,
            (plot.w - 2.0).max(0.0),
            1.0,
            color::VIGNETTE.alpha(0.7),
        );
        painter.rect_outline(
            plot.x,
            plot.y,
            plot.w,
            plot.h,
            color::PANEL_BORDER.alpha(0.6),
        );
    }

    let readings = sample(history, plot);
    // The axis is measured from the samples that are actually drawn: a peak the
    // decimation skipped would leave the plot's own crest unlabelled.
    let peak = readings.iter().fold(0.0_f64, |top, reading| {
        top.max(reading.mean.max(reading.best))
    });
    let axis = Axis::for_peak(peak, gridline_room(plot));
    let scale = Scale {
        peak: axis.peak,
        zero: plot.bottom(),
        height: plot.h,
    };
    axis_grid(painter, plot, &axis, &scale);

    if readings.is_empty() {
        // An instrument with nothing to plot still shows its scale, and says
        // what it is waiting for rather than framing an empty box.
        let awaiting = "Awaiting first Generation";
        if advance(awaiting.chars().count(), font::SMALL) <= plot.w {
            painter.text_aligned(
                [plot.center()[0], line_y(plot.y, plot.h, font::SMALL)],
                font::SMALL,
                color::TEXT_DIM.alpha(0.7),
                Align::Center,
                awaiting,
            );
        }
    } else {
        // The fill first, then the envelope, then the subject: the mean is what
        // the panel is about, so it is drawn last and over everything.
        for pair in readings.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            if let Some((polygon, stops)) = area_fill(
                left.x,
                scale.y(left.mean),
                right.x,
                scale.y(right.mean),
                &scale,
                color::RECORD,
            ) {
                painter.gradient_polygon(&polygon, &stops);
            }
        }
        for pair in readings.windows(2) {
            painter.stroke(
                [pair[0].x, scale.y(pair[0].best)],
                [pair[1].x, scale.y(pair[1].best)],
                BEST_WEIGHT,
                color::RECORD_PEAK,
            );
        }
        for pair in readings.windows(2) {
            painter.stroke(
                [pair[0].x, scale.y(pair[0].mean)],
                [pair[1].x, scale.y(pair[1].mean)],
                MEAN_WEIGHT,
                color::RECORD,
            );
        }
        if readings.len() == 1 {
            // One Generation is a point, not a line, and both series still have
            // one: they are drawn as the pair they are.
            let only = readings[0];
            painter.circle([only.x, scale.y(only.mean)], 2.5, color::RECORD, 12);
            painter.circle([only.x, scale.y(only.best)], 2.0, color::RECORD_PEAK, 10);
        }
        // The newest reading is marked where the curve ends: the number itself
        // stands beside the title, but the eye still needs the point it belongs
        // to.
        let last = readings[readings.len() - 1];
        let tip = [last.x - 1.0, scale.y(last.mean)];
        painter.circle(tip, 2.0, color::RECORD, 10);
        painter.set_gain(theme::light::LAMP);
        painter.luminous_glow(tip, 5.0, color::RECORD.alpha(0.5));
        painter.set_gain(1.0);

        // The legend, in the corner a rising run leaves empty: what the two
        // lines are, where the curve cannot reach them.
        let legend_y = plot.y + 3.0;
        if plot.h >= RUN_LEGEND_MIN_H && plot.w >= advance(22, font::FINE) {
            painter.line(
                [plot.x + 6.0, legend_y + 4.0],
                [plot.x + 18.0, legend_y + 4.0],
                color::RECORD,
            );
            painter.text([plot.x + 22.0, legend_y], font::FINE, color::RECORD, "mean");
            let best_x = plot.x + 22.0 + advance(4, font::FINE) + 12.0;
            painter.line(
                [best_x, legend_y + 4.0],
                [best_x + 12.0, legend_y + 4.0],
                color::RECORD_PEAK,
            );
            painter.text(
                [best_x + 16.0, legend_y],
                font::FINE,
                color::RECORD_PEAK,
                "best",
            );
        }
    }

    // The share: the newest Generation's cleared-Wave fraction, drawn across the
    // plot the series is measured on, so its width can be read off the same
    // scale of one as the x axis.
    if let Some(latest) = history.last() {
        let share = latest.clearing_share.clamp(0.0, 1.0) as f32;
        let bar = registers.share;
        if bar.w > 0.0 && bar.bottom() <= body.bottom() {
            painter.rect(bar.x, bar.y, bar.w, bar.h, color::PANEL_BORDER.alpha(0.5));
            if bar.w * share > 0.0 {
                painter.rect(bar.x, bar.y, bar.w * share, bar.h, color::RECORD_CLEAR);
            }
            let row_y = bar.y - SHARE_GAP - RECORD_ROW_H;
            let label = "cleared Wave 1 · share of Population";
            let readout = format!("{:.0}%", share * 100.0);
            let label_w = advance(label.chars().count(), font::FINE);
            let readout_w = advance(readout.chars().count(), font::FINE);
            if label_w <= plot.w {
                painter.text([plot.x, row_y], font::FINE, color::TEXT_FAINT, label);
            }
            if label_w + readout_w + 8.0 <= plot.w {
                painter.text_aligned(
                    [plot.right(), row_y],
                    font::FINE,
                    color::TEXT,
                    Align::Right,
                    readout,
                );
            }
        }
    }

    // Generations under the plot: the first, the newest, and what they count.
    let axis_y = registers.share.bottom() + 3.0;
    if axis_y + font::SMALL <= body.bottom() && plot.w >= advance(2, font::SMALL) {
        painter.text([plot.x, axis_y], font::SMALL, color::TEXT_DIM, "1");
        let count = history.len();
        if count > 1 {
            painter.text_aligned(
                [plot.right(), axis_y],
                font::SMALL,
                color::TEXT_DIM,
                Align::Right,
                count.to_string(),
            );
            if plot.w > 160.0 {
                painter.text_aligned(
                    [plot.center()[0], axis_y],
                    font::SMALL,
                    color::TEXT_DIM,
                    Align::Center,
                    "Generations",
                );
            }
        }
    }
}

/// The run's history, sampled to the plot: at most one Generation per pixel
/// column, and the newest always among them. Both series ride the one sampling,
/// so the best line can never be drawn from a Generation the mean line skipped.
fn sample(history: &[GenerationStats], plot: Rect) -> Vec<Reading> {
    let count = history.len();
    if count == 0 {
        return Vec::new();
    }
    let newest = count - 1;
    let stride = ((count as f32) / plot.w.max(1.0)).ceil().max(1.0) as usize;
    let x_at = |index: usize| {
        if count == 1 {
            plot.right()
        } else {
            plot.x + plot.w * (index as f32 / newest as f32)
        }
    };
    let read = |index: usize| Reading {
        x: x_at(index),
        mean: history[index].mean,
        best: history[index].best,
    };
    let mut readings: Vec<Reading> = Vec::with_capacity(count.div_ceil(stride) + 1);
    let mut index = 0;
    while index < count {
        readings.push(read(index));
        index += stride;
    }
    if !newest.is_multiple_of(stride) {
        readings.push(read(newest));
    }
    readings
}

/// Where a member lands in the cloud: coverage across the plot, alive time up
/// it. Both are clamped, so a member outside the axis' range is pinned to the
/// edge it ran past rather than drawn outside the housing.
fn project(member: &Member, plot: Rect, scale: &Scale) -> [f32; 2] {
    let across = f64::from(member.coverage).clamp(0.0, 1.0) as f32;
    [plot.x + plot.w * across, scale.y(f64::from(member.alive))]
}

/// The most novel member of the Generation: the ramp's own crest.
///
/// Novelty is a mean distance into a frozen archive whose scale is the
/// archive's business rather than this panel's, so the tint is read against the
/// Generation's own range — which is what the cloud's heading calls it. Never
/// zero, so a Generation that has not run cannot divide by nothing.
fn novelty_top(cohort: &Cohort) -> f32 {
    cohort
        .slots()
        .iter()
        .flatten()
        .fold(0.0_f32, |top, member| top.max(member.novelty))
        .max(1e-6)
}

/// The ink a member's novelty earns it: the Record hue held fixed, with only its
/// opacity rising from [`NOVELTY_FLOOR`] to full as the member climbs toward the
/// Generation's most novel.
///
/// Alpha rather than a ramp through the hues, because a second hue in the cloud
/// would read as a second measurement, and the floor keeps the least novel
/// member legible against the plot's ground rather than fading it out.
fn novelty_ink(novelty: f32, top: f32) -> Rgba {
    let climbed = (novelty / top.max(1e-6)).clamp(0.0, 1.0);
    color::RECORD.alpha(NOVELTY_FLOOR + (NOVELTY_TOP - NOVELTY_FLOOR) * climbed)
}

/// Four arms around a point: the mark the watched member wears, and the same
/// mark again at legend size. A shape rather than a tint, because the one member
/// the Arena is flying has to be identifiable without separating two hues.
fn reticle_arms(painter: &mut Painter, at: [f32; 2], gap: f32, arm: f32, ink: Rgba) {
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        painter.stroke(
            [at[0] + dx * gap, at[1] + dy * gap],
            [at[0] + dx * (gap + arm), at[1] + dy * (gap + arm)],
            1.0,
            ink,
        );
    }
}

/// The watched member as the cloud draws it: the reticle, and one small lamp
/// under it. Panel lamps go through the bloom chain, so this one is deliberately
/// small and dim (`theme::light::LAMP`, ADR 0011).
fn watched_mark(painter: &mut Painter, at: [f32; 2]) {
    reticle_arms(painter, at, WATCH_GAP, WATCH_ARM, color::RECORD_CLEAR);
    painter.set_gain(theme::light::LAMP);
    painter.luminous_glow(at, WATCH_GLOW, color::RECORD_CLEAR.alpha(0.35));
    painter.set_gain(1.0);
}

/// The evaluated Ship's Network, live: inputs, hidden nodes, and the five
/// outputs with the activation each is producing right now.
///
/// Every frame redraws from the `Network` the shell hands over, so what is on
/// screen is the network that is flying, never a stale one.
///
/// The bands are the two largest terms of each node's pre-activation sum:
/// `w·a` over one enabled connection, one hop from an input or a hidden node
/// into the node that sums it. That is all they are — no confidence, no
/// attribution, no causal claim, and not the post-`tanh` activation the output
/// bar beside them shows. The legend prints the same sentence the drawing
/// obeys, and [`signal_legend`] is written from the counts the drawing loop
/// used, so the panel cannot claim a "strongest" it did not draw.
///
/// The instrument is drawn at the height its three columns want and centred in
/// whatever box it is given, rather than stretched to fill it: `ui::stack` hands
/// the Network every spare pixel of a tall window, and three columns of dots at
/// 1,500 pixels of pitch are the same handful of readings held a metre apart.
/// Height past the natural one buys more of the network itself — every hidden
/// node the Genome grew, so fewer edges have an endpoint off the panel — and
/// what is left over stays ground.
pub fn draw_network(painter: &mut Painter, rect: Rect, genome: &Genome, network: &Network) {
    let body = frame(
        painter,
        rect,
        if rect.h < 140.0 {
            "03 / NETWORK · OUTPUTS"
        } else {
            "03 / NETWORK · ONE-HOP TERMS"
        },
    );
    if body.w < 40.0 || body.h < 24.0 {
        return;
    }
    if body.h < 100.0 {
        // At the minimum window size an unlabeled miniature graph is useless.
        // Keep named outputs and actual activations visible in two columns.
        let row_h = ((body.h - 13.0) / 3.0).min(22.0);
        for (i, id) in network.output_ids().iter().enumerate() {
            let x = body.x + (i % 2) as f32 * body.w * 0.5;
            let y = body.y + (i / 2) as f32 * row_h;
            painter.text([x, y], font::MICRO, color::TEXT_DIM, OUTPUT_NAMES[i]);
            painter.text_aligned(
                [x + body.w * 0.5 - 8.0, y],
                font::MICRO,
                color::TEXT,
                Align::Right,
                format!("{:+.2}", network.activation(*id)),
            );
        }
        painter.text(
            [body.x, body.bottom() - 11.0],
            font::MICRO,
            color::TEXT_DIM,
            format!(
                "{} inputs / {} hidden / 5 outputs",
                nn::INPUTS,
                genome.hidden_count()
            ),
        );
        return;
    }
    let area = columns_box(body, genome.hidden_count());
    let columns = Columns::new(area, network, genome.hidden_count());
    let input_count = network
        .nodes()
        .iter()
        .filter(|(_, kind)| *kind == NodeType::Input)
        .count();

    // The bands: the two largest pre-activation terms per node, each a constant
    // wire with a ramp of light along it. The ranking is by |w·a| — a magnitude
    // ranking, not confidence and not attribution — and an edge the columns
    // cannot anchor at both ends is counted as dropped rather than silently
    // skipped.
    let (bands, selected) = signal_bands(network, &columns);
    painter.set_gain(theme::light::SIGNAL);
    for (source, target, contribution) in &bands {
        let ink = if *contribution >= 0.0 {
            color::ACCENT
        } else {
            color::ENERGY
        };
        signal_band(
            painter,
            *source,
            *target,
            ink,
            signal_opacity(contribution.abs().clamp(0.0, 1.0) as f32),
        );
    }
    painter.set_gain(1.0);

    const INPUT_NAMES: [&str; 21] = [
        "ray 0", "ray +40", "ray -40", "ray +80", "ray -80", "ray +120", "ray -120", "ray +160",
        "ray -160", "vel x", "vel y", "bias", "bearing", "near", "closing", "bullets", "lateral",
        "near 2", "size", "pressure", "memory",
    ];
    // Column 1: the sensor inputs as a stack of dots, firing brighter as they do.
    let input_radius = (columns.input_step * 0.5).clamp(0.0, 2.2);
    for (rank, (id, _)) in network
        .nodes()
        .iter()
        .filter(|(_, kind)| *kind == NodeType::Input)
        .enumerate()
    {
        if input_radius <= 0.0 {
            break;
        }
        let lit = network.activation(*id).abs().clamp(0.0, 1.0) as f32;
        let role = INPUT_ROLES
            .get(rank)
            .copied()
            .unwrap_or(InputRole::Internal);
        if columns.input_step >= 10.0 {
            painter.text(
                [columns.inputs.x, columns.input_dot(rank)[1] - 5.0],
                font::FINE,
                if lit > SIGNAL_LIT {
                    color::TEXT
                } else {
                    color::TEXT_FAINT
                },
                INPUT_NAMES.get(rank).copied().unwrap_or("input"),
            );
        }
        node(
            painter,
            columns.input_dot(rank),
            input_radius,
            role.ink(),
            lit,
            8,
        );
    }

    // Column 2: the hidden nodes the Genome grew, newest draws first.
    let hidden_radius = (columns.hidden_step * 0.32).clamp(2.0, 4.0);
    for (rank, (id, _)) in network
        .nodes()
        .iter()
        .filter(|(_, kind)| *kind == NodeType::Hidden)
        .enumerate()
    {
        if rank >= columns.shown_hidden {
            break;
        }
        let lit = network.activation(*id).abs().clamp(0.0, 1.0) as f32;
        node(
            painter,
            columns.hidden_dot(rank),
            hidden_radius,
            color::NODE_HIDDEN,
            lit,
            10,
        );
    }
    if let Some(summary_y) = columns.summary_y {
        painter.text_aligned(
            [columns.hidden.center()[0], summary_y],
            font::SMALL,
            color::TEXT_DIM,
            Align::Center,
            format!("+{} more", genome.hidden_count() - columns.shown_hidden),
        );
    }

    // Column 3: the five outputs, each with its name and a bar that swings both
    // ways from zero, so a shut output reads as shut rather than as small.
    let name_w = advance(6, font::BODY) + 2.0;
    for (rank, id) in network.output_ids().iter().enumerate() {
        let Some(name) = OUTPUT_NAMES.get(rank) else {
            continue;
        };
        let row = columns.output_row(rank);
        if row.h < 2.0 {
            continue;
        }
        // A squeezed panel drops the names rather than letting their line boxes
        // ride over the next output's.
        let name_size = if row.h >= 20.0 {
            font::BODY
        } else {
            font::SMALL
        };
        if row.h >= name_size * 1.35 {
            painter.text(
                [row.x, line_y(row.y, row.h, name_size)],
                name_size,
                color::TEXT,
                *name,
            );
        }
        let bar_h = (row.h * 0.3).clamp(1.0, 5.0);
        let bar = Rect::new(
            row.x + name_w,
            row.center()[1] - bar_h * 0.5,
            (row.w - name_w - 4.0).max(0.0),
            bar_h,
        );
        if bar.w <= 0.0 {
            continue;
        }
        painter.rect(bar.x, bar.y, bar.w, bar.h, color::FIELD);
        let value = network.activation(*id).clamp(-1.0, 1.0) as f32;
        let mid = bar.x + bar.w * 0.5;
        let reach = bar.w * 0.5 * value.abs();
        let fill_x = if value >= 0.0 { mid } else { mid - reach };
        painter.rect(
            fill_x,
            bar.y,
            reach,
            bar.h,
            color::NODE_OUTPUT.mix(LIT, value.abs()),
        );
        painter.line(
            [mid, bar.y - 1.0],
            [mid, bar.bottom() + 1.0],
            color::PANEL_BORDER.alpha(0.8),
        );
        painter.rect_outline(bar.x, bar.y, bar.w, bar.h, color::PANEL_BORDER.alpha(0.5));
    }

    // The lane under the columns, from the bottom up: the legend's two lines of
    // fine print, then the captions that name the three columns. Anchored to the
    // body's own bottom edge and gated on the air the columns keep above it, so
    // a squeezed panel drops a line rather than stacking one on another.
    let legend_top = body.bottom() - LEGEND_AIR - LEGEND_STEP - font::FINE;
    if legend_top >= area.bottom() + 1.0 {
        for (index, line) in signal_legend(selected, bands.len()).iter().enumerate() {
            painter.text(
                [body.x, legend_top + LEGEND_STEP * index as f32],
                font::FINE,
                color::TEXT_DIM,
                line.clone(),
            );
        }
    }
    // What each column is, under it.
    let caption_y = legend_top - font::SMALL - CAPTION_GAP;
    if caption_y >= area.bottom() + 1.0 {
        painter.text(
            [columns.inputs.x, caption_y],
            font::SMALL,
            color::TEXT_DIM,
            format!("{input_count} inputs"),
        );
        let hidden_caption = format!("{} hidden", genome.hidden_count());
        painter.text_aligned(
            [columns.hidden.center()[0], caption_y],
            font::SMALL,
            color::TEXT_DIM,
            Align::Center,
            hidden_caption,
        );
        painter.text_aligned(
            [body.right(), caption_y],
            font::SMALL,
            color::TEXT_DIM,
            Align::Right,
            format!("{} out", network.output_ids().len()),
        );
    }
}

/// One node in the Network panel: its own colour, climbing toward white as it
/// fires, and a halo once it is firing hard enough to matter. `lit` is the
/// absolute activation the Network last produced, nothing else.
fn node(painter: &mut Painter, at: [f32; 2], radius: f32, base: Rgba, lit: f32, segments: usize) {
    if radius <= 0.0 {
        return;
    }
    painter.circle(at, radius, base.mix(LIT, lit), segments);
    if lit > SIGNAL_LIT {
        painter.set_gain(theme::light::SIGNAL);
        painter.luminous_glow(at, radius * 3.2, base.alpha((lit - SIGNAL_LIT) * 0.6));
        painter.set_gain(1.0);
    }
}

/// Two incoming signals per target, ranked by |activation × weight|: the two
/// largest terms of that output's pre-activation sum. Stable tie-breaking
/// avoids flicker; work is linear apart from target lookup.
fn strongest_signals(network: &Network) -> Vec<(u32, u32, f64)> {
    let mut targets = std::collections::BTreeMap::<u32, Vec<(u32, f64)>>::new();
    for (from, to, weight) in network.enabled_connections() {
        let signals = targets.entry(to).or_default();
        signals.push((from, network.activation(from) * weight));
        signals.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()).then(a.0.cmp(&b.0)));
        signals.truncate(2);
    }
    targets
        .into_iter()
        .flat_map(|(to, values)| values.into_iter().map(move |(from, v)| (from, to, v)))
        .collect()
}

/// The bands the panel will draw, each with the two anchors it runs between,
/// and the number of terms the ranking selected before any were dropped.
///
/// The ranking is over every enabled connection, but a band needs both
/// endpoints on the panel: a hidden node past the cap has no anchor. Those terms
/// are dropped *here*, visibly, and the count comes back with the survivors so
/// the legend can say how many of the selected terms are on the panel — the old
/// panel dropped them in the drawing loop and still called its caption
/// "strongest".
/// One drawn signal band: its source and target anchors, and the `w·a` term
/// whose magnitude the band carries.
type Band = ([f32; 2], [f32; 2], f64);

fn signal_bands(network: &Network, columns: &Columns) -> (Vec<Band>, usize) {
    let selected = strongest_signals(network);
    let total = selected.len();
    let bands = selected
        .into_iter()
        .filter_map(|(from, to, contribution)| {
            Some((
                node_anchor(from, network, columns)?,
                node_anchor(to, network, columns)?,
                contribution,
            ))
        })
        .collect();
    (bands, total)
}

/// The two lines of fine print under the Network's columns.
///
/// The first line is the quantity in words: the bands are the two largest
/// `|w·a|` terms of an output's pre-activation sum, one hop in. The second is
/// either how to read the ink — hue for the sign, the ramp for the direction —
/// or, when the ranking selected more terms than the panel could anchor, exactly
/// how many it drew and why the others are missing. Written from the same counts
/// the drawing loop used, so a legend that says "strongest" while drawing fewer
/// edges than it selected cannot be written: it is the bug this line exists to
/// prevent.
fn signal_legend(selected: usize, drawn: usize) -> [String; 2] {
    let quantity =
        "2 largest |w·a| terms per node — one hop into its pre-activation sum".to_string();
    let reading = if drawn >= selected {
        "cyan positive, amber negative · the band fades toward its source".to_string()
    } else {
        format!("{drawn} of {selected} drawn · the rest start past the panel's hidden cap")
    };
    [quantity, reading]
}

/// The opacity a band's target end takes for a term of `magnitude`: the one
/// channel the magnitude rides, from [`SIGNAL_FLOOR`] at nothing to
/// [`SIGNAL_CEIL`] at a full-weight, fully-firing one.
fn signal_opacity(magnitude: f32) -> f32 {
    SIGNAL_FLOOR + (SIGNAL_CEIL - SIGNAL_FLOOR) * magnitude.clamp(0.0, 1.0)
}

/// One connection, drawn: a constant wire with a band of light laid along it.
///
/// The wire is [`color::NODE_EDGE`] at a fixed ink, so the shape of the graph is
/// readable whoever is firing. The light is the ink's own hue, ramping from
/// [`SIGNAL_TAIL`] of `head` at the source to `head` at the target, so the
/// magnitude rides opacity and nothing else — never the width, see
/// [`SIGNAL_WIDTH`] — and the ramp itself is one shape on every band, meaning
/// only "the term runs this way". It brightens as it arrives because the hot
/// end is the target's own sum being assembled: the ranking is stated per node,
/// and the light puts the term where the panel says it belongs.
///
/// The curve is the cubic `vector::connection` lays between two anchors — the
/// controls sit on the horizontal midpoint of the two, which is what keeps short
/// wires free of S-bends — sampled into quads here because a gradient has to
/// carry per-vertex ink and a lyon stroke cannot.
///
/// The caller owns the Painter's gain: the light is written at whatever gain is
/// current, so one setting covers the whole pass.
fn signal_band(painter: &mut Painter, source: [f32; 2], target: [f32; 2], ink: Rgba, head: f32) {
    if head <= 0.0 {
        return;
    }
    let mid = (source[0] + target[0]) * 0.5;
    let sample = |t: f32| -> [f32; 2] {
        let u = 1.0 - t;
        [
            u * u * u * source[0]
                + 3.0 * u * u * t * mid
                + 3.0 * u * t * t * mid
                + t * t * t * target[0],
            u * u * u * source[1]
                + 3.0 * u * u * t * source[1]
                + 3.0 * u * t * t * target[1]
                + t * t * t * target[1],
        ]
    };
    let wire = color::NODE_EDGE.alpha(WIRE_ALPHA);
    let mut previous = sample(0.0);
    let mut previous_alpha = head * SIGNAL_TAIL;
    for step in 1..=SIGNAL_SEGMENTS {
        let t = step as f32 / SIGNAL_SEGMENTS as f32;
        let current = sample(t);
        let alpha = head * (SIGNAL_TAIL + (1.0 - SIGNAL_TAIL) * t);
        let (dx, dy) = (current[0] - previous[0], current[1] - previous[1]);
        let length = dx.hypot(dy).max(f32::EPSILON);
        let half = SIGNAL_WIDTH * 0.5;
        let [nx, ny] = [-dy / length * half, dx / length * half];
        let quad = [
            [previous[0] + nx, previous[1] + ny],
            [current[0] + nx, current[1] + ny],
            [current[0] - nx, current[1] - ny],
            [previous[0] - nx, previous[1] - ny],
        ];
        painter.polygon(&quad, wire);
        painter.luminous_gradient_polygon(
            &quad,
            &[
                ink.alpha(previous_alpha),
                ink.alpha(alpha),
                ink.alpha(alpha),
                ink.alpha(previous_alpha),
            ],
        );
        previous = current;
        previous_alpha = alpha;
    }
}

/// The height the Network's drawing wants for a Genome of `hidden_count` hidden
/// nodes: the tallest of its three columns at that column's own pitch.
///
/// The panel draws at most this and centres what it draws. Height past it is
/// ground, except that it is spent first on the network itself — every hidden
/// node gets [`HIDDEN_MAX_STEP`] of column if the box can give it, because a
/// hidden node the panel does not draw is a node whose edges cannot be drawn
/// either.
fn natural_height(hidden_count: usize) -> f32 {
    (nn::INPUTS as f32 * INPUT_MAX_STEP)
        .max(hidden_count as f32 * HIDDEN_MAX_STEP)
        .max(nn::OUTPUTS as f32 * OUTPUT_MAX_STEP)
}

/// The box the Network's columns are drawn in: `body` less the caption lane and
/// its air, capped at the natural height and centred in whatever is left over.
///
/// An instrument is allowed to be smaller than its box. What it is not allowed
/// to do is stretch three dots into a ladder: at 3840 × 2160 the Network's box
/// is over 1,500 pixels tall, and the readings on it are the same thirty-odd
/// numbers whatever the pitch between them.
fn columns_box(body: Rect, hidden_count: usize) -> Rect {
    let lane = CAPTION_LANE.min(body.h * 0.35);
    let space = (body.h - lane - COLUMN_AIR).max(0.0);
    let drawn = space.min(natural_height(hidden_count));
    Rect::new(
        body.x,
        body.y + ((space - drawn) * 0.5).max(0.0),
        body.w,
        drawn,
    )
}

/// The Network panel's three columns, in window pixels.
struct Columns {
    inputs: Rect,
    hidden: Rect,
    outputs: Rect,
    input_origin: [f32; 2],
    input_step: f32,
    hidden_origin: [f32; 2],
    hidden_step: f32,
    /// The y the first output's row starts at: the rows are centred in their
    /// column when the pitch cap leaves them shorter than it.
    output_origin: f32,
    output_step: f32,
    /// Hidden nodes the panel has room to draw; the rest are summarized.
    shown_hidden: usize,
    /// Where the "+N more" line sits, when nodes were dropped.
    summary_y: Option<f32>,
}

impl Columns {
    /// Three columns in `area` — the box [`columns_box`] resolved, with the
    /// caption lane and any ground left over already taken out of it.
    fn new(area: Rect, network: &Network, hidden_count: usize) -> Columns {
        let height = area.h;
        let input_w = (area.w * 0.20).min(64.0);
        let output_w = (area.w * 0.36).clamp(60.0, 118.0).min(area.w);
        let hidden_w = (area.w - input_w - output_w - COLUMN_GAP * 2.0).max(0.0);
        let inputs = Rect::new(area.x, area.y, input_w, height);
        let hidden = Rect::new(inputs.right() + COLUMN_GAP, area.y, hidden_w, height);
        let outputs = Rect::new(hidden.right() + COLUMN_GAP, area.y, output_w, height);

        // Every column runs at a pitch of its own, never above it, and is centred
        // in the column when it comes up short: 21 dots at no more than 11 px, a
        // hidden dot at no more than 14, and a row per output at no more than 32.
        // A squeezed panel packs the dots tighter rather than spilling into the
        // caption lane; a tall one does not stretch a five-pixel output bar into
        // a five-hundred-pixel column.
        let input_step = (height / nn::INPUTS as f32).clamp(0.0, INPUT_MAX_STEP);
        let input_extent = input_step * (nn::INPUTS - 1) as f32;
        let shown_hidden = hidden_count.min((height / HIDDEN_MIN_STEP).floor().max(0.0) as usize);
        // The nodes that did not fit are said in words under the ones that did,
        // in a lane of their own so the three column captions never collide.
        let (dots_h, summary_y) = if hidden_count > shown_hidden {
            (
                (height - HIDDEN_SUMMARY_H).max(0.0),
                Some(hidden.y + height - HIDDEN_SUMMARY_H + 1.0),
            )
        } else {
            (height, None)
        };
        let hidden_step = if shown_hidden > 0 {
            (dots_h / shown_hidden as f32).min(HIDDEN_MAX_STEP)
        } else {
            0.0
        };
        let hidden_extent = hidden_step * shown_hidden.saturating_sub(1) as f32;
        let output_count = network.output_ids().len();
        let output_step = if output_count == 0 {
            0.0
        } else {
            (height / output_count as f32).min(OUTPUT_MAX_STEP)
        };
        let output_extent = output_step * output_count.saturating_sub(1) as f32;

        Columns {
            inputs,
            hidden,
            outputs,
            input_origin: [
                inputs.right() - 4.0,
                inputs.y + ((height - input_extent) * 0.5).max(0.0),
            ],
            input_step,
            hidden_origin: [
                hidden.center()[0],
                hidden.y + ((dots_h - hidden_extent) * 0.5).max(0.0),
            ],
            hidden_step,
            output_origin: outputs.y + ((height - output_extent) * 0.5).max(0.0),
            output_step,
            shown_hidden,
            summary_y,
        }
    }

    fn input_dot(&self, rank: usize) -> [f32; 2] {
        [
            self.input_origin[0],
            self.input_origin[1] + self.input_step * rank as f32,
        ]
    }

    fn hidden_dot(&self, rank: usize) -> [f32; 2] {
        [
            self.hidden_origin[0],
            self.hidden_origin[1] + self.hidden_step * rank as f32,
        ]
    }

    /// The row an output is drawn in: its name, then its bar.
    fn output_row(&self, rank: usize) -> Rect {
        Rect::new(
            self.outputs.x,
            self.output_origin + self.output_step * rank as f32,
            self.outputs.w,
            self.output_step,
        )
    }

    /// Where an edge lands on an output: the left of its row, at its middle.
    fn output_anchor(&self, rank: usize) -> [f32; 2] {
        let row = self.output_row(rank);
        [row.x, row.center()[1]]
    }
}

/// Where an edge touching `id` should land, or `None` when the panel does not
/// draw that node at all (a hidden node past the cap, or an id the network's own
/// node list does not carry).
fn node_anchor(id: u32, network: &Network, columns: &Columns) -> Option<[f32; 2]> {
    if let Some(rank) = network.output_ids().iter().position(|output| *output == id) {
        return Some(columns.output_anchor(rank));
    }
    let mut inputs = 0;
    let mut hiddens = 0;
    for (node, kind) in network.nodes() {
        if *node == id {
            return match kind {
                NodeType::Input => Some(columns.input_dot(inputs)),
                NodeType::Hidden => {
                    (hiddens < columns.shown_hidden).then(|| columns.hidden_dot(hiddens))
                }
                NodeType::Output => None,
            };
        }
        match kind {
            NodeType::Input => inputs += 1,
            NodeType::Hidden => hiddens += 1,
            NodeType::Output => {}
        }
    }
    None
}

/// The control strip: the buttons, the seed field, and the speed slider, drawn
/// exactly where `Layout` says they can be hit.
///
/// The strip teaches itself. Every control lights under the pointer, the seed
/// field and the slider included; each key that answers to a hotkey wears that
/// hotkey in a chip; and the panel's title row carries either the standing key
/// legend or — while a control is hovered — that control's own hint, which is
/// where `Evolve` gets to name the Genome it needs *before* it is pressed
/// rather than only in the status line afterwards.
pub fn draw_controls(
    painter: &mut Painter,
    layout: &Layout,
    controls: &Controls,
    hot: Option<Hit>,
) {
    frame(painter, layout.controls, "Controls");
    let lit = |hit: Hit| hot == Some(hit);

    button(
        painter,
        layout.pause,
        "space",
        if controls.paused { "Resume" } else { "Pause" },
        if controls.paused {
            State::On
        } else {
            State::Off
        },
        lit(Hit::Pause),
    );
    button(
        painter,
        layout.rays,
        "r",
        "Rays",
        if controls.rays { State::On } else { State::Off },
        lit(Hit::Rays),
    );
    // Reduced motion is a control and not only a hotkey: everything this pass
    // added moves — parallax, dust, the ribbon, the shock front, the tremor —
    // and an option nobody can find is not an option. It reads `Motion`,
    // because that is what it governs, and lights when the frame is still.
    button(
        painter,
        layout.motion,
        "m",
        "Motion",
        if controls.motion {
            State::On
        } else {
            State::Off
        },
        lit(Hit::Motion),
    );
    // Evolving acts on a loaded Genome, and says so on its own face while it has
    // none: a soft key with the one thing it waits for under its label, rather
    // than a dim key silent about why.
    button(
        painter,
        layout.evolve,
        "e",
        "Evolve",
        if controls.watching {
            State::On
        } else {
            State::Waiting("needs a Genome")
        },
        lit(Hit::Evolve),
    );
    // Restart and New seed have no hotkey; their chips would be a lie.
    button(
        painter,
        layout.restart,
        "",
        "Restart",
        State::Off,
        lit(Hit::Restart),
    );
    button(
        painter,
        layout.new_seed,
        "",
        "New seed",
        State::Off,
        lit(Hit::NewSeed),
    );
    button(
        painter,
        layout.save,
        "s",
        "Save",
        State::Off,
        lit(Hit::Save),
    );
    button(
        painter,
        layout.load,
        "l",
        "Load",
        State::Off,
        lit(Hit::Load),
    );

    // The seed field: `seed_text` is the live seed while it is not being edited,
    // and the digits being typed while it is. Being under the pointer is a state
    // of its own, and it lights the field's border rather than touching the
    // digits, which are a reading.
    let field = layout.seed_field;
    if field.w > 8.0 && field.h > 8.0 {
        let border = if controls.seed_editing {
            color::ACCENT.alpha(0.9)
        } else if lit(Hit::SeedField) {
            color::ACCENT.alpha(0.5)
        } else {
            color::PANEL_BORDER
        };
        // A field is cut into the housing rather than standing on it: flat
        // fill, a shadow along its top lip, then its border.
        painter.rect(field.x, field.y, field.w, field.h, color::FIELD);
        painter.rect(
            field.x + 1.0,
            field.y + 1.0,
            (field.w - 2.0).max(0.0),
            1.0,
            color::VIGNETTE.alpha(0.7),
        );
        painter.rect_outline(field.x, field.y, field.w, field.h, border);
        let text_x = field.x + PANEL_PAD - 2.0;
        let text_y = line_y(field.y, field.h, font::BODY);
        if controls.seed_text.is_empty() {
            painter.text([text_x, text_y], font::BODY, color::TEXT_DIM, "seed");
        } else {
            painter.text(
                [text_x, text_y],
                font::BODY,
                color::TEXT,
                controls.seed_text.clone(),
            );
        }
        if controls.seed_editing {
            // Digits only, in a monospace face: the caret is placed by advancing
            // one character width per digit, never measured.
            let caret = (text_x + advance(controls.seed_text.chars().count(), font::BODY))
                .min(field.right() - 3.0);
            painter.rect(
                caret,
                field.y + 3.0,
                1.0,
                (field.h - 6.0).max(1.0),
                color::TEXT,
            );
        }
    }

    // The speed slider: the track it can be grabbed anywhere on, the fader, and
    // what the setting reads as.
    let track = layout.speed_slider;
    if track.w > 8.0 && track.h > 4.0 {
        let bar_h = (track.h * 0.22).clamp(3.0, 5.0);
        let bar = Rect::new(track.x, track.center()[1] - bar_h * 0.5, track.w, bar_h);
        painter.rect(bar.x, bar.y, bar.w, bar.h, color::FIELD);
        let t = Controls::slider_from_speed(controls.speed).clamp(0.0, 1.0);
        let thumb_x = bar.x + bar.w * t;
        // What is set is lit, what is left is not: the fill falls off from the
        // fader back to the floor of the groove.
        if let Some((fill, stops)) = gradient_bar(
            bar.x,
            thumb_x,
            bar.y,
            bar.h,
            color::ACCENT,
            FILL_AT_BASE,
            FILL_AT_THUMB,
        ) {
            painter.gradient_polygon(&fill, &stops);
        }
        painter.rect_outline(
            bar.x,
            bar.y,
            bar.w,
            bar.h,
            if lit(Hit::SpeedSlider) {
                color::ACCENT.alpha(0.55)
            } else {
                color::PANEL_BORDER.alpha(0.8)
            },
        );
        painter.rect(
            thumb_x - THUMB_W * 0.5,
            bar.center()[1] - THUMB_H * 0.5,
            THUMB_W,
            THUMB_H,
            color::ACCENT,
        );
        // The fader's own light answers the pointer too: the track lights under
        // it, so a control that can be grabbed says so before it is.
        painter.set_gain(theme::light::LAMP);
        painter.luminous_glow(
            [thumb_x, bar.center()[1]],
            THUMB_GLOW,
            color::ACCENT.alpha(if lit(Hit::SpeedSlider) { 0.5 } else { 0.3 }),
        );
        painter.set_gain(1.0);
        let unbounded = Controls::is_unbounded(controls.speed);
        painter.text_aligned(
            [
                layout.controls.right() - PANEL_PAD,
                line_y(track.y, track.h, font::BODY),
            ],
            font::BODY,
            if unbounded {
                color::ACCENT
            } else {
                color::TEXT
            },
            Align::Right,
            Controls::speed_label(controls.speed),
        );
    }

    // The strip's own voice, in the title row beside its heading: the hovered
    // control's key and what it does, or the standing legend of the keys no
    // button carries. Both are the same line of fine print in the same lane, and
    // both are dropped rather than crowded when the panel is too narrow for
    // them.
    let row = title_row(layout.controls);
    let hint = match hot {
        Some(hit) => control_hint(hit, controls),
        None => key_legend(),
    };
    let used = TITLE_INSET + advance("Controls".chars().count(), font::HEADING);
    if used + advance(hint.chars().count(), font::FINE) + 8.0 <= row.w {
        painter.text_aligned(
            [row.right(), row.y + (TITLE_H - font::FINE) * 0.5],
            font::FINE,
            color::TEXT_DIM,
            Align::Right,
            hint,
        );
    }
}

/// The keys the strip teaches in its title row: the ones no button carries,
/// because they belong to the seed field, the slider or the window rather than
/// to one key of the strip. Each is a key the shell's `on_key` answers to, and
/// `every_key_the_legend_names_is_one_the_shell_handles` holds the two files to
/// that: a key cannot be printed here without existing there.
const KEY_LEGEND: [(&str, &str); 3] =
    [("TAB", "focus"), ("ENTER", "activate"), ("arrows", "speed")];

/// The standing key legend, as the one line of fine print the lane holds.
fn key_legend() -> String {
    KEY_LEGEND
        .iter()
        .map(|(key, what)| format!("{key} {what}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// What the strip says about the control under the pointer: the key it answers
/// to and what pressing it does.
///
/// The preconditions live here rather than only in the status line, because a
/// control that cannot act yet should say why while the pointer is on it — not
/// after the click, which is too late to be of any use.
fn control_hint(hit: Hit, controls: &Controls) -> String {
    match hit {
        Hit::Pause => {
            if controls.paused {
                "space — resume the run"
            } else {
                "space — pause the run"
            }
        }
        Hit::Rays => "r — draw the nine Sensor Rays",
        Hit::Motion => {
            if controls.motion {
                "m — hold the frame still: no trails, echoes or tremor"
            } else {
                "m — let the trails, echoes and tremor run again"
            }
        }
        Hit::Evolve => {
            if controls.watching {
                "e — evolve from the loaded Genome"
            } else {
                "e — needs a loaded Genome: load one first"
            }
        }
        Hit::Restart => "restart this run at the same seed",
        Hit::NewSeed => "restart the run at a fresh seed",
        Hit::Save => "s — save the best Genome so far",
        Hit::Load => "l — watch the next saved Genome",
        Hit::SeedField => "digits type a seed · ENTER starts it",
        Hit::SpeedSlider => "drag, or focus it and press arrows",
    }
    .to_string()
}

/// The press overlay: what a control looks like in the frame it is actuated in.
///
/// Drawn for the mouse's held button and for a keyboard activation alike, so
/// Enter and a click read the same way, and per control class, so the strip says
/// which *kind* of control is being pressed and not merely that one is:
///
/// - a key takes a lit face and the accent border its hover already uses, a step
///   stronger, because a key is fired;
/// - the seed field takes a lighter wash under the same border, because a field
///   is entered rather than fired and its digits have to stay readable;
/// - the slider takes the grab: the whole groove lights and the fader widens,
///   because a press on a slider is the start of a drag.
///
/// The shell owns which control is pressed, and draws this after the strip, so
/// the overlay sits over whatever state the control's own face is in.
pub fn draw_press(painter: &mut Painter, layout: &Layout, controls: &Controls, hit: Hit) {
    let face = layout.control_rect(hit);
    match hit {
        Hit::SeedField => {
            painter.rect(face.x, face.y, face.w, face.h, color::ACCENT.alpha(0.08));
            painter.rect_outline(face.x, face.y, face.w, face.h, color::ACCENT.alpha(0.9));
        }
        Hit::SpeedSlider => {
            let bar_h = (face.h * 0.22).clamp(3.0, 5.0);
            let bar = Rect::new(face.x, face.center()[1] - bar_h * 0.5, face.w, bar_h);
            painter.rect(bar.x, bar.y, bar.w, bar.h, color::ACCENT.alpha(0.30));
            painter.rect_outline(bar.x, bar.y, bar.w, bar.h, color::ACCENT.alpha(0.9));
            let thumb_x =
                bar.x + bar.w * Controls::slider_from_speed(controls.speed).clamp(0.0, 1.0);
            painter.rect(
                thumb_x - THUMB_W,
                bar.center()[1] - THUMB_H * 0.6,
                THUMB_W * 2.0,
                THUMB_H * 1.2,
                color::ACCENT,
            );
        }
        _ => {
            painter.rect(face.x, face.y, face.w, face.h, color::ACCENT.alpha(0.16));
            painter.rect_outline(face.x, face.y, face.w, face.h, color::ACCENT.alpha(0.9));
        }
    }
}

/// The status line under the strip: what the app is doing, or what went wrong.
pub fn draw_status(painter: &mut Painter, rect: Rect, status: &str, is_error: bool) {
    lit_surface(painter, rect, color::PANEL_BG, Some(color::PANEL_BORDER));
    painter.rect(
        rect.x,
        rect.y,
        2.0,
        rect.h.min(24.0),
        color::ACCENT.alpha(0.6),
    );
    let inner = rect.inset(PANEL_PAD);
    if inner.w <= 0.0 || inner.h < font::SMALL {
        return;
    }
    let colour = if is_error {
        color::WARN
    } else {
        color::TEXT_DIM
    };
    let step = font::SMALL + 2.0;
    for (index, line) in wrap(status, inner.w, font::SMALL, 2)
        .into_iter()
        .enumerate()
    {
        let y = inner.y + step * index as f32;
        if y + font::SMALL > inner.bottom() {
            break;
        }
        painter.text([inner.x, y], font::SMALL, colour, line);
    }
}

/// The banner's opacity `since` seconds into a life of `lifetime`: in over
/// `rise` on the entrance curve, out over `fall` on the exit one.
///
/// The shell owns the clock — it is the one place that sees wall time — and this
/// is the curve it hands the fade through, so the banner arrives on
/// [`motion::ENTRANCE_PRODUCTIVE`] and leaves on [`motion::EXIT_PRODUCTIVE`]
/// rather than on a linear ramp. Both ends are exact: a banner is gone at the
/// end of its life, not at 0.9999 of it.
pub fn banner_fade(since: f64, lifetime: f64, rise: f64, fall: f64) -> f32 {
    let enter = motion::ease(
        motion::ENTRANCE_PRODUCTIVE,
        (since / rise.max(f64::EPSILON)) as f32,
    );
    let leave = motion::ease(
        motion::EXIT_PRODUCTIVE,
        ((lifetime - since) / fall.max(f64::EPSILON)) as f32,
    );
    enter.min(leave).clamp(0.0, 1.0)
}

/// The opacity of a status message that arrived `since` seconds ago: a state
/// change the interface made about itself, so it rides
/// [`motion::STANDARD_PRODUCTIVE`] over [`motion::MODERATE_01`] and is full from
/// there on. Messages are replaced rather than dismissed, so there is no exit
/// half; the shell owns the clock that says how long one has been up.
pub fn status_fade(since: f64) -> f32 {
    motion::ease(
        motion::STANDARD_PRODUCTIVE,
        (since / f64::from(motion::MODERATE_01)) as f32,
    )
}

/// A centred banner over the Arena: Episode transitions, and anything that went
/// wrong badly enough to say so in the middle of the screen.
///
/// The shell fades the banner by handing its line over at a partial alpha — the
/// value [`banner_fade`] produced for the frame — so every mark drawn here:
/// plate, rules, glow, the rules' gradient stops, is scaled by it and the banner
/// arrives and leaves as one object rather than a box that pops and text that
/// dissolves.
pub fn draw_banner(painter: &mut Painter, arena: Rect, lines: &[(String, Rgba)]) {
    let max_w = (arena.w - 2.0 * layout::MARGIN).max(0.0);
    let max_h = (arena.h - 2.0 * layout::MARGIN).max(0.0);
    if lines.is_empty() || max_w < 40.0 || max_h < 24.0 {
        return;
    }
    let fade = lines[0].1.a.clamp(0.0, 1.0);
    if fade <= 0.0 {
        return;
    }
    // The head line is the announcement; the lines under it are the detail, and
    // stay in the monospace face the numbers live in.
    let head_size = font::BODY + 2.0;
    let mut text_w = 0.0_f32;
    let mut box_h = PANEL_PAD * 2.0;
    for (index, (text, _)) in lines.iter().enumerate() {
        let size = if index == 0 { head_size } else { font::SMALL };
        text_w = text_w.max(advance(text.chars().count(), size));
        box_h += size + if index == 0 { 6.0 } else { 3.0 };
    }
    let width = (text_w + PANEL_PAD * 4.0).min(max_w);
    let height = box_h.min(max_h);
    let x = arena.x + (arena.w - width) * 0.5;
    let y = arena.y + 63.0;
    // A dim plate the Arena still reads through, and a whisper of light behind
    // the words so they sit in the air rather than on a box.
    painter.panel(
        x,
        y,
        width,
        height,
        faded(color::PANEL_BG.alpha(0.62), fade),
        None,
    );
    painter.luminous_glow(
        [x + width * 0.5, y + height * 0.5],
        (text_w * 0.5).max(1.0),
        faded(color::TEXT.alpha(0.05), fade),
    );
    // The block is framed by light instead of by a border: a rule above and a
    // rule below, brightest at the centre and gone before either end.
    let rule_ink = faded(lines[0].1, 0.5);
    let rule_half = arena.w * 0.3;
    let center_x = arena.x + arena.w * 0.5;
    for rule_y in [y - 5.0, y + height + 4.0] {
        fading_rule_centered(
            painter,
            center_x - rule_half,
            center_x + rule_half,
            rule_y,
            rule_ink,
        );
    }
    let mut text_y = y + PANEL_PAD;
    for (index, (text, colour)) in lines.iter().enumerate() {
        let size = if index == 0 { head_size } else { font::SMALL };
        if text_y + size > y + height {
            break;
        }
        // The head line's own alpha is the fade; anything under it rides the
        // same curve.
        let colour = if index == 0 {
            *colour
        } else {
            faded(*colour, fade)
        };
        if index == 0 {
            // The Display face, centred by the renderer rather than estimated:
            // the shape of a proportional string is the shaper's to know.
            display_centered(painter, x + width * 0.5, text_y, size, colour, text);
        } else {
            painter.text_aligned(
                [x + width * 0.5, text_y],
                size,
                colour,
                Align::Center,
                text.clone(),
            );
        }
        text_y += size + if index == 0 { 6.0 } else { 3.0 };
    }
}

/// How far down a surface the key light reaches, as a fraction of its height.
const SURFACE_FALLOFF: f32 = 0.62;
/// The hairline where the key light catches a surface's top edge.
const SURFACE_EDGE_ALPHA: f32 = 0.5;

/// A lit surface: the key light grazes the top edge and falls away down the
/// face, with a hairline where it catches. Every panel, key and field is drawn
/// with this, so the sidebar reads as one piece of hardware lit from the same
/// place as the Arena rather than as a stack of flat boxes.
pub(crate) fn lit_surface(painter: &mut Painter, rect: Rect, base: Rgba, border: Option<Rgba>) {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let top = theme::lit(base);
    let knee = (rect.y + rect.h * SURFACE_FALLOFF).min(rect.bottom());
    painter.gradient_polygon(
        &[
            [rect.x, rect.y],
            [rect.right(), rect.y],
            [rect.right(), knee],
            [rect.x, knee],
        ],
        &[top, top, base, base],
    );
    painter.rect(rect.x, knee, rect.w, rect.bottom() - knee, base);
    painter.rect(
        rect.x,
        rect.y,
        rect.w,
        1.0,
        color::PANEL_EDGE.alpha(SURFACE_EDGE_ALPHA),
    );
    if let Some(border) = border {
        painter.rect_outline(rect.x, rect.y, rect.w, rect.h, border);
    }
}

/// The heading row `frame` draws into `rect`: where a panel's own readouts sit
/// when they belong beside the title rather than down in the body.
fn title_row(rect: Rect) -> Rect {
    let inner = rect.inset(PANEL_PAD);
    Rect::new(inner.x, inner.y, inner.w, TITLE_H)
}

/// The panel body: `PANEL_BG` inside a `PANEL_BORDER` outline, with a
/// `PANEL_TITLE` heading and the chrome every panel wears — a section tick, a
/// heading rule that fades out to the right, and corner brackets. Returns the
/// body's inner rect, below the heading.
fn frame(painter: &mut Painter, rect: Rect, title: &str) -> Rect {
    lit_surface(painter, rect, color::PANEL_BG, Some(color::PANEL_BORDER));
    painter.rect(
        rect.x,
        rect.y,
        2.0,
        rect.h.min(24.0),
        color::ACCENT.alpha(0.6),
    );
    // Brackets, set in from the border: the corner of an instrument's face, and
    // the only chrome that says so without a heading beside it.
    let near = [rect.x + BRACKET_INSET, rect.y + BRACKET_INSET];
    let far = [rect.right() - BRACKET_INSET, rect.bottom() - BRACKET_INSET];
    // A panel too small to hold the arms shortens them rather than crossing them.
    let arm = BRACKET_ARM
        .min(((far[0] - near[0]) * 0.5).max(0.0))
        .min(((far[1] - near[1]) * 0.5).max(0.0));
    if arm > 0.0 {
        let bracket = color::ACCENT.alpha(0.45);
        painter.stroke(near, [near[0] + arm, near[1]], BRACKET_WEIGHT, bracket);
        painter.stroke(near, [near[0], near[1] + arm], BRACKET_WEIGHT, bracket);
        painter.stroke(far, [far[0] - arm, far[1]], BRACKET_WEIGHT, bracket);
        painter.stroke(far, [far[0], far[1] - arm], BRACKET_WEIGHT, bracket);
    }
    let inner = rect.inset(PANEL_PAD);
    if inner.w <= 0.0 || inner.h <= 0.0 {
        return inner;
    }
    let mut top = inner.y;
    if inner.h >= TITLE_H + TITLE_GAP {
        let title_y = title_row(rect).y + (TITLE_H - font::HEADING) * 0.5;
        painter.rect(
            inner.x,
            title_y + (font::HEADING - TICK_H) * 0.5,
            TICK_W,
            TICK_H,
            color::ACCENT,
        );
        painter.text(
            [inner.x + TITLE_INSET, title_y],
            font::HEADING,
            color::PANEL_TITLE,
            title,
        );
        // The rule starts the gap under the heading and fades across it: the
        // heading is what the eye lands on, and the rule is where it lands from.
        top += TITLE_H;
        fading_rule(
            painter,
            inner.x,
            inner.right(),
            top,
            color::PANEL_BORDER,
            RULE_ALPHA,
            0.0,
        );
        top += TITLE_GAP;
    }
    Rect::new(inner.x, top, inner.w, (inner.bottom() - top).max(0.0))
}

/// A four-corner quad running `x0..x1` at `y`, with the ink's alpha set per
/// side: the shape a fading rule or a lit groove is made of. `None` when there
/// is no area to fill, so callers never emit a degenerate triangle.
fn gradient_bar(
    x0: f32,
    x1: f32,
    y: f32,
    thickness: f32,
    ink: Rgba,
    left: f32,
    right: f32,
) -> Option<([[f32; 2]; 4], [Rgba; 4])> {
    if x1 <= x0 || thickness <= 0.0 {
        return None;
    }
    let bottom = y + thickness;
    Some((
        [[x0, y], [x1, y], [x1, bottom], [x0, bottom]],
        [
            ink.alpha(left),
            ink.alpha(right),
            ink.alpha(right),
            ink.alpha(left),
        ],
    ))
}

/// A one-pixel hairline whose alpha runs from `left` to `right` across its
/// length: a separator that ends by fading rather than by stopping.
fn fading_rule(painter: &mut Painter, x0: f32, x1: f32, y: f32, ink: Rgba, left: f32, right: f32) {
    if let Some((points, stops)) = gradient_bar(x0, x1, y, 1.0, ink, left, right) {
        painter.gradient_polygon(&points, &stops);
    }
}

/// A hairline brightest at its middle and gone at both ends: two gradient
/// quads, so neither end stops on a line. `ink`'s alpha is the peak.
fn fading_rule_centered(painter: &mut Painter, x0: f32, x1: f32, y: f32, ink: Rgba) {
    let mid = (x0 + x1) * 0.5;
    fading_rule(painter, x0, mid, y, ink, 0.0, ink.a);
    fading_rule(painter, mid, x1, y, ink, ink.a, 0.0);
}

/// The same colour at `fade` of its opacity. A banner's fade arrives as its
/// line's alpha, and every mark it is made of has to ride that one number.
fn faded(color: Rgba, fade: f32) -> Rgba {
    color.alpha(color.a * fade.clamp(0.0, 1.0))
}

/// One sampled Generation of the run: where it stands across the plot, and the
/// two Fitness readings the Population produced there. Both ride one sampling, so
/// the best line can never drift against the mean it stands over.
#[derive(Clone, Copy)]
struct Reading {
    x: f32,
    /// `GenerationStats::mean`, in Fitness.
    mean: f64,
    /// `GenerationStats::best`, in Fitness.
    best: f64,
}

/// A plot's vertical scale: `0..peak` mapped onto the plot, an explicit zero at
/// its foot and the peak at its crest. Both of the Record's plots are measured
/// through [`Scale::y`], so a value past the peak — the Population's tail, often
/// — is pinned to the crest rather than drawn outside the housing.
struct Scale {
    /// The value at the crest: the highest reading the plot's own axis is
    /// measured against.
    peak: f64,
    /// The pixel row the zero line sits on.
    zero: f32,
    /// The plot's height, in pixels.
    height: f32,
}

impl Scale {
    /// The row a value lands on. Anything outside `0..peak` is pinned to the
    /// edge it ran past.
    fn y(&self, value: f64) -> f32 {
        let climbed = (value / self.peak).clamp(0.0, 1.0) as f32;
        self.zero - self.height * climbed
    }
}

/// A plot's y axis: the 1-2-5 ladder its gridlines stand on, and the peak the
/// scale reaches. Both of the Record's plots use it — the run's Fitness and the
/// cloud's alive time differ only in what they are a ladder over.
///
/// A 1-2-5 ladder is what makes an axis read in numbers people count in rather
/// than in fractions of the data's own span — "0.1, 0.2, 0.3" instead of "0.083,
/// 0.167, 0.25". The rule is d3-array's (ISC): the power of ten nearest the
/// span, snapped to 1, 2 or 5 by where the error falls. plotters (MIT) computes
/// the same key points for an `f64` range; either way it is a dozen lines, and
/// the difference is whether a gridline is a round value.
struct Axis {
    /// The value at the crest.
    peak: f64,
    /// The distance between gridlines.
    step: f64,
    /// Every gridline value, from the explicit zero up to the last rung at or
    /// below the peak.
    ticks: Vec<f64>,
}

impl Axis {
    /// The axis a peak of `peak` needs, printing no more than `max` gridlines:
    /// the ladder is stepped up rather than a short plot crowded.
    fn for_peak(peak: f64, max: usize) -> Axis {
        // Nothing scored yet: a unit axis, so an empty plot still has a scale.
        let peak = if peak.is_finite() && peak > 0.0 {
            peak
        } else {
            1.0
        };
        let mut step = tick_step(peak, GRID_INTERVALS);
        while rungs(peak, step) > max {
            step = next_rung(step);
        }
        let last = rungs(peak, step) - 1;
        Axis {
            peak,
            step,
            ticks: (0..=last).map(|k| k as f64 * step).collect(),
        }
    }
}

/// How many gridlines a ladder of `step` prints up to `peak`: the explicit zero
/// and every rung at or below it. The epsilon covers the rung arithmetic lands a
/// hair under, since 0.3 is not three tenths in binary.
fn rungs(peak: f64, step: f64) -> usize {
    ((peak / step + 1e-9).floor().max(0.0) as usize) + 1
}

/// The step an axis covering `span` in `intervals` counts in: the 1-2-5 rung
/// nearest `span / intervals`, by the three geometric thresholds d3-array uses
/// in its own tick increment.
fn tick_step(span: f64, intervals: usize) -> f64 {
    if span <= 0.0 || intervals == 0 {
        return 1.0;
    }
    let rough = span / intervals as f64;
    let power = 10f64.powf(rough.log10().floor());
    let error = rough / power;
    let multiplier = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    multiplier * power
}

/// The next rung up the 1-2-5 ladder: 0.2 from 0.1, 1 from 0.5, 20 from 10. The
/// decade is read back off the step, so a mantissa that lands a hair past ten —
/// logarithms are never quite exact — still climbs one rung rather than ten.
fn next_rung(step: f64) -> f64 {
    let decade = 10f64.powf(step.log10().floor());
    let mantissa = step / decade;
    if mantissa < 1.5 {
        2.0 * decade
    } else if mantissa < 3.5 {
        5.0 * decade
    } else if mantissa < 9.5 {
        10.0 * decade
    } else {
        20.0 * decade
    }
}

/// A gridline's label, written to the precision of the step it stands on: a
/// ladder of tenths reads "0.1" and a ladder of hundreds never grows a decimal
/// point. Four decimals is the most the gutter holds, so a ladder that fine is
/// labelled to it rather than written wider than its own column. Zero is a bare
/// `0`, because it is the baseline everything above it is measured from rather
/// than a reading.
fn tick_label(value: f64, step: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let decimals = ((-step.log10() - 1e-9).ceil().max(0.0) as usize).min(4);
    format!("{value:.decimals$}")
}

/// How many gridlines the plot has room to label: one per line of `font::SMALL`
/// plus a little air, between three — a baseline needs something to stand for —
/// and [`GRID_MAX`].
fn gridline_room(plot: Rect) -> usize {
    let room = (plot.h / (font::SMALL + GRID_GAP)).floor() as usize + 1;
    room.clamp(3, GRID_MAX)
}

/// The y axis as ink: a hairline per gridline with its value in the gutter, and
/// the zero drawn a step brighter than the rest because every reading on the
/// plot is measured from it.
fn axis_grid(painter: &mut Painter, plot: Rect, axis: &Axis, scale: &Scale) {
    let mut last_label = f32::INFINITY;
    for (index, tick) in axis.ticks.iter().enumerate() {
        // The zero lands on the plot's last row rather than a row below it, so
        // the baseline is drawn inside the housing like everything else.
        let line = scale
            .y(*tick)
            .clamp(plot.y, (plot.bottom() - 1.0).max(plot.y));
        let ink = if index == 0 {
            color::PANEL_EDGE.alpha(0.8)
        } else {
            color::PANEL_BORDER.alpha(0.45)
        };
        painter.rect(plot.x, line, plot.w, 1.0, ink);
        // A plot too short to space its labels keeps its lines and drops the
        // numbers that would collide: the zero is printed either way.
        let label =
            (line - font::SMALL * 0.5).clamp(plot.y, (plot.bottom() - font::SMALL).max(plot.y));
        if index == 0 || label <= last_label - (font::SMALL + 1.0) {
            painter.text_aligned(
                [plot.x - 4.0, label],
                font::SMALL,
                color::TEXT_DIM,
                Align::Right,
                tick_label(*tick, axis.step),
            );
            last_label = label;
        }
    }
}

/// The Record's fill ink at the absolute plot height `y`: one ramp across the
/// whole plot, brightest at the crest and all but gone at the floor.
///
/// The ramp takes height alone, which is what keeps the fill a single sheet of
/// light. A ramp per column instead left every column with its own span, so
/// neighbouring columns disagreed along the edge they share — the vertical
/// banding that read as texture rather than as data.
fn fill_alpha(y: f32, scale: &Scale) -> f32 {
    let climbed = ((scale.zero - y) / scale.height.max(1.0)).clamp(0.0, 1.0);
    AREA_FLOOR + (AREA_TOP - AREA_FLOOR) * climbed
}

/// One column of the run's fill: the quad from the mean curve down to the
/// baseline, and its four stops — the ramp sampled at each corner. `None` when
/// the column has no width or the curve is already lying on the floor.
fn area_fill(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    scale: &Scale,
    ink: Rgba,
) -> Option<([[f32; 2]; 4], [Rgba; 4])> {
    if x1 <= x0 || (y0 >= scale.zero && y1 >= scale.zero) {
        return None;
    }
    let stop = |y: f32| ink.alpha(fill_alpha(y, scale));
    Some((
        [[x0, y0], [x1, y1], [x1, scale.zero], [x0, scale.zero]],
        [stop(y0), stop(y1), stop(scale.zero), stop(scale.zero)],
    ))
}

#[cfg(test)]
mod record_tests {
    use super::*;
    use sim::EpisodeOutcome;

    // ---- fixtures ---------------------------------------------------------

    /// One Generation's stats, as the pool banks them. Only the fields the
    /// Record reads are given values; the Waves the lower register no longer
    /// plots stay where they sit.
    fn stats(generation: u32, best: f64, mean: f64, clearing_share: f64) -> GenerationStats {
        GenerationStats {
            generation,
            best,
            mean,
            mean_wave: 0.0,
            median_wave: 0,
            p90_wave: 0,
            clearing_share,
        }
    }

    /// A run that climbs: `count` Generations of shaped Fitness, both series
    /// rising and the cleared share climbing with them.
    fn rising(count: u32) -> Vec<GenerationStats> {
        (1..=count)
            .map(|generation| {
                let climb = f64::from(generation);
                stats(
                    generation,
                    150.0 * climb,
                    120.0 * climb,
                    (0.01 * climb).min(1.0),
                )
            })
            .collect()
    }

    /// An Episode's outcome, as the pool reports one.
    fn outcome(member: usize, alive: f64, coverage: f64, novelty: f64) -> EpisodeOutcome {
        let mut behavior = [0.0; 7];
        behavior[4] = coverage;
        EpisodeOutcome {
            member,
            fitness: 100.0,
            shaped_fitness: 100.0,
            novelty,
            behavior,
            competence: Competence {
                alive_time: alive,
                wave: 0,
                asteroids: 0.0,
            },
            steps: 1,
        }
    }

    /// A cohort mid-Generation: `settled` of `size` Episodes banked, with
    /// coverage, alive time and novelty all rising with the member index, so a
    /// test can point at a member and know where it stands.
    fn cohort_of(size: usize, settled: usize, watched: usize) -> Cohort {
        let mut cohort = Cohort::default();
        cohort.begin(1, size, watched);
        for member in 0..settled {
            let step = member as f64;
            cohort.record(
                member,
                &outcome(member, 10.0 + step, 0.1 + 0.05 * step, 0.1 + 0.05 * step),
            );
        }
        cohort
    }

    /// The Record's panel at the size the sidebar gives it at 1440 × 900.
    fn record_rect() -> Rect {
        Rect::new(1082.0, 232.0, 346.0, 240.0)
    }

    /// The body `frame` leaves inside a panel: the panel inset by the panel pad,
    /// below the title row. The tests lay the registers out with the same
    /// arithmetic the panel does, so they can point at what was drawn.
    fn body_of(rect: Rect) -> Rect {
        let inner = rect.inset(PANEL_PAD);
        Rect::new(
            inner.x,
            inner.y + TITLE_H + TITLE_GAP,
            inner.w,
            (inner.h - TITLE_H - TITLE_GAP).max(0.0),
        )
    }

    /// The cloud at the natural panel size.
    fn cloud_rect() -> Rect {
        Registers::of(body_of(record_rect()))
            .cloud
            .expect("the natural panel is deep enough for the cloud")
    }

    /// The scale the cloud is drawn through: the axis its own ceiling earns.
    fn cloud_scale(cohort: &Cohort, plot: Rect) -> Scale {
        let axis = Axis::for_peak(f64::from(cohort.alive_ceiling()), gridline_room(plot));
        Scale {
            peak: axis.peak,
            zero: plot.bottom(),
            height: plot.h,
        }
    }

    /// Is `point` inside `rect`?
    fn inside(point: [f32; 2], rect: Rect) -> bool {
        point[0] >= rect.x - 0.01
            && point[0] <= rect.right() + 0.01
            && point[1] >= rect.y - 0.01
            && point[1] <= rect.bottom() + 0.01
    }

    /// Does a stored vertex carry this ink's hue? Alpha is a measurement of its
    /// own in half this panel, so the callers that care compare it separately.
    fn same_hue(color: [f32; 4], ink: Rgba) -> bool {
        let linear = ink.to_linear();
        color[..3] == linear[..3]
    }

    /// The vertices the live cloud's dots account for. Member dots are the only
    /// ink inside the cloud plot that is the Record hue at an opacity only they
    /// take, and a dot is one fan of [`DOT_SEGMENTS`] triangles — centre and two
    /// rim points each — so the count is exactly `dots × DOT_VERTICES`.
    fn dot_vertices(painter: &Painter, plot: Rect) -> usize {
        painter
            .triangles
            .iter()
            .filter(|vertex| {
                inside(vertex.pos, plot)
                    && same_hue(vertex.color, color::RECORD)
                    && (NOVELTY_FLOOR..=NOVELTY_TOP).contains(&vertex.color[3])
            })
            .count()
    }

    /// One dot's worth of vertices: three per segment of its fan.
    const DOT_VERTICES: usize = DOT_SEGMENTS * 3;

    /// Every vertex of `ink` inside `plot` at an opacity strong enough to be a
    /// reading rather than a wash.
    fn vertices_of(painter: &Painter, plot: Rect, ink: Rgba) -> Vec<[f32; 2]> {
        painter
            .triangles
            .iter()
            .filter(|vertex| {
                inside(vertex.pos, plot) && same_hue(vertex.color, ink) && vertex.color[3] > 0.3
            })
            .map(|vertex| vertex.pos)
            .collect()
    }

    // ---- the cloud --------------------------------------------------------

    #[test]
    fn an_empty_cohort_shows_its_axes_and_says_what_it_awaits() {
        let cloud = cloud_rect();
        let mut painter = Painter::new();
        draw_record(&mut painter, record_rect(), &rising(4), &cohort_of(5, 0, 2));
        // The alive-time ladder is a real axis: an explicit zero, and rungs above
        // it for the ceiling the Cohort's own clock sets.
        let labels: Vec<String> = painter
            .text
            .iter()
            .filter(|item| item.pos[1] >= cloud.y - 0.01 && item.pos[1] <= cloud.bottom())
            .map(|item| item.content.clone())
            .collect();
        for expected in ["0", "20", "40"] {
            assert!(
                labels.iter().any(|label| label == expected),
                "the cloud's ladder is missing {expected}: {labels:?}"
            );
        }
        assert!(
            painter
                .text
                .iter()
                .any(|item| item.content == "Awaiting first Episode"),
            "the cloud says what it is waiting for"
        );
        assert_eq!(
            dot_vertices(&painter, cloud),
            0,
            "no Episode has landed, so no member is drawn"
        );
    }

    #[test]
    fn three_settled_members_draw_three_points_and_no_more() {
        let cloud = cloud_rect();
        let cohort = cohort_of(5, 3, 1);
        assert_eq!(cohort.settled(), 3);
        let mut painter = Painter::new();
        draw_record(&mut painter, record_rect(), &rising(4), &cohort);
        assert_eq!(
            dot_vertices(&painter, cloud),
            3 * DOT_VERTICES,
            "one dot per settled Episode, and nothing for a slot still waiting"
        );
        // Each settled member stands where its own two descriptors put it.
        let scale = cloud_scale(&cohort, cloud);
        for member in cohort.slots().iter().flatten() {
            let at = project(member, cloud, &scale);
            assert!(
                painter.triangles.iter().any(|vertex| {
                    (vertex.pos[0] - at[0]).abs() < 0.01
                        && (vertex.pos[1] - at[1]).abs() < 0.01
                        && same_hue(vertex.color, color::RECORD)
                }),
                "no dot at {at:?} for a member that has landed"
            );
        }
    }

    #[test]
    fn the_crosshair_stands_on_the_median_of_what_has_landed() {
        let cloud = cloud_rect();
        let cohort = cohort_of(5, 3, 1);
        let median = cohort.median().expect("three members have landed");
        let scale = cloud_scale(&cohort, cloud);
        let at = project(&median, cloud, &scale);
        let mut painter = Painter::new();
        draw_record(&mut painter, record_rect(), &rising(4), &cohort);
        let ink = color::TEXT_FAINT.alpha(0.5).to_linear();
        assert!(
            painter.triangles.iter().any(|vertex| {
                vertex.color == ink
                    && (vertex.pos[0] - cloud.x).abs() < 0.01
                    && (vertex.pos[1] - at[1]).abs() < 0.01
            }),
            "the horizontal crosshair is drawn at the median's alive time"
        );
        assert!(
            painter.triangles.iter().any(|vertex| {
                vertex.color == ink
                    && (vertex.pos[0] - at[0]).abs() < 0.01
                    && (vertex.pos[1] - cloud.y).abs() < 0.01
            }),
            "the vertical crosshair is drawn at the median's coverage"
        );
    }

    #[test]
    fn a_reading_past_the_axis_is_pinned_to_the_edge_it_ran_past() {
        // Kept from the retired band test: the scale still pins a value past the
        // crest to the crest and one under the floor to the floor, which is what
        // lets a member's own numbers be drawn inside the housing rather than
        // outside it. What the old test measured with a median-to-p90 ribbon,
        // this one measures on the projection the cloud is drawn through.
        let scale = Scale {
            peak: 1.0,
            zero: 100.0,
            height: 100.0,
        };
        assert_eq!(scale.y(0.0), 100.0, "zero stands on the baseline");
        assert_eq!(scale.y(1.0), 0.0, "the peak stands on the crest");
        assert_eq!(
            scale.y(2.0),
            0.0,
            "a reading past the crest is pinned to it"
        );
        assert_eq!(scale.y(-0.5), 100.0, "and one under the floor to the floor");

        let plot = Rect::new(10.0, 20.0, 200.0, 80.0);
        let member = Member {
            alive: 2.0,
            coverage: 1.5,
            fitness: 0.0,
            wave: 0,
            novelty: 0.0,
        };
        assert_eq!(
            project(&member, plot, &scale)[0],
            plot.right(),
            "coverage past the plane is pinned to the plane"
        );
        let member = Member {
            coverage: -0.5,
            ..member
        };
        assert_eq!(project(&member, plot, &scale)[0], plot.x);
    }

    #[test]
    fn the_alive_axis_holds_when_the_ghost_is_taller_than_the_cloud() {
        let cloud = cloud_rect();
        // A Generation that dies inside the first Wave, behind a ghost that
        // reached the Wave clock: the ceiling spans both, or the ghost is pinned
        // to a crest the live cloud never reaches.
        let mut cohort = Cohort::default();
        cohort.begin(1, 2, 0);
        cohort.record(0, &outcome(0, 60.0, 0.5, 0.2));
        cohort.record(1, &outcome(1, 62.0, 0.7, 0.3));
        cohort.begin(2, 2, 0);
        cohort.record(0, &outcome(0, 5.0, 0.2, 0.2));
        cohort.record(1, &outcome(1, 6.0, 0.8, 0.3));
        assert_eq!(cohort.ghost().len(), 2);
        assert!(
            cohort.alive_ceiling() > 62.0,
            "the ceiling spans the ghost as well as the cloud"
        );
        let mut painter = Painter::new();
        draw_record(&mut painter, record_rect(), &rising(3), &cohort);
        let scale = cloud_scale(&cohort, cloud);
        let ghost = project(&cohort.ghost()[0], cloud, &scale);
        assert!(
            ghost[1] > cloud.y + 1.0,
            "the tall ghost is not pinned to the crest: {}",
            ghost[1]
        );
        let ink = color::RECORD_GHOST.alpha(GHOST_ALPHA).to_linear();
        assert!(
            painter.triangles.iter().any(|vertex| {
                vertex.color == ink
                    && (vertex.pos[0] - ghost[0]).abs() < 0.01
                    && (vertex.pos[1] - ghost[1]).abs() < 0.01
            }),
            "the ghost is drawn where its own alive time puts it"
        );
    }

    #[test]
    fn a_dot_brightens_with_its_own_novelty_and_keeps_the_record_hue() {
        // The ramp is monotone in novelty over one fixed hue: only the opacity
        // moves, so a second hue cannot read as a second measurement.
        let top = 0.4;
        let mut previous = f32::NEG_INFINITY;
        for novelty in [0.0, 0.02, 0.05, 0.1, 0.2, 0.4, 0.9] {
            let ink = novelty_ink(novelty, top);
            assert_eq!(ink.to_array()[..3], color::RECORD.to_array()[..3]);
            assert!(ink.a >= previous, "the ramp dipped at {novelty}");
            assert!((NOVELTY_FLOOR..=NOVELTY_TOP).contains(&ink.a));
            previous = ink.a;
        }
        // A Generation that never ran has no novelty anywhere: every member sits
        // on the ramp's floor rather than dividing by nothing.
        assert_eq!(novelty_ink(0.0, 0.0).a, NOVELTY_FLOOR);

        // And in the cloud, the more novel of two members is the brighter dot:
        // the tint is the member's own number, not a constant of the cloud.
        let cloud = cloud_rect();
        let mut cohort = Cohort::default();
        cohort.begin(1, 2, 0);
        cohort.record(0, &outcome(0, 10.0, 0.2, 0.05));
        cohort.record(1, &outcome(1, 10.0, 0.6, 0.35));
        let mut painter = Painter::new();
        draw_record(&mut painter, record_rect(), &rising(3), &cohort);
        let scale = cloud_scale(&cohort, cloud);
        let alpha_at = |member: &Member| {
            let at = project(member, cloud, &scale);
            painter
                .triangles
                .iter()
                .find(|vertex| {
                    (vertex.pos[0] - at[0]).abs() < 0.01
                        && (vertex.pos[1] - at[1]).abs() < 0.01
                        && same_hue(vertex.color, color::RECORD)
                })
                .expect("a dot per settled member")
                .color[3]
        };
        let dull = cohort.slots()[0].unwrap();
        let bright = cohort.slots()[1].unwrap();
        assert!(alpha_at(&dull) < alpha_at(&bright));
    }

    // ---- the run ----------------------------------------------------------

    #[test]
    fn the_run_is_drawn_as_a_pair_or_not_at_all() {
        let rect = record_rect();
        let run = Registers::of(body_of(rect)).run;
        let drawn = |history: &[GenerationStats]| -> (usize, usize) {
            let mut painter = Painter::new();
            draw_record(&mut painter, rect, history, &Cohort::default());
            (
                vertices_of(&painter, run, color::RECORD).len(),
                vertices_of(&painter, run, color::RECORD_PEAK).len(),
            )
        };
        assert_eq!(drawn(&[]), (0, 0), "nothing scored: neither series");
        for history in [rising(1), rising(30), vec![stats(1, 200.0, 200.0, 0.0)]] {
            let (mean, best) = drawn(&history);
            assert!(mean > 0, "the mean is the subject and is always drawn");
            assert!(best > 0, "the best is never left off the population's mean");
            assert_eq!(
                best > 0,
                mean > 0,
                "the pair is drawn together or not at all"
            );
        }
        // The run's own history is the Record's ink, not the energy hue a live
        // reading wears — which is what `best` and `mean` used to alias.
        let mut painter = Painter::new();
        draw_record(&mut painter, rect, &rising(30), &Cohort::default());
        assert!(
            vertices_of(&painter, run, color::MEAN).is_empty(),
            "Fitness is not written in the energy hue"
        );
    }

    #[test]
    fn the_share_bar_is_the_exact_fraction_of_the_population_that_cleared() {
        let rect = record_rect();
        let bar = Registers::of(body_of(rect)).share;
        let ink = color::RECORD_CLEAR.to_linear();
        for share in [0.0, 0.25, 0.618, 1.0] {
            let mut painter = Painter::new();
            draw_record(
                &mut painter,
                rect,
                &[stats(1, 120.0, 100.0, share)],
                &Cohort::default(),
            );
            assert!(
                painter
                    .text
                    .iter()
                    .any(|item| item.content == format!("{:.0}%", share * 100.0)),
                "the readout states the share it is drawn at"
            );
            let filled: Vec<[f32; 2]> = painter
                .triangles
                .iter()
                .filter(|vertex| {
                    vertex.color == ink
                        && vertex.pos[1] >= bar.y - 0.01
                        && vertex.pos[1] <= bar.bottom() + 0.01
                })
                .map(|vertex| vertex.pos)
                .collect();
            if share == 0.0 {
                assert!(
                    filled.is_empty(),
                    "a Population that cleared nothing fills nothing"
                );
                continue;
            }
            let left = filled
                .iter()
                .fold(f32::INFINITY, |low, pos| low.min(pos[0]));
            let right = filled
                .iter()
                .fold(f32::NEG_INFINITY, |high, pos| high.max(pos[0]));
            assert!(
                (left - bar.x).abs() < 0.01,
                "the bar starts at the plot's own edge"
            );
            assert!(
                (right - (bar.x + bar.w * share as f32)).abs() < 0.05,
                "the bar's width is the share of the plot: {right} against {}",
                bar.x + bar.w * share as f32
            );
        }
    }

    #[test]
    fn the_newest_generation_is_drawn_and_the_peak_comes_from_one_that_is() {
        let rect = record_rect();
        let run = Registers::of(body_of(rect)).run;
        // Four hundred Generations in a plot a few hundred pixels wide: the
        // sampler has to skip, and the Generation it would skip carries the peak.
        let mut history: Vec<GenerationStats> =
            (1..=400).map(|g| stats(g, 200.0, 100.0, 0.2)).collect();
        history[7].best = 5000.0;
        let mut painter = Painter::new();
        draw_record(&mut painter, rect, &history, &cohort_of(5, 3, 1));
        assert!(
            !painter.text.iter().any(|item| item.content == "5000"),
            "the axis is measured from a sample that is drawn"
        );
        assert!(
            painter.text.iter().any(|item| item.content == "200"),
            "the drawn peak is the axis' crest"
        );
        let newest = vertices_of(&painter, run, color::RECORD)
            .into_iter()
            .fold(f32::NEG_INFINITY, |high, pos| high.max(pos[0]));
        assert!(
            (newest - run.right()).abs() < 0.05,
            "the newest Generation is always drawn: {newest} against {}",
            run.right()
        );
    }

    /// The Record's two ladders, both kept from the Chart: whatever the peak and
    /// however much room the plot has, an axis starts on an explicit zero, stays
    /// on the 1-2-5 ladder, and prints between two and four gridlines.
    ///
    /// The values below used to be mean Waves (0.34 at the top of a run); they
    /// are Fitness now, which is what the lower register plots — the ladder
    /// itself is the same rule either way, which is why this test is unchanged
    /// but for what it names.
    #[test]
    fn the_axis_steps_in_one_two_fives_and_never_crowds_the_plot() {
        // A run that has got as far as 2848 Fitness reads against a ladder of
        // thousands — 0, 1000, 2000 — not against thirds of its own span.
        assert!((tick_step(2848.0, 3) - 1000.0).abs() < 1e-12);
        assert!((tick_step(2848.0, 6) - 500.0).abs() < 1e-12);
        let axis = Axis::for_peak(2848.0, 4);
        assert!((axis.step - 1000.0).abs() < 1e-12);
        assert_eq!(axis.ticks.len(), 3, "0, 1000, 2000");

        // Whatever a run has scored, and however much room the plot has, the
        // axis starts on an explicit zero, stays on the 1-2-5 ladder, and prints
        // between two and four gridlines.
        for peak in [
            0.001, 0.01, 0.05, 0.1, 0.34, 0.5, 0.9, 1.0, 2.0, 4.2, 12.7, 55.0, 300.0, 1000.0,
        ] {
            for room in 3..=4 {
                let axis = Axis::for_peak(peak, room);
                assert!(
                    (2..=room).contains(&axis.ticks.len()),
                    "a peak of {peak} in room for {room} printed {:?}",
                    axis.ticks
                );
                assert_eq!(axis.ticks[0], 0.0, "the zero baseline is explicit");
                let decade = 10f64.powf(axis.step.log10().floor());
                let mantissa = axis.step / decade;
                assert!(
                    [1.0, 2.0, 5.0]
                        .iter()
                        .any(|rung| (mantissa - rung).abs() < 1e-9),
                    "a step of {} is not on the 1-2-5 ladder",
                    axis.step
                );
                for pair in axis.ticks.windows(2) {
                    assert!((pair[1] - pair[0] - axis.step).abs() < 1e-9);
                }
                assert!(axis.ticks.iter().all(|tick| *tick <= peak + 1e-9));
            }
        }

        // Nothing scored yet still reads against a scale rather than dividing by
        // nothing.
        let empty = Axis::for_peak(0.0, 4);
        assert_eq!(empty.peak, 1.0);
        assert_eq!(empty.ticks.len(), 3);
        // A label is written to its step's own precision, and the baseline is
        // not written as a reading.
        assert_eq!(tick_label(0.0, 0.1), "0");
        assert_eq!(tick_label(0.30000000000000004, 0.1), "0.3");
        assert_eq!(tick_label(0.05, 0.05), "0.05");
        assert_eq!(tick_label(200.0, 100.0), "200");
    }

    /// The fill's own geometry, kept from the Chart: one ramp across the whole
    /// plot rather than one per column, which is what stops neighbouring columns
    /// disagreeing along the edge they share. The ink is the Record's now
    /// instead of the perception cyan the old mean was drawn in; the geometry
    /// the test pins did not change with it.
    #[test]
    fn the_fill_is_one_sheet_of_light_rather_than_a_column_of_it() {
        let scale = Scale {
            peak: 1.0,
            zero: 100.0,
            height: 100.0,
        };
        // Neighbouring columns agree along the edge they share, whatever their
        // curves: the stops are a function of height, not of the column's own
        // span, and that is what removes the vertical banding.
        let (_, low) = area_fill(0.0, 60.0, 5.0, 40.0, &scale, color::RECORD).unwrap();
        let (_, high) = area_fill(5.0, 40.0, 10.0, 15.0, &scale, color::RECORD).unwrap();
        assert_eq!(low[1].a, high[0].a, "one ramp, so no step at a shared edge");
        // The ramp climbs the plot: all but gone at the floor, AREA_TOP at the
        // crest, brighter at every step between.
        assert!((fill_alpha(100.0, &scale) - AREA_FLOOR).abs() < 1e-6);
        assert!((fill_alpha(0.0, &scale) - AREA_TOP).abs() < 1e-6);
        let ramp: Vec<f32> = [100.0, 75.0, 50.0, 25.0, 0.0]
            .iter()
            .map(|y| fill_alpha(*y, &scale))
            .collect();
        for pair in ramp.windows(2) {
            assert!(pair[1] > pair[0], "the ramp dipped: {ramp:?}");
        }
        // The column still runs from the curve down to the baseline, and one
        // lying on the floor still has no area under it.
        let (points, _) = area_fill(0.0, 20.0, 5.0, 10.0, &scale, color::RECORD).unwrap();
        assert_eq!(
            points,
            [[0.0, 20.0], [5.0, 10.0], [5.0, 100.0], [0.0, 100.0]],
            "each column runs from the curve down to the baseline"
        );
        assert!(area_fill(0.0, 100.0, 5.0, 100.0, &scale, color::RECORD).is_none());
        assert!(area_fill(0.0, 101.0, 5.0, 100.0, &scale, color::RECORD).is_none());
        assert!(area_fill(5.0, 20.0, 5.0, 10.0, &scale, color::RECORD).is_none());
    }

    /// Kept from the Chart's empty state, moved onto the register it still
    /// belongs to: with no Generation banked the run shows its ladder and says
    /// what it awaits, and its zero is drawn rather than merely written.
    #[test]
    fn an_empty_run_still_shows_its_scale_and_says_what_it_awaits() {
        let rect = record_rect();
        let run = Registers::of(body_of(rect)).run;
        let mut painter = Painter::new();
        draw_record(&mut painter, rect, &[], &cohort_of(100, 0, 0));
        assert!(
            painter
                .text
                .iter()
                .any(|item| item.content == "Awaiting first Generation"),
            "an empty run says so"
        );
        // Behind the message is a real axis: a labelled ladder running up the
        // plot, lowest value at the bottom, every label inside the panel. The
        // labels are the run's own — the cloud above has a ladder too, and it is
        // a different scale.
        let label = |content: &str| {
            painter
                .text
                .iter()
                .find(|item| {
                    item.content == content
                        && item.pos[1] >= run.y - 0.01
                        && item.pos[1] <= run.bottom()
                })
                .unwrap_or_else(|| panic!("no {content} gridline on an empty run"))
                .clone()
        };
        // Nothing has scored, so the ladder is the unit axis: 0, 0.5, 1.
        let (zero, middle, top) = (label("0"), label("0.5"), label("1.0"));
        assert!(zero.pos[1] > middle.pos[1] && middle.pos[1] > top.pos[1]);
        for item in [&zero, &middle, &top] {
            assert!(item.pos[0] < rect.right() && item.pos[1] > rect.y);
        }
        // And the zero is drawn, not merely written: a hairline spans the plot
        // just under its label.
        let mut span = 0.0_f32;
        for row in 0..=font::SMALL as usize {
            let row_y = zero.pos[1] + row as f32;
            let mut left = f32::INFINITY;
            let mut right = f32::NEG_INFINITY;
            for vertex in painter.triangles.iter().filter(|v| v.pos[1] == row_y) {
                left = left.min(vertex.pos[0]);
                right = right.max(vertex.pos[0]);
            }
            span = span.max(right - left);
        }
        assert!(span > 100.0, "the zero baseline spans the plot");
        assert!(run.h > 0.0, "and the run's own plot is the one carrying it");
    }

    /// Kept from the Chart, re-pointed at what the readout now states: the
    /// newest mean Fitness rather than the mean Waves, printed in the title row
    /// where the curve cannot reach it.
    #[test]
    fn the_newest_mean_fitness_reads_beside_the_title_clear_of_the_curve() {
        // The mean is rising and the corner it ends in is the top of the plot,
        // which is exactly where the reading used to be printed over the curve.
        let history = rising(30);
        let newest = history.last().unwrap().mean;
        let mut painter = Painter::new();
        draw_record(&mut painter, record_rect(), &history, &Cohort::default());
        let readout = format!("mean {}", number(newest));
        let printed: Vec<&crate::painter::TextItem> = painter
            .text
            .iter()
            .filter(|item| item.content == readout)
            .collect();
        assert_eq!(
            printed.len(),
            1,
            "the newest mean is printed once: {readout}"
        );
        let printed = printed[0];
        assert_eq!(printed.color, color::TEXT, "a reading, not more curve ink");
        assert!(
            printed.pos[1] <= record_rect().y + PANEL_PAD + TITLE_H,
            "the reading stands in the title row, clear of the plot"
        );
        // The plot still carries the mean in the Record's own ink.
        assert!(painter
            .triangles
            .iter()
            .any(|vertex| vertex.color == color::RECORD.to_linear()));
    }

    // ---- degradation ------------------------------------------------------

    #[test]
    fn the_panel_gives_the_cloud_way_before_the_run() {
        let natural = Registers::of(body_of(record_rect()));
        assert!(
            natural.cloud.is_some(),
            "the natural panel carries both registers"
        );
        assert!(natural.ghost, "and the ghost, with room to spare");
        // Part-way down, the cloud keeps its plane but loses the previous
        // Generation: context goes before state.
        let squashed = Registers::of(body_of(Rect::new(0.0, 0.0, 346.0, 203.0)));
        assert!(
            squashed.cloud.is_some(),
            "a shorter panel still has a plane"
        );
        assert!(!squashed.ghost, "the ghost is the first thing given up");
        // At the panel's floor the cloud is gone whole, and the run keeps the
        // axis the whole panel is measured against.
        let rect = Rect::new(0.0, 0.0, 346.0, 118.0);
        let squeezed = Registers::of(body_of(rect));
        assert!(
            squeezed.cloud.is_none(),
            "below its floor the cloud is dropped whole"
        );
        assert!(
            squeezed.run.h >= 40.0,
            "the run keeps a plot: {}",
            squeezed.run.h
        );
        let mut painter = Painter::new();
        draw_record(&mut painter, rect, &rising(30), &cohort_of(5, 3, 1));
        assert!(
            !vertices_of(&painter, squeezed.run, color::RECORD).is_empty(),
            "the run is drawn"
        );
        // And the register it gave way to is not smuggled in above it: no
        // heading, no keys, no projection.
        assert!(
            painter.text.iter().all(|item| {
                !item.content.starts_with("THIS GENERATION")
                    && item.content != "Awaiting first Episode"
                    && !item.content.starts_with("x coverage")
            }),
            "the cloud's own rows are gone with it"
        );
    }

    #[test]
    fn nothing_the_record_prints_leaves_the_panel() {
        for rect in [
            record_rect(),
            Rect::new(0.0, 0.0, 346.0, 118.0),
            Rect::new(12.0, 40.0, 346.0, 200.0),
            Rect::new(0.0, 0.0, 300.0, 150.0),
        ] {
            // Both states of the instrument: a run under way, and one with
            // nothing to plot yet — the awaiting lines are printed text too.
            for (history, cohort) in [
                (rising(40), cohort_of(5, 3, 1)),
                (Vec::new(), Cohort::default()),
            ] {
                let mut painter = Painter::new();
                draw_record(&mut painter, rect, &history, &cohort);
                // `Painter` clips triangles but not text, so every string is
                // measured against the panel it was printed in.
                let printed: Vec<(String, f32, f32, f32, f32)> = painter
                    .text
                    .iter()
                    .map(|item| {
                        let width = advance(item.content.chars().count(), item.size);
                        let left = match item.align {
                            Align::Left => item.pos[0],
                            Align::Right => item.pos[0] - width,
                            Align::Center => item.pos[0] - width * 0.5,
                        };
                        (item.content.clone(), left, width, item.pos[1], item.size)
                    })
                    .collect();
                for (content, left, width, top, size) in printed {
                    assert!(
                        left >= rect.x - 0.5 && left + width <= rect.right() + 0.5,
                        "{content:?} leaves {rect:?} sideways"
                    );
                    assert!(
                        top >= rect.y - 0.5 && top + size <= rect.bottom() + 0.5,
                        "{content:?} leaves {rect:?} vertically"
                    );
                }
                // The matter is bounded too: a stroke straddles its path, so it
                // may reach half a weight past an edge and no further.
                for vertex in &painter.triangles {
                    let [x, y] = vertex.pos;
                    assert!(
                        x >= rect.x - 1.0
                            && x <= rect.right() + 1.0
                            && y >= rect.y - 1.0
                            && y <= rect.bottom() + 1.0,
                        "ink escapes {rect:?} at {:?}",
                        vertex.pos
                    );
                }
            }
        }
    }
}
/// A centred line in the Display face. `Painter` aligns monospace runs itself,
/// so the face is flipped on the item it just queued — the same thing
/// `display_text` does — and a proportional string is never measured by hand.
fn display_centered(
    painter: &mut Painter,
    center_x: f32,
    y: f32,
    size: f32,
    color: Rgba,
    text: &str,
) {
    painter.text_aligned([center_x, y], size, color, Align::Center, text);
    if let Some(item) = painter.text.last_mut() {
        item.face = Typeface::Display;
    }
}

/// How a button reads: plain, toggled on, or waiting on something it can name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Off,
    On,
    /// Not available yet — a soft face and a quiet border — and saying under its
    /// label what it is waiting for. `Evolve` is the one control that needs
    /// this: it acts on a loaded Genome, and without one it has nothing to do.
    /// A control drawn soft and silent about why reads as broken; one that names
    /// its precondition reads as waiting.
    Waiting(&'static str),
}

/// One button: a key that lights under the pointer, fills in when it is toggled
/// on, and says so with an underline as well as a tint.
///
/// `key` is what the button answers to and is chipped into its trailing corner;
/// a button the shell gives no hotkey passes `""` and is drawn without a chip,
/// because a key that has to be guessed at is worse than one that is not shown.
fn button(painter: &mut Painter, rect: Rect, key: &str, label: &str, state: State, hot: bool) {
    if rect.w < 1.0 || rect.h < 1.0 {
        return;
    }
    let waiting = matches!(state, State::Waiting(_));
    let fill = match (hot, state) {
        (true, _) => color::BUTTON_HOT,
        (false, State::On) => color::BUTTON_ON,
        (false, _) => color::BUTTON,
    };
    let text = if hot || !waiting {
        color::TEXT
    } else {
        // Waiting, but still readable: it says what the strip can do and what it
        // is waiting for.
        color::TEXT_DIM
    };
    let border = if hot {
        color::ACCENT.alpha(0.55)
    } else if waiting {
        color::PANEL_BORDER.alpha(0.45)
    } else {
        color::PANEL_BORDER.alpha(0.85)
    };
    lit_surface(painter, rect, fill, Some(border));
    if state == State::On {
        // A lit key says so twice: colour, and a bar along its foot, so the
        // state survives without the colour being read.
        painter.rect(
            rect.x + 3.0,
            rect.bottom() - 4.0,
            (rect.w - 6.0).max(0.0),
            2.0,
            color::ACCENT,
        );
    }
    // A waiting key prints what it waits for under its label when the row is
    // tall enough for two lines *and* the cell is wide enough for the words, and
    // is otherwise left to the hint lane. The width matters: the strip's first
    // row is four keys across, and a subtitle that overruns its cell would run
    // into its neighbour — nothing clips text, so the fit is checked here.
    let reason = match state {
        State::Waiting(reason)
            if rect.h >= font::BODY + font::FINE + 2.0
                && advance(reason.chars().count(), font::FINE) <= rect.w - 8.0 =>
        {
            Some(reason)
        }
        _ => None,
    };
    match reason {
        Some(reason) => {
            painter.text_aligned(
                [rect.center()[0], rect.y + 3.0],
                font::BODY,
                text,
                Align::Center,
                label,
            );
            painter.text_aligned(
                [rect.center()[0], rect.y + 3.0 + font::BODY],
                font::FINE,
                color::TEXT_DIM,
                Align::Center,
                reason,
            );
        }
        None => painter.text_aligned(
            [rect.center()[0], line_y(rect.y, rect.h, font::BODY)],
            font::BODY,
            text,
            Align::Center,
            label,
        ),
    }
    key_chip(
        painter,
        rect,
        key,
        if hot { color::TEXT } else { color::TEXT_DIM },
    );
}

/// A control's hotkey, printed in the chip language the strip already speaks:
/// the fine print in a hairline box, set into the control's trailing corner.
///
/// A control whose box cannot hold the chip is drawn without one — half a key
/// would be worse than no key — and the title row's legend carries the rest.
fn key_chip(painter: &mut Painter, rect: Rect, key: &str, ink: Rgba) {
    if key.is_empty() {
        return;
    }
    let width = advance(key.chars().count(), font::FINE) + KEY_CHIP_PAD * 2.0;
    let height = font::FINE + KEY_CHIP_PAD * 2.0;
    if width + KEY_CHIP_INSET * 2.0 > rect.w * 0.5 || height + 2.0 > rect.h {
        return;
    }
    let chip = Rect::new(
        rect.right() - width - KEY_CHIP_INSET,
        rect.bottom() - height - 1.0,
        width,
        height,
    );
    painter.rect(chip.x, chip.y, chip.w, chip.h, color::PANEL_BG.alpha(0.9));
    painter.rect_outline(chip.x, chip.y, chip.w, chip.h, color::PANEL_BORDER);
    painter.text_aligned(
        [chip.center()[0], chip.y + KEY_CHIP_PAD * 0.5],
        font::FINE,
        ink,
        Align::Center,
        key,
    );
}

/// The top of a line of `size`-pixel text, centred in a row that starts at
/// `row_y`: the Painter anchors text at the top of its line box.
fn line_y(row_y: f32, row_h: f32, size: f32) -> f32 {
    row_y + ((row_h - size) * 0.5).max(0.0)
}

/// How wide `chars` characters run at `size`, in the monospace face the text
/// renderer shapes with. Panels size boxes with this because the Painter
/// resolves alignment but never reports a width.
fn advance(chars: usize, size: f32) -> f32 {
    chars as f32 * size * CHAR_ADVANCE
}

/// A Fitness-sized number: enough digits to see change, few enough for a column.
fn number(value: f64) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    let magnitude = value.abs();
    if magnitude >= 1000.0 {
        format!("{value:.0}")
    } else if magnitude >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

/// Greedy wrap against the same estimated advance, for the one strip that has to
/// carry a sentence rather than a number.
fn wrap(text: &str, width: f32, size: f32, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut chars = 0usize;
    for word in text.split_whitespace() {
        let word_chars = word.chars().count();
        let candidate = if line.is_empty() {
            word_chars
        } else {
            chars + 1 + word_chars
        };
        if !line.is_empty() && advance(candidate, size) > width {
            lines.push(std::mem::take(&mut line));
            chars = 0;
            if lines.len() >= max_lines {
                return lines;
            }
        }
        if !line.is_empty() {
            line.push(' ');
            chars += 1;
        }
        line.push_str(word);
        chars += word_chars;
    }
    if !line.is_empty() && lines.len() < max_lines {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fade_scales_every_mark_by_the_line_s_own_alpha() {
        let ink = Rgba::rgb(1.0, 1.0, 1.0).alpha(0.5);
        assert_eq!(faded(ink, 1.0).a, 0.5);
        assert_eq!(faded(ink, 0.5).a, 0.25);
        assert_eq!(faded(ink, 0.0).a, 0.0);
        // A fade is an opacity, not a multiplier a caller can overshoot.
        assert_eq!(faded(ink, 2.0).a, 0.5);
        assert_eq!(faded(ink, -1.0).a, 0.0);
        // Colour is untouched: only the alpha rides the fade.
        let text = color::TEXT;
        assert_eq!(faded(text, 0.5).to_array(), [text.r, text.g, text.b, 0.5]);
    }

    #[test]
    fn a_gradient_bar_runs_its_stops_across_the_run() {
        let (points, stops) =
            gradient_bar(10.0, 30.0, 4.0, 1.0, color::PANEL_BORDER, 0.9, 0.0).unwrap();
        assert_eq!(
            points,
            [[10.0, 4.0], [30.0, 4.0], [30.0, 5.0], [10.0, 5.0]],
            "the quad runs left to right, then back along the bottom"
        );
        assert_eq!(stops.map(|stop| stop.a), [0.9, 0.0, 0.0, 0.9]);
        // Nothing to fill: no quad at all, rather than a degenerate one.
        assert!(gradient_bar(10.0, 10.0, 4.0, 1.0, color::PANEL_BORDER, 0.9, 0.0).is_none());
        assert!(gradient_bar(10.0, 30.0, 4.0, 0.0, color::PANEL_BORDER, 0.9, 0.0).is_none());
    }

    #[test]
    fn panel_chrome_stays_inside_the_panel_it_decorates() {
        for rect in [
            Rect::new(20.0, 30.0, 240.0, 120.0),
            Rect::new(0.0, 0.0, 40.0, 24.0),
        ] {
            let mut painter = Painter::new();
            let body = frame(&mut painter, rect, "01 / EVOLUTION");
            assert!(body.x >= rect.x && body.right() <= rect.right());
            assert!(body.y >= rect.y && body.bottom() <= rect.bottom());
            for vertex in &painter.triangles {
                let [x, y] = vertex.pos;
                // A line is centred on its path, so it may reach half a weight
                // past an edge — and no further.
                assert!(
                    x >= rect.x - 1.0
                        && x <= rect.right() + 1.0
                        && y >= rect.y - 1.0
                        && y <= rect.bottom() + 1.0,
                    "chrome escapes {rect:?} at {:?}",
                    vertex.pos
                );
            }
        }
    }

    #[test]
    fn strongest_signals_are_ranked_by_weighted_activation_and_bounded() {
        let mut rng = sim::Rng::from_seed(2);
        let mut tracker = sim::InnovationTracker::new();
        let genome = Genome::new(&mut rng, &mut tracker);
        let mut net = Network::from_genome(&genome);
        net.activate(&[0.5; nn::INPUTS]);
        let selected = strongest_signals(&net);
        assert_eq!(selected.len(), nn::OUTPUTS * 2);
        for to in net.output_ids() {
            let mut expected: Vec<_> = net
                .enabled_connections()
                .into_iter()
                .filter(|(_, t, _)| *t == to)
                .map(|(f, t, w)| (f, t, net.activation(f) * w))
                .collect();
            expected.sort_by(|a, b| b.2.abs().total_cmp(&a.2.abs()).then(a.0.cmp(&b.0)));
            assert_eq!(
                selected
                    .iter()
                    .filter(|(_, t, _)| *t == to)
                    .copied()
                    .collect::<Vec<_>>(),
                expected[..2]
            );
        }
    }

    #[test]
    fn wrapping_keeps_the_status_to_the_strip_and_its_two_lines() {
        let width = advance(30, font::SMALL);
        let lines = wrap("a long status", width, font::SMALL, 1);
        assert_eq!(lines, vec!["a long status".to_string()]);
        let lines = wrap(
            "aaaa bbbb cccc dddd",
            advance(9, font::SMALL),
            font::SMALL,
            2,
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines,
            vec!["aaaa bbbb".to_string(), "cccc dddd".to_string()]
        );
    }

    #[test]
    fn numbers_get_shorter_as_they_get_bigger() {
        assert_eq!(number(1.234), "1.23");
        assert_eq!(number(12.34), "12.3");
        assert_eq!(number(1234.6), "1235");
        assert_eq!(number(f64::NAN), "—");
    }
}

/// The control strip's, the Network's and the banner's own tests, kept in a
/// module of their own at the end of the file so the Record's tests and these
/// cannot collide.
#[cfg(test)]
mod control_tests {
    use super::*;

    /// A window with room for the whole sidebar.
    fn window() -> Layout {
        Layout::new(1440.0, 900.0)
    }

    /// The matter inks a Painter holds, sorted. Two frames are compared on what
    /// they printed, not on the order they printed it in.
    fn inks(painter: &Painter) -> Vec<[u32; 4]> {
        let mut inks: Vec<[u32; 4]> = painter
            .triangles
            .iter()
            .map(|vertex| vertex.color.map(f32::to_bits))
            .collect();
        inks.sort_unstable();
        inks
    }

    /// The distinct strengths an overlay was drawn in: one is a flat wash.
    fn strengths(painter: &Painter) -> usize {
        let mut alphas: Vec<u32> = painter
            .triangles
            .iter()
            .map(|vertex| vertex.color[3].to_bits())
            .collect();
        alphas.sort_unstable();
        alphas.dedup();
        alphas.len()
    }

    #[test]
    fn every_control_answers_the_pointer() {
        let layout = window();
        let controls = Controls::default();
        let mut plain = Painter::new();
        draw_controls(&mut plain, &layout, &controls, None);
        for hit in [
            Hit::Pause,
            Hit::Rays,
            Hit::Evolve,
            Hit::Restart,
            Hit::NewSeed,
            Hit::Save,
            Hit::Load,
            Hit::SeedField,
            Hit::SpeedSlider,
        ] {
            let mut painter = Painter::new();
            draw_controls(&mut painter, &layout, &controls, Some(hit));
            assert_ne!(inks(&plain), inks(&painter), "{hit:?} ignores the pointer");
        }
    }

    #[test]
    fn a_press_reads_differently_for_each_class_of_control() {
        let layout = window();
        let controls = Controls::default();
        let press = |hit: Hit| {
            let mut painter = Painter::new();
            draw_press(&mut painter, &layout, &controls, hit);
            painter
        };
        let key = press(Hit::Pause);
        let field = press(Hit::SeedField);
        let slider = press(Hit::SpeedSlider);
        assert_ne!(inks(&key), inks(&field));
        assert_ne!(inks(&field), inks(&slider));
        assert_ne!(inks(&key), inks(&slider));
        // A press is a shape rather than the flat wash it used to be: every
        // overlay carries more than one strength — its face, and the border that
        // closes it.
        for painter in [&key, &field, &slider] {
            assert!(strengths(painter) >= 2, "a press is a flat tint");
        }
    }

    /// Every key the window prints, and the arm in `appstate.rs`'s `on_key` that
    /// has to exist for the print to be true. The panel cannot call a private
    /// method of the shell, and the property under test is exactly that the two
    /// files agree about which keys exist, so the shell is read as the text it
    /// is.
    const SHELL_KEYS: [(&str, &str); 9] = [
        ("space", "NamedKey::Space"),
        ("m", "\"m\" =>"),
        ("r", "\"r\" =>"),
        ("s", "\"s\" =>"),
        ("l", "\"l\" =>"),
        ("e", "\"e\" =>"),
        ("TAB", "NamedKey::Tab"),
        ("ENTER", "NamedKey::Enter"),
        ("arrows", "NamedKey::ArrowLeft | NamedKey::ArrowRight"),
    ];

    #[test]
    fn every_key_the_window_names_is_one_the_shell_handles() {
        let shell = include_str!("appstate.rs");
        for (key, arm) in SHELL_KEYS {
            assert!(
                shell.contains(arm),
                "the window names {key}, the shell has no {arm}"
            );
        }
        // The standing legend names the keys no button carries, and every one of
        // them is on the list above.
        let legend = key_legend();
        for (key, what) in KEY_LEGEND {
            assert!(legend.contains(&format!("{key} {what}")));
            assert!(
                SHELL_KEYS.iter().any(|(known, _)| *known == key),
                "the legend names {key}, which is not a key the shell handles"
            );
        }
        // The chips are printed by the buttons that answer to them, once each:
        // a key printed twice would be two controls claiming one hotkey.
        let mut painter = Painter::new();
        draw_controls(&mut painter, &window(), &Controls::default(), None);
        let printed: Vec<&str> = painter
            .text
            .iter()
            .map(|item| item.content.as_str())
            .collect();
        for (chip, _) in SHELL_KEYS {
            if ["space", "r", "s", "l", "e"].contains(&chip) {
                assert_eq!(
                    printed.iter().filter(|text| **text == chip).count(),
                    1,
                    "the chip for {chip} is not printed exactly once"
                );
            }
        }
    }

    #[test]
    fn a_key_that_cannot_act_yet_names_what_it_waits_for() {
        let layout = window();
        let idle = Controls::default();
        let mut painter = Painter::new();
        draw_controls(&mut painter, &layout, &idle, None);
        assert!(
            painter
                .text
                .iter()
                .any(|item| item.content == "needs a Genome"),
            "Evolve is drawn waiting with no reason on its face"
        );
        // Its hint says the same thing before the click, and says how to resolve
        // it.
        let hint = control_hint(Hit::Evolve, &idle);
        assert!(hint.contains("needs a loaded Genome"));
        // With a Genome loaded it is an ordinary key, and the hint says what it
        // does rather than what it lacks.
        let loaded = Controls {
            watching: true,
            ..Controls::default()
        };
        let mut painter = Painter::new();
        draw_controls(&mut painter, &layout, &loaded, None);
        assert!(!painter
            .text
            .iter()
            .any(|item| item.content == "needs a Genome"));
        assert!(!control_hint(Hit::Evolve, &loaded).contains("needs a loaded Genome"));
        // A waiting key is still drawn as a key: its label, and its chip.
        for text in ["Evolve", "e"] {
            assert!(painter.text.iter().any(|item| item.content == text));
        }
    }

    /// A Genome with `hidden` hidden nodes spliced into it, and the Network it
    /// evaluates to: enough topology for the panel to have to drop edges.
    fn grown_network(hidden: usize) -> (Genome, Network) {
        let mut rng = sim::Rng::from_seed(7);
        let mut tracker = sim::InnovationTracker::new();
        let mut genome = Genome::new(&mut rng, &mut tracker);
        for _ in 0..hidden {
            genome.mutate_add_node(&mut rng, &mut tracker);
        }
        let mut network = Network::from_genome(&genome);
        network.activate(&[0.5; nn::INPUTS]);
        (genome, network)
    }

    /// The number of terms a partial legend says are on the panel. A complete
    /// legend makes no count — "2 largest per output" is the rule it ranked by,
    /// not a number drawn — so this is `None` there.
    fn claimed_terms(legend: &[String; 2]) -> Option<usize> {
        legend[1].split_whitespace().next()?.parse().ok()
    }

    #[test]
    fn the_legend_never_claims_more_terms_than_the_panel_draws() {
        // More hidden nodes than a short panel can anchor, so the drawing really
        // does have to drop some of the terms the ranking selected.
        let (genome, network) = grown_network(40);
        for height in [96.0, 178.0, 236.0, 400.0, 1600.0] {
            let rect = Rect::new(20.0, 30.0, 370.0, height);
            let mut painter = Painter::new();
            let body = frame(&mut painter, rect, "03 / NETWORK · ONE-HOP TERMS");
            let area = columns_box(body, genome.hidden_count());
            let columns = Columns::new(area, &network, genome.hidden_count());
            let (bands, selected) = signal_bands(&network, &columns);
            let legend = signal_legend(selected, bands.len());
            if let Some(claimed) = claimed_terms(&legend) {
                assert_eq!(
                    claimed,
                    bands.len(),
                    "{height}: the legend claims {claimed} terms, the panel draws {}",
                    bands.len()
                );
                assert!(
                    bands.len() < selected,
                    "{height}: a partial legend, fully drawn"
                );
            } else {
                assert_eq!(
                    bands.len(),
                    selected,
                    "{height}: a complete legend must draw every selected term"
                );
            }
            // The legend is measured against the lane it is printed in.
            for line in &legend {
                assert!(
                    advance(line.chars().count(), font::FINE) <= body.w,
                    "{height}: {line:?} is wider than the panel"
                );
            }
        }
    }

    #[test]
    fn a_tall_box_shows_more_of_the_network_and_leaves_the_rest_as_ground() {
        let body = Rect::new(0.0, 100.0, 330.0, 1500.0);
        let small = columns_box(body, 4);
        let large = columns_box(body, 40);
        // The drawing stops at the natural height and is centred in its box...
        assert_eq!(small.h, natural_height(4));
        assert_eq!(large.h, natural_height(40));
        for box_ in [small, large] {
            // Centred in the space it draws in: the caption lane and its air are
            // the panel's foot, not part of what the instrument centres itself
            // in.
            let lane = CAPTION_LANE.min(body.h * 0.35);
            let space = body.h - lane - COLUMN_AIR;
            let above = box_.y - body.y;
            assert!(
                (above - (space - box_.h) * 0.5).abs() < 0.5,
                "the instrument is not centred in its box"
            );
            assert!(
                body.bottom() - box_.bottom() >= lane + COLUMN_AIR - 0.5,
                "the columns run into the caption lane"
            );
        }
        // ...and height buys more of the network before it buys anything else.
        assert!(large.h > small.h);
        let (genome, network) = grown_network(40);
        let area = columns_box(Rect::new(0.0, 0.0, 330.0, 1600.0), genome.hidden_count());
        let columns = Columns::new(area, &network, genome.hidden_count());
        assert_eq!(
            columns.shown_hidden,
            genome.hidden_count(),
            "the height was there: every hidden node is drawn"
        );
        // No column stretches past its own pitch, however tall the box.
        assert!(columns.input_step <= INPUT_MAX_STEP);
        assert!(columns.hidden_step <= HIDDEN_MAX_STEP);
        assert!(columns.output_step <= OUTPUT_MAX_STEP);
        // A short box still packs the columns inside itself rather than spilling
        // out of it.
        let (_, plain) = grown_network(0);
        let tight = Columns::new(columns_box(Rect::new(0.0, 0.0, 330.0, 120.0), 0), &plain, 0);
        assert!(tight.inputs.bottom() <= 120.0);
        assert!(tight.output_row(nn::OUTPUTS - 1).bottom() <= 120.0);
    }

    #[test]
    fn a_bands_magnitude_rides_its_ink_and_never_its_geometry() {
        let band = |head: f32| {
            let mut painter = Painter::new();
            signal_band(
                &mut painter,
                [10.0, 10.0],
                [90.0, 50.0],
                color::ACCENT,
                head,
            );
            painter
        };
        let faint = band(signal_opacity(0.0));
        let strong = band(signal_opacity(1.0));
        // The wire is the same ink at every magnitude — it is the connection's
        // existence, not its strength — and the line is the same line...
        assert_eq!(inks(&faint), inks(&strong));
        assert_eq!(
            inks(&faint)[0],
            color::NODE_EDGE
                .alpha(WIRE_ALPHA)
                .to_linear()
                .map(f32::to_bits)
        );
        assert_eq!(
            faint
                .triangles
                .iter()
                .map(|vertex| vertex.pos)
                .collect::<Vec<_>>(),
            strong
                .triangles
                .iter()
                .map(|vertex| vertex.pos)
                .collect::<Vec<_>>()
        );
        // ...and the light is where the magnitude went.
        assert_ne!(
            faint
                .luminous
                .iter()
                .map(|vertex| vertex.color.map(f32::to_bits))
                .collect::<Vec<_>>(),
            strong
                .luminous
                .iter()
                .map(|vertex| vertex.color.map(f32::to_bits))
                .collect::<Vec<_>>()
        );
        // The strength is monotone in the magnitude and bounded by the floor and
        // the ceiling the panel drew with.
        let mut previous = -1.0;
        for step in 0..=10 {
            let opacity = signal_opacity(step as f32 / 10.0);
            assert!(opacity > previous);
            assert!((SIGNAL_FLOOR..=SIGNAL_CEIL).contains(&opacity));
            previous = opacity;
        }
        // Width is constant along the band: a straight one's quads span exactly
        // one band width, at its weakest and at its strongest alike.
        for magnitude in [0.0, 1.0] {
            let mut painter = Painter::new();
            signal_band(
                &mut painter,
                [0.0, 0.0],
                [100.0, 0.0],
                color::ACCENT,
                signal_opacity(magnitude),
            );
            let span = (painter.triangles[0].pos[1] - painter.triangles[2].pos[1]).abs();
            assert!((span - SIGNAL_WIDTH).abs() < 1e-4);
        }
    }

    #[test]
    fn the_network_draws_inside_its_panel_at_every_size() {
        let (genome, network) = grown_network(6);
        for height in [96.0, 120.0, 236.0, 900.0, 1600.0] {
            let rect = Rect::new(20.0, 30.0, 370.0, height);
            let mut painter = Painter::new();
            draw_network(&mut painter, rect, &genome, &network);
            for item in &painter.text {
                assert!(
                    item.pos[0] >= rect.x - 1.0
                        && item.pos[0] <= rect.right() + 1.0
                        && item.pos[1] >= rect.y - 1.0
                        && item.pos[1] <= rect.bottom() + 1.0,
                    "caption {:?} escapes {rect:?}",
                    item.content
                );
            }
            for vertex in &painter.triangles {
                let [x, y] = vertex.pos;
                assert!(
                    x >= rect.x - 1.0
                        && x <= rect.right() + 1.0
                        && y >= rect.y - 1.0
                        && y <= rect.bottom() + 1.0,
                    "ink escapes {rect:?} at {x}, {y}"
                );
            }
            // The panel says what its columns are and what its bands are at
            // every size it draws the full instrument at — the lane is not
            // there to be skipped.
            if height >= 236.0 {
                let printed: Vec<&str> = painter
                    .text
                    .iter()
                    .map(|item| item.content.as_str())
                    .collect();
                for caption in ["21 inputs", "6 hidden", "5 out"] {
                    assert!(printed.contains(&caption), "{height}: no {caption:?}");
                }
                assert!(printed.iter().any(|line| line.contains("|w·a|")));
                assert!(printed
                    .iter()
                    .any(|line| line.contains("pre-activation sum")));
            }
        }
    }

    #[test]
    fn a_banner_arrives_fast_and_leaves_fast() {
        let (lifetime, rise, fall) = (1.2, 0.15, 0.30);
        // Exact at both ends: absent at birth, gone at death.
        assert_eq!(banner_fade(0.0, lifetime, rise, fall), 0.0);
        assert_eq!(banner_fade(lifetime, lifetime, rise, fall), 0.0);
        // Full through the hold in between.
        assert_eq!(banner_fade(0.6, lifetime, rise, fall), 1.0);
        // The entrance curve is ahead of a linear ramp — a third of the way in,
        // the banner is already more than a third of the way up — and the exit
        // curve is ahead of one on the way out.
        let entering = banner_fade(0.05, lifetime, rise, fall);
        assert!(
            entering > (0.05 / rise) as f32,
            "the banner arrives on a linear ramp: {entering}"
        );
        let leaving = banner_fade(lifetime - 0.1, lifetime, rise, fall);
        assert!(
            leaving < (0.1 / fall) as f32,
            "the banner leaves on a linear ramp: {leaving}"
        );
    }

    #[test]
    fn a_status_message_fades_in_on_the_standard_curve() {
        assert_eq!(status_fade(0.0), 0.0);
        assert_eq!(status_fade(f64::from(motion::MODERATE_01)), 1.0);
        // A state change the interface made: there on the productive curve,
        // which is ahead of a linear ramp from the first frame.
        let half = f64::from(motion::MODERATE_01) * 0.5;
        assert!(status_fade(half) > 0.5, "the message fades in linearly");
        // And it stays full: a message is replaced, not dismissed.
        assert_eq!(status_fade(10.0), 1.0);
    }
}
