//! Sweep runner: the paired-seed evidence protocol of ADR 0007.
//!
//! A seed still reproduces a run (ADR 0005), so a Candidate is judged seed by
//! seed: run the same seeds through the default and then through the Candidate,
//! and compare the headline Competence number — the Population's mean Waves at
//! a fixed Generation. This is a binary target of the `sim` crate, like
//! `neuroarena-headless`, so it links the same simulation the app runs: no
//! subprocess, no second source of truth.
//!
//! ```text
//! cargo run --release --bin neuroarena-sweep -- --seeds 10 --generations 300 --population 500
//! cargo run --release --bin neuroarena-sweep -- --seeds 3 --generations 40 --population 100 --baseline default.sweep
//! ```
//!
//! Rows are keyed by seed, so a previous sweep's output is the baseline of the
//! next: `--baseline FILE` matches the file's rows to this run's rows by seed,
//! prints the per-seed delta of the headline, and applies the win rule. That
//! rule runs in two stages. The screen stage (`--floor 0`, the default) wins on
//! a median paired delta that is merely positive; the confirm stage
//! (`--floor 0.05`) also demands the materiality floor, so a Candidate that
//! gains a hundredth of a Wave does not clear the bar. Both stages insist that
//! fewer than half the paired seeds regress, and both print the numbers they
//! read, so the bar can be recalibrated against a baseline's own spread. Seeds
//! present on only one side are listed, never dropped.
//!
//! Wall-clock seconds are recorded in comments, never in a row, so the same
//! sweep prints byte-identical per-seed rows however long the machine took.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use sim::{default_workers, GateVerdict, Run, RunOptions};

/// The programme's screening protocol (ADR 0007): 10 seeds × 300 Generations at
/// Population 500. These are the sweep's defaults, not tuning constants, which
/// is why they live here and not in `sim::config` — and why the Population
/// default is deliberately not the app's own `neat::POP_SIZE`.
const SEEDS: u32 = 10;
const GENERATIONS: u32 = 300;
const POPULATION: usize = 500;

struct Args {
    seeds: u32,
    generations: u32,
    population: usize,
    workers: usize,
    baseline: Option<PathBuf>,
    /// The materiality floor for the confirm stage, in mean Waves; 0.0 is the
    /// screen stage, which only asks for a positive median delta.
    floor: f64,
    quiet: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seeds: SEEDS,
        generations: GENERATIONS,
        population: POPULATION,
        workers: default_workers(),
        baseline: None,
        floor: 0.0,
        quiet: false,
    };
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        let mut value = |name: &str| -> Result<String, String> {
            argv.next().ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "--seeds" => {
                args.seeds = value("--seeds")?
                    .parse()
                    .map_err(|_| "--seeds must be a positive integer".to_string())?
            }
            "--generations" => {
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
            "--baseline" => args.baseline = Some(PathBuf::from(value("--baseline")?)),
            "--floor" => {
                args.floor = value("--floor")?
                    .parse()
                    .map_err(|_| "--floor must be a number of Waves".to_string())?
            }
            "--quiet" => args.quiet = true,
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}\n\n{}", usage())),
        }
    }
    if args.seeds == 0 {
        return Err("--seeds must be a positive integer".to_string());
    }
    if args.generations == 0 {
        return Err("--generations must be a positive integer".to_string());
    }
    if args.population == 0 {
        return Err("--population must be a positive integer".to_string());
    }
    if args.workers == 0 {
        return Err("--workers must be a positive integer".to_string());
    }
    // Reject a NaN floor too: it would silently never win.
    if args.floor.is_nan() || args.floor < 0.0 {
        return Err("--floor must be a non-negative number of Waves".to_string());
    }
    Ok(args)
}

fn usage() -> String {
    format!(
        "neuroarena-sweep — run the paired-seed evidence protocol (ADR 0007)\n\n\
         usage: neuroarena-sweep [--seeds N] [--generations G] [--population P] [--workers W] [--baseline FILE] [--floor WAVES] [--quiet]\n\n\
         runs seeds 1..=N, one Run per seed, and prints the final Generation's mean, median and p90 Waves\n\
         against a baseline sweep's rows matched by seed (--baseline), with the win rule applied\n\
         --floor is the confirm-stage materiality bar in mean Waves (default 0, the screen stage)\n\n\
         defaults: seeds 1..={}, {} Generations, population {}, workers {} (this machine)",
        SEEDS,
        GENERATIONS,
        POPULATION,
        default_workers()
    )
}

