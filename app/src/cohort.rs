//! The Cohort: the hundred Agents of one Generation, as the app records them.
//!
//! The Arena shows one Agent. The other ninety-nine are evaluated on the worker
//! pool and, until this module existed, left the run as two numbers in a
//! banner. Everything here is a presentation-side record of outcomes the
//! simulation already produced — [`sim::EpisodeOutcome`], read and never
//! written — so the Population can be drawn without the simulation growing a
//! history it does not need.
//!
//! One record: [`Cohort`], *this* Generation filling in as Episodes are banked.
//! One slot per member, `None` until that member's Episode lands, and the
//! previous Generation kept whole behind it as the ghost the current one is
//! moving away from. The per-Generation curve the Chart draws is a different
//! thing and lives where it is produced: `Run::history`.
//!
//! Nothing here steps, samples or mutates the World. A member appears in the
//! record when the pool reports its outcome and not before, so the instrument
//! can never show an Episode that has not been run.

use sim::EpisodeOutcome;

/// The Arena's own time cap on one Episode, which is the ceiling the Cohort's
/// alive-time axis is measured against.
pub const EPISODE_CAP: f32 = sim::config::world::EPISODE_HARD_CAP as f32;

/// The first Wave's clock. Most Episodes end well inside it, so it is the
/// natural floor for an axis that has to stay stable while the Population is
/// still learning to survive.
pub const WAVE_CLOCK: f32 = sim::config::world::WAVE_TIME_LIMIT as f32;

/// One evaluated member, reduced to what the instrument draws. Every field is
/// taken straight off the Episode's own outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    /// Seconds the Agent stayed alive — raw Competence, not Fitness.
    pub alive: f32,
    /// The fraction of the Arena the Agent visited: behaviour descriptor 4,
    /// the same number the novelty archive scores against.
    pub coverage: f32,
    /// The Fitness that breeds: shaped score plus the novelty bonus.
    pub fitness: f32,
    /// The Wave the Episode reached.
    pub wave: u32,
    /// How far this member's behaviour descriptor sits from the frozen archive
    /// of earlier Generations — the exploration bonus's own input, not a
    /// derived score. Zero in a Generation that never ran.
    pub novelty: f32,
}

impl Member {
    pub fn from_outcome(outcome: &EpisodeOutcome) -> Self {
        Self {
            alive: outcome.competence.alive_time as f32,
            coverage: outcome.behavior[4] as f32,
            fitness: outcome.fitness as f32,
            wave: outcome.competence.wave,
            novelty: outcome.novelty as f32,
        }
    }
}

/// The Population of one Generation, filling in as the pool reports.
///
/// The previous Generation is kept whole as a *ghost*: the cloud the new one is
/// moving away from. That comparison is the whole point of the instrument, so
/// it is state rather than something recomputed from a history.
#[derive(Clone, Debug, Default)]
pub struct Cohort {
    generation: u32,
    live: Vec<Option<Member>>,
    ghost: Vec<Member>,
    watched: usize,
}

impl Cohort {
    /// Start recording a Generation: whatever is complete becomes the ghost,
    /// and `size` empty slots wait for the pool.
    pub fn begin(&mut self, generation: u32, size: usize, watched: usize) {
        let settled: Vec<Member> = self.live.iter().flatten().copied().collect();
        if !settled.is_empty() {
            self.ghost = settled;
        }
        self.generation = generation;
        self.live = vec![None; size];
        self.watched = watched.min(size.saturating_sub(1));
    }

    /// Forget everything: a new seed, a restart, a load.
    pub fn clear(&mut self) {
        self.generation = 0;
        self.live.clear();
        self.ghost.clear();
        self.watched = 0;
    }

