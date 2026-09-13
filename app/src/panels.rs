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

use crate::painter::{Align, Painter, Rgba, Typeface};
use crate::theme::{self, color, font, layout};
use crate::ui::{Controls, Hit, Layout, Rect, PANEL_PAD, TITLE_GAP, TITLE_H};

/// A character's width in the monospace face the text renderer shapes with.
const CHAR_ADVANCE: f32 = 0.6;

/// The white a Network node climbs toward as it fires.
const LIT: Rgba = Rgba::rgb(1.0, 1.0, 1.0);

/// The gap between the Network panel's three columns.
const COLUMN_GAP: f32 = 10.0;

/// How strong a weighted signal has to be before the path carrying it is lit
/// rather than merely drawn. Magnitude of `activation × weight`, which is the
/// same quantity the ranking uses.
const SIGNAL_LIT: f32 = 0.35;

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

/// Hidden nodes the Network panel draws before summarizing the rest.
const HIDDEN_MAX_DRAWN: usize = 12;

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

/// The Chart's area fill: the ink's alpha at the plot's crest and at its floor.
/// The ramp between the two is a function of absolute height, one for the whole
/// plot, which is what makes the fill a single sheet of light.
const AREA_TOP: f32 = 0.28;
const AREA_FLOOR: f32 = 0.02;

/// The most gridlines the Chart's y axis will print, the explicit zero
/// included. A shorter plot gets fewer, never a pile of labels.
const GRID_MAX: usize = 4;

/// How many rungs the axis asks the 1-2-5 ladder to divide its peak into,
/// before the fit check steps the ladder up.
const GRID_INTERVALS: usize = 3;

/// The smallest gap two gridline labels may keep on a short plot.
const GRID_GAP: f32 = 4.0;

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

