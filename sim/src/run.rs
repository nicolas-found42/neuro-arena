//! Run: a seed, a Population, and the loop that evaluates a Generation and
//! breeds the next one.
//!
//! This is the whole public seam of the simulation. Construct it from a seed,
//! step it by Generation, and observe Fitness, the Competence Gate, the
//! Population and Species, and the Genome a save file would carry. It never
//! touches a window, a GPU or the wall clock.
//!
//! **Structure of a Generation.** [`Run::begin_generation`] freezes everything
//! an Episode needs — the Population's Networks, the novelty archive as it
//! stands, and the archive bonus for this Generation. The returned
//! [`Generation`] is immutable and shareable, so the app can step the member it
//! is watching on the main thread while the others evaluate on a worker pool:
//! Episodes are independent, so the result is the same at any core count
//! (ADR 0005). [`Run::complete_generation`] banks the outcomes, folds them into
//! the Gate, and breeds.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::competence::{Competence, CompetenceGate, EpisodeRecord, GateVerdict, WaveStats};
use crate::config::neat;
use crate::evaluation::{Behavior, EpisodeOutcome, GenerationStats, NoveltyArchive};
use crate::genome::Genome;
use crate::network::Network;
use crate::population::Population;
use crate::rng::{derive_stream, Lane};
use crate::world::World;

/// How a Run is configured. The Population size is fixed at 100 unless a test
/// or a probe says otherwise.
#[derive(Clone, Copy, Debug)]
pub struct RunOptions {
    pub population_size: usize,
    pub workers: usize,
}

impl RunOptions {
    pub fn new(population_size: usize, workers: usize) -> Self {
        Self {
            population_size: population_size.max(1),
            workers: workers.max(1),
        }
    }
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            population_size: neat::POP_SIZE,
            workers: default_workers(),
        }
    }
}

/// The machine's core count, which never changes what a run produces — only how
/// long it takes.
pub fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// The best member of the run so far, with the context a save file needs.
#[derive(Clone, Debug)]
pub struct BestRun {
    pub genome: Genome,
    pub network: Network,
    pub fitness: f64,
    pub generation: u32,
    pub competence: Competence,
    pub member: usize,
}

/// A Generation frozen for evaluation: independent of the Run, shareable across
/// threads, and cheap to clone by `Arc`.
pub struct Generation {
    run_seed: u32,
    index: u32,
    networks: Vec<Network>,
    archive: Vec<Behavior>,
    bonus: f64,
}

impl Generation {
    pub fn len(&self) -> usize {
        self.networks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.networks.is_empty()
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    /// The novelty bonus weight in force for this Generation.
    pub fn bonus(&self) -> f64 {
        self.bonus
    }

    pub fn network(&self, member: usize) -> &Network {
        &self.networks[member]
    }

    /// A fresh World for one member, drawing from that member's Episode stream.
    /// Two Worlds built from the same `(seed, generation, member)` are identical.
    pub fn world(&self, member: usize) -> World {
        let rng = derive_stream(self.run_seed, self.index, member as u32, Lane::Episode);
        World::new(rng, Some(self.networks[member].clone()))
    }

    /// Bank a finished World as an outcome. `member` names which Episode it was.
    pub fn outcome(&self, member: usize, world: &World) -> EpisodeOutcome {
        let behavior = world.behavior();
        let novelty = crate::evaluation::novelty_against(&self.archive, &behavior);
        let shaped = world.agent.fitness;
        EpisodeOutcome {
            member,
            fitness: shaped + self.bonus * novelty,
            shaped_fitness: shaped,
            novelty,
            behavior,
            competence: Competence {
                alive_time: world.agent.stats.alive_time,
                wave: world.wave,
                asteroids: world.agent.stats.asteroid_points,
            },
            steps: (world.time / crate::config::DT).round() as u64,
        }
    }

    /// Evaluate one member from scratch, to Episode end.
    pub fn evaluate_member(&self, member: usize) -> EpisodeOutcome {
        let mut world = self.world(member);
        world.run_to_end();
        self.outcome(member, &world)
    }

    /// Evaluate every member, spreading Episodes over `workers` threads.
    pub fn evaluate_all(&self, workers: usize) -> Vec<EpisodeOutcome> {
        self.evaluate_except(workers, &[])
            .into_iter()
            .map(|outcome| outcome.expect("no member was skipped"))
            .collect()
    }

    /// Evaluate every member except the ones in `skip`, which are returned as
    /// `None`. The worker pool claims members off a shared counter and files
    /// each outcome under its own index, so the result never depends on
    /// scheduling.
    pub fn evaluate_except(&self, workers: usize, skip: &[usize]) -> Vec<Option<EpisodeOutcome>> {
        let mut results: Vec<Option<EpisodeOutcome>> =
            (0..self.networks.len()).map(|_| None).collect();
        let queue: Vec<usize> = (0..self.networks.len())
            .filter(|member| !skip.contains(member))
            .collect();
        let workers = workers.clamp(1, queue.len().max(1));
        if queue.is_empty() {
            return results;
        }
        if workers == 1 {
            for member in queue {
                results[member] = Some(self.evaluate_member(member));
            }
            return results;
        }
        let next = AtomicUsize::new(0);
        let collected: Mutex<Vec<Option<EpisodeOutcome>>> = Mutex::new(results);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    let claim = next.fetch_add(1, Ordering::Relaxed);
                    if claim >= queue.len() {
                        break;
                    }
                    let member = queue[claim];
                    let outcome = self.evaluate_member(member);
                    collected.lock().expect("no panic holds this lock")[member] = Some(outcome);
                });
            }
        });
        collected.into_inner().expect("no panic holds this lock")
    }
}