/// One seed's line: the final Generation's Competence, and what the run cost.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SweepRow {
    seed: u32,
    /// The headline Competence number.
    mean_wave: f64,
    median_wave: u32,
    p90_wave: u32,
    /// A fraction here; the column prints it as a percentage, as the headless
    /// binary does.
    clearing_share: f64,
    median_alive_time: f64,
    steps: u64,
}

fn sweep_seed(seed: u32, args: &Args) -> SweepRow {
    let mut run = Run::with_options(seed, RunOptions::new(args.population, args.workers));
    let mut steps: u64 = 0;
    let mut final_gate: Option<GateVerdict> = None;
    for _ in 0..args.generations {
        let generation = run.begin_generation();
        let outcomes = generation.evaluate_all(args.workers);
        steps += outcomes.iter().map(|outcome| outcome.steps).sum::<u64>();
        final_gate = Some(run.complete_generation(outcomes).gate);
    }
    let gate = final_gate.expect("--generations is at least 1");
    SweepRow {
        seed,
        mean_wave: gate.mean_wave,
        median_wave: gate.median_wave,
        p90_wave: gate.p90_wave,
        clearing_share: gate.clearing_share,
        median_alive_time: gate.median_alive_time,
        steps,
    }
}

/// The column widths shared by the header, the rows and the aggregate row. The
/// headline column is wide enough for the aggregate's `median/max` pair.
const SEED_WIDTH: usize = 4;
const MEAN_WIDTH: usize = 13;
const WAVE_WIDTH: usize = 7;
const CLEAR_WIDTH: usize = 6;
const ALIVE_WIDTH: usize = 9;
const STEPS_WIDTH: usize = 12;

fn column_header() -> String {
    format!(
        "# {:>SEED_WIDTH$}  {:>MEAN_WIDTH$}  {:>WAVE_WIDTH$}  {:>WAVE_WIDTH$}  {:>CLEAR_WIDTH$}  {:>ALIVE_WIDTH$}  {:>STEPS_WIDTH$}",
        "seed", "meanWave", "medWave", "p90Wave", "clear%", "medAliveT", "steps"
    )
}

fn render_row(row: &SweepRow) -> String {
    format!(
        "  {:>SEED_WIDTH$}  {:>MEAN_WIDTH$.4}  {:>WAVE_WIDTH$}  {:>WAVE_WIDTH$}  {:>CLEAR_WIDTH$.1}  {:>ALIVE_WIDTH$.1}  {:>STEPS_WIDTH$}",
        row.seed,
        row.mean_wave,
        row.median_wave,
        row.p90_wave,
        row.clearing_share * 100.0,
        row.median_alive_time,
        row.steps
    )
}

/// The aggregate row: Median/Max of every Wave column across seeds, the median
/// clearing share and alive time, and the total steps. `--baseline` reads only
/// the seed rows, so this one is marked `agg` and skipped by the parser.
fn render_aggregate(rows: &[SweepRow]) -> String {
    let mut mean_waves: Vec<f64> = rows.iter().map(|row| row.mean_wave).collect();
    let mut median_waves: Vec<u32> = rows.iter().map(|row| row.median_wave).collect();
    let mut p90_waves: Vec<u32> = rows.iter().map(|row| row.p90_wave).collect();
    let mut clearing: Vec<f64> = rows.iter().map(|row| row.clearing_share * 100.0).collect();
    let mut alive_times: Vec<f64> = rows.iter().map(|row| row.median_alive_time).collect();
    let steps: u64 = rows.iter().map(|row| row.steps).sum();
    format!(
        "  {:>SEED_WIDTH$}  {:>MEAN_WIDTH$}  {:>WAVE_WIDTH$}  {:>WAVE_WIDTH$}  {:>CLEAR_WIDTH$.1}  {:>ALIVE_WIDTH$.1}  {:>STEPS_WIDTH$}",
        "agg",
        spread_f64(&mut mean_waves),
        spread_u32(&mut median_waves),
        spread_u32(&mut p90_waves),
        median_f64(&mut clearing),
        median_f64(&mut alive_times),
        steps,
    )
}

