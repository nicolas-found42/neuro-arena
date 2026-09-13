//! Fitness shaping: the action-usage entropy bonus, the behavior descriptor,
//! and the novelty archive with its decaying bonus.
//!
//! Selection is shaped (alive time, movement, bullet cost, action entropy, a
//! novelty bonus that decays to a floor); raw skill is banked separately by the
//! Competence Gate, so a HUD never confuses the two (ADR 0003).
//!
//! **Archive snapshot semantics.** Episodes evaluate concurrently, so a member's
//! novelty cannot depend on whether a sibling finished first. Every member of a
//! Generation scores against the archive as it stood when the Generation began,
//! and the whole Generation's descriptors are appended in member order once it
//! ends. That is what makes a run independent of the core count (ADR 0005); the
//! archive is still a bounded, time-decaying sample of behaviors.

use serde::{Deserialize, Serialize};

use crate::config::{fitness, ship as ship_cfg};
use crate::rng::Rng;
use crate::world::{coverage_fraction, Agent};

/// Seven normalized dimensions: four action-usage rates, arena coverage, mean
/// speed fraction, and Wave reached. Two behaviors that look alike here flew
/// alike — that is the whole point of the descriptor.
pub type Behavior = [f64; 7];

/// Shannon entropy over the four action-usage frequencies, normalized to
/// `[0, 1]`: 1 means every control was used equally, ~0.6 means two of four.
///
/// Sensorimotor-curiosity shaping (arXiv:1006.4959, arXiv:2608.12534).
pub fn entropy_bonus(agent: &Agent) -> f64 {
    let stats = &agent.stats;
    if stats.steps == 0 {
        return 0.0;
    }
    let counts = [stats.left, stats.right, stats.thrust, stats.fire];
    let mut entropy = 0.0;
    for count in counts {
        if count == 0 {
            continue;
        }
        let p = count as f64 / stats.steps as f64;
        entropy -= p * p.ln();
    }
    fitness::ACTION_ENTROPY_BONUS * (entropy / 4.0_f64.ln())
}

/// The novelty descriptor for a finished Episode.
pub fn behavior(agent: &Agent, wave: u32) -> Behavior {
    let stats = &agent.stats;
    let n = stats.steps.max(1) as f64;
    [
        stats.left as f64 / n,
        stats.right as f64 / n,
        stats.thrust as f64 / n,
        stats.fire as f64 / n,
        coverage_fraction(agent),
        stats.speed_sum / n / ship_cfg::MAX_SPEED,
        f64::from(wave.min(5)) / 5.0,
    ]
}

/// Mean distance to the k nearest archived behaviors; 0 when the archive is
/// empty, so the first Generation is never rewarded or punished for novelty.
pub fn novelty_against(entries: &[Behavior], behavior: &Behavior) -> f64 {
    if entries.is_empty() {
        return 0.0;
    }
    let k = fitness::NOVELTY_K.min(entries.len());
    let mut distances: Vec<f64> = entries
        .iter()
        .map(|entry| {
            let mut sum = 0.0;
            for i in 0..behavior.len() {
                let d = behavior[i] - entry[i];
                sum += d * d;
            }
            sum.sqrt()
        })
        .collect();
    distances.sort_by(f64::total_cmp);
    distances[..k].iter().sum::<f64>() / k as f64
}

/// Bounded archive of behavior descriptors (arXiv:1902.03142), with random
/// eviction so it stays a time-decaying sample rather than a record of the
/// first 400 Episodes.
///
/// The archive owns its own stream: eviction order is deterministic and cannot
/// be shifted by Episode scheduling (ADR 0005).
#[derive(Clone, Debug)]
pub struct NoveltyArchive {
    entries: Vec<Behavior>,
    rng: Rng,
}

impl NoveltyArchive {
    pub fn new(rng: Rng) -> Self {
        Self {
            entries: Vec::new(),
            rng,
        }
    }

    pub fn entries(&self) -> &[Behavior] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The descriptor set a Generation scores against, frozen at its start.
    pub fn snapshot(&self) -> Vec<Behavior> {
        self.entries.clone()
    }

    /// Bonus weight for a Generation: linear decay to a floor fraction
    /// (arXiv:2209.03618 adaptive explore/exploit).
    pub fn bonus_for(&self, generation: u32) -> f64 {
        let decay = (1.0 - f64::from(generation) / fitness::NOVELTY_DECAY_GENS)
            .max(fitness::NOVELTY_FLOOR_FRAC);
        fitness::NOVELTY_BONUS * decay
    }

    /// Append one descriptor, evicting a random entry once the archive is full.
    pub fn add(&mut self, behavior: Behavior) {
        self.entries.push(behavior);
        if self.entries.len() > fitness::ARCHIVE_SIZE {
            let victim = self.rng.below(self.entries.len());
            self.entries.remove(victim);
        }
    }

    /// Append a Generation's descriptors in member order.
    pub fn add_all(&mut self, behaviors: impl IntoIterator<Item = Behavior>) {
        for behavior in behaviors {
            self.add(behavior);
        }
    }
}

/// What one evaluated member contributes to the Generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpisodeOutcome {
    pub member: usize,
    /// Fitness that breeds: shaped fitness plus `bonus * novelty`.
    pub fitness: f64,
    /// Fitness the World banked: alive time, movement, points, entropy bonus.
    pub shaped_fitness: f64,
    pub novelty: f64,
    pub behavior: Behavior,
    pub competence: crate::competence::Competence,
    /// Simulation steps the Episode ran.
    pub steps: u64,
}

/// The per-Generation summary the Chart plots and the headless binary prints.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerationStats {
    pub generation: u32,
    pub best: f64,
    pub mean: f64,
    /// The Population's mean Waves: the headline Competence number.
    pub mean_wave: f64,
    /// The Population's median Waves, reported beside the headline.
    pub median_wave: u32,
    /// The Population's 90th-percentile Waves, reported beside the headline.
    pub p90_wave: u32,
    /// Share of the Population that has cleared at least the first Wave.
    pub clearing_share: f64,
}
