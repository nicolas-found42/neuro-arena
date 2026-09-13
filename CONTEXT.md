# NeuroArena

Neuroevolution playground that evolves neural-network pilots to survive an Asteroids arena, as a native macOS application.

## Language

### Arena & Simulation

**Arena**: The 960×600 toroidal playfield where physics and rendering happen. _Avoid_: board, stage, canvas

**World**: Reusable toroidal physics container that steps one Agent through one Episode. _Avoid_: game, env, simulation

**Ship**: The physical vessel state inside a World — position, heading, and velocity. _Avoid_: player, entity

**Agent**: The live pairing inside a World of a Ship with its controlling Network for one Episode. _Avoid_: player, entity

**Asteroid**: Toroidal polygon obstacle with jittered vertices that wraps via Seam Copies and splits L→M→S when shot. _Avoid_: rock, meteor

**Wave**: One Asteroid field; clearing it spawns the next with growing count. _Avoid_: level, round

**Episode**: One scored evaluation run inside a World from spawn until death or cap. _Avoid_: trial, rollout, run

**Seam Copies**: Extra draws of an entity at ±W/±H when it straddles the toroidal edge. _Avoid_: wrapping, cloning

**Sensor Ray**: The toroidal vision sensor fixed to the Ship's heading that reports normalized distance to the nearest Asteroid intersection. _Avoid_: raycast, beam, lidar

**Threat Slot**: One of the three nearest Asteroids the Sensorium describes — bearing, closeness, closing rate, lateral rate and size. _Avoid_: entity, target, nearest rock

**Sensorium**: Everything an Agent perceives in one step — Sensor Rays, Threat Slots, velocity, bias and memory. _Avoid_: observation, input vector, state

### Evolution

**Genome**: Evolvable genotype of nodes and weighted connections with stable innovation numbers. _Avoid_: brain, chromosome, DNA, weights

**Body Plan**: The designed founding topology of a Genome — the nodes and connections a fresh Genome starts from. _Avoid_: seed topology, skeleton, template, blueprint

**Network**: Feedforward phenotype derived from a Genome that maps Sensors to actions. _Avoid_: brain, net graph, model

**Population**: The fixed-size set (100) of Genomes partitioned into Species each Generation. _Avoid_: pool, swarm, cohort

**Generation**: One complete cycle that evaluates every member of the Population then breeds the next. _Avoid_: epoch, iteration, round

**Species**: Compatibility cluster of similar Genomes that protects new structure from immediate competition. _Avoid_: deme, niche, family

**Fitness**: Shaped selection score that determines breeding. _Avoid_: score, points, reward

**Competence**: The raw skill an Episode banks — alive time, Wave reached, Asteroids destroyed — held apart from Fitness. _Avoid_: score, raw fitness, gate score

**Competence Gate**: The stagnation verdict over the Generation headline pair of Competence — mean Waves, tie-broken by median alive time — shown beside Fitness in the HUD. _Avoid_: fitness gate, gate score

**Innovation Tracker**: The ordered source of innovation numbers that lets crossover align matching genes. _Avoid_: counter, ID generator, innovator

### Presentation

**HUD**: Overlay panel showing Generation, member index, time, Wave, gate state, and the current Fitness as a number. _Avoid_: stats, info bar

**Chart**: Sparkline of the headline Competence per Generation — the Population's mean Waves, tie-broken by alive time. _Avoid_: graph, fitness chart

### Program

**Candidate**: A proposed change to the controller, judged by the paired-seed protocol; the default changes only when a Candidate clears the bar. _Avoid_: experiment, variant, arm
