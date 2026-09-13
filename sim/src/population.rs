//! Population lifecycle: speciation, the dynamic compatibility threshold,
//! stagnation culling and reproduction.
//!
//! Breeding is sequential and consumes its own stream, derived from
//! `(run_seed, generation)` — never the Episode streams — so the next
//! Generation is the same whatever order the Episodes finished in (ADR 0005).

use crate::competence::WaveStats;
use crate::config::neat;
use crate::evaluation::GenerationStats;
use crate::genome::{Genome, InnovationTracker};
use crate::network::Network;
use crate::rng::{derive_stream, Lane, Rng};

/// A compatibility cluster. `members` index the Generation that was just
/// evaluated, so a Species is only meaningful between `evolve` calls.
#[derive(Clone, Debug)]
pub struct Species {
    pub id: u32,
    pub representative: Genome,
    pub best_fitness: f64,
    pub stagnation: u32,
    pub members: Vec<(usize, f64)>,
    survivors: Vec<(usize, f64)>,
    adjusted: f64,
    champion_copied: bool,
    pub offspring: usize,
}

impl Species {
    fn new(id: u32, representative: Genome) -> Self {
        Self {
            id,
            representative,
            best_fitness: f64::NEG_INFINITY,
            stagnation: 0,
            members: Vec::new(),
            survivors: Vec::new(),
            adjusted: 0.0,
            champion_copied: false,
            offspring: 0,
        }
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }

