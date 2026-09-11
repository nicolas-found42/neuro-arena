//! Genome: the evolvable genotype. Nodes carry typed ids, connections carry a
//! stable innovation number, a weight and an enabled flag.
//!
//! Ordering is explicit, never incidental: nodes are kept sorted by id and
//! connections by innovation number. The browser rendition got its determinism
//! partly from `Map` insertion order — an accident. Here the order is a
//! documented property of the container, so iteration is stable across runs,
//! machines and refactors.
//!
//! Draw *counts* are part of the contract, not an implementation detail:
//! add-connection retries up to [`neat::ADD_CONNECTION_ATTEMPTS`] times, and
//! weight mutation makes exactly two draws per connection. Both are preserved
//! here so one seed still means one run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{neat, nn};
use crate::rng::Rng;

pub type NodeId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Input,
    Hidden,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Connection {
    pub from: NodeId,
    pub to: NodeId,
    pub weight: f64,
    pub enabled: bool,
}

/// The ordered source of innovation numbers. Owned by a Population — never a
/// module-level singleton — so two runs in one process cannot share genes.
#[derive(Clone, Debug)]
pub struct InnovationTracker {
    next_id: NodeId,
    by_pair: BTreeMap<(NodeId, NodeId), u32>,
}

impl Default for InnovationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl InnovationTracker {
    pub fn new() -> Self {
        Self {
            next_id: nn::FIRST_HIDDEN_NODE_ID,
            by_pair: BTreeMap::new(),
        }
    }

    /// The innovation number for a connection, allocated on first sight.
    pub fn innovation(&mut self, from: NodeId, to: NodeId) -> u32 {
        if let Some(existing) = self.by_pair.get(&(from, to)) {
            return *existing;
        }
        let innov = self.next_id;
        self.next_id += 1;
        self.by_pair.insert((from, to), innov);
        innov
    }

    /// A fresh node id, drawn from the same counter as innovations.
    pub fn new_node_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Rebuild a tracker around an existing Genome — a loaded save, or the
    /// champion a watch run replays — so a later mutation cannot hand out an
    /// innovation number or node id the Genome already uses, and a pair that
    /// already has a gene keeps its number.
    pub fn from_genome(genome: &Genome) -> Self {
        let mut tracker = Self::new();
        let mut next = nn::FIRST_HIDDEN_NODE_ID;
        for (innovation, connection) in genome.connections() {
            tracker
                .by_pair
                .insert((connection.from, connection.to), innovation);
            next = next.max(innovation + 1);
        }
        for (id, _) in genome.nodes() {
            next = next.max(id + 1);
        }
        tracker.next_id = next;
        tracker
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Genome {
    nodes: BTreeMap<NodeId, NodeType>,
    connections: BTreeMap<u32, Connection>,
}

impl Genome {
    /// The initial genome: every input wired to every output, no hidden nodes.
    pub fn new(rng: &mut Rng, tracker: &mut InnovationTracker) -> Self {
        let mut nodes = BTreeMap::new();
        for i in 0..nn::INPUTS as NodeId {
            nodes.insert(i, NodeType::Input);
        }
        for out in nn::OUTPUT_IDS {
            nodes.insert(out, NodeType::Output);
        }
        let mut connections = BTreeMap::new();
        for i in 0..nn::INPUTS as NodeId {
            for out in nn::OUTPUT_IDS {
                let innov = tracker.innovation(i, out);
                connections.insert(
                    innov,
                    Connection {
                        from: i,
                        to: out,
                        weight: rng.range(neat::WEIGHT_INIT_MIN, neat::WEIGHT_INIT_MAX),
                        enabled: true,
                    },
                );
            }
        }
        Self { nodes, connections }
    }

    /// An empty genome, the starting point for crossover and for loading.
    pub fn blank() -> Self {
        Self {
            nodes: BTreeMap::new(),
            connections: BTreeMap::new(),
        }
    }

    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, NodeType)> + '_ {
        self.nodes.iter().map(|(id, t)| (*id, *t))
    }

