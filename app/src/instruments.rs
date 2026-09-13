//! Read-only flight instruments: the bench copy of what the Ship wears.
//!
//! Everything in this strip is the same measurement the Arena draws on the hull
//! — the polar dial is the nine Sensor Rays the corona shows, the motor column
//! is the four requests the intent ring shows — read at rest, with numbers on
//! it. It shares the Arena's colour ramp on purpose: a reading that is amber
//! out there is amber in here.
//!
//! No interpolation or decorative activity is called telemetry. Every value is
//! taken straight from the World's stored Sensorium and the live Network.

use crate::{
    painter::{Align, Painter, Rgba},
    theme::{color, font, light},
    ui::Rect,
    vector,
};
use sim::{
    config::{nn, sensors},
    World,
};
use std::f32::consts::{FRAC_PI_2, TAU};

/// The dial's radius, and how far a fully closed ray pulls its spoke in.
const DIAL_RADIUS: f32 = 43.0;
const DIAL_REACH: f32 = 0.88;
/// The proximity a reading has to pass before it runs warm, matching the
/// corona's ramp on the hull.
const WARM_AT: f32 = 0.6;

/// A meter's groove and its fill, so a bar reads as something cut into the
/// housing with light in it rather than as two stacked rectangles.
const METER_H: f32 = 4.0;

/// The colour of a proximity reading: cyan at the edge of range, running to
/// amber and then to the Arena's warning colour as it closes. The same ramp the
/// corona uses, so the dial and the hull agree at a glance.
pub fn proximity_ink(value: f32) -> Rgba {
    color::ACCENT
        .mix(color::ENERGY, value)
        .mix(color::WARN, (value - WARM_AT).max(0.0) * 2.0)
}

