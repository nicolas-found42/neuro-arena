//! The sidebar's panels and the Arena's banner.
//!
//! Every panel is drawn in window pixels at 1:1 (ADR 0006): the Arena scales
//! with the window, the sidebar never does, so HUD text stays crisp and legible
//! at any window size. Nothing here measures text — the renderer shapes it and
//! resolves alignment — so panels size boxes from the monospace advance and let
//! the Painter place the glyphs.

use sim::config::gate::STAGNATION_LIMIT;
use sim::config::nn;
use sim::{Competence, CompetenceGate, GenerationStats, Genome, Network, NodeType};

use crate::painter::{Align, Painter, Rgba};
use crate::theme::{color, font, layout};
use crate::ui::{Controls, Hit, Layout, Rect, PANEL_PAD, TITLE_GAP, TITLE_H};

/// One line of body text.
const ROW: f32 = font::LINE_STEP;

/// The line the dominant numbers get: bigger than the body, short of a heading.
const HEADLINE: f32 = 24.0;

/// A character's width in the monospace face the text renderer shapes with.
const CHAR_ADVANCE: f32 = 0.6;

/// The white a Network node climbs toward as it fires.
const LIT: Rgba = Rgba::rgb(1.0, 1.0, 1.0);

/// The gap between the Network panel's three columns.
const COLUMN_GAP: f32 = 10.0;

/// Hidden nodes the Network panel draws before summarizing the rest.
const HIDDEN_MAX_DRAWN: usize = 12;

/// The tightest a hidden node's dot may be packed before it stops reading as one.
const HIDDEN_MIN_STEP: f32 = 8.0;

/// Room kept under the hidden dots for the "+N more" summary.
const HIDDEN_SUMMARY_H: f32 = 12.0;

/// The width of the live number's column in the HUD's gate table.
const GATE_VALUE_W: f32 = 52.0;