    /// Bank one member's Episode. Out-of-range members are ignored rather than
    /// panicking: the instrument is never worth a crash.
    pub fn record(&mut self, member: usize, outcome: &EpisodeOutcome) {
        if let Some(slot) = self.live.get_mut(member) {
            *slot = Some(Member::from_outcome(outcome));
        }
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn size(&self) -> usize {
        self.live.len()
    }

    pub fn watched(&self) -> usize {
        self.watched
    }

    /// How many Episodes of this Generation have landed.
    pub fn settled(&self) -> usize {
        self.live.iter().flatten().count()
    }

    /// The slots, in member order, so a caller can tell the watched member from
    /// the rest and an unfinished Episode from a finished one.
    pub fn slots(&self) -> &[Option<Member>] {
        &self.live
    }

    pub fn ghost(&self) -> &[Member] {
        &self.ghost
    }

    /// The Population's middle, over the members that have landed: the median
    /// coverage and the median alive time, which is the crosshair the cloud is
    /// read against. `None` until something has landed.
    pub fn median(&self) -> Option<Member> {
        let mut alive: Vec<f32> = self.live.iter().flatten().map(|m| m.alive).collect();
        if alive.is_empty() {
            return None;
        }
        let mut coverage: Vec<f32> = self.live.iter().flatten().map(|m| m.coverage).collect();
        let mut wave: Vec<u32> = self.live.iter().flatten().map(|m| m.wave).collect();
        alive.sort_by(f32::total_cmp);
        coverage.sort_by(f32::total_cmp);
        wave.sort_unstable();
        let mid = alive.len() / 2;
        Some(Member {
            alive: alive[mid],
            coverage: coverage[mid],
            fitness: 0.0,
            wave: wave[mid],
            novelty: 0.0,
        })
    }

    /// The longest Episode in the record — live and ghost together, so the
    /// axis does not jump when the ghost is taller than the cloud in front of
    /// it. Floored at the first Wave's clock so an early Generation is not
    /// drawn against a two-second ceiling.
    pub fn alive_ceiling(&self) -> f32 {
        let top = self
            .live
            .iter()
            .flatten()
            .chain(self.ghost.iter())
            .fold(0.0_f32, |top, member| top.max(member.alive));
        nice_ceiling(top.max(WAVE_CLOCK * 0.5)).min(EPISODE_CAP)
    }
}

/// Round a ceiling up to something an axis can label: 1, 2 or 5 times a power
/// of ten. Keeps the alive-time axis from re-scaling on every new record.
pub fn nice_ceiling(value: f32) -> f32 {
    if !value.is_finite() || value <= 0.0 {
        return 1.0;
    }
    let decade = 10.0_f32.powf(value.log10().floor());
    let steps = [1.0, 2.0, 5.0, 10.0];
    for step in steps {
        if value <= decade * step * 1.000_01 {
            return decade * step;
        }
    }
    decade * 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::Competence;

    fn outcome(
        member: usize,
        alive: f64,
        coverage: f64,
        wave: u32,
        fitness: f64,
    ) -> EpisodeOutcome {
        let mut behavior = [0.0; 7];
        behavior[4] = coverage;
        EpisodeOutcome {
            member,
            fitness,
            shaped_fitness: fitness,
            novelty: 0.0,
            behavior,
            competence: Competence {
                alive_time: alive,
                wave,
                asteroids: 0.0,
            },
            steps: 1,
        }
    }

    #[test]
    fn a_cohort_fills_in_as_episodes_land() {
        let mut cohort = Cohort::default();
        cohort.begin(1, 4, 2);
        assert_eq!(cohort.settled(), 0);
        assert_eq!(cohort.size(), 4);
        assert_eq!(cohort.watched(), 2);
        cohort.record(0, &outcome(0, 12.0, 0.25, 0, 100.0));
        cohort.record(3, &outcome(3, 30.0, 0.5, 1, 400.0));
        assert_eq!(cohort.settled(), 2);
        // Members keep their slot: the Arena's member is identified by index.
        assert!(cohort.slots()[1].is_none());
        assert_eq!(cohort.slots()[3].unwrap().wave, 1);
    }

    #[test]
    fn the_previous_generation_becomes_the_ghost() {
        let mut cohort = Cohort::default();
        cohort.begin(1, 2, 0);
        cohort.record(0, &outcome(0, 10.0, 0.1, 0, 1.0));
        cohort.record(1, &outcome(1, 20.0, 0.2, 0, 2.0));
        cohort.begin(2, 2, 0);
        assert_eq!(cohort.ghost().len(), 2);
        assert_eq!(cohort.settled(), 0);
        // A Generation that banked nothing must not wipe the ghost: the
        // comparison the instrument exists for would vanish for one frame.
        cohort.begin(3, 2, 0);
        assert_eq!(cohort.ghost().len(), 2);
    }

    #[test]
    fn the_median_is_over_what_has_landed() {
        let mut cohort = Cohort::default();
        cohort.begin(1, 3, 0);
        assert!(cohort.median().is_none());
        cohort.record(0, &outcome(0, 10.0, 0.1, 0, 1.0));
        cohort.record(1, &outcome(1, 50.0, 0.3, 1, 2.0));
        cohort.record(2, &outcome(2, 30.0, 0.2, 0, 3.0));
        let median = cohort.median().unwrap();
        assert_eq!(median.alive, 30.0);
        assert_eq!(median.coverage, 0.2);
    }

    #[test]
    fn the_alive_axis_is_stable_and_bounded() {
        let mut cohort = Cohort::default();
        cohort.begin(1, 2, 0);
        // An early Generation is measured against a real ceiling, not against
        // its own two-second best.
        cohort.record(0, &outcome(0, 2.0, 0.0, 0, 0.0));
        assert!(cohort.alive_ceiling() >= WAVE_CLOCK * 0.5);
        // And no Episode can push the axis past the Arena's own cap.
        cohort.record(1, &outcome(1, EPISODE_CAP as f64, 1.0, 9, 0.0));
        assert!(cohort.alive_ceiling() <= EPISODE_CAP);
    }

    #[test]
    fn a_ceiling_lands_on_a_labellable_rung() {
        assert_eq!(nice_ceiling(0.0), 1.0);
        assert_eq!(nice_ceiling(7.0), 10.0);
        assert_eq!(nice_ceiling(12.0), 20.0);
        assert_eq!(nice_ceiling(30.0), 50.0);
        assert_eq!(nice_ceiling(50.0), 50.0);
        assert_eq!(nice_ceiling(2400.0), 5000.0);
    }

    #[test]
    fn clearing_a_cohort_forgets_the_ghost_too() {
        let mut cohort = Cohort::default();
        cohort.begin(1, 1, 0);
        cohort.record(0, &outcome(0, 10.0, 0.1, 0, 1.0));
        cohort.begin(2, 1, 0);
        cohort.clear();
        assert!(cohort.ghost().is_empty());
        assert_eq!(cohort.size(), 0);
        assert!(cohort.median().is_none());
    }
}
