//! Observatory composition and read-only, bounded presentation history.
use crate::{
    painter::{Align, Painter},
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
            p.stroke(a, b, 1.0 + age * 1.8, color::SHIP.alpha(age * 0.24));
        }
        p.set_clip(None);
    }
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
    }
}

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
    p.display_text([x, region.y + 6.0], 34.0, color::TEXT, "NEUROARENA");
    p.text(
        [x, region.y + 44.0],
        font::SMALL,
        color::TEXT_DIM,
        "FLIGHT OBSERVATORY   /   EVOLVING INTELLIGENCE",
    );
    let state = if controls.paused {
        if controls.watching {
            "II PAUSED / REPLAY"
        } else {
            "II  PAUSED"
        }
    } else if controls.watching {
        "REPLAY"
    } else {
        "●  EVOLVING"
    };
    p.text_aligned(
        [region.right() - 20.0, region.y + 17.0],
        font::BODY,
        if controls.paused {
            color::SHIP_FLAME
        } else {
            color::ACCENT
        },
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
    let panel = Rect::new(
        region.x + 16.0,
        region.bottom() - 172.0,
        (region.w - 32.0).max(0.0),
        156.0,
    );
    crate::instruments::sensorium(p, panel, world, trails);
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
}