/// The five outputs, in `Network::output_ids` order — left, right, thrust, fire,
/// memory (`config::nn::OUTPUT_IDS`).
const OUTPUT_NAMES: [&str; 5] = ["left", "right", "thrust", "fire", "memory"];

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
    let body = frame(painter, rect, "Run");
    if body.w < 2.0 || body.h < 2.0 {
        return;
    }
    let gap = 12.0;
    let column_w = ((body.w - gap) * 0.5).max(0.0);
    let mut run = Column::new(Rect::new(body.x, body.y, column_w, body.h));
    let mut skill = Column::new(Rect::new(body.x + column_w + gap, body.y, column_w, body.h));
    let live_right = skill.right - GATE_VALUE_W;

    // Left: the run being watched.
    run.pair(painter, ROW, "Seed", info.seed.to_string(), color::TEXT);
    run.headline(
        painter,
        HEADLINE,
        "Gen",
        info.generation.to_string(),
        color::TEXT,
    );
    if info.watching {
        run.pair(
            painter,
            ROW,
            "Member",
            "watching".to_string(),
            color::ACCENT,
        );
    } else {
        run.pair(
            painter,
            ROW,
            "Member",
            format!(
                "{}/{}",
                (info.member + 1).min(info.population),
                info.population
            ),
            color::TEXT,
        );
    }
    run.pair(
        painter,
        ROW,
        "Species",
        info.species.to_string(),
        color::TEXT,
    );
    run.pair(
        painter,
        ROW,
        "Episode",
        format!("{:.1} s", info.episode_time),
        color::TEXT,
    );
    run.pair(
        painter,
        ROW,
        "Wave",
        (info.wave + 1).to_string(),
        color::TEXT,
    );
    run.pair(
        painter,
        ROW,
        "Speed",
        Controls::speed_label(info.speed),
        color::TEXT,
    );
    run.pair(
        painter,
        ROW,
        "Rate",
        Controls::speed_label(info.measured_rate),
        color::TEXT,
    );

    // Right: the score. Shaped Fitness first, then the raw Gate — this ship's
    // numbers beside the run's banked best.
    skill.headline(
        painter,
        HEADLINE,
        "Fitness",
        number(info.fitness),
        color::BEST,
    );
    if let Some(y) = skill.next(ROW) {
        let text_y = line_y(y, ROW, font::SMALL);
        painter.text([skill.x, text_y], font::SMALL, color::PANEL_TITLE, "GATE");
        painter.text_aligned(
            [live_right, text_y],
            font::SMALL,
            color::TEXT_DIM,
            Align::Right,
            "live",
        );
        painter.text_aligned(
            [skill.right, text_y],
            font::SMALL,
            color::TEXT_DIM,
            Align::Right,
            "best",
        );
    }
    gate_row(
        painter,
        &mut skill,
        "alive",
        format!("{:.1}", info.competence.alive_time),
        format!("{:.1}", info.gate.best_alive_time),
        live_right,
    );
    gate_row(
        painter,
        &mut skill,
        "wave",
        (info.competence.wave + 1).to_string(),
        (info.gate.best_wave + 1).to_string(),
        live_right,
    );
    gate_row(
        painter,
        &mut skill,
        "rocks",
        format!("{:.0}", info.competence.rocks),
        format!("{:.0}", info.gate.best_rocks),
        live_right,
    );
    if let Some(y) = skill.next(ROW) {
        let text_y = line_y(y, ROW, font::BODY);
        painter.text([skill.x, text_y], font::BODY, color::TEXT_DIM, "stagnant");
        painter.text_aligned(
            [live_right, text_y],
            font::BODY,
            color::TEXT,
            Align::Right,
            format!("{}/{}", info.gate.run_of_stagnant, STAGNATION_LIMIT),
        );
    }
    if let Some(y) = skill.next(ROW) {
        let text_y = line_y(y, ROW, font::SMALL);
        let (verdict, colour) = match info.gate.tripped_generation {
            Some(generation) => (format!("tripped gen {generation}"), color::WARN),
            None if info.gate.tripped => ("tripped".to_string(), color::WARN),
            None => ("clear".to_string(), color::TEXT_DIM),
        };
        painter.text([skill.x, text_y], font::SMALL, color::TEXT_DIM, "gate");
        painter.text_aligned(
            [skill.right, text_y],
            font::SMALL,
            colour,
            Align::Right,
            verdict,
        );
    }
}

/// One row of the Gate table: a label, this ship's number, the run's best.
fn gate_row(
    painter: &mut Painter,
    column: &mut Column,
    label: &str,
    live: String,
    best: String,
    live_right: f32,
) {
    let Some(y) = column.next(ROW) else {
        return;
    };
    let text_y = line_y(y, ROW, font::BODY);
    painter.text([column.x, text_y], font::BODY, color::TEXT_DIM, label);
    painter.text_aligned([live_right, text_y], font::BODY, color::TEXT, Align::Right, live);
    painter.text_aligned(
        [column.right, text_y],
        font::BODY,
        color::TEXT,
        Align::Right,
        best,
    );
}

