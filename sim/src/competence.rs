//! The Competence Gate: unshaped skill, banked beside Fitness.
//!
//! Fitness is shaped to steer search; the Gate is what tells the owner whether
//! the Population is actually getting better. It watches the best alive time,
//! Wave and Asteroid points a Generation reached, plus the median alive time
//! over a rolling window, and reports when a run has stopped making progress
//! (ADR 0003).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::config::gate;

/// Raw skill metrics for one Episode.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Competence {
    pub alive_time: f64,
    pub wave: u32,
    pub rocks: f64,
}

/// What one Generation contributes to the Gate.
#[derive(Clone, Copy, Debug)]
pub struct EpisodeRecord {
    pub member: usize,
    pub competence: Competence,
}

/// What the Gate made of one Generation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateVerdict {
    pub best_alive_time: f64,
    pub best_wave: u32,
    pub best_rocks: f64,
    pub median_alive_time: f64,
    /// No best was beaten and the median alive time is below the window's floor.
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
    pub best_rocks: f64,
    medians: VecDeque<f64>,
    pub run_of_stagnant: u32,
    pub tripped: bool,
    pub tripped_generation: Option<u32>,
}

impl Default for CompetenceGate {
    fn default() -> Self {
        Self {
            best_alive_time: 0.0,
            best_wave: 0,
            best_rocks: 0.0,
            medians: VecDeque::new(),
            run_of_stagnant: 0,
            tripped: false,
            tripped_generation: None,
        }
    }
}

impl CompetenceGate {
    /// Fold one Generation's records into the Gate.
    ///
    /// The comparison is against the oldest median in the window — including
    /// the one this Generation just pushed — so the Gate asks "is this
    /// Generation better than the worst of the recent past", not "better than
    /// the mean".
    pub fn observe(&mut self, records: &[EpisodeRecord], generation: u32) -> GateVerdict {
        if records.is_empty() {
            return GateVerdict {
                best_alive_time: self.best_alive_time,
                best_wave: self.best_wave,
                best_rocks: self.best_rocks,
                median_alive_time: self.median_alive_time(),
                stagnant: false,
                run_of_stagnant: self.run_of_stagnant,
                tripped_now: false,
            };
        }
        let mut alive_times: Vec<f64> = records.iter().map(|r| r.competence.alive_time).collect();
        let best_alive_time = alive_times.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let best_wave = records.iter().map(|r| r.competence.wave).max().unwrap_or(0);
        let best_rocks = records
            .iter()
            .map(|r| r.competence.rocks)
            .fold(f64::NEG_INFINITY, f64::max);
        alive_times.sort_by(f64::total_cmp);
        let median = alive_times[alive_times.len() / 2];

        let improved = best_alive_time > self.best_alive_time
            || best_wave > self.best_wave
            || best_rocks > self.best_rocks;
        self.best_alive_time = self.best_alive_time.max(best_alive_time);
        self.best_wave = self.best_wave.max(best_wave);
        self.best_rocks = self.best_rocks.max(best_rocks);

        self.medians.push_back(median);
        while self.medians.len() > gate::MEDIAN_WINDOW {
            self.medians.pop_front();
        }
        let floor = self.medians.front().copied().unwrap_or(median);
        let stagnant = !improved && median < gate::STAGNATION_RATIO * floor;
        self.run_of_stagnant = if stagnant { self.run_of_stagnant + 1 } else { 0 };
        let tripped_now = self.run_of_stagnant >= gate::STAGNATION_LIMIT && !self.tripped;
        if tripped_now {
            self.tripped = true;
            self.tripped_generation = Some(generation);
        }
        GateVerdict {
            best_alive_time,
            best_wave,
            best_rocks,
            median_alive_time: median,
            stagnant,
            run_of_stagnant: self.run_of_stagnant,
            tripped_now,
        }
    }

    pub fn median_alive_time(&self) -> f64 {
        let mut sorted: Vec<f64> = self.medians.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        sorted.get(sorted.len() / 2).copied().unwrap_or(0.0)
    }

    pub fn medians(&self) -> impl Iterator<Item = f64> + '_ {
        self.medians.iter().copied()
    }
}
