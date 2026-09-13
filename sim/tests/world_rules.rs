//! World rules: the twenty-odd literally-true invariants of the retired browser
//! harness, rewritten as Rust tests against the `sim` public API.
//!
//! Every assertion here is something a player of the simulation can observe:
//! a Ship dies, a Wave grows, an asteroid splits, a Sensor Ray sees across the seam.

mod common;

use common::{asteroid, moving_asteroid, quiet_world, wired_genome};
use sim::config::asteroid::{Size, SPEED_CAP};
use sim::config::{nn, ship as ship_cfg, world as world_cfg, DT};
use sim::genome::NodeType;
use sim::{Network, World};

#[test]
fn a_ship_that_flies_into_an_asteroid_dies() {
    let mut world = quiet_world(1);
    world.agent.ship.x = 60.0;
    world.agent.ship.y = 300.0;
    world.agent.ship.heading = std::f64::consts::PI;
    // 65 px away over the seam: the Ship must fly across it to reach the asteroid.
    world.asteroids.push(asteroid(Size::Small, 955.0, 300.0));
    assert!(world.agent.alive);

    world.agent.ship.vx = -100.0;
    let mut steps = 0;
    while world.agent.alive && steps < 120 {
        world.step_fixed();
        steps += 1;
    }
    assert!(
        !world.agent.alive,
        "the Ship crossed the seam into an asteroid"
    );
    assert!(
        (15..60).contains(&steps),
        "the Ship flew into the asteroid rather than spawning on it: {steps} steps"
    );
    assert!(
        world.agent.ship.x < 30.0,
        "it died on the far side of the Arena, heading for the seam: {}",
        world.agent.ship.x
    );
}

#[test]
fn a_ship_survives_an_asteroid_that_misses() {
    let mut world = quiet_world(2);
    world.agent.ship.x = 480.0;
    world.agent.ship.y = 300.0;
    world.agent.ship.vx = 200.0; // flying past an asteroid 100 px off the line
    world.asteroids.push(asteroid(Size::Small, 480.0, 200.0));
    for _ in 0..600 {
        world.step_fixed();
    }
    assert!(world.agent.alive);
    assert!(world.agent.ship.x != 480.0, "the Ship moved");
}

#[test]
fn clearing_the_field_spawns_the_next_wave_with_grown_count() {
    let mut world = quiet_world(3);
    assert_eq!(world.wave, 0);
    world.asteroids.clear();
    world.step_fixed();
    assert_eq!(world.wave, 1, "an empty field escalates the Wave");
    assert_eq!(world.asteroids.len(), 7, "ceil(5 * 1.25) asteroids");

    // Every Wave asteroid spawns clear of the Ship.
    for asteroid in &world.asteroids {
        assert!(
            sim::math::tdist(
                asteroid.x,
                asteroid.y,
                world.agent.ship.x,
                world.agent.ship.y
            ) >= 150.0,
            "Wave asteroids keep their distance from the Ship"
        );
    }

    world.asteroids.clear();
    world.step_fixed();
    assert_eq!(world.asteroids.len(), 9, "ceil(7 * 1.25)");
}

#[test]
fn clearing_a_wave_resets_the_wave_clock() {
    let mut world = quiet_world(4);
    world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
    for _ in 0..600 {
        world.step_fixed();
    }
    assert!(world.wave_time > 9.0, "ten seconds of Wave clock");
    world.asteroids.clear();
    world.step_fixed();
    assert!(
        world.wave_time < DT * 2.0,
        "the clock restarts with the Wave"
    );
}

#[test]
fn the_wave_clock_ends_the_episode() {
    let mut world = quiet_world(5);
    world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
    let mut steps = 0;
    while !world.done && steps < 100_000 {
        world.step_fixed();
        steps += 1;
    }
    assert!(world.done);
    assert!(
        (world.time - world_cfg::WAVE_TIME_LIMIT).abs() < 2.0 * DT,
        "the Episode ends at the Wave clock, not later: {}",
        world.time
    );
}