/// Median/Max of a Wave column, as one cell.
fn spread_u32(values: &mut [u32]) -> String {
    values.sort_unstable();
    format!("{}/{}", values[values.len() / 2], values[values.len() - 1])
}

fn spread_f64(values: &mut [f64]) -> String {
    values.sort_by(f64::total_cmp);
    format!(
        "{:.4}/{:.4}",
        values[values.len() / 2],
        values[values.len() - 1]
    )
}

/// The headline across seeds: the median of the per-seed headlines, taking the
/// upper-middle sample for an even count, as the Population's own medians do.
fn headline_mean_wave(rows: &[SweepRow]) -> f64 {
    let mut means: Vec<f64> = rows.iter().map(|row| row.mean_wave).collect();
    median_f64(&mut means)
}

fn median_f64(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// One seed, matched across the two sweeps by its headline mean Waves.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Paired {
    seed: u32,
    baseline_mean_wave: f64,
    candidate_mean_wave: f64,
}

impl Paired {
    fn delta(&self) -> f64 {
        self.candidate_mean_wave - self.baseline_mean_wave
    }
}

/// The paired-seed comparison: the matched seeds, the seeds only one side has,
/// the spread of the paired deltas, and the counts the win rule reads.
#[derive(Clone, Debug, PartialEq)]
struct Comparison {
    paired: Vec<Paired>,
    /// Seeds the baseline has and the candidate does not.
    unpaired_baseline: Vec<u32>,
    /// Seeds the candidate has and the baseline does not.
    unpaired_candidate: Vec<u32>,
    /// The median paired delta; for an even count, the mean of the two middles,
    /// so a lopsided pair cannot shift the verdict on its own.
    median_delta: f64,
    min_delta: f64,
    max_delta: f64,
    /// The materiality floor the verdict was read against, in mean Waves.
    floor: f64,
    improved: usize,
    matched: usize,
    regressed: usize,
}

impl Comparison {
    /// The two-stage bar. A win needs a median paired delta of at least the
    /// floor — and, at the screen stage's floor of zero, strictly more than
    /// zero, because a Candidate that matches the baseline has not won
    /// anything. Both stages also insist that fewer than half the paired seeds
    /// regress, and the verdict line prints the numbers it read.
    fn win(&self) -> bool {
        self.median_delta > 0.0
            && self.median_delta >= self.floor
            && self.regressed * 2 < self.paired.len()
    }

    fn seeds_compared(&self) -> usize {
        self.paired.len() + self.unpaired_baseline.len() + self.unpaired_candidate.len()
    }
}

/// The comparison, as a pure function of the two sweeps' rows and the floor.
fn compare(baseline: &[SweepRow], candidate: &[SweepRow], floor: f64) -> Comparison {
    let baseline_waves: BTreeMap<u32, f64> = baseline
        .iter()
        .map(|row| (row.seed, row.mean_wave))
        .collect();
    let candidate_waves: BTreeMap<u32, f64> = candidate
        .iter()
        .map(|row| (row.seed, row.mean_wave))
        .collect();

    let mut paired = Vec::new();
    let mut unpaired_baseline = Vec::new();
    for (&seed, &baseline_mean_wave) in &baseline_waves {
        match candidate_waves.get(&seed) {
            Some(&candidate_mean_wave) => paired.push(Paired {
                seed,
                baseline_mean_wave,
                candidate_mean_wave,
            }),
            None => unpaired_baseline.push(seed),
        }
    }
    let unpaired_candidate: Vec<u32> = candidate_waves
        .keys()
        .filter(|seed| !baseline_waves.contains_key(seed))
        .copied()
        .collect();

    let mut deltas: Vec<f64> = paired.iter().map(|pair| pair.delta()).collect();
    deltas.sort_by(f64::total_cmp);
    let (median_delta, min_delta, max_delta) = if deltas.is_empty() {
        (0.0, 0.0, 0.0)
    } else {
        let median = if deltas.len() % 2 == 1 {
            deltas[deltas.len() / 2]
        } else {
            (deltas[deltas.len() / 2 - 1] + deltas[deltas.len() / 2]) / 2.0
        };
        (median, deltas[0], deltas[deltas.len() - 1])
    };

    Comparison {
        improved: paired.iter().filter(|pair| pair.delta() > 0.0).count(),
        matched: paired.iter().filter(|pair| pair.delta() == 0.0).count(),
        regressed: paired.iter().filter(|pair| pair.delta() < 0.0).count(),
        paired,
        unpaired_baseline,
        unpaired_candidate,
        median_delta,
        min_delta,
        max_delta,
        floor,
    }
}