/// Fitness over Generations: best and mean, growing with the run.
///
/// The x axis is the run's own history, never a fixed window, and the y axis
/// starts at zero so two Generations and five hundred read the same way.
pub fn draw_chart(painter: &mut Painter, rect: Rect, history: &[GenerationStats]) {
    let body = frame(painter, rect, "Fitness");
    if body.w < 40.0 || body.h < 20.0 {
        return;
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
    for fraction in [0.25, 0.5, 0.75] {
        let y = plot.bottom() - plot.h * fraction;
        painter.line([plot.x, y], [plot.right(), y], color::PANEL_BORDER.alpha(0.35));
    }
    painter.rect_outline(
        plot.x,
        plot.y,
        plot.w,
        plot.h,
        color::PANEL_BORDER.alpha(0.6),
    );

    if history.is_empty() {
        painter.text_aligned(
            [plot.center()[0], line_y(plot.y, plot.h, font::SMALL)],
            font::SMALL,
            color::TEXT_DIM,
            Align::Center,
            "no Generations yet",
        );
        return;
    }

    let count = history.len();
    let highest = history
        .iter()
        .fold(0.0_f64, |top, stats| top.max(stats.best).max(stats.mean));
    // An all-zero history would divide by nothing: keep the axis at 1 and draw
    // the lines along the floor.
    let top = if highest > 0.0 { highest } else { 1.0 };
    let newest_index = count - 1;
    let x_at = |index: usize| -> f32 {
        if count == 1 {
            plot.right()
        } else {
            plot.x + plot.w * (index as f32 / newest_index as f32)
        }
    };
    let y_at = |value: f64| -> f32 { plot.bottom() - plot.h * (value / top).clamp(0.0, 1.0) as f32 };

    // A long run has more Generations than the plot has pixels; sample the
    // history at no more than one point per pixel column, newest included.
    let stride = ((count as f32) / plot.w.max(1.0)).ceil().max(1.0) as usize;
    let sampled = count.div_ceil(stride);
    let mut best_points: Vec<[f32; 2]> = Vec::with_capacity(sampled + 1);
    let mut mean_points: Vec<[f32; 2]> = Vec::with_capacity(sampled + 1);
    let mut index = 0;
    while index < count {
        let stats = history[index];
        let x = x_at(index);
        best_points.push([x, y_at(stats.best)]);
        mean_points.push([x, y_at(stats.mean)]);
        index += stride;
    }
    if newest_index % stride != 0 {
        let stats = history[newest_index];
        let x = x_at(newest_index);
        best_points.push([x, y_at(stats.best)]);
        mean_points.push([x, y_at(stats.mean)]);
    }

    if best_points.len() < 2 {
        painter.circle(best_points[0], 2.5, color::BEST, 12);
        painter.circle(mean_points[0], 2.5, color::MEAN, 12);
    } else {
        painter.polyline(&best_points, color::BEST, false);
        painter.polyline(&mean_points, color::MEAN, false);
    }

    // The largest value on the axis, and the newest best at the edge it lands on.
    if highest > 0.0 {
        painter.text_aligned(
            [plot.x - 4.0, plot.y],
            font::SMALL,
            color::TEXT_DIM,
            Align::Right,
            number(highest),
        );
    }
    let newest_y = y_at(history[newest_index].best);
    let newest_label_y = if newest_y - plot.y > font::SMALL + 6.0 {
        newest_y - font::SMALL - 5.0
    } else {
        newest_y + 4.0
    }
    .clamp(plot.y, (plot.bottom() - font::SMALL).max(plot.y));
    painter.text_aligned(
        [plot.right() - 3.0, newest_label_y],
        font::SMALL,
        color::BEST,
        Align::Right,
        number(history[newest_index].best),
    );
    painter.circle([x_at(newest_index) - 1.0, newest_y], 2.0, color::BEST, 10);

    // The legend, in the corner the newest value does not claim.
    let legend_y = plot.y + 3.0;
    painter.line(
        [plot.x + 6.0, legend_y + 5.0],
        [plot.x + 18.0, legend_y + 5.0],
        color::BEST,
    );
    painter.text(
        [plot.x + 22.0, legend_y],
        font::SMALL,
        color::BEST,
        "best",
    );
    painter.line(
        [plot.x + 52.0, legend_y + 5.0],
        [plot.x + 64.0, legend_y + 5.0],
        color::MEAN,
    );
    painter.text(
        [plot.x + 68.0, legend_y],
        font::SMALL,
        color::MEAN,
        "mean",
    );

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
/// screen is the brain that is flying, never a stale one.
pub fn draw_network(painter: &mut Painter, rect: Rect, genome: &Genome, network: &Network) {
    let body = frame(painter, rect, "Network");
    if body.w < 40.0 || body.h < 24.0 {
        return;
    }
    let caption_h = (font::SMALL + 3.0).min(body.h * 0.3);
    let columns = Columns::new(body, caption_h, network, genome.hidden_count());
    let input_count = network
        .nodes()
        .iter()
        .filter(|(_, kind)| *kind == NodeType::Input)
        .count();

    // Edges first, so the nodes sit on top of the hairball.
    for (from, to, weight) in network.enabled_connections() {
        let (Some(a), Some(b)) = (
            node_anchor(from, network, &columns),
            node_anchor(to, network, &columns),
        ) else {
            continue;
        };
        let colour = if weight.abs() < 0.25 {
            color::NODE_EDGE.alpha(0.25)
        } else {
            color::NODE_EDGE
        };
        let thickness = 1.0 + 2.0 * (weight.abs() / 4.0).min(1.0) as f32;
        thick_line(painter, a, b, thickness, colour);
    }

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
        painter.circle(
            columns.input_dot(rank),
            input_radius,
            color::NODE_INPUT.mix(LIT, lit),
            8,
        );
    }

    // Column 2: the hidden nodes the Genome grew, newest draws first.
    let hidden_radius = (columns.hidden_step * 0.32).clamp(2.0, 4.0);
    let mut rank = 0;
    for (id, _) in network
        .nodes()
        .iter()
        .filter(|(_, kind)| *kind == NodeType::Hidden)
    {
        if rank >= columns.shown_hidden {
            break;
        }
        let lit = network.activation(*id).abs().clamp(0.0, 1.0) as f32;
        painter.circle(
            columns.hidden_dot(rank),
            hidden_radius,
            color::NODE_HIDDEN.mix(LIT, lit),
            10,
        );
        rank += 1;
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
        painter.rect_outline(
            bar.x,
            bar.y,
            bar.w,
            bar.h,
            color::PANEL_BORDER.alpha(0.5),
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
        let input_w = (body.w * 0.08).min(24.0);
        let output_w = (body.w * 0.36).clamp(60.0, 118.0).min(body.w);
        let hidden_w = (body.w - input_w - output_w - COLUMN_GAP * 2.0).max(0.0);
        let inputs = Rect::new(body.x, body.y, input_w, height);
        let hidden = Rect::new(inputs.right() + COLUMN_GAP, body.y, hidden_w, height);
        let outputs = Rect::new(hidden.right() + COLUMN_GAP, body.y, output_w, height);

        // 21 dots at no more than 4 px pitch, and never taller than the column:
        // a squeezed panel packs them tighter rather than spilling into the
        // caption lane below.
        let input_step = (height / nn::INPUTS as f32).clamp(0.0, 4.0);
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
            input_origin: [inputs.x + 6.0, inputs.y + ((height - input_extent) * 0.5).max(0.0)],
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
pub fn draw_controls(painter: &mut Painter, layout: &Layout, controls: &Controls, hot: Option<Hit>) {
    frame(painter, layout.controls, "Controls");
    let pressed = |hit: Hit| hot == Some(hit);

    button(
        painter,
        layout.pause,
        if controls.paused { "Resume" } else { "Pause" },
        if controls.paused { State::On } else { State::Off },
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
        if controls.watching { State::On } else { State::Dim },
        pressed(Hit::Evolve),
    );
    button(painter, layout.restart, "Restart", State::Off, pressed(Hit::Restart));
    button(painter, layout.new_seed, "New seed", State::Off, pressed(Hit::NewSeed));
    button(painter, layout.save, "Save", State::Off, pressed(Hit::Save));
    button(painter, layout.load, "Load", State::Off, pressed(Hit::Load));

    // The seed field: `seed_text` is the live seed while it is not being edited,
    // and the digits being typed while it is.
    let field = layout.seed_field;
    if field.w > 8.0 && field.h > 8.0 {
        let border = if controls.seed_editing {
            color::ACCENT
        } else {
            color::PANEL_BORDER
        };
        painter.panel(field.x, field.y, field.w, field.h, color::FIELD, Some(border));
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

    // The speed slider: the track it can be grabbed anywhere on, the knob, and
    // what the setting reads as.
    let track = layout.speed_slider;
    if track.w > 8.0 && track.h > 4.0 {
        let bar_h = (track.h * 0.22).clamp(3.0, 5.0);
        let bar = Rect::new(
            track.x,
            track.center()[1] - bar_h * 0.5,
            track.w,
            bar_h,
        );
        painter.rect(bar.x, bar.y, bar.w, bar.h, color::FIELD);
        let t = Controls::slider_from_speed(controls.speed).clamp(0.0, 1.0);
        if t > 0.0 {
            painter.rect(bar.x, bar.y, bar.w * t, bar.h, color::ACCENT.alpha(0.45));
        }
        painter.rect_outline(bar.x, bar.y, bar.w, bar.h, color::PANEL_BORDER.alpha(0.8));
        painter.circle(
            [bar.x + bar.w * t, bar.center()[1]],
            (bar.h * 0.5 + 2.0).max(3.0),
            color::ACCENT,
            16,
        );
        let unbounded = Controls::is_unbounded(controls.speed);
        painter.text_aligned(
            [layout.controls.right() - PANEL_PAD, line_y(track.y, track.h, font::BODY)],
            font::BODY,
            if unbounded { color::ACCENT } else { color::TEXT },
            Align::Right,
            Controls::speed_label(controls.speed),
        );
    }
}

/// The status line under the strip: what the app is doing, or what went wrong.
pub fn draw_status(painter: &mut Painter, rect: Rect, status: &str, is_error: bool) {
    painter.panel(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        color::PANEL_BG,
        Some(color::PANEL_BORDER),
    );
    let inner = rect.inset(PANEL_PAD);
    if inner.w <= 0.0 || inner.h < font::SMALL {
        return;
    }
    let colour = if is_error { color::WARN } else { color::TEXT_DIM };
    let step = font::SMALL + 2.0;
    for (index, line) in wrap(status, inner.w, font::SMALL, 2).into_iter().enumerate() {
        let y = inner.y + step * index as f32;
        if y + font::SMALL > inner.bottom() {
            break;
        }
        painter.text([inner.x, y], font::SMALL, colour, line);
    }
}

/// A centred banner over the Arena: Episode transitions, and anything that went
/// wrong badly enough to say so in the middle of the screen.
pub fn draw_banner(painter: &mut Painter, arena: Rect, lines: &[(String, Rgba)]) {
    let max_w = (arena.w - 2.0 * layout::MARGIN).max(0.0);
    let max_h = (arena.h - 2.0 * layout::MARGIN).max(0.0);
    if lines.is_empty() || max_w < 40.0 || max_h < 24.0 {
        return;
    }
    let mut text_w = 0.0_f32;
    let mut box_h = PANEL_PAD * 2.0;
    for (index, (text, _)) in lines.iter().enumerate() {
        let size = if index == 0 { font::BIG } else { font::BODY };
        text_w = text_w.max(advance(text.chars().count(), size));
        box_h += size + if index == 0 { 6.0 } else { 3.0 };
    }
    let width = (text_w + PANEL_PAD * 4.0).min(max_w);
    let height = box_h.min(max_h);
    let x = arena.x + (arena.w - width) * 0.5;
    let y = arena.y + (arena.h - height) * 0.5;
    painter.panel(
        x,
        y,
        width,
        height,
        color::PANEL_BG.alpha(0.82),
        Some(color::PANEL_BORDER.alpha(0.7)),
    );
    let mut text_y = y + PANEL_PAD;
    for (index, (text, colour)) in lines.iter().enumerate() {
        let size = if index == 0 { font::BIG } else { font::BODY };
        if text_y + size > y + height {
            break;
        }
        painter.text_aligned(
            [x + width * 0.5, text_y],
            size,
            *colour,
            Align::Center,
            text.clone(),
        );
        text_y += size + if index == 0 { 6.0 } else { 3.0 };
    }
}

/// The panel body: `PANEL_BG` inside a `PANEL_BORDER` outline, with a
/// `PANEL_TITLE` heading. Returns the body's inner rect, below the heading.
fn frame(painter: &mut Painter, rect: Rect, title: &str) -> Rect {
    painter.panel(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        color::PANEL_BG,
        Some(color::PANEL_BORDER),
    );
    let inner = rect.inset(PANEL_PAD);
    if inner.w <= 0.0 || inner.h <= 0.0 {
        return inner;
    }
    let mut top = inner.y;
    if inner.h >= TITLE_H + TITLE_GAP {
        painter.text(
            [inner.x, top + (TITLE_H - font::HEADING) * 0.5],
            font::HEADING,
            color::PANEL_TITLE,
            title,
        );
        top += TITLE_H + TITLE_GAP;
    }
    Rect::new(inner.x, top, inner.w, (inner.bottom() - top).max(0.0))
}

/// A run of lines down a panel body. Lines that no longer fit are dropped, so a
/// panel squeezed by a short window never draws outside itself.
struct Column {
    x: f32,
    right: f32,
    y: f32,
    limit: f32,
}

impl Column {
    fn new(body: Rect) -> Self {
        Self {
            x: body.x,
            right: body.right(),
            y: body.y,
            limit: body.bottom(),
        }
    }

    /// The next line's top edge, or `None` once the body is full.
    fn next(&mut self, height: f32) -> Option<f32> {
        if height <= 0.0 || self.y + height > self.limit {
            return None;
        }
        let y = self.y;
        self.y += height;
        Some(y)
    }

    /// A dim label at the left edge and a value right-aligned at the right.
    fn pair(
        &mut self,
        painter: &mut Painter,
        height: f32,
        label: &str,
        value: String,
        colour: Rgba,
    ) {
        let Some(y) = self.next(height) else {
            return;
        };
        let text_y = line_y(y, height, font::BODY);
        painter.text([self.x, text_y], font::BODY, color::TEXT_DIM, label);
        painter.text_aligned([self.right, text_y], font::BODY, colour, Align::Right, value);
    }

    /// The same line one size up: the number a glance should catch.
    fn headline(
        &mut self,
        painter: &mut Painter,
        height: f32,
        label: &str,
        value: String,
        colour: Rgba,
    ) {
        let Some(y) = self.next(height) else {
            return;
        };
        let text_y = line_y(y, height, font::BIG);
        painter.text([self.x, text_y], font::SMALL, color::TEXT_DIM, label);
        painter.text_aligned([self.right, text_y], font::BIG, colour, Align::Right, value);
    }
}

/// How a button reads: plain, toggled on, or not available.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Off,
    On,
    Dim,
}

/// One button: a plain rect that lights under the pointer and fills in when it
/// is toggled on.
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
        color::TEXT_DIM
    };
    painter.panel(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        fill,
        Some(color::PANEL_BORDER.alpha(0.7)),
    );
    painter.text_aligned(
        [rect.center()[0], line_y(rect.y, rect.h, font::BODY)],
        font::BODY,
        text,
        Align::Center,
        label,
    );
}

/// A line of a given thickness, as a quad: the Painter's lines carry no width,
/// and a Network's weights are worth seeing.
fn thick_line(painter: &mut Painter, a: [f32; 2], b: [f32; 2], thickness: f32, colour: Rgba) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = (dx * dx + dy * dy).sqrt();
    if length < 0.01 || thickness <= 0.0 {
        return;
    }
    let half = thickness * 0.5;
    let (nx, ny) = (-dy / length * half, dx / length * half);
    painter.polygon(
        &[
            [a[0] + nx, a[1] + ny],
            [b[0] + nx, b[1] + ny],
            [b[0] - nx, b[1] - ny],
            [a[0] - nx, a[1] - ny],
        ],
        colour,
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
    fn wrapping_keeps_the_status_to_the_strip_and_its_two_lines() {
        let width = advance(30, font::SMALL);
        let lines = wrap("a long status", width, font::SMALL, 1);
        assert_eq!(lines, vec!["a long status".to_string()]);
        let lines = wrap("aaaa bbbb cccc dddd", advance(9, font::SMALL), font::SMALL, 2);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines, vec!["aaaa bbbb".to_string(), "cccc dddd".to_string()]);
    }

    #[test]
    fn numbers_get_shorter_as_they_get_bigger() {
        assert_eq!(number(1.234), "1.23");
        assert_eq!(number(12.34), "12.3");
        assert_eq!(number(1234.6), "1235");
        assert_eq!(number(f64::NAN), "—");
    }
}