#[test]
fn the_episode_hard_cap_bounds_wave_chaining() {
    let mut world = quiet_world(6);
    world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
    let mut steps = 0;
    while !world.done && steps < 100_000 {
        // Hold the Wave clock open so only the hard cap can end the Episode.
        world.wave_time = 0.0;
        world.step_fixed();
        steps += 1;
    }
    assert!(world.done, "the hard cap ended the Episode");
    assert!(
        world.time >= world_cfg::EPISODE_HARD_CAP
            && world.time < world_cfg::EPISODE_HARD_CAP + 2.0 * DT,
        "ended at {}",
        world.time
    );
}

#[test]
fn alive_time_and_movement_are_banked_into_fitness() {
    let mut world = quiet_world(7);
    world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
    for _ in 0..600 {
        world.step_fixed();
    }
    // No thrust, no starting velocity: only the alive-time term accrues.
    let expected = sim::config::fitness::ALIVE_PER_SECOND * 600.0 * DT;
    assert!(
        (world.agent.fitness - expected).abs() < 1e-6,
        "fitness {} != alive time term {expected}",
        world.agent.fitness
    );
    assert!((world.agent.stats.alive_time - 10.0).abs() < 1e-9);
}

#[test]
fn thrust_accelerates_the_ship() {
    let mut world = World::new(
        sim::Rng::from_seed(8),
        Some(Network::from_genome(&wired_genome(
            11,
            nn::OUTPUT_IDS[2],
            5.0,
        ))),
    );
    // A clear runway along +x and one asteroid far off the line of flight.
    world.agent.ship.x = 100.0;
    world.agent.ship.y = 300.0;
    world.agent.ship.heading = 0.0;
    world.asteroids.clear();
    world.asteroids.push(asteroid(Size::Small, 900.0, 550.0));
    let steps = 120;
    for _ in 0..steps {
        world.step_fixed();
    }
    assert!(world.agent.thrusting, "the wired Genome always thrusts");
    // Thrust adds before damping removes, so the speed is a geometric sum.
    let mut expected = 0.0;
    for _ in 0..steps {
        expected = (expected + ship_cfg::THRUST * DT) * (1.0 - ship_cfg::DAMPING * DT);
    }
    let speed = world.agent.ship.vx.hypot(world.agent.ship.vy);
    assert!(
        (speed - expected).abs() < 1e-9,
        "speed {speed} does not match the thrust/damping sum {expected}"
    );
    assert!(speed > 150.0, "two seconds of thrust really moves");
    let mean_fraction = world.behavior()[5];
    assert!(
        (0.0..=1.0).contains(&mean_fraction) && mean_fraction > 0.0,
        "mean speed fraction {mean_fraction}"
    );
}

#[test]
fn ship_speed_is_capped() {
    let mut world = quiet_world(8);
    world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
    world.agent.ship.vx = 900.0;
    world.agent.ship.vy = 400.0;
    world.step_fixed();
    let speed = world.agent.ship.vx.hypot(world.agent.ship.vy);
    assert!(
        (speed - ship_cfg::MAX_SPEED).abs() < 1e-9,
        "an over-fast Ship is clamped, got {speed}"
    );
}

#[test]
fn the_action_threshold_is_strictly_above_one_half() {
    // tanh(0.55) = 0.5005 > 0.5, tanh(0.549) = 0.4996 < 0.5.
    for (weight, expected) in [(0.55, true), (0.549, false)] {
        let mut world = World::new(
            sim::Rng::from_seed(9),
            Some(Network::from_genome(&wired_genome(
                11,
                nn::OUTPUT_IDS[2],
                weight,
            ))),
        );
        world.asteroids.clear();
        world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
        world.step_fixed();
        assert_eq!(
            world.agent.thrusting,
            expected,
            "weight {weight} gives tanh {:.6}",
            weight.tanh()
        );
    }
}

