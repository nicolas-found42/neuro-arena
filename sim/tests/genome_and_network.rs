//! Genome and Network: mutation, crossover, innovation numbering, the
//! activation order, and the invariants that keep a Genotype a valid graph.

mod common;

use common::{silent_genome, wired_genome};
use sim::config::neat;
use sim::genome::{Connection, Genome, InnovationTracker, NodeType};
use sim::{Network, Rng};

fn weights(genome: &Genome) -> Vec<f64> {
    genome.connections().map(|(_, c)| c.weight).collect()
}

#[test]
fn a_network_evaluates_its_hidden_layer_in_topological_order() {
    let mut genome = silent_genome();
    genome.insert_node(26, NodeType::Hidden);
    genome.insert_connection(
        100,
        Connection {
            from: 0,
            to: 26,
            weight: 2.0,
            enabled: true,
        },
    );
    genome.insert_connection(
        101,
        Connection {
            from: 26,
            to: 21,
            weight: 3.0,
            enabled: true,
        },
    );
    let mut network = Network::from_genome(&genome);
    let mut inputs = [0.0; sim::config::nn::INPUTS];
    inputs[0] = 1.0;
    let out = network.activate(&inputs);
    assert!((out[0] - (3.0 * 2.0_f64.tanh()).tanh()).abs() < 1e-12);
    assert!((network.activation(26) - 2.0_f64.tanh()).abs() < 1e-12);
    // The panel can read every node's last activation, hidden ones included.
    assert_eq!(network.activation(0), 1.0);
}

#[test]
fn a_disabled_connection_stops_contributing() {
    let mut genome = wired_genome(11, 21, 4.0);
    let mut network = Network::from_genome(&genome);
    let mut inputs = [0.0; sim::config::nn::INPUTS];
    inputs[11] = 1.0;
    assert!((network.activate(&inputs)[0] - 4.0_f64.tanh()).abs() < 1e-12);

    genome.insert_connection(
        1000,
        Connection {
            from: 11,
            to: 21,
            weight: 4.0,
            enabled: false,
        },
    );
    let mut network = Network::from_genome(&genome);
    assert_eq!(network.activate(&inputs)[0], 0.0);
    assert_eq!(network.enabled_connection_count(), 0);
}

#[test]
fn crossover_takes_matching_genes_and_keeps_the_fitter_parents_excess() {
    let mut a = wired_genome(11, 21, 1.0);
    a.insert_connection(
        5,
        Connection {
            from: 0,
            to: 22,
            weight: 2.0,
            enabled: true,
        },
    );
    let mut b = wired_genome(11, 21, -1.0);
    b.insert_connection(
        9,
        Connection {
            from: 1,
            to: 23,
            weight: 3.0,
            enabled: true,
        },
    );
    let mut rng = Rng::from_seed(4);

    let fitter = Genome::crossover(&a, &b, Some(true), &mut rng);
    // The shared innovation takes the fitter parent's weight; the excess gene
    // comes from the fitter parent only.
    let shared = fitter
        .connections()
        .find(|(innovation, _)| *innovation == 1000)
        .expect("the shared gene survives");
    assert_eq!(shared.1.weight, 1.0);
    assert!(fitter.connections().any(|(innovation, _)| innovation == 5));
    assert!(!fitter.connections().any(|(innovation, _)| innovation == 9));

    let other = Genome::crossover(&a, &b, Some(false), &mut rng);
    let shared = other
        .connections()
        .find(|(innovation, _)| *innovation == 1000)
        .expect("the shared gene survives");
    assert_eq!(shared.1.weight, -1.0);
    assert!(other.connections().any(|(innovation, _)| innovation == 9));
    assert!(!other.connections().any(|(innovation, _)| innovation == 5));

    let equal = Genome::crossover(&a, &b, None, &mut rng);
    let shared_weight = equal
        .connections()
        .find(|(innovation, _)| *innovation == 1000)
        .unwrap()
        .1
        .weight;
    assert!(shared_weight == 1.0 || shared_weight == -1.0);
    assert_eq!(
        equal.connection_count(),
        3,
        "equal parents contribute every gene"
    );
}

