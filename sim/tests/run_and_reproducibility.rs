//! Run-level behaviour: reproducibility, core-count independence, Population
//! invariants, the Competence Gate, watch mode and resume.

mod common;

use common::wired_genome;
use sim::config::fitness;
use sim::{Competence, CompetenceGate, EpisodeRecord, Run, RunOptions};

/// Small enough to keep the suite fast, large enough for speciation to run.
fn options(workers: usize) -> RunOptions {
    RunOptions::new(12, workers)
}

fn curve(run: &Run) -> Vec<(u32, f64, f64)> {
    run.history()
        .iter()
        .map(|stats| (stats.generation, stats.best, stats.mean))
        .collect()
}

#[test]
fn one_seed_produces_the_same_fitness_curve_twice() {
    let mut first = Run::with_options(1234, options(1));
    let mut second = Run::with_options(1234, options(1));
    for _ in 0..2 {
        first.evaluate_generation();
        second.evaluate_generation();
    }
    assert_eq!(curve(&first), curve(&second));
    assert_eq!(first.history().len(), 2);
    assert_eq!(
        first.best().map(|b| b.fitness),
        second.best().map(|b| b.fitness)
    );
}

#[test]
fn a_different_seed_produces_a_different_run() {
    let mut first = Run::with_options(1, options(1));
    let mut second = Run::with_options(2, options(1));
    first.evaluate_generation();
    second.evaluate_generation();
    assert_ne!(curve(&first), curve(&second));
}

#[test]
fn the_core_count_does_not_change_the_run() {
    let mut single = Run::with_options(99, options(1));
    let mut many = Run::with_options(99, options(8));
    for _ in 0..2 {
        single.evaluate_generation();
        many.evaluate_generation();
    }
    assert_eq!(
        curve(&single),
        curve(&many),
        "a run is reproducible at any core count"
    );
    // And the Population itself is identical, not just its summary.
    assert_eq!(
        single.population().genomes,
        many.population().genomes,
        "breeding followed the same path"
    );
    assert_eq!(
        single.population().genomes[0].connection_count(),
        many.population().genomes[0].connection_count()
    );
}

#[test]
fn members_evaluate_independently_of_each_other() {
    let mut run = Run::with_options(7, options(1));
    let generation = run.begin_generation();
    let solo = generation.evaluate_member(3);
    let in_parallel = generation.evaluate_all(4);
    assert_eq!(
        solo.fitness, in_parallel[3].fitness,
        "an Episode never depends on its siblings"
    );
    // The same member evaluated twice is the same Episode.
    let again = generation.evaluate_member(3);
    assert_eq!(solo.fitness, again.fitness);
    assert_eq!(solo.behavior, again.behavior);
    // Different members are different Episodes.
    assert_ne!(solo.behavior, generation.evaluate_member(4).behavior);
}

#[test]
fn an_evaluated_episode_carries_its_competence_and_novelty() {
    let mut run = Run::with_options(5, options(1));
    let generation = run.begin_generation();
    let outcome = generation.evaluate_member(0);
    assert!(outcome.competence.alive_time >= 0.0, "alive time is banked");
    assert!(outcome.shaped_fitness.is_finite());
    assert!(outcome.fitness.is_finite());
    assert_eq!(
        outcome.fitness,
        outcome.shaped_fitness + generation.bonus() * outcome.novelty
    );
    assert!(
        outcome.novelty >= 0.0,
        "the first Generation scores zero novelty against an empty archive"
    );
    assert_eq!(outcome.novelty, 0.0);
}

#[test]
fn the_population_keeps_its_size_and_its_genomes_stay_well_formed() {
    let mut run = Run::with_options(21, options(1));
    for _ in 0..3 {
        run.evaluate_generation();
        let population = run.population();
        assert_eq!(population.genomes.len(), 12);
        assert_eq!(population.networks.len(), 12);
        for genome in &population.genomes {
            for (_, connection) in genome.connections() {
                assert!(
                    genome.has_node(connection.from) && genome.has_node(connection.to),
                    "orphan connection after breeding"
                );
            }
        }
    }
    assert_eq!(run.generation(), 4);
}

