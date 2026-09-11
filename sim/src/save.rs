//! The save file: self-describing, versioned, and readable without this
//! program's source.
//!
//! The retired `champions/*.json` format is not read — there is no loader for
//! it and no legacy branch anywhere (ADR 0004). A file whose `format` or
//! `version` is not known is rejected with a message that names the file and
//! the reason, so bad data never takes the app down.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::competence::Competence;
use crate::config::nn;
use crate::genome::{Connection, Genome, NodeId, NodeType};

pub const FORMAT: &str = "neuroarena-genome";
pub const VERSION: u32 = 1;

/// A Genome with the context needed to interpret it: the seed and Generation it
/// came from, its banked Fitness, and the Competence Gate metrics of its run.
#[derive(Clone, Debug)]
pub struct GenomeFile {
    pub seed: u32,
    pub generation: u32,
    pub fitness: f64,
    pub competence: Competence,
    pub genome: Genome,
}

#[derive(Serialize, Deserialize)]
struct Wire {
    format: String,
    version: u32,
    seed: u32,
    generation: u32,
    fitness: f64,
    competence: Competence,
    nodes: Vec<(NodeId, NodeType)>,
    connections: Vec<(u32, NodeId, NodeId, f64, bool)>,
}

#[derive(Debug)]
pub enum SaveError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    NotJson {
        path: PathBuf,
        reason: String,
    },
    UnknownFormat {
        path: PathBuf,
        found: String,
    },
    UnknownVersion {
        path: PathBuf,
        found: String,
    },
    Invalid {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io { path, source } => {
                write!(f, "{}: cannot read or write the file ({source})", path.display())
            }
            SaveError::NotJson { path, reason } => write!(
                f,
                "{} is not a NeuroArena save file: not readable as JSON ({reason})",
                path.display()
            ),
            SaveError::UnknownFormat { path, found } => write!(
                f,
                "{} is not a NeuroArena save file: format is \"{found}\", expected \"{FORMAT}\"",
                path.display()
            ),
            SaveError::UnknownVersion { path, found } => write!(
                f,
                "{} is a NeuroArena save file of version {found}; this build reads version {VERSION}",
                path.display()
            ),
            SaveError::Invalid { path, reason } => {
                write!(f, "{} is not a valid NeuroArena save file: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for SaveError {}

impl GenomeFile {
    /// The suggested file name for a save: seed and Generation, so a saved
    /// Genome is identifiable in a directory listing.
    pub fn file_name(&self) -> String {
        format!("seed{}-g{}.json", self.seed, self.generation)
    }

    pub fn to_json(&self) -> String {
        let wire = Wire {
            format: FORMAT.to_string(),
            version: VERSION,
            seed: self.seed,
            generation: self.generation,
            fitness: self.fitness,
            competence: self.competence,
            nodes: self.genome.nodes().collect(),
            connections: self
                .genome
                .connections()
                .map(|(innovation, c)| (innovation, c.from, c.to, c.weight, c.enabled))
                .collect(),
        };
        serde_json::to_string_pretty(&wire).expect("a Genome is always serializable")
    }

    /// Write the file, creating the directory if needed.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SaveError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|source| SaveError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
        }
        std::fs::write(path, self.to_json()).map_err(|source| SaveError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Read and validate a save file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SaveError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| SaveError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_json_str(&text, path)
    }

    /// Parse a save file from text. `path` is carried only so error messages can
    /// name the file the text came from.
    pub fn from_json_str(text: &str, path: impl AsRef<Path>) -> Result<Self, SaveError> {
        let path = path.as_ref();
        let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
            SaveError::NotJson {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;

        match value.get("format").and_then(|v| v.as_str()) {
            Some(FORMAT) => {}
            Some(other) => {
                return Err(SaveError::UnknownFormat {
                    path: path.to_path_buf(),
                    found: other.to_string(),
                })
            }
            None => {
                return Err(SaveError::UnknownFormat {
                    path: path.to_path_buf(),
                    found: "missing".to_string(),
                })
            }
        }
        match value.get("version").and_then(|v| v.as_u64()) {
            Some(v) if v == u64::from(VERSION) => {}
            Some(other) => {
                return Err(SaveError::UnknownVersion {
                    path: path.to_path_buf(),
                    found: other.to_string(),
                })
            }
            None => {
                return Err(SaveError::UnknownVersion {
                    path: path.to_path_buf(),
                    found: "missing".to_string(),
                })
            }
        }

        let wire: Wire = serde_json::from_value(value).map_err(|error| SaveError::Invalid {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;

        let mut genome = Genome::blank();
        for (id, node_type) in &wire.nodes {
            genome.insert_node(*id, *node_type);
        }
        if !path_has_fixed_io(&genome) {
            return Err(SaveError::Invalid {
                path: path.to_path_buf(),
                reason: format!(
                    "expected {} input and {} output nodes, found {} and {}",
                    nn::INPUTS,
                    nn::OUTPUTS,
                    genome
                        .nodes()
                        .filter(|(_, t)| *t == NodeType::Input)
                        .count(),
                    genome
                        .nodes()
                        .filter(|(_, t)| *t == NodeType::Output)
                        .count()
                ),
            });
        }
        for (innovation, from, to, weight, enabled) in &wire.connections {
            if !genome.has_node(*from) || !genome.has_node(*to) {
                return Err(SaveError::Invalid {
                    path: path.to_path_buf(),
                    reason: format!(
                        "connection {innovation} references a missing node ({from} → {to})"
                    ),
                });
            }
            genome.insert_connection(
                *innovation,
                Connection {
                    from: *from,
                    to: *to,
                    weight: *weight,
                    enabled: *enabled,
                },
            );
        }
        if !wire.fitness.is_finite() {
            return Err(SaveError::Invalid {
                path: path.to_path_buf(),
                reason: "fitness is not a finite number".to_string(),
            });
        }

        Ok(Self {
            seed: wire.seed,
            generation: wire.generation,
            fitness: wire.fitness,
            competence: wire.competence,
            genome,
        })
    }
}

/// The input/output set is fixed; a file that disagrees is not one of ours.
fn path_has_fixed_io(genome: &Genome) -> bool {
    let inputs = genome
        .nodes()
        .filter(|(_, t)| *t == NodeType::Input)
        .count();
    let outputs = genome
        .nodes()
        .filter(|(_, t)| *t == NodeType::Output)
        .count();
    inputs == nn::INPUTS && outputs == nn::OUTPUTS
}