#[test]
fn crossover_never_leaves_a_connection_without_its_nodes() {
    let mut a = silent_genome();
    let mut tracker = InnovationTracker::new();
    a.mutate_add_node(&mut Rng::from_seed(1), &mut tracker);
    let mut b = silent_genome();
    b.mutate_add_node(&mut Rng::from_seed(2), &mut tracker);
    let child = Genome::crossover(&a, &b, None, &mut Rng::from_seed(3));
    for (_, connection) in child.connections() {
        assert!(
            child.has_node(connection.from) && child.has_node(connection.to),
            "connection {connection:?} has an endpoint that is not a node"
        );
    }
}

#[test]
fn adding_a_node_splits_one_connection_into_two() {
    let mut genome = wired_genome(11, 21, 2.5);
    let mut tracker = InnovationTracker::from_genome(&genome);
    let connections_before = genome.connection_count();
    genome.mutate_add_node(&mut Rng::from_seed(5), &mut tracker);

    assert_eq!(genome.node_count(), 21 + 5 + 1);
    assert_eq!(genome.connection_count(), connections_before + 2);
    assert_eq!(
        genome.enabled_connection_count(),
        connections_before - 1 + 2,
        "the split connection is disabled and two new ones appear"
    );
    let hidden: Vec<u32> = genome
        .nodes()
        .filter(|(_, node_type)| *node_type == NodeType::Hidden)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(hidden.len(), 1);
    let hidden = hidden[0];
    let incoming = genome
        .connections()
        .find(|(_, c)| c.from == 11 && c.to == hidden)
        .expect("input feeds the new node");
    assert_eq!(incoming.1.weight, 1.0);
    let outgoing = genome
        .connections()
        .find(|(_, c)| c.from == hidden && c.to == 21)
        .expect("the new node feeds the old target");
    assert_eq!(outgoing.1.weight, 2.5, "the old weight moves to the far side");
}

#[test]
fn mutations_keep_the_graph_acyclic_and_orphan_free() {
    let mut tracker = InnovationTracker::new();
    let mut rng = Rng::from_seed(6);
    let mut genome = Genome::new(&mut rng, &mut tracker);
    for _ in 0..400 {
        genome.mutate_add_node(&mut rng, &mut tracker);
        genome.mutate_add_connection(&mut rng, &mut tracker);
        genome.mutate_weights(&mut rng);
    }
    let edges: Vec<(u32, u32)> = genome
        .connections()
        .filter(|(_, c)| c.enabled)
        .map(|(_, c)| (c.from, c.to))
        .collect();
    for (_, connection) in genome.connections() {
        assert!(
            genome.has_node(connection.from) && genome.has_node(connection.to),
            "orphan connection {connection:?}"
        );
    }
    // No path may lead from a connection's target back to its source.
    for (from, to) in &edges {
        let mut stack = vec![*to];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            assert_ne!(id, *from, "cycle through {from} -> {to}");
            if !seen.insert(id) {
                continue;
            }
            for (a, b) in &edges {
                if *a == id {
                    stack.push(*b);
                }
            }
        }
    }
    // A feedforward Network still evaluates to finite outputs.
    let mut network = Network::from_genome(&genome);
    let out = network.activate(&[1.0; sim::config::nn::INPUTS]);
    assert!(out.iter().all(|v| v.is_finite()));
}

#[test]
fn innovation_numbers_are_stable_per_pair_and_fresh_per_node() {
    let mut tracker = InnovationTracker::new();
    let first = tracker.innovation(3, 22);
    assert_eq!(tracker.innovation(3, 22), first, "the same pair reuses its id");
    assert_ne!(tracker.innovation(4, 22), first);
    let mut fresh = InnovationTracker::new();
    assert_eq!(fresh.new_node_id(), sim::config::nn::FIRST_HIDDEN_NODE_ID);
    assert_eq!(
        fresh.new_node_id(),
        sim::config::nn::FIRST_HIDDEN_NODE_ID + 1,
        "node ids and innovations share one counter"
    );
}

