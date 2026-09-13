//! NeuroArena's simulation core: the Arena, the World and its Episode rules, the
//! Genome and Network, the Population and its evolution, and the Run that steps
//! one Generation at a time.
//!
//! This crate has no windowing or GPU dependency — that is a compile-time fact,
//! not a promise (ADR 0004), so the whole of the simulation is testable headless
//! and the headless binary compiles against exactly the API the app uses.
//!
//! The vocabulary is the project's own (see `CONTEXT.md`): Arena, World, Ship,
//! Agent, Asteroid, Wave, Episode, Seam Copies, Sensor Ray; Genome, Network,
//! Population, Generation, Species, Fitness, Competence Gate, Innovation
//! Tracker; HUD, Chart.
//!
//! ```no_run
//! use sim::{Run, RunOptions};
//!
//! let mut run = Run::with_options(42, RunOptions::new(20, 4));
//! let report = run.evaluate_generation();
//! println!("gen {} best {:.1}", report.generation, report.best);
//! ```

pub mod asteroid;
pub mod competence;
pub mod config;
pub mod evaluation;
pub mod genome;
pub mod math;
pub mod network;
pub mod population;
pub mod rng;
pub mod run;
pub mod save;
pub mod sensors;
pub mod world;

pub use asteroid::Asteroid;
pub use competence::{Competence, CompetenceGate, EpisodeRecord, GateVerdict, WaveStats};
pub use config::DT;
pub use evaluation::{
    entropy_bonus, novelty_against, Behavior, EpisodeOutcome, GenerationStats, NoveltyArchive,
};
pub use genome::{Connection, Genome, InnovationTracker, NodeId, NodeType};
pub use network::Network;
pub use population::{Population, Species};
pub use rng::{derive_stream, Lane, Rng};
pub use run::{default_workers, BestRun, Generation, GenerationReport, Run, RunOptions};
pub use save::{GenomeFile, SaveError};
pub use world::{Agent, Bullet, Ship, Stats, World};
