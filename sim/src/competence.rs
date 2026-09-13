//! The Competence Gate: unshaped skill, banked beside Fitness.
//!
//! Fitness is shaped to steer search; the Gate is what tells the owner whether
//! the Population is actually getting better. It banks the best alive time,
//! Wave and Asteroid points a Generation reached, and judges stagnation on the
//! headline pair — the Population's mean Waves, tie-broken by median alive
//! time — over a rolling window of Generations (ADR 0003, ADR 0007, ADR 0008).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::config::gate;

/// Raw skill metrics for one Episode.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Competence {
    pub alive_time: f64,
    pub wave: u32,
    /// The save-file key is spec-pinned; the glossary bans the synonym in code.
    #[serde(rename = "rocks")]
    pub asteroids: f64,
}

/// What one Generation contributes to the Gate.
#[derive(Clone, Copy, Debug)]
pub struct EpisodeRecord {
    pub member: usize,
    pub competence: Competence,
}

/// The Population's Waves for one Generation: the headline number and the
/// shape of the distribution around it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaveStats {
    /// The headline Competence number: the mean Waves of the Population. The
    /// mean, not the median, because most Ships survive the wave clock without
    /// clearing a field — the median sits on that camping mode and cannot move
    /// until the whole Population does.
    pub mean: f64,
    pub median: u32,
    pub p90: u32,
    /// Share of the Population that has cleared at least the first Wave.
    pub clearing_share: f64,
}

/// What the Gate made of one Generation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateVerdict {
    pub best_alive_time: f64,
    pub best_wave: u32,
    pub best_asteroids: f64,
    pub median_alive_time: f64,
    /// The headline Competence number: the Population's mean Waves.
    pub mean_wave: f64,
    /// The Population's median Waves, reported beside the headline.
    pub median_wave: u32,
    /// The Population's 90th-percentile Waves, reported beside the headline.
    pub p90_wave: u32,
    /// Share of the Population that has cleared at least the first Wave.
    pub clearing_share: f64,
    /// The tie-broken pair failed to beat the oldest pair in the window.
    pub stagnant: bool,
    /// Consecutive stagnant Generations.
    pub run_of_stagnant: u32,
    /// The Gate tripped on this Generation.
    pub tripped_now: bool,
}

#[derive(Clone, Debug)]
pub struct CompetenceGate {
    pub best_alive_time: f64,
    pub best_wave: u32,
    pub best_asteroids: f64,
    /// One entry per Generation: the tie-broken headline pair, oldest first.
    window: VecDeque<(f64, f64)>,
    pub run_of_stagnant: u32,
    pub tripped: bool,
    pub tripped_generation: Option<u32>,
}

impl Default for CompetenceGate {
    fn default() -> Self {
        Self {
            best_alive_time: 0.0,
            best_wave: 0,
            best_asteroids: 0.0,
            window: VecDeque::new(),
            run_of_stagnant: 0,
            tripped: false,
            tripped_generation: None,
        }
    }
}

impl CompetenceGate {
    /// Fold one Generation's records into the Gate.
    ///
    /// The comparison is against the oldest pair in the window, which is the
    /// recent past and not the Generation itself: the Gate asks "is this
    /// Generation better than the worst of the recent past", not "better than
    /// the mean". A Generation with no history behind it is never stagnant.
    pub fn observe(&mut self, records: &[EpisodeRecord], generation: u32) -> GateVerdict {
        if records.is_empty() {
            return GateVerdict {
                best_alive_time: self.best_alive_time,
                best_wave: self.best_wave,
                best_asteroids: self.best_asteroids,
                median_alive_time: self.median_alive_time(),
                mean_wave: 0.0,
                median_wave: 0,
                p90_wave: 0,
                clearing_share: 0.0,
                stagnant: false,
                run_of_stagnant: self.run_of_stagnant,
                tripped_now: false,
            };
        }
        let mut alive_times: Vec<f64> = records.iter().map(|r| r.competence.alive_time).collect();
        let mut waves: Vec<u32> = records.iter().map(|r| r.competence.wave).collect();
        let best_alive_time = alive_times
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let best_wave = waves.iter().copied().max().unwrap_or(0);
        let best_asteroids = records
            .iter()
            .map(|r| r.competence.asteroids)
            .fold(f64::NEG_INFINITY, f64::max);
        alive_times.sort_by(f64::total_cmp);
        waves.sort_unstable();
        let median_alive_time = alive_times[alive_times.len() / 2];
        let median_wave = waves[waves.len() / 2];
        let p90_wave = percentile(&waves, 0.9);
        let mean_wave = waves.iter().map(|&wave| f64::from(wave)).sum::<f64>() / waves.len() as f64;
        let clearing_share =
            waves.iter().filter(|&&wave| wave > 0).count() as f64 / waves.len() as f64;

        self.best_alive_time = self.best_alive_time.max(best_alive_time);
        self.best_wave = self.best_wave.max(best_wave);
        self.best_asteroids = self.best_asteroids.max(best_asteroids);

        // Waves decide; alive time only breaks a tie at equal mean Waves (ADR 0008).
        // The tie-break earns its place in the camping regime, where the mean is
        // exactly zero because nobody has cleared a field yet.
        let stagnant = match self.window.front() {
            None => false,
            Some(&(floor_wave, floor_alive)) => {
                !(mean_wave > floor_wave
                    || (mean_wave == floor_wave
                        && median_alive_time > gate::STAGNATION_RATIO * floor_alive))
            }
        };
        self.window.push_back((mean_wave, median_alive_time));
        while self.window.len() > gate::MEDIAN_WINDOW {
            self.window.pop_front();
        }
        self.run_of_stagnant = if stagnant {
            self.run_of_stagnant + 1
        } else {
            0
        };
        let tripped_now = self.run_of_stagnant >= gate::STAGNATION_LIMIT && !self.tripped;
        if tripped_now {
            self.tripped = true;
            self.tripped_generation = Some(generation);
        }
        GateVerdict {
            best_alive_time,
            best_wave,
            best_asteroids,
            median_alive_time,
            mean_wave,
            median_wave,
            p90_wave,
            clearing_share,
            stagnant,
            run_of_stagnant: self.run_of_stagnant,
            tripped_now,
        }
    }

    pub fn median_alive_time(&self) -> f64 {
        let mut sorted: Vec<f64> = self.window.iter().map(|&(_, alive)| alive).collect();
        sorted.sort_by(f64::total_cmp);
        sorted.get(sorted.len() / 2).copied().unwrap_or(0.0)
    }
}

/// Nearest-rank percentile of an ascending slice: the smallest sample at or
/// above `fraction` of the samples. Waves are integers, so the rank rounds up.
fn percentile(sorted: &[u32], fraction: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (fraction * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}
