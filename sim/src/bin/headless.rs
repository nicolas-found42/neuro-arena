//! Headless runner: the tuning evidence tool and the determinism oracle.
//!
//! It is a binary target of the simulation crate, not a separate crate, so it
//! compiles against exactly the `sim` public API the app uses: a headless
//! result therefore means something about the app (ADR 0004).
//!
//! ```text
//! cargo run --release --bin neuroarena-headless -- --seed 42 --generations 40
//! ```
//!
//! It prints one line per Generation: shaped Fitness (best and mean), the raw
//! Competence Gate numbers (alive time, Wave, Asteroid points, median alive
//! time) and the Species count. The measured step rate is printed as evidence —
//! never asserted, because a machine-specific threshold is a flaky test.

use std::time::Instant;

use sim::{Run, RunOptions, default_workers};

struct Args {
    seed: u32,
    generations: u32,
    population: usize,
    workers: usize,
    quiet: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seed: 1,
        generations: 10,
        population: sim::config::neat::POP_SIZE,
        workers: default_workers(),
        quiet: false,
    };
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        let mut value = |name: &str| -> Result<String, String> {
            argv.next()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "--seed" => {
                args.seed = value("--seed")?
                    .parse()
                    .map_err(|_| "--seed must be a 32-bit unsigned integer".to_string())?
            }
            "--generations" | "--gens" => {
                args.generations = value("--generations")?
                    .parse()
                    .map_err(|_| "--generations must be a positive integer".to_string())?
            }
            "--population" => {
                args.population = value("--population")?
                    .parse()
                    .map_err(|_| "--population must be a positive integer".to_string())?
            }
            "--workers" => {
                args.workers = value("--workers")?
                    .parse()
                    .map_err(|_| "--workers must be a positive integer".to_string())?
            }
            "--quiet" => args.quiet = true,
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}\n\n{}", usage())),
        }
    }
    Ok(args)
}

fn usage() -> String {
    format!(
        "neuroarena-headless — run Generations without a window\n\n\
         usage: neuroarena-headless [--seed N] [--generations N] [--population N] [--workers N] [--quiet]\n\n\
         defaults: seed 1, 10 Generations, population {}, workers {} (this machine)",
        sim::config::neat::POP_SIZE,
        default_workers()
    )
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    if !args.quiet {
        println!(
            "# NeuroArena headless — seed {} · {} Generations · population {} · {} workers",
            args.seed, args.generations, args.population, args.workers
        );
        println!(
            "# {:>3}  {:>10}  {:>10}  {:>7}  {:>4}  {:>9}  {:>7}  {:>4}  {:>5}  {:>6}",
            "gen", "best", "mean", "aliveT", "wave", "asteroids", "medAT", "spec", "delta", "gate"
        );
    }

    let mut run = Run::with_options(args.seed, RunOptions::new(args.population, args.workers));
    let started = Instant::now();
    let mut steps: u64 = 0;
    let mut generations = 0;

    for _ in 0..args.generations {
        let generation = run.begin_generation();
        let outcomes = generation.evaluate_all(args.workers);
        let episode_steps: u64 = outcomes.iter().map(|outcome| outcome.steps).sum();
        let report = run.complete_generation(outcomes);
        generations += 1;
        steps += episode_steps;
        if !args.quiet {
            println!(
                "  {:>3}  {:>10.1}  {:>10.1}  {:>7.1}  {:>4}  {:>9.0}  {:>7.1}  {:>4}  {:>5.2}  {:>3}/{}",
                report.generation,
                report.best,
                report.mean,
                report.gate.best_alive_time,
                report.gate.best_wave,
                report.gate.best_asteroids,
                report.gate.median_alive_time,
                report.species_count,
                report.delta_target,
                report.gate.run_of_stagnant,
                sim::config::gate::STAGNATION_LIMIT,
            );
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    let sim_seconds = steps as f64 * sim::DT;
    println!(
        "# {} Generations, {} Episodes, {} steps ({:.1} sim-seconds) in {:.2}s — measured {:.0} steps/s, {:.1}x realtime",
        generations,
        generations as usize * args.population,
        steps,
        sim_seconds,
        elapsed,
        steps as f64 / elapsed.max(f64::MIN_POSITIVE),
        sim_seconds / elapsed.max(f64::MIN_POSITIVE),
    );
    if let Some(verdict) = run.last_verdict() {
        if verdict.tripped_now {
            println!(
                "# Competence Gate tripped at Generation {}: no best beaten for {} Generations",
                run.generation(),
                sim::config::gate::STAGNATION_LIMIT
            );
        }
    }
    if let Some(best) = run.best() {
        println!(
            "# best Genome: fitness {:.1}, Generation {}, alive {:.1}s, Wave {}, {} asteroids, {} nodes / {} connections",
            best.fitness,
            best.generation,
            best.competence.alive_time,
            best.competence.wave,
            best.competence.asteroids as u64,
            best.genome.node_count(),
            best.genome.enabled_connection_count(),
        );
    }
}