#[test]
fn the_novelty_archive_grows_and_stays_bounded() {
    let mut run = Run::with_options(31, RunOptions::new(8, 1));
    for _ in 0..3 {
        run.evaluate_generation();
    }
    assert_eq!(run.archive_size(), 24, "one descriptor per Episode");
    // The bonus decays toward its floor as Generations pass.
    let early = run.begin_generation().bonus();
    assert!(early > 0.0 && early <= fitness::NOVELTY_BONUS);
}

#[test]
fn the_gate_reports_the_best_of_the_generation() {
    let mut run = Run::with_options(41, options(1));
    let generation = run.begin_generation();
    let outcomes = generation.evaluate_all(2);
    let best_alive = outcomes
        .iter()
        .map(|o| o.competence.alive_time)
        .fold(f64::NEG_INFINITY, f64::max);
    let best_wave = outcomes.iter().map(|o| o.competence.wave).max().unwrap();
    let report = run.complete_generation(outcomes);
    assert_eq!(report.gate.best_alive_time, best_alive);
    assert_eq!(report.gate.best_wave, best_wave);
    assert_eq!(run.gate().best_alive_time, best_alive);
    assert_eq!(report.generation, 1);
    assert!(report.species_count >= 1);
    assert!(
        !report.gate.tripped_now,
        "one Generation cannot trip the Gate"
    );
}

#[test]
fn the_gate_trips_when_skill_stops_improving() {
    let record = |alive_time: f64| EpisodeRecord {
        member: 0,
        competence: Competence {
            alive_time,
            wave: 0,
            asteroids: 0.0,
        },
    };
    // A flat run: every Generation reaches the same competence.
    let mut flat = CompetenceGate::default();
    let mut tripped_at = None;
    for generation in 1..=20 {
        let verdict = flat.observe(&[record(10.0)], generation);
        if verdict.tripped_now {
            tripped_at = Some(generation);
        }
    }
    assert_eq!(
        tripped_at,
        Some(sim::config::gate::STAGNATION_LIMIT + 1),
        "the Gate trips after a run of stagnant Generations"
    );

    // A run that keeps improving never trips.
    let mut rising = CompetenceGate::default();
    for generation in 1..=40 {
        let verdict = rising.observe(&[record(f64::from(generation) * 5.0)], generation);
        assert!(!verdict.stagnant, "Generation {generation} improved");
    }
    assert!(!rising.tripped);
    assert_eq!(rising.best_alive_time, 200.0);
}

#[test]
fn waves_decide_stagnation_before_alive_time() {
    let record = |wave: u32, alive_time: f64| EpisodeRecord {
        member: 0,
        competence: Competence {
            alive_time,
            wave,
            asteroids: 0.0,
        },
    };
    // A settled baseline: a full window of Wave 1 at 60 s.
    let settled = || {
        let mut gate = CompetenceGate::default();
        for generation in 1..=sim::config::gate::MEDIAN_WINDOW as u32 {
            gate.observe(&[record(1, 60.0)], generation);
        }
        gate
    };

    assert!(
        !settled().observe(&[record(2, 20.0)], 16).stagnant,
        "a higher median Wave is progress even when the Ships die sooner"
    );
    assert!(
        settled().observe(&[record(1, 60.0)], 16).stagnant,
        "matching the floor on both is not progress"
    );
    assert!(
        settled().observe(&[record(0, 300.0)], 16).stagnant,
        "alive time is the tie-break, not the headline"
    );
    assert!(
        !settled().observe(&[record(1, 70.0)], 16).stagnant,
        "at equal Waves, alive time above the ratio is progress"
    );

    // The removed best-ever escape hatch: one member at a record 500 s alive
    // while every other member sits on the floor. The old rule — stagnant
    // unless any member beat an all-time record — let that record set
    // `improved`; the shipped rule judges only the tie-broken pair, so the
    // Generation is stagnant: the mean Waves stay exactly 1.0 and the median
    // alive time exactly 60 s.
    let mut one_record = vec![record(1, 60.0); 19];
    one_record.push(record(1, 500.0));
    assert!(
        settled().observe(&one_record, 16).stagnant,
        "a single record-breaking alive time does not excuse a stalled headline"
    );
}