// The comparison is evidence, not data: every line is a comment, so a sweep
// written with `--baseline` stays parseable as another run's baseline.
fn print_comparison(baseline: &Path, comparison: &Comparison) {
    println!(
        "# paired: baseline {} vs candidate (this run) · headline mean Waves",
        baseline.display()
    );
    println!(
        "# {:>SEED_WIDTH$}  {:>11}  {:>11}  {:>9}",
        "seed", "baseline", "candidate", "delta"
    );
    for pair in &comparison.paired {
        println!(
            "# {:>SEED_WIDTH$}  {:>11.4}  {:>11.4}  {:>9}",
            pair.seed,
            pair.baseline_mean_wave,
            pair.candidate_mean_wave,
            signed(pair.delta())
        );
    }
    println!(
        "# paired {} of {} seeds · unpaired: baseline {} · candidate {}",
        comparison.paired.len(),
        comparison.seeds_compared(),
        seed_list(&comparison.unpaired_baseline),
        seed_list(&comparison.unpaired_candidate),
    );
    println!(
        "# paired delta of mean Waves: min {} · median {} · max {} · improved {} · matched {} · regressed {}",
        signed(comparison.min_delta),
        signed(comparison.median_delta),
        signed(comparison.max_delta),
        comparison.improved,
        comparison.matched,
        comparison.regressed,
    );
    let verdict = if comparison.win() { "win" } else { "no win" };
    println!(
        "# verdict: {verdict} — median paired delta {} (floor {:.4}), regressed {} of {} seeds",
        signed(comparison.median_delta),
        comparison.floor,
        comparison.regressed,
        comparison.paired.len(),
    );
    if comparison.floor == 0.0 {
        println!(
            "#   (provisional screen bar: median delta positive and fewer than half the paired seeds regressed)"
        );
    } else {
        println!(
            "#   (confirm bar: median delta at least the floor and fewer than half the paired seeds regressed)"
        );
    }
}

fn seed_list(seeds: &[u32]) -> String {
    let listed: Vec<String> = seeds.iter().map(u32::to_string).collect();
    format!("[{}]", listed.join(", "))
}

/// A delta as a column cell: `+0.0210`, `0.0000`, `-0.0040`.
fn signed(delta: f64) -> String {
    if delta > 0.0 {
        format!("+{delta:.4}")
    } else {
        format!("{delta:.4}")
    }
}

/// Read a previous sweep's rows. Only the per-seed rows carry data: comments
/// and the `agg` row are skipped, and a line that starts with something that is
/// not a seed is malformed rather than ignored.
fn parse_rows(text: &str) -> Result<Vec<SweepRow>, String> {
    let mut rows: Vec<SweepRow> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let mut fields = line.split_whitespace();
        let Some(first) = fields.next() else { continue };
        if first.starts_with('#') || first == "agg" {
            continue;
        }
        let seed: u32 = first
            .parse()
            .map_err(|_| format!("line {line_number}: expected a seed number, found {first:?}"))?;
        if rows.iter().any(|row| row.seed == seed) {
            return Err(format!("line {line_number}: seed {seed} appears twice"));
        }
        let rest: Vec<&str> = fields.collect();
        if rest.len() != 6 {
            return Err(format!(
                "line {line_number}: expected 7 columns (seed meanWave medWave p90Wave clear% medAliveT steps), found {}",
                rest.len() + 1
            ));
        }
        let clearing_percent: f64 = field(rest[3], line_number, "clear%")?;
        rows.push(SweepRow {
            seed,
            mean_wave: field(rest[0], line_number, "meanWave")?,
            median_wave: field(rest[1], line_number, "medWave")?,
            p90_wave: field(rest[2], line_number, "p90Wave")?,
            clearing_share: clearing_percent / 100.0,
            median_alive_time: field(rest[4], line_number, "medAliveT")?,
            steps: field(rest[5], line_number, "steps")?,
        });
    }
    Ok(rows)
}

