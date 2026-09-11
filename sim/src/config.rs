//! Every pinned constant, moved across from the retired browser configuration
//! table unchanged. Changing one is a tuning decision taken with the headless
//! binary (`neuroarena-headless`), never a side effect of a port.
//!
//! Presentation constants do not live here: the native look is new, and the
//! simulation has no opinion about it.

/// Fixed simulation timestep (seconds).
pub const DT: f64 = 1.0 / 60.0;

/// Toroidal playfield. The Arena is a fixed logical space at any window size
/// (ADR 0006): every constant below is expressed in these units.
pub mod arena {
    pub const WIDTH: f64 = 960.0;
    pub const HEIGHT: f64 = 600.0;
}

pub mod ship {
    pub const RADIUS: f64 = 9.0;
    /// rad/s
    pub const ROTATE_SPEED: f64 = 3.2;
    /// px/s^2
    pub const THRUST: f64 = 140.0;
    /// v *= max(0, 1 - DAMPING * dt)
    pub const DAMPING: f64 = 0.6;
    pub const MAX_SPEED: f64 = 320.0;
    /// Dies when dist < asteroid_r + RADIUS * HITBOX_FACTOR.
    pub const HITBOX_FACTOR: f64 = 0.7;
    /// px around the Arena centre
    pub const SPAWN_JITTER: f64 = 80.0;
}

pub mod bullet {
    /// Plus the firing Ship's velocity.
    pub const SPEED: f64 = 420.0;
    pub const LIFE: f64 = 1.1;
    pub const RADIUS: f64 = 2.0;
    /// Spawn at ship_pos + heading * NOSE_OFFSET
    pub const NOSE_OFFSET: f64 = 12.0;
    pub const COOLDOWN: f64 = 0.35;
    pub const MAX_ALIVE_PER_SHIP: u32 = 4;
    /// px/s backward nudge per shot
    pub const RECOIL: f64 = 10.0;
}

pub mod asteroid {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Size {
        Large,
        Medium,
        Small,
    }

    impl Size {
        /// The size a split produces, if any.
        pub const fn child(self) -> Option<Size> {
            match self {
                Size::Large => Some(Size::Medium),
                Size::Medium => Some(Size::Small),
                Size::Small => None,
            }
        }

        pub const fn radius(self) -> f64 {
            match self {
                Size::Large => 38.0,
                Size::Medium => 21.0,
                Size::Small => 11.0,
            }
        }

        pub const fn points(self) -> f64 {
            match self {
                Size::Large => 20.0,
                Size::Medium => 50.0,
                Size::Small => 100.0,
            }
        }

        pub const fn speed_range(self) -> (f64, f64) {
            match self {
                Size::Large => (20.0, 45.0),
                Size::Medium => (35.0, 65.0),
                Size::Small => (55.0, 95.0),
            }
        }
    }

    pub const VERTICES: usize = 10;
    pub const JITTER_MIN: f64 = 0.75;
    pub const JITTER_MAX: f64 = 1.25;
    /// rad/s, uniform(-SPIN_MAX, SPIN_MAX), visual only
    pub const SPIN_MAX: f64 = 1.0;
    /// Rocks in Wave 1; later Waves: ceil(prev * WAVE_GROWTH)
    pub const INITIAL_COUNT: usize = 5;
    /// Wave rocks spawn at least this far (toroidally) from the Ship.
    pub const INITIAL_MIN_DIST_FROM_SHIP: f64 = 150.0;
    /// Degrees off the parent direction.
    pub const SPLIT_ANGLE_MIN: f64 = 20.0;
    pub const SPLIT_ANGLE_MAX: f64 = 70.0;
    /// Fraction of the child speed range added on top of inherited velocity.
    pub const SPLIT_IMPULSE_FACTOR: f64 = 0.5;
    /// Global speed cap (momentum inheritance compounds through splits).
    pub const SPEED_CAP: f64 = 160.0;
    /// Rock-rock elasticity (1 = perfectly elastic).
    pub const RESTITUTION: f64 = 1.0;
    pub const WAVE_GROWTH: f64 = 1.25;
}

pub mod sensors {
    /// Index order is input order.
    pub const RAY_OFFSETS_DEG: [f64; 9] = [0.0, 40.0, -40.0, 80.0, -80.0, 120.0, -120.0, 160.0, -160.0];
    pub const RANGE: f64 = 500.0;
    pub const VEL_SCALE: f64 = 300.0;
    /// Proximity-pressure sensor normalization
    pub const PRESSURE_NORM: f64 = 8.0;
}