    /// Connections in innovation order, with their innovation numbers.
    pub fn connections(&self) -> impl Iterator<Item = (u32, Connection)> + '_ {
        self.connections.iter().map(|(i, c)| (*i, *c))
    }

    pub fn node_type(&self, id: NodeId) -> Option<NodeType> {
        self.nodes.get(&id).copied()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn enabled_connection_count(&self) -> usize {
        self.connections.values().filter(|c| c.enabled).count()
    }

    pub fn hidden_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|t| **t == NodeType::Hidden)
            .count()
    }

    pub fn has_node(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    /// Insert a node, used by the save loader.
    pub fn insert_node(&mut self, id: NodeId, node_type: NodeType) {
        self.nodes.insert(id, node_type);
    }

    /// Insert a connection under an explicit innovation number, used by the
    /// save loader. A connection whose endpoints are missing is a broken file,
    /// so the caller validates before inserting.
    pub fn insert_connection(&mut self, innovation: u32, connection: Connection) {
        self.connections.insert(innovation, connection);
    }

    pub fn deep_copy(&self) -> Self {
        self.clone()
    }

    /// Perturb and replace weights. Two draws per connection, in innovation
    /// order; a draw always happens whether or not the first one fired.
    pub fn mutate_weights(&mut self, rng: &mut Rng) {
        for connection in self.connections.values_mut() {
            if rng.chance(neat::WEIGHT_PERTURB_RATE) {
                connection.weight = crate::math::clamp(
                    connection.weight + rng.normal() * neat::WEIGHT_PERTURB_SIGMA,
                    neat::WEIGHT_MIN,
                    neat::WEIGHT_MAX,
                );
            }
            if rng.chance(neat::WEIGHT_REPLACE_RATE) {
                connection.weight =
                    rng.range(neat::WEIGHT_REPLACE_MIN, neat::WEIGHT_REPLACE_MAX);
            }
        }
    }

    /// Is there a path `from → to` over enabled connections?
    fn has_path(&self, from: NodeId, to: NodeId) -> bool {
        let mut stack = vec![from];
        let mut seen = vec![false; self.nodes.keys().copied().max().unwrap_or(0) as usize + 1];
        while let Some(id) = stack.pop() {
            if id == to {
                return true;
            }
            if seen[id as usize] {
                continue;
            }
            seen[id as usize] = true;
            for connection in self.connections.values() {
                if connection.enabled && connection.from == id {
                    stack.push(connection.to);
                }
            }
        }
        false
    }

    /// Try to add one connection. Rejected candidates (self-loops, existing
    /// pairs, cycles) consume an attempt; the attempt budget is fixed, so the
    /// stream position after this call is data-dependent but reproducible.
    pub fn mutate_add_connection(&mut self, rng: &mut Rng, tracker: &mut InnovationTracker) {
        let a_pool: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, t)| **t != NodeType::Output)
            .map(|(id, _)| *id)
            .collect();
        let b_pool: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, t)| **t != NodeType::Input)
            .map(|(id, _)| *id)
            .collect();
        if a_pool.is_empty() || b_pool.is_empty() {
            return;
        }
        for _ in 0..neat::ADD_CONNECTION_ATTEMPTS {
            let a = a_pool[rng.below(a_pool.len())];
            let b = b_pool[rng.below(b_pool.len())];
            if a == b {
                continue;
            }
            if self
                .connections
                .values()
                .any(|c| c.from == a && c.to == b)
            {
                continue;
            }
            if self.has_path(b, a) {
                continue;
            }
            let innov = tracker.innovation(a, b);
            self.connections.insert(
                innov,
                Connection {
                    from: a,
                    to: b,
                    weight: rng.range(neat::WEIGHT_INIT_MIN, neat::WEIGHT_INIT_MAX),
                    enabled: true,
                },
            );
            return;
        }
    }

    /// Split one enabled connection with a new hidden node: the old connection
    /// is disabled and the new node inherits its weight on the outgoing edge.
    pub fn mutate_add_node(&mut self, rng: &mut Rng, tracker: &mut InnovationTracker) {
        let enabled: Vec<u32> = self
            .connections
            .iter()
            .filter(|(_, c)| c.enabled)
            .map(|(innov, _)| *innov)
            .collect();
        if enabled.is_empty() {
            return;
        }
        let innov = enabled[rng.below(enabled.len())];
        let connection = self.connections.get_mut(&innov).expect("just listed");
        connection.enabled = false;
        let from = connection.from;
        let to = connection.to;
        let weight = connection.weight;
        let id = tracker.new_node_id();
        self.nodes.insert(id, NodeType::Hidden);
        let first = tracker.innovation(from, id);
        let second = tracker.innovation(id, to);
        self.connections.insert(
            first,
            Connection {
                from,
                to: id,
                weight: 1.0,
                enabled: true,
            },
        );
        self.connections.insert(
            second,
            Connection {
                from: id,
                to,
                weight,
                enabled: true,
            },
        );
    }

    /// Crossover aligning matching innovations.
    ///
    /// `a_fitter`: `Some(true)` when `a` wins, `Some(false)` when `b` wins,
    /// `None` when the parents are equal — then every gene comes from both and
    /// matching pairs are chosen by coin flip.
    ///
    /// Child nodes are the fixed input/output set plus every hidden endpoint a
    /// carried gene references. Unioning the parents' node sets would create
    /// connection-less orphans.
    pub fn crossover(a: &Genome, b: &Genome, a_fitter: Option<bool>, rng: &mut Rng) -> Genome {
        let mut child = Genome::blank();
        for i in 0..nn::INPUTS as NodeId {
            child.nodes.insert(i, NodeType::Input);
        }
        for out in nn::OUTPUT_IDS {
            child.nodes.insert(out, NodeType::Output);
        }
        let equal = a_fitter.is_none();
        for (innov, ca) in &a.connections {
            match b.connections.get(innov) {
                Some(cb) => {
                    let pick = if equal {
                        if rng.chance(0.5) {
                            ca
                        } else {
                            cb
                        }
                    } else if a_fitter == Some(true) {
                        ca
                    } else {
                        cb
                    };
                    child.connections.insert(*innov, *pick);
                }
                None => {
                    if a_fitter != Some(false) {
                        child.connections.insert(*innov, *ca);
                    }
                }
            }
        }
        if a_fitter != Some(true) {
            for (innov, cb) in &b.connections {
                if !a.connections.contains_key(innov) {
                    child.connections.insert(*innov, *cb);
                }
            }
        }
        let endpoints: Vec<(NodeId, NodeId)> = child
            .connections
            .values()
            .map(|c| (c.from, c.to))
            .collect();
        for (from, to) in endpoints {
            child.ensure_endpoint(from);
            child.ensure_endpoint(to);
        }
        child
    }

    fn ensure_endpoint(&mut self, id: NodeId) {
        if self.nodes.contains_key(&id) {
            return;
        }
        let node_type = if (id as usize) < nn::INPUTS {
            NodeType::Input
        } else if nn::OUTPUT_IDS.contains(&id) {
            NodeType::Output
        } else {
            NodeType::Hidden
        };
        self.nodes.insert(id, node_type);
    }

    /// Compatibility distance: `(c1 * excess + c2 * disjoint) / size + c3 * mean
    /// matching weight difference`, where size is the larger gene count. Only
    /// enabled-matching pairs contribute to the weight term.
    pub fn distance(a: &Genome, b: &Genome) -> f64 {
        let max_a = a.connections.keys().next_back().copied();
        let max_b = b.connections.keys().next_back().copied();
        let mut excess = 0.0;
        let mut disjoint = 0.0;
        let mut weight_sum = 0.0;
        let mut weight_count = 0.0;
        for (innov, ca) in &a.connections {
            match b.connections.get(innov) {
                Some(cb) => {
                    if ca.enabled && cb.enabled {
                        weight_sum += (ca.weight - cb.weight).abs();
                        weight_count += 1.0;
                    }
                }
                None => {
                    if Some(*innov) > max_b {
                        excess += 1.0;
                    } else {
                        disjoint += 1.0;
                    }
                }
            }
        }
        for innov in b.connections.keys() {
            if !a.connections.contains_key(innov) {
                if Some(*innov) > max_a {
                    excess += 1.0;
                } else {
                    disjoint += 1.0;
                }
            }
        }
        let size = a
            .connections
            .len()
            .max(b.connections.len())
            .max(1) as f64;
        let weight_bar = if weight_count > 0.0 {
            weight_sum / weight_count
        } else {
            0.0
        };
        (neat::DISTANCE_C1 * excess + neat::DISTANCE_C2 * disjoint) / size + neat::DISTANCE_C3 * weight_bar
    }
}