fn field<T: std::str::FromStr>(value: &str, line_number: usize, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("line {line_number}: {name} must be a number, found {value:?}"))
}

fn read_baseline(path: &Path) -> Result<Vec<SweepRow>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read baseline {}: {error}", path.display()))?;
    let rows =
        parse_rows(&text).map_err(|message| format!("baseline {}: {message}", path.display()))?;
    if rows.is_empty() {
        return Err(format!(
            "baseline {}: no per-seed rows found",
            path.display()
        ));
    }
    Ok(rows)
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    // Read the baseline before the sweep: a mistyped path should fail in a
    // second, not after a 500-Generation run.
    let baseline = match args.baseline.as_deref().map(read_baseline).transpose() {
        Ok(baseline) => baseline,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    if !args.quiet {
        println!(
            "# neuroarena-sweep — seeds 1..={} · {} Generations · population {} · {} workers",
            args.seeds, args.generations, args.population, args.workers
        );
        println!(
            "# rows: the final Generation's Wave statistics · agg row: median/max across seeds, steps summed"
        );
        println!("{}", column_header());
    }

    let mut rows: Vec<SweepRow> = Vec::with_capacity(args.seeds as usize);
    let mut seconds: f64 = 0.0;
    for seed in 1..=args.seeds {
        let started = Instant::now();
        let row = sweep_seed(seed, &args);
        let seed_seconds = started.elapsed().as_secs_f64();
        seconds += seed_seconds;
        println!("{}", render_row(&row));
        if !args.quiet {
            println!(
                "# seed {} · {} Generations · {} steps · {:.2}s",
                row.seed, args.generations, row.steps, seed_seconds
            );
        }
        rows.push(row);
    }

    println!("{}", render_aggregate(&rows));
    if !args.quiet {
        let steps: u64 = rows.iter().map(|row| row.steps).sum();
        println!(
            "# headline — mean Wave across {} seeds: {:.4}",
            rows.len(),
            headline_mean_wave(&rows)
        );
        println!(
            "# {} steps ({:.1} sim-hours) in {:.2}s — measured {:.0} steps/s",
            steps,
            steps as f64 * sim::DT / 3600.0,
            seconds,
            steps as f64 / seconds.max(f64::MIN_POSITIVE),
        );
    }

    if let (Some(path), Some(baseline_rows)) = (args.baseline.as_deref(), baseline) {
        print_comparison(path, &compare(&baseline_rows, &rows, args.floor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sweep of `(seed, headline mean Waves)` pairs. The columns the
    /// comparison does not read carry fixed values: it pairs on the headline.
    fn series(pairs: &[(u32, f64)]) -> Vec<SweepRow> {
        pairs
            .iter()
            .map(|&(seed, mean_wave)| SweepRow {
                seed,
                mean_wave,
                median_wave: 0,
                p90_wave: 1,
                clearing_share: 0.25,
                median_alive_time: 20.0,
                steps: 1_000,
            })
            .collect()
    }

    #[test]
    fn identical_series_is_no_win() {
        let run = series(&[(1, 0.5), (2, 0.2), (3, 0.9), (4, 0.1), (5, 0.4)]);
        let comparison = compare(&run, &run, 0.0);
        assert_eq!(comparison.median_delta, 0.0);
        assert_eq!(comparison.improved, 0);
        assert_eq!(comparison.matched, 5);
        assert_eq!(comparison.regressed, 0);
        assert!(comparison.unpaired_baseline.is_empty());
        assert!(comparison.unpaired_candidate.is_empty());
        assert!(!comparison.win(), "a delta of zero Waves must never win");
    }

    #[test]
    fn a_series_shifted_up_everywhere_wins() {
        // Binary-exact values, so the delta is exactly one Wave and not a
        // rounding away from it.
        let baseline = series(&[(1, 0.25), (2, 0.5), (3, 0.75), (4, 1.0), (5, 1.25)]);
        let candidate = series(&[(1, 1.25), (2, 1.5), (3, 1.75), (4, 2.0), (5, 2.25)]);
        let comparison = compare(&baseline, &candidate, 0.0);
        assert_eq!(comparison.median_delta, 1.0);
        assert_eq!(comparison.min_delta, 1.0);
        assert_eq!(comparison.max_delta, 1.0);
        assert_eq!(comparison.improved, 5);
        assert_eq!(comparison.regressed, 0);
        assert!(comparison.win());
    }

    #[test]
    fn a_gained_median_with_half_the_seeds_regressed_is_no_win() {
        // Deltas +0.3, +0.3, +0.3, -0.1, -0.1, -0.1: the median is positive,
        // but exactly half the seeds regressed, and the bar asks for fewer than
        // half.
        let baseline = series(&[(1, 0.5), (2, 0.5), (3, 0.5), (4, 0.5), (5, 0.5), (6, 0.5)]);
        let candidate = series(&[(1, 0.8), (2, 0.8), (3, 0.8), (4, 0.4), (5, 0.4), (6, 0.4)]);
        let comparison = compare(&baseline, &candidate, 0.0);
        assert!(comparison.median_delta > 0.0);
        assert_eq!(comparison.regressed, 3);
        assert!(!comparison.win());
    }

    #[test]
    fn one_regression_in_four_still_wins_when_the_median_is_up() {
        // Deltas +0.1, +0.1, +0.1, -0.1: three of four seeds improved, which is
        // fewer than half regressed.
        let baseline = series(&[(1, 0.2), (2, 0.2), (3, 0.2), (4, 0.2)]);
        let candidate = series(&[(1, 0.3), (2, 0.3), (3, 0.3), (4, 0.1)]);
        let comparison = compare(&baseline, &candidate, 0.0);
        assert!(comparison.median_delta > 0.0);
        assert_eq!(comparison.improved, 3);
        assert_eq!(comparison.regressed, 1);
        assert!(comparison.win());
    }

    #[test]
    fn a_median_below_the_floor_is_no_win_even_without_regressions() {
        // About +0.04 Waves everywhere: a win at the screen stage's floor of
        // zero, a no-win against the confirm stage's 0.05 materiality floor.
        let baseline = series(&[(1, 0.2), (2, 0.2), (3, 0.2), (4, 0.2), (5, 0.2)]);
        let candidate = series(&[(1, 0.24), (2, 0.24), (3, 0.24), (4, 0.24), (5, 0.24)]);
        let screen = compare(&baseline, &candidate, 0.0);
        assert_eq!(screen.regressed, 0);
        assert!(screen.median_delta > 0.0 && screen.median_delta < 0.05);
        assert!(screen.win());

        let confirm = compare(&baseline, &candidate, 0.05);
        assert_eq!(confirm.floor, 0.05);
        assert_eq!(confirm.median_delta, screen.median_delta);
        assert!(!confirm.win(), "0.04 Waves must not clear a 0.05 floor");

        // The bar is "at least the floor": a delta exactly on it wins.
        assert!(compare(&baseline, &candidate, screen.median_delta).win());
    }

    #[test]
    fn unpaired_seeds_are_reported_not_dropped() {
        let baseline = series(&[(1, 0.2), (2, 0.2), (3, 0.2)]);
        let candidate = series(&[(2, 0.3), (3, 0.3), (4, 0.3)]);
        let comparison = compare(&baseline, &candidate, 0.0);
        assert_eq!(comparison.paired.len(), 2);
        assert_eq!(
            comparison.paired,
            vec![
                Paired {
                    seed: 2,
                    baseline_mean_wave: 0.2,
                    candidate_mean_wave: 0.3
                },
                Paired {
                    seed: 3,
                    baseline_mean_wave: 0.2,
                    candidate_mean_wave: 0.3
                },
            ]
        );
        assert_eq!(comparison.unpaired_baseline, vec![1]);
        assert_eq!(comparison.unpaired_candidate, vec![4]);
        // The delta is computed over the matched seeds only.
        assert!(comparison.median_delta > 0.0);
        assert_eq!(comparison.seeds_compared(), 4);
        assert!(comparison.win());
    }

    #[test]
    fn a_rendered_row_round_trips_through_the_parser() {
        let row = SweepRow {
            seed: 7,
            mean_wave: 0.021,
            median_wave: 3,
            p90_wave: 5,
            clearing_share: 0.125,
            median_alive_time: 42.5,
            steps: 12_345_678,
        };
        let printed = format!("{}\n{}\n", render_row(&row), render_aggregate(&[row]));
        assert_eq!(parse_rows(&printed), Ok(vec![row]));
    }
}