#[test]
fn turning_moves_the_heading_at_the_rotate_speed() {
    let mut world = World::new(
        sim::Rng::from_seed(10),
        Some(Network::from_genome(&wired_genome(
            11,
            nn::OUTPUT_IDS[1],
            5.0,
        ))),
    );
    world.asteroids.clear();
    world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
    let before = world.agent.ship.heading;
    world.step_fixed();
    let turned = world.agent.ship.heading - before;
    assert!(
        (turned - ship_cfg::ROTATE_SPEED * DT).abs() < 1e-12,
        "one step of turning is rotate_speed * dt, got {turned}"
    );
}

#[test]
fn a_ship_never_holds_more_than_four_bullets() {
    let mut world = World::new(
        sim::Rng::from_seed(11),
        Some(Network::from_genome(&wired_genome(
            11,
            nn::OUTPUT_IDS[3],
            5.0,
        ))),
    );
    // A still Ship with an empty firing line: no bullet is consumed early.
    world.agent.ship.x = 100.0;
    world.agent.ship.y = 300.0;
    world.agent.ship.heading = 0.0;
    world.asteroids.clear();
    world.asteroids.push(asteroid(Size::Small, 900.0, 550.0));

    // The cooldown and the 1.1 s life mean roughly three bullets overlap; the
    // cap is a rail. Hold the cooldown open to drive the Ship into it.
    let mut most_alive = 0;
    for _ in 0..120 {
        world.agent.fire_cooldown = 0.0;
        world.step_fixed();
        assert!(world.agent.alive, "the Ship never meets the asteroid");
        most_alive = most_alive.max(world.bullets.len());
        assert!(
            world.bullets.len() <= 4,
            "{} bullets in flight",
            world.bullets.len()
        );
        assert_eq!(
            world.bullets.len() as u32,
            world.agent.bullets_out,
            "the guns counter tracks the live bullets"
        );
    }
    assert_eq!(most_alive, 4, "held flat out, the four-bullet cap binds");
    assert!(
        world.agent.stats.fire > 4,
        "the Ship pulled the trigger more often than the cap allowed: {}",
        world.agent.stats.fire
    );
}

#[test]
fn the_firing_cooldown_spaces_shots_out() {
    let mut world = World::new(
        sim::Rng::from_seed(11),
        Some(Network::from_genome(&wired_genome(
            11,
            nn::OUTPUT_IDS[3],
            5.0,
        ))),
    );
    world.agent.ship.x = 100.0;
    world.agent.ship.y = 300.0;
    world.agent.ship.heading = 0.0;
    world.asteroids.clear();
    world.asteroids.push(asteroid(Size::Small, 900.0, 550.0));
    for _ in 0..600 {
        world.step_fixed();
    }
    // Ten seconds at a 0.35 s cooldown: no more than 3 shots per second.
    let expected = (10.0 / sim::config::bullet::COOLDOWN).ceil() as u64 + 1;
    assert!(
        world.agent.stats.fire <= expected,
        "{} shots in ten seconds",
        world.agent.stats.fire
    );
    assert!(world.agent.stats.fire >= 25, "and it keeps firing");
}

#[test]
fn a_bullet_splits_a_large_asteroid_into_two_mediums() {
    let mut world = quiet_world(12);
    world.agent.ship.x = 480.0;
    world.agent.ship.y = 300.0;
    world.agent.ship.heading = 0.0;
    world.asteroids.push(asteroid(Size::Large, 580.0, 300.0));
    // Fire a bullet by hand: the World only moves them, it does not care who did.
    world.bullets.push(sim::Bullet {
        x: 500.0,
        y: 300.0,
        vx: sim::config::bullet::SPEED,
        vy: 0.0,
        life: sim::config::bullet::LIFE,
    });
    world.agent.bullets_out = 1;
    let points_before = world.agent.stats.asteroid_points;

    for _ in 0..30 {
        world.step_fixed();
    }
    assert!(world.bullets.is_empty(), "the bullet was consumed");
    assert_eq!(
        world.asteroids.len(),
        2,
        "one Large asteroid became two Mediums"
    );
    for asteroid in &world.asteroids {
        assert_eq!(asteroid.size, Size::Medium);
        assert_eq!(asteroid.points, Size::Medium.points());
        assert!(
            asteroid.vx.hypot(asteroid.vy) <= SPEED_CAP + 1e-9,
            "children respect the speed cap"
        );
    }
    assert!(world.agent.stats.asteroid_points >= points_before + Size::Large.points());
}