/// Competence over Generations: the Population's mean Waves, the run's headline
/// number.
///
/// The Chart plots Competence, not the shaped Fitness the Population breeds on
/// (ADR 0003). Fitness is a breeding score — the shaping in it is the search's,
/// not the operator's — so it rising says nothing about whether the ships fly
/// better. Mean Waves is the headline Competence number, the one the Competence
/// Gate rules on and the one a Candidate has to move, so it is drawn as the
/// subject: a line in [`color::BEST`] over a fill that brightens with it, with
/// the newest reading printed beside the title, where the curve cannot reach it.
///
/// Behind the line stands what the Population actually spread across: the ribbon
/// from its median Waves to its p90, in [`color::BAND`]. Both are whole Waves
/// while the mean is a fraction of one, so on the mean's own scale the ribbon is
/// often pinned to the top of the plot — which is the reading and not a fault:
/// half the Population is at the floor and its tail is past this chart's
/// ceiling, and the axis labels say by how much.
///
/// The x axis is the run's own history, never a fixed window. The y axis starts
/// at an explicit zero and steps in 1-2-5 numbers, so two Generations and five
/// hundred read the same way and every gridline is a round value.
pub fn draw_chart(painter: &mut Painter, rect: Rect, history: &[GenerationStats]) {
    let title = "02 / COMPETENCE · MEAN WAVES";
    let body = frame(painter, rect, title);
    if body.w < 40.0 || body.h < 20.0 {
        return;
    }
    // The newest mean, as the panel's own readout, beside the title: printed in
    // the curve's own ink over the curve, where it used to be, it was a number
    // nobody could read.
    if let Some(latest) = history.last() {
        let heading = title_row(rect);
        let readout = waves(latest.mean_wave);
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
    let axis_h = (font::SMALL + 4.0).min(body.h * 0.3);
    let gutter = advance(6, font::SMALL).min(body.w * 0.25);
    let plot = Rect::new(
        body.x + gutter,
        body.y,
        (body.w - gutter).max(0.0),
        (body.h - axis_h).max(0.0),
    );
    painter.rect(plot.x, plot.y, plot.w, plot.h, color::FIELD);
    // The plot is cut into the housing, like the meters: a shadow along its
    // top lip is what says "inset" rather than "printed on".
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

    // The axis, drawn under the data and measured from an explicit zero: what a
    // plot is worth is only readable against a scale, and the scale is what
    // tells the reader whether the run is standing still.
    let peak = history
        .iter()
        .fold(0.0_f64, |top, stats| top.max(stats.mean_wave));
    let axis = Axis::for_peak(peak, gridline_room(plot));
    let scale = Scale {
        peak: axis.peak,
        zero: plot.bottom(),
        height: plot.h,
    };
    axis_grid(painter, plot, &axis, &scale);

    if history.is_empty() {
        // An instrument with nothing to plot still shows its scale, and says
        // what it is waiting for rather than framing an empty box.
        painter.text_aligned(
            [plot.center()[0], line_y(plot.y, plot.h, font::SMALL)],
            font::SMALL,
            color::TEXT_DIM.alpha(0.7),
            Align::Center,
            "Awaiting first Generation",
        );
        return;
    }

    let count = history.len();
    let newest_index = count - 1;
    let x_at = |index: usize| -> f32 {
        if count == 1 {
            plot.right()
        } else {
            plot.x + plot.w * (index as f32 / newest_index as f32)
        }
    };

    // A long run has more Generations than the plot has pixels; sample the
    // history at no more than one point per pixel column, newest included. All
    // three readings ride the same sampling, so the band can never drift against
    // the line it stands behind.
    let stride = ((count as f32) / plot.w.max(1.0)).ceil().max(1.0) as usize;
    let sample = |index: usize| Reading {
        x: x_at(index),
        mean: scale.y(history[index].mean_wave),
        median: scale.y(f64::from(history[index].median_wave)),
        tail: scale.y(f64::from(history[index].p90_wave)),
    };
    let mut readings: Vec<Reading> = Vec::with_capacity(count.div_ceil(stride) + 1);
    let mut index = 0;
    while index < count {
        readings.push(sample(index));
        index += stride;
    }
    if !newest_index.is_multiple_of(stride) {
        readings.push(sample(newest_index));
    }

    if readings.len() < 2 {
        painter.circle([readings[0].x, readings[0].mean], 2.5, color::BEST, 12);
    } else {
        for pair in readings.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            // The band first, then the fill, then the line: the spread is the
            // range the Population held, the fill the subject's own light, and
            // the line the reading itself.
            if let Some((polygon, stops)) =
                band_column(left.x, right.x, left.median, left.tail, color::BAND)
            {
                painter.gradient_polygon(&polygon, &stops);
            }
            if let Some((polygon, stops)) = area_fill(
                left.x,
                left.mean,
                right.x,
                right.mean,
                &scale,
                color::ACCENT,
            ) {
                painter.gradient_polygon(&polygon, &stops);
            }
            painter.stroke([left.x, left.mean], [right.x, right.mean], 1.6, color::BEST);
        }
    }

    // The newest reading is marked where the line ends: the number itself stands
    // beside the title, but the eye still needs the point it belongs to.
    let last = readings[readings.len() - 1];
    painter.circle([last.x - 1.0, last.mean], 2.0, color::ACCENT, 10);
    painter.set_gain(theme::light::LAMP);
    painter.luminous_glow([last.x - 1.0, last.mean], 5.0, color::ACCENT.alpha(0.5));
    painter.set_gain(1.0);

    // The legend, in the corner the readout has left free: what the line is, and
    // what the wash standing behind it is.
    let legend_y = plot.y + 4.0;
    painter.line(
        [plot.x + 6.0, legend_y + 5.0],
        [plot.x + 18.0, legend_y + 5.0],
        color::BEST,
    );
    painter.text([plot.x + 22.0, legend_y], font::SMALL, color::BEST, "mean");
    let band_x = plot.x + 22.0 + advance(4, font::SMALL) + 14.0;
    if plot.right() - band_x >= advance(11, font::SMALL) {
        painter.rect(band_x, legend_y + 2.0, 12.0, 6.0, color::BAND);
        painter.text(
            [band_x + 16.0, legend_y],
            font::SMALL,
            color::TEXT_DIM,
            "median–p90",
        );
    }

    // Generations under the plot: the first, the newest, and what they count.
    let axis_y = plot.bottom() + 3.0;
    if axis_y + font::SMALL <= body.bottom() {
        painter.text([plot.x, axis_y], font::SMALL, color::TEXT_DIM, "1");
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

/// The evaluated Ship's Network, live: inputs, hidden nodes, and the five
/// outputs with the activation each is producing right now.
///
/// Every frame redraws from the `Network` the shell hands over, so what is on
/// screen is the network that is flying, never a stale one.
pub fn draw_network(painter: &mut Painter, rect: Rect, genome: &Genome, network: &Network) {
    let body = frame(
        painter,
        rect,
        if rect.h < 140.0 {
            "03 / NETWORK · OUTPUT SNAPSHOT"
        } else {
            "03 / NETWORK · STRONGEST SIGNALS"
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
    let caption_h = (font::SMALL + 18.0).min(body.h * 0.3);
    let columns = Columns::new(body, caption_h, network, genome.hidden_count());
    let input_count = network
        .nodes()
        .iter()
        .filter(|(_, kind)| *kind == NodeType::Input)
        .count();

    // Show the two strongest incoming weighted signals per visible target.
    // This is a magnitude ranking, not confidence or causal attribution.
    for (from, to, contribution) in strongest_signals(network) {
        let (Some(a), Some(b)) = (
            node_anchor(from, network, &columns),
            node_anchor(to, network, &columns),
        ) else {
            continue;
        };
        let activity = contribution.abs().clamp(0.0, 1.0) as f32;
        let ink = if contribution >= 0.0 {
            color::ACCENT
        } else {
            color::ENERGY
        };
        let path = crate::vector::connection(a, b, contribution < 0.0);
        painter.path(
            &path,
            0.65 + activity * 1.15,
            ink.alpha(0.12 + activity * 0.45),
        );
        // A strong signal carries light as well as width, so the paths that are
        // actually doing the work separate from the ones that merely exist.
        // The threshold and the brightness are both |activation × weight|.
        if activity > SIGNAL_LIT {
            painter.set_gain(theme::light::SIGNAL);
            painter.path(
                &path,
                0.5 + activity * 0.6,
                ink.alpha((activity - SIGNAL_LIT) * 0.5),
            );
            painter.set_gain(1.0);
        }
    }

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

    if caption_h >= 25.0 {
        painter.text(
            [body.x, body.bottom() - 11.0],
            font::FINE,
            color::TEXT_DIM,
            "TOP 2/NODE  + SOLID / - DASHED  |a × w|",
        );
    }
    // What each column is, under it.
    let caption_y = body.bottom() - caption_h + 1.0;
    if caption_y + font::SMALL > body.bottom() {
        return;
    }
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

/// Two incoming signals per target, ranked by |activation × weight|.
/// Stable tie-breaking avoids flicker; work is linear apart from target lookup.
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

/// The Network panel's three columns, in window pixels.
struct Columns {
    inputs: Rect,
    hidden: Rect,
    outputs: Rect,
    input_origin: [f32; 2],
    input_step: f32,
    hidden_origin: [f32; 2],
    hidden_step: f32,
    output_step: f32,
    /// Hidden nodes the panel has room to draw; the rest are summarized.
    shown_hidden: usize,
    /// Where the "+N more" line sits, when nodes were dropped.
    summary_y: Option<f32>,
}

impl Columns {
    fn new(body: Rect, caption_h: f32, network: &Network, hidden_count: usize) -> Columns {
        let height = (body.h - caption_h).max(0.0);
        let input_w = (body.w * 0.20).min(64.0);
        let output_w = (body.w * 0.36).clamp(60.0, 118.0).min(body.w);
        let hidden_w = (body.w - input_w - output_w - COLUMN_GAP * 2.0).max(0.0);
        let inputs = Rect::new(body.x, body.y, input_w, height);
        let hidden = Rect::new(inputs.right() + COLUMN_GAP, body.y, hidden_w, height);
        let outputs = Rect::new(hidden.right() + COLUMN_GAP, body.y, output_w, height);

        // 21 dots at no more than 4 px pitch, and never taller than the column:
        // a squeezed panel packs them tighter rather than spilling into the
        // caption lane below.
        let input_step = (height / nn::INPUTS as f32).clamp(0.0, 11.0);
        let input_extent = input_step * (nn::INPUTS - 1) as f32;
        let shown_hidden = hidden_count
            .min(HIDDEN_MAX_DRAWN)
            .min((height / HIDDEN_MIN_STEP).floor().max(0.0) as usize);
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
            (dots_h / shown_hidden as f32).min(14.0)
        } else {
            0.0
        };
        let hidden_extent = hidden_step * shown_hidden.saturating_sub(1) as f32;

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
            output_step: if network.output_ids().is_empty() {
                0.0
            } else {
                height / network.output_ids().len() as f32
            },
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
            self.outputs.y + self.output_step * rank as f32,
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
pub fn draw_controls(
    painter: &mut Painter,
    layout: &Layout,
    controls: &Controls,
    hot: Option<Hit>,
) {
    frame(painter, layout.controls, "Controls");
    let pressed = |hit: Hit| hot == Some(hit);

    button(
        painter,
        layout.pause,
        if controls.paused { "Resume" } else { "Pause" },
        if controls.paused {
            State::On
        } else {
            State::Off
        },
        pressed(Hit::Pause),
    );
    button(
        painter,
        layout.rays,
        "Rays",
        if controls.rays { State::On } else { State::Off },
        pressed(Hit::Rays),
    );
    // Evolving only means something while a loaded Genome is being replayed.
    button(
        painter,
        layout.evolve,
        "Evolve",
        if controls.watching {
            State::On
        } else {
            State::Dim
        },
        pressed(Hit::Evolve),
    );
    button(
        painter,
        layout.restart,
        "Restart",
        State::Off,
        pressed(Hit::Restart),
    );
    button(
        painter,
        layout.new_seed,
        "New seed",
        State::Off,
        pressed(Hit::NewSeed),
    );
    button(painter, layout.save, "Save", State::Off, pressed(Hit::Save));
    button(painter, layout.load, "Load", State::Off, pressed(Hit::Load));

    // The seed field: `seed_text` is the live seed while it is not being edited,
    // and the digits being typed while it is.
    let field = layout.seed_field;
    if field.w > 8.0 && field.h > 8.0 {
        let border = if controls.seed_editing {
            color::ACCENT.alpha(0.9)
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
        painter.rect_outline(bar.x, bar.y, bar.w, bar.h, color::PANEL_BORDER.alpha(0.8));
        painter.rect(
            thumb_x - THUMB_W * 0.5,
            bar.center()[1] - THUMB_H * 0.5,
            THUMB_W,
            THUMB_H,
            color::ACCENT,
        );
        painter.set_gain(theme::light::LAMP);
        painter.luminous_glow(
            [thumb_x, bar.center()[1]],
            THUMB_GLOW,
            color::ACCENT.alpha(0.3),
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

/// A centred banner over the Arena: Episode transitions, and anything that went
/// wrong badly enough to say so in the middle of the screen.
///
/// The shell fades the banner by handing its line over at a partial alpha; that
/// alpha is the fade, so every mark drawn here — plate, rules, glow, the rules'
/// gradient stops — is scaled by it and the banner arrives and leaves as one
/// object rather than a box that pops and text that dissolves.
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

/// One sampled column of the Chart: where it stands across the plot, and the
/// three readings the Population produced there. All three ride one sampling, so
/// the band can never drift against the line it stands behind.
#[derive(Clone, Copy)]
struct Reading {
    x: f32,
    /// The mean Waves, in plot pixels.
    mean: f32,
    /// The median Waves, in plot pixels.
    median: f32,
    /// The 90th-percentile Waves, in plot pixels.
    tail: f32,
}

/// The Chart's vertical scale: `0..peak` mapped onto the plot, an explicit zero
/// at its foot and the peak at its crest. Everything the Chart draws is measured
/// through [`Scale::y`], so a value past the peak — the Population's tail, often
/// — is pinned to the crest rather than drawn outside the housing.
struct Scale {
    /// The value at the crest: the highest mean the run has scored.
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

/// The Chart's y axis: the 1-2-5 ladder its gridlines stand on, and the peak
/// the scale reaches.
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
        // Nothing scored yet: a unit axis, so an empty Chart still has a scale.
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

/// The Chart's fill ink at the absolute plot height `y`: one ramp across the
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

/// One Chart column's fill: the quad from the mean curve down to the baseline,
/// and its four stops — the ramp sampled at each corner. `None` when the column
/// has no width or the curve is already lying on the floor.
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

/// One Chart column of the Population's spread: the rectangle from its median
/// Waves to its p90 over one Generation's width, in the band's own flat wash.
/// `None` when the column has no width or the two are the same Waves — a
/// Population whose middle and whose tail are both at zero has no range to draw.
///
/// The band holds each Generation's range across that Generation's own span
/// rather than slanting from one to the next: the median and the p90 are whole
/// Waves counted per Generation, so an edge between two of them would draw
/// ranges in between that no Population ever had. The two edges are ordered
/// here, because which of them stands higher on the plot is the data's business:
/// the median cannot pass the p90, and the Chart should not depend on that.
fn band_column(
    x0: f32,
    x1: f32,
    median: f32,
    tail: f32,
    ink: Rgba,
) -> Option<([[f32; 2]; 4], [Rgba; 4])> {
    if x1 <= x0 {
        return None;
    }
    let top = median.min(tail);
    let bottom = median.max(tail);
    if top >= bottom {
        return None;
    }
    Some(([[x0, top], [x1, top], [x1, bottom], [x0, bottom]], [ink; 4]))
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

/// How a button reads: plain, toggled on, or not available.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Off,
    On,
    Dim,
}

/// One button: a key that lights under the pointer, fills in when it is toggled
/// on, and says so with an underline as well as a tint.
fn button(painter: &mut Painter, rect: Rect, label: &str, state: State, hot: bool) {
    if rect.w < 1.0 || rect.h < 1.0 {
        return;
    }
    let fill = match (hot, state) {
        (true, _) => color::BUTTON_HOT,
        (false, State::On) => color::BUTTON_ON,
        (false, _) => color::BUTTON,
    };
    let text = if hot || state != State::Dim {
        color::TEXT
    } else {
        // Not available, but still readable: it says what the strip can do.
        color::TEXT_DIM.alpha(0.6)
    };
    let border = if hot {
        color::ACCENT.alpha(0.55)
    } else if state == State::Dim {
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
    painter.text_aligned(
        [rect.center()[0], line_y(rect.y, rect.h, font::BODY)],
        font::BODY,
        text,
        Align::Center,
        label,
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

/// A mean Wave count, as the Chart labels it: the headline is a fraction of a
/// Wave, so two decimals are what show its movement.
fn waves(value: f64) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    format!("{value:.2}")
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

    /// A Generation's stats, as the Chart reads them: nine of these fields are
    /// somebody else's business, so only the four the Chart plots are named.
    fn stats(generation: u32, mean: f64, median: u32, p90: u32) -> GenerationStats {
        GenerationStats {
            generation,
            best: 0.0,
            mean: 0.0,
            mean_wave: mean,
            median_wave: median,
            p90_wave: p90,
            clearing_share: 0.0,
        }
    }

    /// A Chart panel of the size the sidebar gives it at 1440 × 900.
    fn chart_rect() -> Rect {
        Rect::new(1100.0, 232.0, 330.0, 120.0)
    }

    #[test]
    fn the_chart_axis_steps_in_one_two_fives_and_never_crowds_the_plot() {
        // A run that has got as far as 0.34 Waves reads against a ladder of
        // tenths — 0, 0.1, 0.2, 0.3 — not against thirds of its own span.
        assert!((tick_step(0.34, 3) - 0.1).abs() < 1e-12);
        assert!((tick_step(0.34, 6) - 0.05).abs() < 1e-12);
        let axis = Axis::for_peak(0.34, 4);
        assert!((axis.step - 0.1).abs() < 1e-12);
        assert_eq!(axis.ticks.len(), 4, "0, 0.1, 0.2, 0.3");

        // Whatever the run has scored and however much room the plot has, the
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

    #[test]
    fn the_band_maps_the_median_to_the_p90_and_clamps_to_the_plot() {
        // A plot a hundred pixels tall whose crest is one Wave.
        let scale = Scale {
            peak: 1.0,
            zero: 100.0,
            height: 100.0,
        };
        assert_eq!(scale.y(0.0), 100.0, "zero stands on the baseline");
        assert_eq!(scale.y(1.0), 0.0, "the peak stands on the crest");
        // A tail past the crest is pinned to it and a value under the floor to
        // the floor: nothing the Chart plots leaves the plot.
        assert_eq!(scale.y(2.0), 0.0);
        assert_eq!(scale.y(-0.5), 100.0);

        // Half the Population on zero Waves and the tail at one: the column runs
        // from the baseline to the crest...
        let (points, stops) =
            band_column(10.0, 20.0, scale.y(0.0), scale.y(1.0), color::BAND).unwrap();
        assert_eq!(
            points,
            [[10.0, 0.0], [20.0, 0.0], [20.0, 100.0], [10.0, 100.0]]
        );
        assert!(
            stops.iter().all(|stop| *stop == color::BAND),
            "one flat wash, so a clamped column cannot band"
        );
        // The two edges are ordered, not trusted: a tail that somehow read below
        // the median still draws a column rather than an inverted one.
        let (points, _) = band_column(10.0, 20.0, 90.0, 40.0, color::BAND).unwrap();
        assert_eq!(
            points,
            [[10.0, 40.0], [20.0, 40.0], [20.0, 90.0], [10.0, 90.0]]
        );
        // ...and where the middle and the tail are the same Waves there is no
        // column at all, rather than a line of nothing.
        assert!(band_column(10.0, 20.0, 50.0, 50.0, color::BAND).is_none());
        assert!(band_column(20.0, 10.0, 40.0, 30.0, color::BAND).is_none());
    }

    #[test]
    fn the_chart_fill_is_one_sheet_of_light_rather_than_a_column_of_it() {
        let scale = Scale {
            peak: 1.0,
            zero: 100.0,
            height: 100.0,
        };
        // Neighbouring columns agree along the edge they share, whatever their
        // curves: the stops are a function of height, not of the column's own
        // span, and that is what removes the vertical banding.
        let (_, low) = area_fill(0.0, 60.0, 5.0, 40.0, &scale, color::ACCENT).unwrap();
        let (_, high) = area_fill(5.0, 40.0, 10.0, 15.0, &scale, color::ACCENT).unwrap();
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
        let (points, _) = area_fill(0.0, 20.0, 5.0, 10.0, &scale, color::ACCENT).unwrap();
        assert_eq!(
            points,
            [[0.0, 20.0], [5.0, 10.0], [5.0, 100.0], [0.0, 100.0]],
            "each column runs from the curve down to the baseline"
        );
        assert!(area_fill(0.0, 100.0, 5.0, 100.0, &scale, color::ACCENT).is_none());
        assert!(area_fill(0.0, 101.0, 5.0, 100.0, &scale, color::ACCENT).is_none());
        assert!(area_fill(5.0, 20.0, 5.0, 10.0, &scale, color::ACCENT).is_none());
    }

    #[test]
    fn an_empty_chart_still_shows_its_scale_and_says_what_it_awaits() {
        let rect = chart_rect();
        let mut painter = Painter::new();
        draw_chart(&mut painter, rect, &[]);
        assert!(
            painter
                .text
                .iter()
                .any(|item| item.content == "Awaiting first Generation"),
            "an empty Chart says so"
        );
        // Behind the message is a real axis: a labelled ladder running up the
        // plot, lowest value at the bottom, every label inside the panel.
        let label = |content: &str| {
            painter
                .text
                .iter()
                .find(|item| item.content == content)
                .unwrap_or_else(|| panic!("no {content} gridline on an empty Chart"))
                .clone()
        };
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
    }

    #[test]
    fn the_newest_mean_reads_beside_the_title_clear_of_the_curve() {
        // The mean is rising and the corner it ends in is the top of the plot,
        // which is exactly where the reading used to be printed over the curve.
        let history: Vec<GenerationStats> = (1..=30)
            .map(|generation| stats(generation, 0.01 * f64::from(generation), 0, 1))
            .collect();
        let mut painter = Painter::new();
        draw_chart(&mut painter, chart_rect(), &history);
        let readout: Vec<&crate::painter::TextItem> = painter
            .text
            .iter()
            .filter(|item| item.content == "0.30")
            .collect();
        assert_eq!(readout.len(), 1, "the newest mean is printed once");
        let readout = readout[0];
        assert_eq!(readout.color, color::TEXT, "a reading, not more curve ink");
        assert!(
            readout.pos[1] <= chart_rect().y + PANEL_PAD + TITLE_H,
            "the reading stands in the title row, clear of the plot"
        );
        // The plot still carries the mean in its own ink.
        assert!(painter
            .triangles
            .iter()
            .any(|vertex| vertex.color == color::BEST.to_linear()));
    }

    #[test]
    fn the_chart_draws_the_spread_it_was_given() {
        // Triangles drawn in the band's ink, which only the ribbon and its
        // legend swatch use.
        let banded = |history: &[GenerationStats]| {
            let mut painter = Painter::new();
            draw_chart(&mut painter, chart_rect(), history);
            painter
                .triangles
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|triangle| triangle[0].color == color::BAND.to_linear())
                .count()
        };
        // Every Generation has cleared nothing while the tail has reached a
        // Wave: the ribbon is drawn along the whole run, column by column.
        let spread: Vec<GenerationStats> = (1..=20)
            .map(|generation| stats(generation, 0.01 * f64::from(generation), 0, 1))
            .collect();
        // A Population whose middle and tail are both at zero Waves has no
        // spread, and gets no ribbon: only the legend's own swatch is left.
        let flat: Vec<GenerationStats> = (1..=20)
            .map(|generation| stats(generation, 0.01 * f64::from(generation), 0, 0))
            .collect();
        let (spread_ink, flat_ink) = (banded(&spread), banded(&flat));
        assert!(
            spread_ink >= 20,
            "the ribbon is drawn, column by column ({spread_ink})"
        );
        assert!(
            flat_ink <= 2,
            "no spread, no ribbon: the legend's swatch only ({flat_ink})"
        );
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