/// One Generation's summary, as printed by the headless binary and plotted by
/// the Chart.
#[derive(Clone, Debug)]
pub struct GenerationReport {
    pub generation: u32,
    pub best: f64,
    pub mean: f64,
    pub best_member: usize,
    pub gate: GateVerdict,
    pub species_count: usize,
    pub delta_target: f64,
    pub episodes: usize,
}

/// The simulation, as a value: seed in, Generations out.
pub struct Run {
    seed: u32,
    options: RunOptions,
    population: Population,
    archive: NoveltyArchive,
    gate: CompetenceGate,
    best: Option<BestRun>,
    watching: bool,
    last_verdict: Option<GateVerdict>,
}

impl Run {
    /// An evolving run from a fresh Population.
    pub fn new(seed: u32) -> Self {
        Self::with_options(seed, RunOptions::default())
    }

    pub fn with_options(seed: u32, options: RunOptions) -> Self {
        Self {
            seed,
            population: Population::new(options.population_size, seed),
            // The archive is the one sequential consumer of randomness that
            // spans Generations; its stream is derived once and consumed in the
            // fixed order Episodes are banked.
            archive: NoveltyArchive::new(derive_stream(seed, 0, 0, Lane::Archive)),
            options,
            gate: CompetenceGate::default(),
            best: None,
            watching: false,
            last_verdict: None,
        }
    }

    /// Watch mode: every slot replays `genome` and nothing breeds.
    pub fn watch(seed: u32, genome: &Genome, options: RunOptions) -> Self {
        Self {
            seed,
            population: Population::showcase(genome, options.population_size, seed),
            archive: NoveltyArchive::new(derive_stream(seed, 0, 0, Lane::Archive)),
            options,
            gate: CompetenceGate::default(),
            best: None,
            watching: true,
            last_verdict: None,
        }
    }

