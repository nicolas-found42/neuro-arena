//! Network: the feedforward phenotype derived from a Genome.
//!
//! Evaluation is `tanh` over the enabled incoming connections, in Kahn
//! topological order. Connections added by mutation can only ever point
//! forward, but a loaded genome is untrusted input, so the order falls back to
//! the node-id order for anything the topological sort cannot reach rather than
//! looping.
//!
//! The Network keeps the last activation of every node, which is what the app's
//! network panel draws: a live trace from Sensors to actions.

use crate::config::nn;
use crate::genome::{Genome, NodeId, NodeType};

#[derive(Clone, Debug)]
pub struct Network {
    nodes: Vec<(NodeId, NodeType)>,
    /// Evaluation order: Kahn order, then any node the sort could not reach.
    order: Vec<NodeId>,
    is_input: Vec<bool>,
    /// Incoming enabled connections per node id.
    incoming: Vec<Vec<(NodeId, f64)>>,
    values: Vec<f64>,
    outputs: [NodeId; nn::OUTPUTS],
}

impl Network {
    pub fn from_genome(genome: &Genome) -> Self {
        let nodes: Vec<(NodeId, NodeType)> = genome.nodes().collect();
        let size = nodes
            .iter()
            .map(|(id, _)| *id as usize + 1)
            .max()
            .unwrap_or(0);
        let present = {
            let mut present = vec![false; size];
            for (id, _) in &nodes {
                present[*id as usize] = true;
            }
            present
        };
        let mut is_input = vec![false; size];
        for (id, node_type) in &nodes {
            is_input[*id as usize] = *node_type == NodeType::Input;
        }

        let mut incoming: Vec<Vec<(NodeId, f64)>> = vec![Vec::new(); size];
        let mut adjacency: Vec<Vec<NodeId>> = vec![Vec::new(); size];
        let mut in_degree: Vec<u32> = vec![0; size];
        for (_, connection) in genome.connections() {
            if !connection.enabled {
                continue;
            }
            let (from, to) = (connection.from as usize, connection.to as usize);
            if !present[from] || !present[to] {
                // A gene whose endpoint is missing cannot contribute; the save
                // loader rejects such files, and mutation never creates them.
                continue;
            }
            incoming[to].push((connection.from, connection.weight));
            adjacency[from].push(connection.to);
            in_degree[to] += 1;
        }

        let mut queue: std::collections::VecDeque<NodeId> = nodes
            .iter()
            .filter(|(id, _)| in_degree[*id as usize] == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut order = Vec::with_capacity(nodes.len());
        let mut ordered = vec![false; size];
        while let Some(id) = queue.pop_front() {
            order.push(id);
            ordered[id as usize] = true;
            for next in &adjacency[id as usize] {
                in_degree[*next as usize] -= 1;
                if in_degree[*next as usize] == 0 {
                    queue.push_back(*next);
                }
            }
        }
        for (id, _) in &nodes {
            if !ordered[*id as usize] {
                order.push(*id);
            }
        }

        Self {
            nodes,
            order,
            is_input,
            incoming,
            values: vec![0.0; size],
            outputs: nn::OUTPUT_IDS,
        }
    }

    /// Evaluate one step. `inputs` is indexed by input node id.
    pub fn activate(&mut self, inputs: &[f64; nn::INPUTS]) -> [f64; nn::OUTPUTS] {
        self.values[..nn::INPUTS].copy_from_slice(inputs);
        for index in 0..self.order.len() {
            let id = self.order[index] as usize;
            if self.is_input[id] {
                continue;
            }
            let mut sum = 0.0;
            for (from, weight) in &self.incoming[id] {
                sum += weight * self.values[*from as usize];
            }
            self.values[id] = sum.tanh();
        }
        let mut out = [0.0; nn::OUTPUTS];
        for (slot, id) in self.outputs.iter().enumerate() {
            out[slot] = self.values[*id as usize];
        }
        out
    }

    /// Last activation of a node, for the network panel. Unknown ids read 0.
    pub fn activation(&self, id: NodeId) -> f64 {
        self.values.get(id as usize).copied().unwrap_or(0.0)
    }

    /// Nodes in id order, for the network panel's layout.
    pub fn nodes(&self) -> &[(NodeId, NodeType)] {
        &self.nodes
    }

    /// Enabled connections as `(from, to, weight)`, in a stable `(from, to)`
    /// order, for the network panel.
    pub fn enabled_connections(&self) -> Vec<(NodeId, NodeId, f64)> {
        let mut out = Vec::new();
        for (to, list) in self.incoming.iter().enumerate() {
            for (from, weight) in list {
                out.push((*from, to as NodeId, *weight));
            }
        }
        out.sort_by_key(|(from, to, _)| (*from, *to));
        out
    }

    pub fn output_ids(&self) -> [NodeId; nn::OUTPUTS] {
        self.outputs
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn enabled_connection_count(&self) -> usize {
        self.incoming.iter().map(Vec::len).sum()
    }
}