#[test]
fn a_small_asteroid_vanishes_when_shot() {
    let mut world = quiet_world(13);
    world.agent.ship.x = 480.0;
    world.agent.ship.y = 300.0;
    world.asteroids.push(asteroid(Size::Small, 580.0, 300.0));
    // A witness asteroid keeps the field non-empty, so no Wave escalation fires.
    world.asteroids.push(asteroid(Size::Large, 100.0, 100.0));
    world.bullets.push(sim::Bullet {
        x: 500.0,
        y: 300.0,
        vx: sim::config::bullet::SPEED,
        vy: 0.0,
        life: sim::config::bullet::LIFE,
    });
    world.agent.bullets_out = 1;
    for _ in 0..30 {
        world.step_fixed();
    }
    assert_eq!(world.asteroids.len(), 1, "the Small asteroid is gone");
    assert_eq!(world.asteroids[0].size, Size::Large);
    assert_eq!(world.wave, 0, "no Wave was cleared");
    assert!(world.agent.stats.asteroid_points >= Size::Small.points());
}

#[test]
fn bullets_expire_and_free_their_slot() {
    let mut world = quiet_world(14);
    world.agent.ship.x = 480.0;
    world.agent.ship.y = 300.0;
    world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
    world.bullets.push(sim::Bullet {
        x: 480.0,
        y: 300.0,
        vx: 0.0,
        vy: 0.0,
        life: sim::config::bullet::LIFE,
    });
    world.agent.bullets_out = 1;
    let steps = (sim::config::bullet::LIFE / DT).ceil() as usize + 1;
    for _ in 0..steps {
        world.step_fixed();
    }
    assert!(world.bullets.is_empty());
    assert_eq!(world.agent.bullets_out, 0);
}

#[test]
fn colliding_asteroids_separate_and_exchange_momentum() {
    let mut asteroids = vec![
        moving_asteroid(Size::Large, 100.0, 300.0, 40.0, 0.0),
        moving_asteroid(Size::Large, 170.0, 300.0, -40.0, 0.0),
    ];
    let before = asteroids[0].vx * 38.0 * 38.0 + asteroids[1].vx * 38.0 * 38.0;
    sim::asteroid::collide_asteroids(&mut asteroids);
    let separation = (asteroids[1].x - asteroids[0].x).abs();
    assert!(
        separation >= 76.0 - 1e-9,
        "equal-mass asteroids de-overlap to their contact distance, got {separation}"
    );
    let after = asteroids[0].vx * 38.0 * 38.0 + asteroids[1].vx * 38.0 * 38.0;
    assert!(
        (before - after).abs() < 1e-6,
        "a perfectly elastic collision conserves momentum: {before} vs {after}"
    );
    assert!(
        asteroids[0].vx <= 0.0 && asteroids[1].vx >= 0.0,
        "they bounce apart"
    );
}

#[test]
fn rays_see_an_asteroid_across_the_seam() {
    let mut world = quiet_world(15);
    world.agent.ship.x = 5.0;
    world.agent.ship.y = 300.0;
    world.agent.ship.heading = std::f64::consts::PI; // nose pointing at -x
    world.asteroids.push(asteroid(Size::Large, 900.0, 300.0));
    world.step_fixed();
    let inputs = world.agent.inputs;
    assert!(
        inputs[0] > 0.9,
        "the forward Ray sees the asteroid across the seam: {}",
        inputs[0]
    );
    assert_eq!(inputs[4], 0.0, "rays pointing away see nothing");
    assert!(
        world.agent.alive,
        "the asteroid is 65 px away, not under the nose"
    );
}