#[test]
fn the_gate_summarizes_the_populations_waves() {
    let record = |member: usize, wave: u32| EpisodeRecord {
        member,
        competence: Competence {
            alive_time: 1.0,
            wave,
            asteroids: 0.0,
        },
    };
    let mut gate = CompetenceGate::default();
    let verdict = gate.observe(
        &[
            record(0, 0),
            record(1, 0),
            record(2, 1),
            record(3, 2),
            record(4, 5),
        ],
        1,
    );
    assert_eq!(verdict.median_wave, 1, "the middle of five Waves");
    assert_eq!(verdict.best_wave, 5);
    assert_eq!(
        verdict.p90_wave, 5,
        "the top of five is the 90th percentile"
    );
    assert!(
        (verdict.mean_wave - 1.6).abs() < 1e-9,
        "the headline is the mean of the Waves, not the middle one"
    );
    assert!(
        (verdict.clearing_share - 0.6).abs() < 1e-9,
        "three of five cleared the first Wave"
    );
    assert!(
        !verdict.stagnant,
        "a Generation with no history behind it is never stagnant"
    );
}

#[test]
fn species_are_never_culled_below_two_or_in_empty_runs() {
    let mut run = Run::with_options(101, RunOptions::new(30, 2));
    for _ in 0..4 {
        run.evaluate_generation();
        let population = run.population();
        assert!(
            !population.species.is_empty(),
            "the Population keeps its Species"
        );
        for species in &population.species {
            assert!(
                species.stagnation < sim::config::neat::STAGNATION_LIMIT
                    || population.best_ever.species_id == Some(species.id),
                "a stagnant Species survived without holding the global best"
            );
        }
    }
    assert!(run.population().delta_target >= sim::config::neat::DELTA_MIN);
    assert!(run.population().delta_target <= sim::config::neat::DELTA_MAX);
}

#[test]
fn watch_mode_replays_one_genome_and_breeds_nothing() {
    let genome = wired_genome(11, sim::config::nn::OUTPUT_IDS[2], 2.0);
    let mut run = Run::watch(55, &genome, RunOptions::new(4, 1));
    assert!(run.is_watching());
    run.evaluate_generation();
    run.evaluate_generation();
    assert_eq!(run.generation(), 3, "watch mode still advances the clock");
    assert_eq!(run.history().len(), 2, "and still records the curve");
    for member in &run.population().genomes {
        assert_eq!(member, &genome, "no member ever diverges");
    }
    assert_eq!(run.population().species.len(), 0, "nothing is speciated");
}

#[test]
fn resuming_from_a_genome_keeps_the_champion_and_evolves_around_it() {
    let genome = wired_genome(11, sim::config::nn::OUTPUT_IDS[2], 1.5);
    let mut run = Run::resume_from(77, &genome, RunOptions::new(6, 1));
    assert!(!run.is_watching());
    assert_eq!(run.population().genomes[0], genome, "the champion leads");
    let mutated = run
        .population()
        .genomes
        .iter()
        .filter(|g| *g != &genome)
        .count();
    assert!(mutated > 0, "the rest of the Population is mutated copies");

    run.evaluate_generation();
    assert_eq!(run.generation(), 2, "and it evolves from there");
}

#[test]
fn the_best_genome_carries_the_context_of_its_run() {
    let mut run = Run::with_options(88, options(1));
    run.evaluate_generation();
    let best = run.best().expect("a Generation was evaluated");
    assert_eq!(best.generation, 1);
    assert!(best.competence.alive_time > 0.0, "the Episode ran");
    let file = run.best_file().expect("the best Genome can be saved");
    assert_eq!(file.seed, 88);
    assert_eq!(file.generation, 1);
    assert_eq!(file.fitness, best.fitness);
    assert_eq!(file.genome, best.genome);
}