    /// Evolve again, starting from a saved Genome: a Population of mutated
    /// copies with the loaded Genome itself kept as the first member. This is
    /// how a watch run is not a dead end.
    pub fn resume_from(seed: u32, genome: &Genome, options: RunOptions) -> Self {
        Self {
            seed,
            population: Population::from_champion(genome, options.population_size, seed),
            archive: NoveltyArchive::new(derive_stream(seed, 0, 0, Lane::Archive)),
            options,
            gate: CompetenceGate::default(),
            best: None,
            watching: false,
            last_verdict: None,
        }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    pub fn generation(&self) -> u32 {
        self.population.generation
    }

    pub fn options(&self) -> RunOptions {
        self.options
    }

    pub fn population(&self) -> &Population {
        &self.population
    }

    pub fn gate(&self) -> &CompetenceGate {
        &self.gate
    }

    pub fn history(&self) -> &[GenerationStats] {
        &self.population.history
    }

    pub fn best(&self) -> Option<&BestRun> {
        self.best.as_ref()
    }

    pub fn is_watching(&self) -> bool {
        self.watching
    }

    pub fn archive_size(&self) -> usize {
        self.archive.len()
    }

    /// Freeze the current Generation for evaluation.
    pub fn begin_generation(&mut self) -> Generation {
        let generation = self.population.generation;
        Generation {
            run_seed: self.seed,
            index: generation,
            networks: self.population.networks.clone(),
            archive: self.archive.snapshot(),
            bonus: self.archive.bonus_for(generation),
        }
    }

    /// Begin, evaluate every member in parallel, and close the Generation.
    pub fn evaluate_generation(&mut self) -> GenerationReport {
        let generation = self.begin_generation();
        let outcomes = generation.evaluate_all(self.options.workers);
        self.complete_generation(outcomes)
    }

    /// Bank a Generation's outcomes, then breed.
    ///
    /// Outcomes may arrive in any order; they are filed by member index, which
    /// is what makes the result independent of the worker count.
    pub fn complete_generation(&mut self, outcomes: Vec<EpisodeOutcome>) -> GenerationReport {
        let size = self.population.genomes.len();
        let mut ordered: Vec<Option<EpisodeOutcome>> = (0..size).map(|_| None).collect();
        for outcome in outcomes {
            let member = outcome.member;
            assert!(member < size, "outcome for an unknown member");
            ordered[member] = Some(outcome);
        }
        let ordered: Vec<EpisodeOutcome> = ordered
            .into_iter()
            .enumerate()
            .map(|(member, outcome)| {
                outcome.unwrap_or_else(|| panic!("member {member} was not evaluated"))
            })
            .collect();

        let mut fitnesses = vec![0.0; size];
        let mut behaviors = Vec::with_capacity(size);
        let mut records = Vec::with_capacity(size);
        let mut best_member = 0;
        for (member, outcome) in ordered.iter().enumerate() {
            fitnesses[member] = outcome.fitness;
            behaviors.push(outcome.behavior);
            records.push(EpisodeRecord {
                member,
                competence: outcome.competence,
            });
            if outcome.fitness > ordered[best_member].fitness {
                best_member = member;
            }
            if self
                .best
                .as_ref()
                .map(|best| outcome.fitness > best.fitness)
                .unwrap_or(true)
            {
                self.best = Some(BestRun {
                    genome: self.population.genomes[member].clone(),
                    network: self.population.networks[member].clone(),
                    fitness: outcome.fitness,
                    generation: self.population.generation,
                    competence: outcome.competence,
                    member,
                });
            }
        }

        // The whole Generation's descriptors are archived in member order, after
        // every Episode has been scored against the snapshot taken at its start.
        self.archive.add_all(behaviors);

        let generation = self.population.generation;
        let verdict = self.gate.observe(&records, generation);
        self.last_verdict = Some(verdict);
        let waves = WaveStats {
            mean: verdict.mean_wave,
            median: verdict.median_wave,
            p90: verdict.p90_wave,
            clearing_share: verdict.clearing_share,
        };

        let best = fitnesses.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mean = fitnesses.iter().sum::<f64>() / size as f64;
        if self.watching {
            self.population.advance_without_breeding(&fitnesses, waves);
        } else {
            self.population.evolve(&fitnesses, waves);
        }

        GenerationReport {
            generation,
            best,
            mean,
            best_member,
            gate: verdict,
            species_count: self.population.species.len(),
            delta_target: self.population.delta_target,
            episodes: size,
        }
    }

    /// The Gate verdict from the most recent completed Generation.
    pub fn last_verdict(&self) -> Option<GateVerdict> {
        self.last_verdict
    }

    /// The save file for the best Genome of this run so far.
    pub fn best_file(&self) -> Option<crate::save::GenomeFile> {
        self.best.as_ref().map(|best| crate::save::GenomeFile {
            seed: self.seed,
            generation: best.generation,
            fitness: best.fitness,
            competence: best.competence,
            genome: best.genome.clone(),
        })
    }
}