#[test]
fn threat_bearing_is_normalized_and_signed() {
    let mut world = quiet_world(16);
    world.agent.ship.x = 480.0;
    world.agent.ship.y = 300.0;
    world.agent.ship.heading = 0.0;
    world.asteroids.push(asteroid(Size::Large, 480.0, 250.0));
    world.step_fixed();
    let inputs = world.agent.inputs;
    assert!(
        (inputs[12] + 0.5).abs() < 1e-12,
        "an asteroid straight up reads -0.5, got {}",
        inputs[12]
    );
    assert!(
        inputs[13] > 0.8,
        "closeness rises as the asteroid approaches"
    );
    assert_eq!(inputs[18], 1.0, "a Large asteroid is the largest threat");
    assert_eq!(inputs[11], 1.0, "the bias input is always on");
}

#[test]
fn encirclement_pressure_grows_with_a_crowded_field() {
    let mut sparse = quiet_world(17);
    sparse.agent.ship.x = 480.0;
    sparse.agent.ship.y = 300.0;
    sparse.asteroids.push(asteroid(Size::Small, 400.0, 200.0));
    sparse.step_fixed();

    let mut crowded = quiet_world(17);
    crowded.agent.ship.x = 480.0;
    crowded.agent.ship.y = 300.0;
    for offset in 0..8 {
        crowded
            .asteroids
            .push(asteroid(Size::Small, 380.0 + offset as f64 * 20.0, 200.0));
    }
    crowded.step_fixed();
    assert!(
        crowded.agent.inputs[19] > sparse.agent.inputs[19],
        "pressure {} vs {}",
        crowded.agent.inputs[19],
        sparse.agent.inputs[19]
    );
}

#[test]
fn the_memory_output_is_fed_back_as_an_input() {
    let mut world = World::new(
        sim::Rng::from_seed(18),
        Some(Network::from_genome(&wired_genome(
            11,
            nn::OUTPUT_IDS[nn::MEMORY_OUTPUT],
            5.0,
        ))),
    );
    world.asteroids.clear();
    world.asteroids.push(asteroid(Size::Small, 100.0, 100.0));
    world.step_fixed();
    assert!(
        (world.agent.memory - 5.0_f64.tanh()).abs() < 1e-12,
        "memory holds last step's output"
    );
    world.step_fixed();
    assert!(
        (world.agent.inputs[nn::MEMORY_INPUT] - 5.0_f64.tanh()).abs() < 1e-12,
        "and comes back as input 20"
    );
}

#[test]
fn coverage_tracks_where_the_ship_has_been() {
    let mut world = quiet_world(19);
    world.agent.ship.x = 60.0;
    world.agent.ship.y = 60.0;
    world.asteroids.push(asteroid(Size::Small, 900.0, 550.0));
    world.step_fixed();
    assert_eq!(world.agent.stats.covered_cells(), 1);
    world.agent.ship.x = 800.0;
    world.agent.ship.y = 500.0;
    world.step_fixed();
    assert_eq!(world.agent.stats.covered_cells(), 2);
    assert!((world.behavior()[4] - 2.0 / 40.0).abs() < 1e-12);
}

#[test]
fn the_agent_carries_its_network_for_the_panel() {
    let genome = wired_genome(11, nn::OUTPUT_IDS[0], 3.0);
    let world = World::new(sim::Rng::from_seed(20), Some(Network::from_genome(&genome)));
    let network = world.agent.network().expect("the Agent holds its Network");
    assert_eq!(network.node_count(), genome.node_count());
    assert_eq!(network.enabled_connection_count(), 1);
    for (id, node_type) in network.nodes() {
        assert_eq!(genome.node_type(*id), Some(*node_type));
    }
    assert!(genome.node_type(0) == Some(NodeType::Input));
}