/// Input order, fixed by the retired sensor module:
/// 0-8 Sensor Rays (toroidal), 9 vx, 10 vy, 11 bias, 12 threat bearing,
/// 13 threat closeness, 14 threat closing, 15 guns, 16 threat lateral,
/// 17 second-nearest closeness, 18 threat size, 19 encirclement pressure,
/// 20 memory (fed back from output 25 each step).
pub mod nn {
    pub const INPUTS: usize = 21;
    pub const OUTPUTS: usize = 5;
    /// left, right, thrust, fire, memory
    pub const OUTPUT_IDS: [u32; OUTPUTS] = [21, 22, 23, 24, 25];
    /// Strict `>` on the first four outputs.
    pub const ACTION_THRESHOLD: f64 = 0.5;
    pub const FIRST_HIDDEN_NODE_ID: u32 = 26;
    pub const BIAS_INPUT: usize = 11;
    pub const MEMORY_INPUT: usize = 20;
    pub const MEMORY_OUTPUT: usize = 4;
}

pub mod neat {
    pub const POP_SIZE: usize = 100;
    pub const WEIGHT_INIT_MIN: f64 = -1.0;
    pub const WEIGHT_INIT_MAX: f64 = 1.0;
    pub const ADD_CONNECTION_RATE: f64 = 0.1;
    /// Rejection-sampling budget for one add-connection mutation; the retry
    /// count is part of the stream contract, not an implementation detail.
    pub const ADD_CONNECTION_ATTEMPTS: usize = 10;
    pub const ADD_NODE_RATE: f64 = 0.02;
    pub const WEIGHT_PERTURB_RATE: f64 = 0.08;
    pub const WEIGHT_PERTURB_SIGMA: f64 = 0.4;
    pub const WEIGHT_REPLACE_RATE: f64 = 0.01;
    pub const WEIGHT_REPLACE_MIN: f64 = -1.5;
    pub const WEIGHT_REPLACE_MAX: f64 = 1.5;
    pub const WEIGHT_MIN: f64 = -4.0;
    pub const WEIGHT_MAX: f64 = 4.0;
    /// Excess-gene coefficient.
    pub const DISTANCE_C1: f64 = 1.0;
    /// Disjoint-gene coefficient.
    pub const DISTANCE_C2: f64 = 1.0;
    /// Mean matching weight difference coefficient.
    pub const DISTANCE_C3: f64 = 0.4;
    pub const DELTA_TARGET_INIT: f64 = 3.0;
    pub const DELTA_STEP: f64 = 0.15;
    pub const DELTA_MIN: f64 = 1.0;
    pub const DELTA_MAX: f64 = 6.0;
    pub const SPECIES_COUNT_MIN: usize = 8;
    pub const SPECIES_COUNT_MAX: usize = 12;
    pub const STAGNATION_LIMIT: u32 = 15;
    /// Drop the bottom 50%.
    pub const SURVIVAL_FRACTION: f64 = 0.5;
    pub const CHAMPION_MIN_SIZE: usize = 5;
    pub const CROSSOVER_RATE: f64 = 0.75;
}

pub mod world {
    /// Sim seconds per Wave; clearing a Wave resets this clock.
    pub const WAVE_TIME_LIMIT: f64 = 60.0;
    /// Absolute sim seconds per Episode, bounds Wave chaining.
    pub const EPISODE_HARD_CAP: f64 = 300.0;
}

pub mod fitness {
    pub const ALIVE_PER_SECOND: f64 = 10.0;
    pub const BULLET_COST: f64 = 1.0;
    /// Movement reward: speed fraction per second (arXiv:2311.02283).
    pub const MOVE_RATE: f64 = 4.0;
    /// Action-usage entropy bonus, maximal at uniform use of all four controls
    /// (arXiv:1006.4959, arXiv:2608.12534).
    pub const ACTION_ENTROPY_BONUS: f64 = 300.0;
    /// Behavior-descriptor novelty bonus with generation decay to a floor
    /// (arXiv:1902.03142, arXiv:2209.03618).
    pub const NOVELTY_BONUS: f64 = 400.0;
    pub const NOVELTY_DECAY_GENS: f64 = 60.0;
    pub const NOVELTY_FLOOR_FRAC: f64 = 0.2;
    pub const ARCHIVE_SIZE: usize = 400;
    pub const NOVELTY_K: usize = 15;
}

/// Competence Gate: the window of Generation medians that decides stagnation.
pub mod gate {
    /// Median alive times kept for the stagnation comparison.
    pub const MEDIAN_WINDOW: usize = 15;
    /// Alive-time ratio below which a Generation counts as stagnant.
    pub const STAGNATION_RATIO: f64 = 1.10;
    /// Consecutive stagnant Generations before the Gate trips.
    pub const STAGNATION_LIMIT: u32 = 15;
}