/// The nine sampled ray distances in a Ship-relative polar instrument.
pub fn sensorium(p: &mut Painter, panel: Rect, world: Option<&World>, motion: bool) {
    crate::panels::lit_surface(p, panel, color::PANEL_BG, Some(color::PANEL_BORDER));
    let Some(w) = world else {
        return;
    };
    let compact = panel.w < 740.0;
    let radar_w = if compact { 138.0 } else { 190.0 };
    let center = [panel.x + radar_w * 0.5, panel.y + 84.0];
    heading(p, [panel.x + 12.0, panel.y + 10.0], "01 / PERCEPTION");

    // The dial's face: two rings and a cross, drawn before anything is on it.
    for (r, alpha) in [(DIAL_RADIUS * 0.5, 0.55), (DIAL_RADIUS, 0.9)] {
        p.path(
            &vector::arc(center, r, 0.0, TAU),
            0.7,
            color::PANEL_BORDER.alpha(alpha),
        );
    }
    for (dx, dy) in [(1.0, 0.0), (0.0, 1.0)] {
        p.line(
            [
                center[0] - (DIAL_RADIUS + 5.0) * dx,
                center[1] - (DIAL_RADIUS + 5.0) * dy,
            ],
            [
                center[0] + (DIAL_RADIUS + 5.0) * dx,
                center[1] + (DIAL_RADIUS + 5.0) * dy,
            ],
            color::PANEL_BORDER.alpha(0.4),
        );
    }
    if w.time > 0.0 {
        for (i, offset) in sensors::RAY_OFFSETS_DEG.iter().enumerate() {
            let value = w.agent.inputs[i].clamp(0.0, 1.0) as f32;
            let angle = (*offset as f32).to_radians() - FRAC_PI_2;
            let length = DIAL_RADIUS * (1.0 - value * DIAL_REACH);
            let tip = [
                center[0] + angle.cos() * length,
                center[1] + angle.sin() * length,
            ];
            if value > 0.0 {
                let ink = proximity_ink(value);
                p.stroke(center, tip, 1.4, ink.alpha(0.35 + value * 0.4));
                p.set_gain(light::CORONA);
                p.luminous_circle(tip, 2.6, ink, 12);
                p.set_gain(1.0);
            } else {
                // An empty slot is still a slot: a tick at the rim says the ray
                // is sensing and found nothing, which is not the same as absent.
                let rim = [
                    center[0] + angle.cos() * DIAL_RADIUS,
                    center[1] + angle.sin() * DIAL_RADIUS,
                ];
                p.stroke(
                    [
                        center[0] + angle.cos() * (DIAL_RADIUS - 4.0),
                        center[1] + angle.sin() * (DIAL_RADIUS - 4.0),
                    ],
                    rim,
                    1.0,
                    color::ACCENT.alpha(0.28),
                );
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
        font::MICRO,
        color::TEXT_FAINT,
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
        divider(p, tx, panel.y + 12.0, panel.bottom() - 12.0);
        heading(p, [tx + 16.0, panel.y + 10.0], "02 / THREAT TELEMETRY");
        for (i, (label, value, signed)) in [
            ("PROXIMITY", w.agent.inputs[13], false),
            ("CLOSING - / + AWAY", w.agent.inputs[14], true),
            ("PRESSURE", w.agent.inputs[19], false),
        ]
        .iter()
        .enumerate()
        {
            let y = panel.y + 39.0 + i as f32 * 31.0;
            let bar = Rect::new(tx + 16.0, y + 15.0, threat_w - 34.0, METER_H);
            p.text([bar.x, y], font::MICRO, color::TEXT_DIM, *label);
            p.text_aligned(
                [bar.right(), y],
                font::MICRO,
                color::TEXT,
                Align::Right,
                format!("{value:+.2}"),
            );
            let magnitude = value.abs().min(1.0) as f32;
            meter(p, bar, *value as f32, *signed, proximity_ink(magnitude));
        }
    }

    let ax = tx + threat_w + 16.0;
    let width = panel.right() - ax - 14.0;
    if width < 180.0 {
        return;
    }
    heading(p, [ax, panel.y + 10.0], "03 / MOTOR REQUESTS");
    if let Some(net) = w.agent.network() {
        let cell = width / 4.0;
        for (i, name) in ["LEFT", "RIGHT", "THRUST", "FIRE"].iter().enumerate() {
            let value = net.activation(net.output_ids()[i]) as f32;
            let on = value > nn::ACTION_THRESHOLD as f32 && w.time > 0.0;
            let x = ax + i as f32 * cell;
            let r = Rect::new(x, panel.y + 34.0, cell - 6.0, 62.0);
            crate::panels::lit_surface(
                p,
                r,
                if on { color::BUTTON_ON } else { color::FIELD },
                Some(if on {
                    color::ACCENT.alpha(0.45)
                } else {
                    color::PANEL_BORDER.alpha(0.6)
                }),
            );
            p.text(
                [x + 7.0, r.y + 7.0],
                font::MICRO,
                if on { color::ACCENT } else { color::TEXT_DIM },
                *name,
            );
            p.text(
                [x + 7.0, r.y + 24.0],
                if compact { 15.0 } else { 19.0 },
                if on { color::TEXT } else { color::TEXT_DIM },
                format!("{value:+.2}"),
            );
            let bar = Rect::new(x + 7.0, r.bottom() - 8.0, r.w - 14.0, METER_H);
            meter(
                p,
                bar,
                value,
                true,
                if on { color::ENERGY } else { color::ACCENT },
            );
            // Threshold tick is spatial as well as color-coded.
            let threshold = bar.x + bar.w * (nn::ACTION_THRESHOLD as f32 + 1.0) * 0.5;
            p.line(
                [threshold, bar.y - 2.5],
                [threshold, bar.bottom() + 2.5],
                color::TEXT_DIM,
            );
        }
    }
    p.text(
        [ax, panel.y + 106.0],
        font::MICRO,
        color::TEXT_FAINT,
        "REQUEST WHEN > +0.50 / MEMORY IN NETWORK",
    );
    p.text(
        [ax, panel.bottom() - 18.0],
        font::MICRO,
        color::TEXT_FAINT,
        if motion {
            "M  MOTION ON    SPACE  PAUSE    R  RAYS"
        } else {
            "M  REDUCED MOTION    SPACE  PAUSE"
        },
    );
}

/// A section heading in the strip: the accent tick the sidebar panels wear, at
/// the size this strip is set in.
fn heading(p: &mut Painter, at: [f32; 2], text: &str) {
    p.rect(at[0] - 8.0, at[1], 2.0, font::HEADING - 1.0, color::ACCENT);
    p.text(at, font::SMALL, color::PANEL_TITLE, text);
}

/// A hairline between two sections, fading out at both ends so it separates
/// without boxing anything in.
fn divider(p: &mut Painter, x: f32, top: f32, bottom: f32) {
    let middle = (top + bottom) * 0.5;
    for (a, b, from, to) in [(top, middle, 0.0, 1.0), (middle, bottom, 1.0, 0.0)] {
        p.gradient_polygon(
            &[[x, a], [x + 1.0, a], [x + 1.0, b], [x, b]],
            &[
                color::PANEL_BORDER.alpha(from),
                color::PANEL_BORDER.alpha(from),
                color::PANEL_BORDER.alpha(to),
                color::PANEL_BORDER.alpha(to),
            ],
        );
    }
}

/// One bar: a groove cut into the housing, with the reading lit inside it. A
/// signed meter fills out from its middle and marks the zero it counts from.
fn meter(p: &mut Painter, r: Rect, value: f32, signed: bool, ink: Rgba) {
    p.rect(r.x, r.y, r.w, r.h, color::FIELD);
    p.rect(r.x, r.y, r.w, 1.0, color::VIGNETTE.alpha(0.8));
    p.rect_outline(r.x, r.y, r.w, r.h, color::PANEL_BORDER.alpha(0.7));
    let value = value.clamp(if signed { -1.0 } else { 0.0 }, 1.0);
    let zero = if signed { r.x + r.w * 0.5 } else { r.x };
    let end = zero + value * r.w * if signed { 0.5 } else { 1.0 };
    let (from, to) = (zero.min(end), zero.max(end));
    if to - from > 0.5 {
        p.rect(from, r.y + 1.0, to - from, r.h - 2.0, ink);
    }
    if signed {
        p.line([zero, r.y - 2.0], [zero, r.bottom() + 2.0], color::TEXT_DIM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reading_runs_from_perception_to_warning_as_it_closes() {
        let far = proximity_ink(0.0);
        let near = proximity_ink(0.5);
        let touching = proximity_ink(1.0);
        // Cyan at range: nothing is happening yet.
        assert_eq!(far.to_array(), color::ACCENT.to_array());
        // Warm in the middle of the range, and past the warm point it keeps
        // going toward the colour the Arena uses for danger.
        assert!(near.r > far.r && near.b < far.b);
        assert!(touching.g < near.g && touching.b < near.b);
    }

    #[test]
    fn a_meter_fills_from_the_zero_it_counts_from() {
        let rect = Rect::new(0.0, 0.0, 100.0, METER_H);
        // The fill is the only thing in the meter whose bottom edge sits one
        // pixel inside the groove, so its own corners identify it.
        let fill_extent = |p: &Painter| {
            let xs: Vec<f32> = p
                .triangles
                .iter()
                .filter(|v| v.pos[1] == METER_H - 1.0)
                .map(|v| v.pos[0])
                .collect();
            (
                xs.iter().copied().fold(f32::MAX, f32::min),
                xs.iter().copied().fold(f32::MIN, f32::max),
            )
        };

        let mut p = Painter::new();
        meter(&mut p, rect, -1.0, true, color::ACCENT);
        // A signed meter at its negative end fills the left half and nothing
        // right of the middle.
        let (from, to) = fill_extent(&p);
        assert_eq!((from, to), (0.0, 50.0), "a negative reading crossed zero");

        p.clear();
        meter(&mut p, rect, 0.5, true, color::ACCENT);
        let (from, to) = fill_extent(&p);
        assert_eq!((from, to), (50.0, 75.0), "a signed reading left its zero");

        p.clear();
        meter(&mut p, rect, 1.0, false, color::ACCENT);
        let (from, to) = fill_extent(&p);
        assert_eq!((from, to), (0.0, 100.0), "an unsigned reading did not fill");

        // A reading of nothing draws no fill at all, rather than a sliver.
        p.clear();
        meter(&mut p, rect, 0.0, true, color::ACCENT);
        assert!(p.triangles.iter().all(|v| v.pos[1] != METER_H - 1.0));
    }
}
