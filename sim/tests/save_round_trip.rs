//! The save format: a Genome round-trips into a Network that behaves the same,
//! and anything that is not one of our files is refused with a message that
//! names the file and the reason.

mod common;

use std::path::PathBuf;

use common::wired_genome;
use sim::config::nn;
use sim::save::{GenomeFile, SaveError, FORMAT, VERSION};
use sim::{Competence, Network, Run, RunOptions};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("neuroarena-save-tests");
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join(name)
}

fn sample_file() -> GenomeFile {
    let mut genome = wired_genome(11, 21, 1.25);
    genome.insert_node(26, sim::NodeType::Hidden);
    genome.insert_connection(
        900,
        sim::Connection {
            from: 21,
            to: 26,
            weight: -0.5,
            enabled: true,
        },
    );
    genome.insert_connection(
        901,
        sim::Connection {
            from: 26,
            to: 22,
            weight: 0.75,
            enabled: false,
        },
    );
    GenomeFile {
        seed: 42,
        generation: 29,
        fitness: 9010.937933603656,
        competence: Competence {
            alive_time: 214.3,
            wave: 4,
            asteroids: 63.0,
        },
        genome,
    }
}

#[test]
fn a_saved_genome_reloads_into_an_identical_network() {
    let original = sample_file();
    let path = scratch("round-trip.json");
    original.save(&path).expect("the file is written");
    let reloaded = GenomeFile::load(&path).expect("the file is read back");

    assert_eq!(reloaded.seed, original.seed);
    assert_eq!(reloaded.generation, original.generation);
    assert_eq!(reloaded.fitness, original.fitness);
    assert_eq!(reloaded.competence, original.competence);
    assert_eq!(reloaded.genome, original.genome);

    let mut before = Network::from_genome(&original.genome);
    let mut after = Network::from_genome(&reloaded.genome);
    let inputs = [0.3; nn::INPUTS];
    assert_eq!(before.activate(&inputs), after.activate(&inputs));
    assert_eq!(
        before.enabled_connections(),
        after.enabled_connections(),
        "the panel draws the same structure"
    );
}

#[test]
fn the_file_says_what_it_is() {
    let path = scratch("self-describing.json");
    sample_file().save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["format"], FORMAT);
    assert_eq!(value["version"], VERSION);
    assert_eq!(value["seed"], 42);
    assert_eq!(value["generation"], 29);
    assert!(value["competence"]["alive_time"].is_number());
    assert!(value["nodes"].is_array());
    assert!(value["connections"].is_array());
    assert_eq!(value["connections"][0].as_array().unwrap().len(), 5);
}

#[test]
fn a_genome_saved_from_a_run_can_be_watched_again() {
    let mut run = Run::with_options(606, RunOptions::new(8, 1));
    run.evaluate_generation();
    let file = run.best_file().expect("a best Genome exists");
    let path = scratch("from-run.json");
    file.save(&path).unwrap();
    let reloaded = GenomeFile::load(&path).unwrap();
    assert_eq!(reloaded.seed, 606);
    assert_eq!(reloaded.genome, file.genome);
    assert_eq!(
        file.file_name(),
        format!("seed606-g{}.json", file.generation)
    );

    let mut watched = Run::watch(999, &reloaded.genome, RunOptions::new(3, 1));
    watched.evaluate_generation();
    assert_eq!(watched.population().genomes[0], reloaded.genome);
}

#[test]
fn a_file_that_is_not_json_is_refused_by_name() {
    let path = scratch("garbage.json");
    std::fs::write(&path, "not json at all {").unwrap();
    let error = GenomeFile::load(&path).expect_err("refused");
    assert!(matches!(error, SaveError::NotJson { .. }));
    let message = error.to_string();
    assert!(message.contains("garbage.json"), "{message}");
    assert!(message.contains("not a NeuroArena save file"), "{message}");
}

#[test]
fn a_foreign_format_is_refused_and_the_message_names_both_formats() {
    let path = scratch("champion.json");
    std::fs::write(&path, r#"{"format":"neuroarena-champion","version":1}"#).unwrap();
    let error = GenomeFile::load(&path).expect_err("refused");
    assert!(matches!(error, SaveError::UnknownFormat { .. }));
    let message = error.to_string();
    assert!(message.contains("champion.json"), "{message}");
    assert!(message.contains("neuroarena-champion"), "{message}");
    assert!(message.contains(FORMAT), "{message}");
}

#[test]
fn an_unknown_version_is_refused_with_the_version_it_found() {
    let path = scratch("future.json");
    std::fs::write(&path, format!(r#"{{"format":"{FORMAT}","version":99}}"#)).unwrap();
    let error = GenomeFile::load(&path).expect_err("refused");
    assert!(matches!(error, SaveError::UnknownVersion { .. }));
    let message = error.to_string();
    assert!(message.contains("future.json"), "{message}");
    assert!(message.contains("99"), "{message}");
    assert!(message.contains(&VERSION.to_string()), "{message}");
}

#[test]
fn a_well_formed_file_with_a_broken_graph_is_refused() {
    let mut file = sample_file();
    file.genome = wired_genome(11, 21, 1.0);
    let mut text: serde_json::Value = serde_json::from_str(&file.to_json()).expect("valid JSON");
    // A connection to a node that does not exist.
    text["connections"] = serde_json::json!([[1, 11, 77, 0.5, true]]);
    let path = scratch("orphan.json");
    std::fs::write(&path, text.to_string()).unwrap();
    let error = GenomeFile::load(&path).expect_err("refused");
    assert!(matches!(error, SaveError::Invalid { .. }));
    assert!(error.to_string().contains("missing node"), "{error}");
}

#[test]
fn a_file_without_the_fixed_input_and_output_set_is_refused() {
    let mut file = sample_file();
    file.genome = wired_genome(11, 21, 1.0);
    let mut text: serde_json::Value = serde_json::from_str(&file.to_json()).unwrap();
    text["nodes"] = serde_json::json!([[0, "input"], [21, "output"]]);
    text["connections"] = serde_json::json!([]);
    let path = scratch("truncated.json");
    std::fs::write(&path, text.to_string()).unwrap();
    let error = GenomeFile::load(&path).expect_err("refused");
    assert!(matches!(error, SaveError::Invalid { .. }));
    assert!(error.to_string().contains("expected 21"), "{error}");
}

#[test]
fn a_missing_file_is_reported_with_its_path() {
    let path = scratch("does-not-exist.json");
    let error = GenomeFile::load(&path).expect_err("refused");
    assert!(matches!(error, SaveError::Io { .. }));
    assert!(error.to_string().contains("does-not-exist.json"), "{error}");
}

#[test]
fn saving_creates_the_directory() {
    let path = scratch("nested/deeper/save.json");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    sample_file().save(&path).expect("the directory is created");
    assert!(path.exists());
    GenomeFile::load(&path).expect("and the file reads back");
}
