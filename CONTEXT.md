# Neuroevolution Asteroids

Browser neuroevolution playground that evolves neural-network pilots to survive an Asteroids arena on GH Pages.

## Language

**Arena**: The 960×600 toroidal playfield where physics and rendering happen. _Avoid_: board, stage, canvas
**Asteroid**: Toroidal polygon obstacle with jittered vertices that wraps via seam copies. _Avoid_: rock, meteor
**Ship**: The evolved pilot's vessel; evolves via NEAT genome and sensors (rays). _Avoid_: agent, player
**HUD**: Overlay text panel showing generation, fitness, and run state. _Avoid_: stats, info bar
**Chart**: Fitness-over-generations sparkline. _Avoid_: graph
**Network**: Visualized NEAT genome (nodes and connections) for the champion. _Avoid_: brain view, net graph
**Arcade Sketch**: Locked Win98-bevel graphics variant (teal desktop, outset chrome, title bars, Rough.js sketch) — no runtime variant switcher. _Avoid_: theme, skin, arcade mode
**Seam Copies**: Extra draws of an entity at ±W/±H when it straddles the toroidal edge (up to 4 copies). _Avoid_: wrapping, cloning
