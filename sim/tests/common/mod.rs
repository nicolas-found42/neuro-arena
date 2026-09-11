//! Shared fixtures for the simulation tests. Everything here builds a scenario
//! out of the crate's public API only, exactly as the app does.

use sim::config::asteroid::Size;
use sim::genome::{Connection, Genome, NodeId, NodeType};
use sim::{Asteroid, Rng, World};

/// A World with no rock field and no brain: tests add exactly the rocks they
/// mean to reason about.
pub fn quiet_world(seed: u32) -> World {
    let mut world = World::new(Rng::from_seed(seed), None);
    world.asteroids.clear();
    world
}

/// A stationary rock at `(x, y)`: shape and spin are drawn from a fixed seed so
/// the scenario is identical every run.
pub fn rock(size: Size, x: f64, y: f64) -> Asteroid {
    let mut rng = Rng::from_seed(0x5EED);
    let mut asteroid = Asteroid::new(size, x, y, 0.0, 0.0, &mut rng);
    asteroid.vx = 0.0;
    asteroid.vy = 0.0;
    asteroid.spin = 0.0;
    asteroid
}

/// The same, with a velocity.
pub fn moving_rock(size: Size, x: f64, y: f64, vx: f64, vy: f64) -> Asteroid {
    let mut asteroid = rock(size, x, y);
    asteroid.vx = vx;
    asteroid.vy = vy;
    asteroid
}

/// A Genome that never acts: the fixed input/output set with no connections.
pub fn silent_genome() -> Genome {
    let mut genome = Genome::blank();
    for id in 0..sim::config::nn::INPUTS as NodeId {
        genome.insert_node(id, NodeType::Input);
    }
    for id in sim::config::nn::OUTPUT_IDS {
        genome.insert_node(id, NodeType::Output);
    }
    genome
}

/// A Genome with one wire: input `from` drives output `to` with `weight`.
/// With `from = 11` (bias) the output is a constant `tanh(weight)`.
pub fn wired_genome(from: NodeId, to: NodeId, weight: f64) -> Genome {
    let mut genome = silent_genome();
    genome.insert_connection(
        1000,
        Connection {
            from,
            to,
            weight,
            enabled: true,
        },
    );
    genome
}
