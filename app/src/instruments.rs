//! Read-only flight instruments. No interpolation or decorative activity is called telemetry.
use crate::{
    painter::{Align, Painter},
    theme::{color, font},
    ui::Rect,
    vector,
};
use sim::{
    config::{nn, sensors},
    World,
};
use std::f32::consts::{FRAC_PI_2, TAU};

/// The nine sampled ray distances in a Ship-relative polar instrument.
pub fn sensorium(p: &mut Painter, panel: Rect, world: Option<&World>, motion: bool) {
    p.panel(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        color::PANEL_BG,
        Some(color::PANEL_BORDER),
    );
    let Some(w) = world else {
        return;
    };
    let compact = panel.w < 740.0;
    let radar_w = if compact { 138.0 } else { 190.0 };
    let center = [panel.x + radar_w * 0.5, panel.y + 84.0];
    let radius = 43.0;
    p.text(
        [panel.x + 12.0, panel.y + 10.0],
        font::SMALL,
        color::ACCENT,
        "01 / PERCEPTION",
    );
    for r in [radius * 0.5, radius] {
        p.path(&vector::arc(center, r, 0.0, TAU), 0.7, color::PANEL_BORDER);
    }
    p.line(
        [center[0] - radius - 5.0, center[1]],
        [center[0] + radius + 5.0, center[1]],
        color::PANEL_BORDER.alpha(0.4),
    );
    p.line(
        [center[0], center[1] - radius - 5.0],
        [center[0], center[1] + radius + 5.0],
        color::PANEL_BORDER.alpha(0.4),
    );
    if w.time > 0.0 {
        for (i, offset) in sensors::RAY_OFFSETS_DEG.iter().enumerate() {
            let value = w.agent.inputs[i].clamp(0.0, 1.0) as f32;
            let angle = (*offset as f32).to_radians() - FRAC_PI_2;
            let length = radius * (1.0 - value * 0.88);
            let tip = [
                center[0] + angle.cos() * length,
                center[1] + angle.sin() * length,
            ];
            let ink = if value > 0.0 {
                color::SHIP_FLAME
            } else {
                color::ACCENT.alpha(0.25)
            };
            p.stroke(
                center,
                tip,
                1.0,
                ink.alpha(if value > 0.0 { 0.5 } else { 0.12 }),
            );
            if value > 0.0 {
                p.circle(tip, 2.5, ink, 12);
            }
        }
    }
    p.triangle(
        [center[0], center[1] - 6.0],
        [center[0] - 4.0, center[1] + 4.0],
        [center[0] + 4.0, center[1] + 4.0],
        color::SHIP,
    );
    p.text_aligned(
        [center[0], panel.bottom() - 18.0],
        9.0,
        color::TEXT_DIM,
        Align::Center,
        if w.time > 0.0 {
            "9 RAYS / NOSE UP"
        } else {
            "AWAITING SAMPLE"
        },
    );

    let threat_w = if compact {
        0.0
    } else {
        (panel.w * 0.26).min(280.0)
    };
    let tx = panel.x + radar_w;
    if !compact {
        p.line(
            [tx, panel.y + 12.0],
            [tx, panel.bottom() - 12.0],
            color::PANEL_BORDER,
        );
        p.text(
            [tx + 16.0, panel.y + 10.0],
            font::SMALL,
            color::TEXT_DIM,
            "02 / THREAT TELEMETRY",
        );
        for (i, (label, value, signed)) in [
            ("PROXIMITY", w.agent.inputs[13], false),
            ("CLOSING - / + AWAY", w.agent.inputs[14], true),
            ("PRESSURE", w.agent.inputs[19], false),
        ]
        .iter()
        .enumerate()
        {
            let y = panel.y + 39.0 + i as f32 * 31.0;
            let bar = Rect::new(tx + 16.0, y + 15.0, threat_w - 34.0, 3.0);
            p.text([bar.x, y], 9.0, color::TEXT_DIM, *label);
            p.text_aligned(
                [bar.right(), y],
                9.0,
                color::TEXT,
                Align::Right,
                format!("{value:+.2}"),
            );
            meter(p, bar, *value as f32, *signed, color::SHIP_FLAME);
        }
    }

    let ax = tx + threat_w + 16.0;
    let width = panel.right() - ax - 14.0;
    if width < 180.0 {
        return;
    }
    p.text(
        [ax, panel.y + 10.0],
        font::SMALL,
        color::TEXT_DIM,
        "03 / MOTOR REQUESTS",
    );
    if let Some(net) = w.agent.network() {
        let cell = width / 4.0;
        for (i, name) in ["LEFT", "RIGHT", "THRUST", "FIRE"].iter().enumerate() {
            let value = net.activation(net.output_ids()[i]) as f32;
            let on = value > nn::ACTION_THRESHOLD as f32 && w.time > 0.0;
            let x = ax + i as f32 * cell;
            let r = Rect::new(x, panel.y + 34.0, cell - 6.0, 62.0);
            p.rect(
                r.x,
                r.y,
                r.w,
                r.h,
                if on {
                    color::BUTTON_ON.alpha(0.55)
                } else {
                    color::FIELD
                },
            );
            p.text(
                [x + 7.0, r.y + 7.0],
                9.0,
                if on { color::ACCENT } else { color::TEXT_DIM },
                *name,
            );
            p.text(
                [x + 7.0, r.y + 24.0],
                if compact { 15.0 } else { 19.0 },
                color::TEXT,
                format!("{value:+.2}"),
            );
            let bar = Rect::new(x + 7.0, r.bottom() - 7.0, r.w - 14.0, 3.0);
            meter(p, bar, value, true, color::ACCENT);
            // Threshold tick is spatial as well as color-coded.
            let threshold = bar.x + bar.w * 0.75;
            p.line(
                [threshold, bar.y - 2.0],
                [threshold, bar.bottom() + 2.0],
                color::TEXT_DIM,
            );
        }
    }
    p.text(
        [ax, panel.y + 106.0],
        9.0,
        color::TEXT_DIM,
        "REQUEST WHEN > +0.50 / MEMORY IN NETWORK",
    );
    p.text(
        [ax, panel.bottom() - 18.0],
        9.0,
        color::TEXT_DIM,
        if motion {
            "M  MOTION ON    SPACE  PAUSE    R  RAYS"
        } else {
            "M  REDUCED MOTION    SPACE  PAUSE"
        },
    );
}

fn meter(p: &mut Painter, r: Rect, value: f32, signed: bool, ink: crate::painter::Rgba) {
    p.rect(r.x, r.y, r.w, r.h, color::PANEL_BORDER);
    let value = value.clamp(if signed { -1.0 } else { 0.0 }, 1.0);
    let zero = if signed { r.x + r.w * 0.5 } else { r.x };
    let end = zero + value * r.w * if signed { 0.5 } else { 1.0 };
    p.rect(zero.min(end), r.y, (end - zero).abs(), r.h, ink);
    if signed {
        p.line([zero, r.y - 2.0], [zero, r.bottom() + 2.0], color::TEXT_DIM);
    }
}