    /// Mean fitness of the Species divided by its size: NEAT's adjusted fitness,
    /// which stops a large Species from swamping the offspring quota.
    pub fn adjusted_fitness(&self) -> f64 {
        self.adjusted
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BestEver {
    pub fitness: f64,
    pub generation: u32,
    pub species_id: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct Population {
    pub genomes: Vec<Genome>,
    pub networks: Vec<Network>,
    pub generation: u32,
    pub history: Vec<GenerationStats>,
    pub species: Vec<Species>,
    pub best_ever: BestEver,
    pub delta_target: f64,
    run_seed: u32,
    tracker: InnovationTracker,
    next_species_id: u32,
}

impl Population {
    /// The founding Generation: every Genome fully wired input→output with
    /// random weights, all drawn from the breeding stream of Generation 1.
    pub fn new(size: usize, run_seed: u32) -> Self {
        let mut rng = derive_stream(run_seed, 1, 0, Lane::Breeding);
        let mut tracker = InnovationTracker::new();
        let genomes: Vec<Genome> = (0..size)
            .map(|_| Genome::new(&mut rng, &mut tracker))
            .collect();
        Self::from_genomes(genomes, run_seed, tracker)
    }

    /// A Population that replays one Genome in every slot — watch mode, where
    /// breeding is switched off.
    pub fn showcase(genome: &Genome, size: usize, run_seed: u32) -> Self {
        let tracker = InnovationTracker::from_genome(genome);
        let genomes = (0..size).map(|_| genome.clone()).collect();
        Self::from_genomes(genomes, run_seed, tracker)
    }

    /// A Population seeded from one Genome: the loaded Genome as the first
    /// member, the rest mutated copies made with the standard operator set —
    /// the honest way to bring an old lineage back and keep evolving it. No new
    /// operator is introduced.
    pub fn from_champion(genome: &Genome, size: usize, run_seed: u32) -> Self {
        let mut rng = derive_stream(run_seed, 1, 0, Lane::Breeding);
        let mut tracker = InnovationTracker::from_genome(genome);
        let size = size.max(1);
        let mut genomes = Vec::with_capacity(size);
        genomes.push(genome.clone());
        for _ in 1..size {
            let mut child = genome.clone();
            if rng.chance(neat::ADD_NODE_RATE) {
                child.mutate_add_node(&mut rng, &mut tracker);
            }
            if rng.chance(neat::ADD_CONNECTION_RATE) {
                child.mutate_add_connection(&mut rng, &mut tracker);
            }
            child.mutate_weights(&mut rng);
            genomes.push(child);
        }
        Self::from_genomes(genomes, run_seed, tracker)
    }

    fn from_genomes(genomes: Vec<Genome>, run_seed: u32, tracker: InnovationTracker) -> Self {
        let networks = genomes.iter().map(Network::from_genome).collect();
        Self {
            genomes,
            networks,
            generation: 1,
            history: Vec::new(),
            species: Vec::new(),
            best_ever: BestEver {
                fitness: f64::NEG_INFINITY,
                generation: 0,
                species_id: None,
            },
            delta_target: neat::DELTA_TARGET_INIT,
            run_seed,
            tracker,
            next_species_id: 1,
        }
    }

    /// Record a Generation's fitnesses and Waves, then breed the next one.
    pub fn evolve(&mut self, fitnesses: &[f64], waves: WaveStats) {
        let size = fitnesses.len();
        debug_assert_eq!(size, self.genomes.len());
        // Breeding draws from its own stream, derived per Generation (ADR 0005):
        // the founders come from stream 1, and each Generation is bred from its own.
        let mut rng = derive_stream(self.run_seed, self.generation + 1, 0, Lane::Breeding);
        let best = fitnesses.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mean = fitnesses.iter().sum::<f64>() / size as f64;
        self.history.push(GenerationStats {
            generation: self.generation,
            best,
            mean,
            mean_wave: waves.mean,
            median_wave: waves.median,
            p90_wave: waves.p90,
            clearing_share: waves.clearing_share,
        });

        // --- Speciate against the representatives carried over from last time.
        for species in &mut self.species {
            species.members.clear();
        }
        let old = std::mem::take(&mut self.genomes);
        let mut genome_species = vec![0usize; size];
        for (index, genome) in old.iter().enumerate() {
            let mut found = None;
            for (species_index, species) in self.species.iter().enumerate() {
                if Genome::distance(genome, &species.representative) < self.delta_target {
                    found = Some(species_index);
                    break;
                }
            }
            let species_index = match found {
                Some(index) => index,
                None => {
                    let id = self.next_species_id;
                    self.next_species_id += 1;
                    self.species.push(Species::new(id, genome.clone()));
                    self.species.len() - 1
                }
            };
            self.species[species_index]
                .members
                .push((index, fitnesses[index]));
            genome_species[index] = species_index;
        }

        // --- Global best, before the stats below so the holder is identifiable.
        let mut best_index = 0;
        for index in 1..size {
            if fitnesses[index] > fitnesses[best_index] {
                best_index = index;
            }
        }
        if fitnesses[best_index] > self.best_ever.fitness {
            self.best_ever = BestEver {
                fitness: fitnesses[best_index],
                generation: self.generation,
                species_id: Some(self.species[genome_species[best_index]].id),
            };
        }

        // --- Stagnation counters.
        for species in &mut self.species {
            let member_best = species
                .members
                .iter()
                .map(|(_, f)| *f)
                .fold(f64::NEG_INFINITY, f64::max);
            if member_best > species.best_fitness {
                species.best_fitness = member_best;
                species.stagnation = 0;
            } else {
                species.stagnation += 1;
            }
        }

        // --- Dynamic compatibility threshold: too many Species and the
        // threshold rises, too few and it falls, so structure keeps diversifying.
        let species_count = self.species.len();
        if species_count > neat::SPECIES_COUNT_MAX {
            self.delta_target = (self.delta_target + neat::DELTA_STEP).min(neat::DELTA_MAX);
        } else if species_count < neat::SPECIES_COUNT_MIN {
            self.delta_target = (self.delta_target - neat::DELTA_STEP).max(neat::DELTA_MIN);
        }

        // --- Stagnation culling: never the global-best Species, never below two.
        let protected = self.best_ever.species_id;
        let viable: Vec<Species> = self
            .species
            .iter()
            .filter(|s| s.stagnation < neat::STAGNATION_LIMIT || Some(s.id) == protected)
            .cloned()
            .collect();
        if viable.len() >= 2 {
            self.species = viable;
        }

        // --- Survivors, champions and adjusted fitness.
        let mut next: Vec<Genome> = Vec::with_capacity(size);
        for species in &mut self.species {
            species
                .members
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            species.champion_copied = false;
            if species.members.len() >= neat::CHAMPION_MIN_SIZE {
                next.push(old[species.members[0].0].clone());
                species.champion_copied = true;
            }
            let survivors =
                (species.members.len() as f64 * (1.0 - neat::SURVIVAL_FRACTION)).ceil() as usize;
            species.survivors = species.members[..survivors].to_vec();
            let mean_fitness =
                species.members.iter().map(|(_, f)| *f).sum::<f64>() / species.members.len() as f64;
            species.adjusted = mean_fitness / species.members.len() as f64;
        }

        // --- Offspring quota per Species, proportional to adjusted fitness.
        let weights: Vec<f64> = self.species.iter().map(|s| s.adjusted.max(0.0)).collect();
        let quota = largest_remainder(&weights, size - next.len());
        let mut species = std::mem::take(&mut self.species);
        for (index, species) in species.iter_mut().enumerate() {
            species.offspring = quota[index];
            for _ in 0..quota[index] {
                next.push(self.offspring(species, &old, &mut rng));
            }
        }

        // --- Safety fill for rounding and degenerate quotas, then exact size.
        let pool: Vec<(usize, f64)> = species
            .iter()
            .flat_map(|s| s.survivors.iter().copied())
            .collect();
        while next.len() < size && !pool.is_empty() {
            let pick = pool[rng.below(pool.len())];
            next.push(old[pick.0].clone());
        }
        next.truncate(size);

        // --- Representatives carried over: a random member of each Species that
        // produced a champion or offspring.
        species.retain(|s| s.champion_copied || s.offspring > 0);
        for species in &mut species {
            if !species.members.is_empty() {
                let pick = rng.below(species.members.len());
                species.representative = old[species.members[pick].0].clone();
            }
        }
        self.species = species;

        self.genomes = next;
        self.networks = self.genomes.iter().map(Network::from_genome).collect();
        self.generation += 1;
    }

    /// Watch mode: record the Generation and advance the counter, breeding
    /// nothing. Every member replays the same Genome.
    pub fn advance_without_breeding(&mut self, fitnesses: &[f64], waves: WaveStats) {
        let best = fitnesses.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mean = fitnesses.iter().sum::<f64>() / fitnesses.len() as f64;
        self.history.push(GenerationStats {
            generation: self.generation,
            best,
            mean,
            mean_wave: waves.mean,
            median_wave: waves.median,
            p90_wave: waves.p90,
            clearing_share: waves.clearing_share,
        });
        if best > self.best_ever.fitness {
            self.best_ever = BestEver {
                fitness: best,
                generation: self.generation,
                species_id: None,
            };
        }
        self.generation += 1;
    }

    /// One child: crossover or clone, then the structural and weight mutations,
    /// in a fixed order so a seed replays exactly.
    fn offspring(&mut self, species: &Species, old: &[Genome], mut rng: &mut Rng) -> Genome {
        let pick = |rng: &mut Rng| species.survivors[rng.below(species.survivors.len())];
        let mut child = if rng.chance(neat::CROSSOVER_RATE) {
            let a = pick(&mut rng);
            let b = pick(&mut rng);
            if a.0 == b.0 {
                old[a.0].clone()
            } else {
                let a_fitter = if a.1 > b.1 {
                    Some(true)
                } else if a.1 < b.1 {
                    Some(false)
                } else {
                    None
                };
                Genome::crossover(&old[a.0], &old[b.0], a_fitter, &mut rng)
            }
        } else {
            old[pick(&mut rng).0].clone()
        };
        if rng.chance(neat::ADD_NODE_RATE) {
            child.mutate_add_node(&mut rng, &mut self.tracker);
        }
        if rng.chance(neat::ADD_CONNECTION_RATE) {
            child.mutate_add_connection(&mut rng, &mut self.tracker);
        }
        child.mutate_weights(&mut rng);
        child
    }
}

/// Integer quotas summing exactly to `total`, proportional to `weights`
/// (largest remainder). All-zero weights fall back to a uniform split so the
/// Population survives a Generation where nothing scored.
pub fn largest_remainder(weights: &[f64], total: usize) -> Vec<usize> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    let sum: f64 = weights.iter().sum();
    let mut out = vec![0usize; n];
    if sum <= 0.0 {
        let base = total / n;
        for slot in out.iter_mut() {
            *slot = base;
        }
        for slot in out.iter_mut().take(total - base * n) {
            *slot += 1;
        }
        return out;
    }
    let exact: Vec<f64> = weights.iter().map(|w| (w / sum) * total as f64).collect();
    for (index, value) in exact.iter().enumerate() {
        out[index] = value.floor() as usize;
    }
    let assigned: usize = out.iter().sum();
    let mut remainder = total.saturating_sub(assigned);
    let mut fractions: Vec<(f64, usize)> = exact
        .iter()
        .enumerate()
        .map(|(index, value)| (value - value.floor(), index))
        .collect();
    fractions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut index = 0;
    while remainder > 0 {
        out[fractions[index % n].1] += 1;
        remainder -= 1;
        index += 1;
    }
    out
}