#[test]
fn a_rebuilt_tracker_does_not_reissue_an_existing_id() {
    let mut tracker = InnovationTracker::new();
    let mut rng = Rng::from_seed(7);
    let mut genome = Genome::new(&mut rng, &mut tracker);
    for _ in 0..20 {
        genome.mutate_add_node(&mut rng, &mut tracker);
    }
    let mut rebuilt = InnovationTracker::from_genome(&genome);
    let hidden = genome
        .nodes()
        .filter(|(_, t)| *t == NodeType::Hidden)
        .map(|(id, _)| id)
        .max()
        .expect("hidden nodes exist");
    assert!(rebuilt.new_node_id() > hidden, "node ids never collide");
    for (innovation, connection) in genome.connections() {
        assert_eq!(
            rebuilt.innovation(connection.from, connection.to),
            innovation,
            "a pair that already has a gene keeps its number"
        );
    }
}

#[test]
fn mutation_is_reproducible_from_a_seed() {
    let run = |seed: u32| {
        let mut rng = Rng::from_seed(seed);
        let mut tracker = InnovationTracker::new();
        let mut genome = Genome::new(&mut rng, &mut tracker);
        for _ in 0..50 {
            genome.mutate_add_node(&mut rng, &mut tracker);
            genome.mutate_add_connection(&mut rng, &mut tracker);
            genome.mutate_weights(&mut rng);
        }
        genome
    };
    let first = run(11);
    let second = run(11);
    assert_eq!(first, second, "one seed, one lineage");
    assert_eq!(weights(&first), weights(&second));
    assert_ne!(first.connection_count(), 105, "the lineage really grew");
}

#[test]
fn weights_stay_inside_the_pinned_range() {
    let mut tracker = InnovationTracker::new();
    let mut rng = Rng::from_seed(8);
    let mut genome = Genome::new(&mut rng, &mut tracker);
    for _ in 0..500 {
        genome.mutate_add_node(&mut rng, &mut tracker);
        genome.mutate_add_connection(&mut rng, &mut tracker);
        genome.mutate_weights(&mut rng);
    }
    for (_, connection) in genome.connections() {
        assert!(
            (neat::WEIGHT_MIN..=neat::WEIGHT_MAX).contains(&connection.weight),
            "weight {} out of range",
            connection.weight
        );
    }
}

#[test]
fn distance_is_zero_for_identical_genomes_and_grows_with_divergence() {
    let mut tracker = InnovationTracker::new();
    let mut rng = Rng::from_seed(9);
    let mut a = Genome::new(&mut rng, &mut tracker);
    let mut b = a.clone();
    assert_eq!(Genome::distance(&a, &b), 0.0);

    b.mutate_add_node(&mut rng, &mut tracker);
    assert!(Genome::distance(&a, &b) > 0.0, "a new gene shows up");

    let far = Genome::new(&mut rng, &mut tracker);
    a.mutate_weights(&mut rng);
    assert!(Genome::distance(&a, &far) > 0.0, "weights count too");
}

#[test]
fn add_connection_refuses_a_pair_that_already_exists() {
    let mut tracker = InnovationTracker::new();
    let mut rng = Rng::from_seed(10);
    let mut genome = Genome::new(&mut rng, &mut tracker);
    let before = genome.connection_count();
    assert_eq!(before, 105, "the founding Genome is fully wired");
    for _ in 0..200 {
        // Every input-output pair already exists, so nothing can be added
        // until a hidden node opens new pairs.
        genome.mutate_add_connection(&mut rng, &mut tracker);
    }
    assert_eq!(
        genome.connection_count(),
        before,
        "a fully wired Genome has no free pair"
    );
    genome.mutate_add_node(&mut rng, &mut tracker);
    for _ in 0..200 {
        genome.mutate_add_connection(&mut rng, &mut tracker);
    }
    let pairs: std::collections::HashSet<(u32, u32)> = genome
        .connections()
        .map(|(_, c)| (c.from, c.to))
        .collect();
    assert_eq!(
        pairs.len(),
        genome.connection_count(),
        "no pair is wired twice"
    );
    assert!(genome.connection_count() > before, "some were added");
}
