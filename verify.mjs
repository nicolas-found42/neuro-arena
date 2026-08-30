// Headless stub for `import rough from 'roughjs'` — idempotent, keeps `node verify.mjs` green on fresh clone (gitignored)
import { mkdirSync, writeFileSync, existsSync } from 'node:fs';
try {
  const dir = 'node_modules/roughjs';
  const pkg = `${dir}/package.json`;
  const idx = `${dir}/index.js`;
  if (!existsSync(pkg) || !existsSync(idx)) {
    mkdirSync(dir, { recursive: true });
    writeFileSync(pkg, JSON.stringify({ name: 'roughjs', version: '4.6.6', type: 'module', main: 'index.js' }, null, 2));
    const stub = `export default {
  generator(opts={}) {
    const b={...opts};
    return {
      polygon(v,o={}) { return {_kind:'polygon', verts: v.slice(), options:{...b,...o}}; }
    };
  },
  canvas(el) {
    return {
      draw(d){ if(el){ el.__roughDrawCalls=(el.__roughDrawCalls||0)+1; } },
    };
  }
};
`;
    writeFileSync(idx, stub);
  }
} catch (e) {
  console.warn('[verify] roughjs stub creation failed:', e?.message ?? e);
  throw e;
}


// Verification harness: solo evaluation, sensors, fitness semantics, NEAT
// structure, seeded determinism, and champion JSON round-trip.
const { CONFIG } = await import('./js/config.js');
const { World } = await import('./js/game.js');
const { Genome, Network, resetInnovation } = await import('./js/neat.js');
const { Population } = await import('./js/population.js');

let pass = 0, fail = 0;
const ok = (name, cond, detail = '') => {
  if (cond) { pass++; console.log(`  ok  ${name}`); }
  else { fail++; console.log(`FAIL  ${name} ${detail}`); }
};
const dt = CONFIG.dt;

// 1. Config shape.
// 1b. HiDPI render-only invariant: logical arena unchanged, dprCap present, game units untouched.
ok('arena still 960x600 logical', CONFIG.arena.width === 960 && CONFIG.arena.height === 600);
ok('render.dprCap == 2', CONFIG.render.dprCap === 2);
ok('chart/net still 300x110 / 280x420 logical', CONFIG.render.chart.width === 300 && CONFIG.render.chart.height === 110 && CONFIG.render.network.width === 280 && CONFIG.render.network.height === 420);
// 2. Population/network shape.
resetInnovation();
const pop = new Population(100);
const out0 = pop.networks[0].activate(new Array(21).fill(0));
ok('activate returns 5 outputs', Array.isArray(out0) && out0.length === 5 && out0.every(Number.isFinite), JSON.stringify(out0));
const connCount = pop.genomes[0].connections.size;
ok('initial connections 21*5=105', connCount === 105, String(connCount));

// Stub brain: captures inputs, returns fixed controls [left,right,thrust,fire,memory].
let captured = null;
const stub = { activate: (inp) => { captured = inp.slice(); return [0, 0, 0.9, 0, 0.42]; } };
const neverFire = { activate: () => [0, 0, 0, 0, 0] };

// 3. Toroidal ray across seam.
{
  const w = new World([stub]);
  const a = w.agents[0];
  a.x = 950; a.y = 300; a.heading = 0; a.vx = 0; a.vy = 0;
  w.asteroids = [{ x: 10, y: 300, vx: 0, vy: 0, r: 38, pts: 20, size: 'L', shape: [1], angle: 0, spin: 0 }];
  w.step(dt);
  ok('toroidal ray sees across seam (ray0)', captured[0] > 0.9, String(captured[0]));
  ok('closeness = 1 - 20/500 (center dist)', Math.abs(captured[13] - 0.96) < 1e-9, String(captured[13]));
  ok('pressure > 0', captured[19] > 0, String(captured[19]));
  ok('memory fed back (0 first step)', captured[20] === 0, String(captured[20]));
  ok('thrusting flag set', a.thrusting === true);
}

// 4. Seam collision kills.
{
  const w = new World([neverFire]);
  const a = w.agents[0];
  a.x = 10; a.y = 300;
  w.asteroids = [{ x: CONFIG.arena.width - 5, y: 300, vx: 0, vy: 0, r: 38, pts: 20, size: 'L', shape: [1], angle: 0, spin: 0 }];
  w.step(dt);
  ok('seam collision kills ship', a.alive === false);
}

// 5. Memory loop.
{
  const w = new World([stub]);
  w.step(dt);
  ok('agent.memory = output 25', Math.abs(w.agents[0].memory - 0.42) < 1e-9, String(w.agents[0].memory));
  w.step(dt);
  ok('input 20 = previous memory', Math.abs(captured[20] - 0.42) < 1e-9, String(captured[20]));
}

// 6. Wave escalation.
{
  const w = new World([neverFire]);
  ok('wave 0 start: 5 rocks', w.asteroids.length === 5 && w.wave === 0);
  w.asteroids = [];
  w.step(dt);
  ok('wave 1 spawns 7 rocks', w.wave === 1 && w.asteroids.length === 7, `${w.wave} ${w.asteroids.length}`);
  w.asteroids = [];
  w.step(dt);
  ok('wave 2 spawns 9 rocks', w.wave === 2 && w.asteroids.length === 9, `${w.wave} ${w.asteroids.length}`);
  ok('wave clock reset', w.waveTime <= dt * 2, String(w.waveTime));
}

// 7. Episode ends by wave clock while alive: immobile ship at center, immobile rocks
// parked at corners (toroidal distance ~552 > hit range, no wave can clear them).
{
  const w = new World([neverFire]);
  const a = w.agents[0];
  a.x = 480; a.y = 300;
  const parked = (x, y) => ({ x, y, vx: 0, vy: 0, r: 38, pts: 20, size: 'L', shape: [1], angle: 0, spin: 0 });
  w.asteroids = [parked(10, 10), parked(950, 10), parked(10, 590), parked(950, 590), parked(480, 10)];
  let steps = 0;
  while (!w.done && steps < 4000) { w.step(dt); steps++; }
  ok('episode ends by 60s wave clock, alive', w.done && w.agents[0].alive && Math.abs(w.waveTime - 60) < dt * 2, `done=${w.done} alive=${w.agents[0].alive} t=${w.waveTime} steps=${steps}`);
}

// 8. Hard cap overrides wave resets.
{
  const w = new World([neverFire]);
  w.time = 299.9;
  let steps = 0;
  while (!w.done && steps < 10) { w.step(dt); steps++; }
  ok('episode hard cap at 300s', w.done === true && w.time >= 300, `done=${w.done} t=${w.time}`);
}

// 9. Fire discipline: cooldown + max 4 + recoil.
{
  const alwaysFire = { activate: (inp) => { captured = inp.slice(); return [0, 0, 0, 1, 0]; } };
  const w = new World([alwaysFire]);
  const a = w.agents[0];
  a.heading = 0; a.vx = 0; a.vy = 0;
  for (let i = 0; i < 200; i++) w.step(dt);
  ok('max 4 bullets alive', a.bulletsOut <= 4, String(a.bulletsOut));
  ok('recoil pushed ship backward', a.x < 10 || a.vx < 0, `x=${a.x.toFixed(1)} vx=${a.vx.toFixed(1)}`);
  ok('guns sensor reads bulletsOut/4', Math.abs(captured[15] - a.bulletsOut / 4) < 1e-9, `${captured[15]} vs ${a.bulletsOut / 4}`);
}

// 10. Solo driver mini-run: 3 brains then evolve; orphans after 5 cycles.
{
  resetInnovation();
  const p2 = new Population(100);
  const fit = new Array(100).fill(0);
  for (let b = 0; b < 3; b++) {
    const w = new World([p2.networks[b]]);
    let steps = 0;
    while (!w.done && steps < 4000) { w.step(dt); steps++; }
    fit[b] = w.agents[0].fitness;
    ok(`brain ${b} solo episode finite`, Number.isFinite(fit[b]) && w.agents.length === 1, String(fit[b]));
  }
  for (let g = 0; g < 5; g++) {
    const f = Array.from({ length: 100 }, (_, i) => (Math.sin(i * 12.9898 + g * 78.233) * 43758.5453 % 1 + 1) * 500);
    p2.evolve(f);
  }
  ok('generation advanced to 6', p2.generation === 6, String(p2.generation));
  let orphans = 0;
  for (const g of p2.genomes) {
    const ids = new Set(g.nodes.keys());
    for (const c of g.connections.values()) {
      if (!ids.has(c.in) || !ids.has(c.out)) orphans++;
    }
  }
  ok('0 orphan connections after 5 evolves', orphans === 0, String(orphans));
  ok('networks rebuilt with 5 outputs', p2.networks[0].activate(new Array(21).fill(0)).length === 5);
}


// 12. Shaping fitness semantics (v5: movement reward, entropy bonus, novelty archive).
{
  const { NoveltyArchive } = await import('./js/population.js');
  // Movement reward: full-thrust brain vs immobile corner rocks (no early death).
  const mover = { activate: () => [0, 0, 0.9, 0, 0] };
  const wm = new World([mover]);
  const ship = wm.agents[0];
  ship.x = 480; ship.y = 300; ship.heading = 0; // east: never approaches any parked rock
  const parked = (x, y) => ({ x, y, vx: 0, vy: 0, r: 38, pts: 20, size: 'L', shape: [1], angle: 0, spin: 0 });
  wm.asteroids = [parked(10, 10), parked(950, 10), parked(10, 590), parked(950, 590), parked(480, 10)];
  for (let i = 0; i < 180; i++) wm.step(dt); // exactly 3s of full thrust
  const aliveOnly = CONFIG.fitness.alivePerSecond * wm.time;
  ok('movement reward accrued (>=12% of alive reward)', wm.agents[0].fitness > aliveOnly * 1.12, `${wm.agents[0].fitness.toFixed(0)} vs ${aliveOnly.toFixed(0)}`);
  ok('stats histogram counts steps', wm.agents[0].stats.steps > 0 && wm.agents[0].stats.thrust === wm.agents[0].stats.steps);
  const beh = wm.agentBehavior();
  ok('behavior descriptor has 7 normalized dims', beh.length === 7 && beh.every((v) => v >= 0 && v <= 1.01), JSON.stringify(beh.map((v) => +v.toFixed(2))));
  let steps = 0;
  const idle = { activate: () => [0, 0, 0, 0, 0] };
  const wi = new World([idle]);
  steps = 0;
  while (!wi.done && steps < 600) { wi.step(dt); steps++; }
  ok('mixed actions outscore idle per-second', (wm.agents[0].fitness / wm.time) > (wi.agents[0].fitness / wi.time) + 1,
    `${(wm.agents[0].fitness / wm.time).toFixed(1)} vs ${(wi.agents[0].fitness / wi.time).toFixed(1)}`);
  // Novelty archive: identical behavior -> ~0, distinct -> > 0; decay reaches floor.
  const arch = new NoveltyArchive();
  ok('empty archive novelty 0', arch.novelty(beh) === 0);
  arch.add(beh);
  ok('identical behavior novelty ~0', arch.novelty(beh) < 1e-9);
  const far = [1, 0, 0, 0, 1, 1, 1];
  ok('distinct behavior novelty > 0.5', arch.novelty(far) > 0.5, String(arch.novelty(far)));
  ok('bonus decays to floor by decayGens', Math.abs(arch.bonusFor(CONFIG.fitness.noveltyDecayGens + 1) - CONFIG.fitness.noveltyBonus * CONFIG.fitness.noveltyFloorFrac) < 1e-9,
    String(arch.bonusFor(CONFIG.fitness.noveltyDecayGens + 1)));
  ok('bonus full at gen 1', arch.bonusFor(1) > CONFIG.fitness.noveltyBonus * 0.9);
}
{
  const w = new World([stub]);
  for (let i = 0; i < 600; i++) w.step(dt);
  const t0 = performance.now();
  for (let i = 0; i < 20000; i++) w.step(dt);
  const per = (performance.now() - t0) / 20000;
  console.log(`  info solo step = ${per.toFixed(5)} ms`);
  ok('solo step < 0.05ms', per < 0.05, String(per));
}


const { setSeed, rng, randInt } = await import('./js/rng.js');

// 13. Seeded stream: randInt bounds + same-seed reproducibility.
{
  setSeed(42);
  const draws = Array.from({ length: 10000 }, () => randInt(7));
  ok('randInt(7) in [0,7) over 1e4 draws', draws.every((v) => Number.isInteger(v) && v >= 0 && v < 7));
  ok('randInt(7) covers all values', new Set(draws).size === 7);
  const r1 = [rng(), rng(), rng()];
  setSeed(42);
  Array.from({ length: 10000 }, () => randInt(7));
  const r2 = [rng(), rng(), rng()];
  ok('same seed -> same stream', JSON.stringify(r1) === JSON.stringify(r2));
  ok('rng() in [0,1)', r1.every((v) => v >= 0 && v < 1));
}

// 14. Genome JSON round-trip: toJSON -> fromJSON -> Network produces identical
// activate() outputs; fromJSON throws on wrong input/output node count.
{
  resetInnovation();
  setSeed(1234);
  const p3 = new Population(20);
  const g = p3.genomes[0];
  g.mutateAddNode();
  g.mutateAddNode();
  g.mutateAddConnection();
  const inputs = Array.from({ length: 21 }, (_, i) => Math.sin(i * 1.7));
  const outA = Network.fromGenome(g).activate(inputs);
  const g2 = Genome.fromJSON(JSON.parse(JSON.stringify(g.toJSON())));
  const outB = Network.fromGenome(g2).activate(inputs);
  ok('round-trip activate identical', JSON.stringify(outA) === JSON.stringify(outB), `${JSON.stringify(outA)} vs ${JSON.stringify(outB)}`);
  ok('round-trip preserves node/connection counts', g2.nodes.size === g.nodes.size && g2.connections.size === g.connections.size);
  const bad = { version: 1, nodes: [[0, 'input'], [1, 'input'], [21, 'output']], connections: [] };
  let threw = false;
  try { Genome.fromJSON(bad); } catch (e) { threw = true; }
  ok('fromJSON throws on wrong IO count', threw);
}

// 15. Gate instrumentation: aliveTime tracks world time; rockPts banks on a hit.
{
  const shooter = { activate: () => [0, 0, 0, 1, 0] };
  const w = new World([shooter]);
  const a = w.agents[0];
  a.x = 480; a.y = 300; a.heading = 0; a.vx = 0; a.vy = 0;
  w.asteroids = [{ x: 540, y: 300, vx: 0, vy: 0, r: 38, pts: 20, size: 'L', shape: [1], angle: 0, spin: 0 }];
  let steps = 0;
  while (!w.done && steps < 600) { w.step(dt); steps++; }
  ok('rockPts > 0 after forced bullet split', a.stats.rockPts > 0, String(a.stats.rockPts));
  ok('aliveTime tracks world.time', Math.abs(a.stats.aliveTime - w.time) < dt * 2, `${a.stats.aliveTime} vs ${w.time}`);
}

// 16. Seeded full repro: population + 5 complete episodes, twice from scratch,
// must bank identical fitness arrays (novelty path included).
{
  const { NoveltyArchive } = await import('./js/population.js');
  const runFive = () => {
    setSeed(42);
    resetInnovation();
    const p = new Population(100);
    const arch = new NoveltyArchive();
    const fit = [];
    for (let i = 0; i < 5; i++) {
      const w = new World([p.networks[i]]);
      let s = 0;
      while (!w.done && s < 20000) { w.step(dt); s++; }
      const beh = w.agentBehavior();
      fit.push(w.agents[0].fitness + arch.bonusFor(p.generation) * arch.novelty(beh));
      arch.add(beh);
    }
    return fit;
  };
  const ra = runFive();
  const rb = runFive();
  ok('seeded 5-episode replay deep-equal', JSON.stringify(ra) === JSON.stringify(rb));
  ok('seeded fitnesses finite', ra.every(Number.isFinite));
}
// 17. HiDPI helper: mock canvas, backing = css*DPR capped, transform = DPR*scale (render-only — no physics drift).
{
  const { setupHiDPI } = await import('./js/render.js');
  const mockCtx = { sx: 0, sy: 0, setTransform(sx, _a, _b, sy) { this.sx = sx; this.sy = sy; } };
  const makeMock = () => ({ width: 0, height: 0, style: {}, getContext() { return mockCtx; } });
  // DPR 1 fallback (node, no window → dpr 1)
  {
    const c = makeMock();
    const r = setupHiDPI(c, 300, 110);
    ok('HiDPI mock DPR1 300x110 backing 300x110', c.width === 300 && c.height === 110 && r.dpr === 1 && mockCtx.sx === 1, `${c.width}x${c.height} dpr=${r.dpr} sx=${mockCtx.sx}`);
    ok('HiDPI style stays CSS pixels DPR1', c.style.width === '300px' && c.style.height === '110px');
  }
  // Fitted arena: emulate DPR 2 via global window stub — should get 1920x1200 backing and sx=2, sy=2
  {
    const prevWin = globalThis.window;
    globalThis.window = { devicePixelRatio: 2, matchMedia: () => ({ addEventListener() {}, addListener() {} }) };
    // Re-import to capture rawDpr? setupHiDPI reads window at call time, so no re-import needed.
    const c = makeMock();
    // css == logical (scale 1) → sx = DPR
    setupHiDPI(c, 300, 110, 300, 110);
    ok('HiDPI DPR2 chart 300x110 -> 600x220 sx=2', c.width === 600 && c.height === 220 && mockCtx.sx === 2 && mockCtx.sy === 2, `${c.width}x${c.height} sx=${mockCtx.sx}`);
    const a = makeMock();
    // Arena fitted scale 0.5: logical 960x600, css 480x300 → backing 960x600 (480*2), sx = 2*0.5=1
    setupHiDPI(a, 480, 300, 960, 600);
    ok('HiDPI DPR2 fitted arena 480x300/logical 960x600 -> 960x600 sx=1', a.width === 960 && a.height === 600 && mockCtx.sx === 1 && mockCtx.sy === 1, `${a.width}x${a.height} sx=${mockCtx.sx}`);
    const b = makeMock();
    // Arena scale 1: logical 960x600 css 960x600 DPR2 -> 1920x1200 sx=2
    setupHiDPI(b, 960, 600, 960, 600);
    ok('HiDPI DPR2 arena 960x600 logical -> 1920x1200 sx=2', b.width === 1920 && b.height === 1200 && mockCtx.sx === 2 && mockCtx.sy === 2, `${b.width}x${b.height} sx=${mockCtx.sx}`);
    // Cap at 2: DPR 3 → still 2
    globalThis.window.devicePixelRatio = 3;
    const cap = makeMock();
    setupHiDPI(cap, 960, 600, 960, 600);
    ok('HiDPI DPR cap 2: raw 3 -> dpr 2', cap._dpr === 2 && cap.width === 1920, `dpr=${cap._dpr} w=${cap.width}`);
    globalThis.window = prevWin;
  }
  // Determinism unchanged: a single World step hash with HiDPI setup preceding it must match baseline.
  {
    setSeed(7);
    resetInnovation();
    const p = new Population(5);
    const w = new World([p.networks[0]]);
    for (let i = 0; i < 10; i++) w.step(dt);
    const h = `${w.agents[0].x.toFixed(6)},${w.agents[0].y.toFixed(6)},${w.asteroids[0].x.toFixed(2)}`;
    // Run again after a mock HiDPI call that should not touch rng/world.
    const { setupHiDPI: sh } = await import('./js/render.js');
    const c = { width: 0, height: 0, style: {}, getContext() { return { setTransform() {} }; } };
    sh(c, 960, 600, 960, 600);
    setSeed(7);
    resetInnovation();
    const p2 = new Population(5);
    const w2 = new World([p2.networks[0]]);
    for (let i = 0; i < 10; i++) w2.step(dt);
    const h2 = `${w2.agents[0].x.toFixed(6)},${w2.agents[0].y.toFixed(6)},${w2.asteroids[0].x.toFixed(2)}`;
    ok('HiDPI does not perturb deterministic physics', h === h2, `${h} vs ${h2}`);
  }
}
// 18. Locked Arcade Sketch — Win98 bevel + Rough ESM mandatory, no vanilla fallback
{
  // NOTE: hachure/draw-count asserts below exercise the headless stub's echo (node_modules/roughjs),
  // not the real jsDelivr Rough ESM — they are wiring-regression guards (cache hit, seam 4×/1×, no vanilla stroke).
  // Real Rough API compat is proven by browser smoke at http://127.0.0.1:8899/index.html?seed=1409546675
  // (importmap rough.esm.js 200 CORS *, canvas pixelAvg ~52, 98.css/NES all 200, favicon 404 only).
  const fs = await import('node:fs');
  const src = fs.readFileSync('js/render.js', 'utf8');

  // Importmap ESM path + singleton generator present
  ok('render.js imports rough from roughjs', src.includes("import rough from 'roughjs'") || src.includes('import rough from "roughjs"'));
  ok('render.js uses rough.generator singleton via _roughGen', src.includes('rough.generator') && src.includes('_roughGen'));

  // No vanilla jitter polygon stroke remains — stroke #9aa4b0 lineWidth 1.5 deleted
  ok('render.js no vanilla asteroid stroke #9aa4b0', !src.includes("strokeStyle = '#9aa4b0'") && !src.includes('strokeStyle = "#9aa4b0"'));

  // Cached drawable and per-seam rc.draw with save/translate/restore
  ok('render.js caches _roughDrawable', src.includes('_roughDrawable'));
  ok('render.js per seam rc.draw with save/translate/restore', src.includes('rc.draw') && src.includes('ctx.save()') && src.includes('ctx.translate') && src.includes('ctx.restore()') && src.includes('rough.canvas'));


  // Exports intact
  ok('render.js exports setupHiDPI', src.includes('export function setupHiDPI'));
  ok('render.js exports seamCopies', src.includes('export function seamCopies'));

  // FONT updated to Space Mono / Press Start 2P
  ok('render.js FONT uses Space Mono / Press Start 2P', src.includes('Space Mono') && src.includes('Press Start 2P'));

  // Starfield still 120 dots, not replaced
  ok('render.js starfield still 120 dots', src.includes('starCount') && src.includes('Math.random() * W'));

  const makeRoughMock = () => {
    const canvasEl = { width: 0, height: 0, style: {}, __roughDrawCalls: 0 };
    const ctx = {
      canvas: canvasEl,
      fillStyle: '', strokeStyle: '', lineWidth: 1, __rects: [],
      fillRect() { this.__fillRects = (this.__fillRects || 0) + 1; },
      strokeRect(x, y, w, h) { this.__rects.push([x, y, w, h, this.strokeStyle]); },
      beginPath() {}, arc() {}, fill() {}, stroke() {},
      moveTo() {}, lineTo() {}, closePath() {}, save() {}, restore() {}, translate() {}, rotate() {},
      setTransform() {}, clearRect() {}, createRadialGradient() { return { addColorStop() {} }; },
    };
    canvasEl.getContext = () => ctx;
    ctx.canvas = canvasEl;
    return { canvasEl, ctx };
  };

  // seamCopies wrap correctness (4× at corner, 2 at edge, 1 centered)
  {
    const { seamCopies } = await import('./js/render.js');
    ok('seamCopies center -> 1 copy', seamCopies(480, 300, 10).length === 1);
    ok('seamCopies edge left -> 2 copies', seamCopies(5, 300, 10).length === 2);
    ok('seamCopies corner -> 4 copies', seamCopies(5, 5, 10).length === 4);
    ok('seamCopies opposite edge still 2', seamCopies(955, 300, 10).length === 2);
  }

  // Rough drawable cache hit + seam draw count (GC-safe: 0 per-frame alloc of Drawable)
  {
    const { renderArena } = await import('./js/render.js');
    const { canvasEl, ctx } = makeRoughMock();
    const world = {
      asteroids: [{ x: 5, y: 5, r: 38, shape: Array(10).fill(1), angle: 0, spin: 0, size: 'L' }],
      agents: [{ x: 480, y: 300, heading: 0, alive: false, inputs: null, thrusting: false }],
      bullets: [],
    };
    renderArena(ctx, world, -1, false);
    const d1 = world.asteroids[0]._roughDrawable;
    ok('asteroid._roughDrawable created on first render', !!d1 && d1._kind === 'polygon');
    // ADR 0001 locked polygon option set — asserted on the drawable echo, not source text
    ok('Rough drawable matches ADR 0001 locked option set', d1.options && d1.options.stroke === '#1a1a1a' && d1.options.fill === '#9aa4b0' && d1.options.fillStyle === 'hachure' && d1.options.roughness === 1.1 && d1.options.bowing === 1 && d1.options.strokeWidth === 1 && d1.options.fillWeight === undefined && d1.options.hachureGap === undefined);
    ok('Rough polygon seed is a nonzero integer', Number.isInteger(d1.options.seed) && d1.options.seed >= 1);
    const callsAfter1 = canvasEl.__roughDrawCalls;
    ok('rough.canvas draw called per seam copy (4 at corner)', callsAfter1 === 4, String(callsAfter1));
    // Inset bevel rects per prototype Target1 — behavioral: captured strokeRect calls
    ok('arena inset bevel rects 0.5/959 + 1.5/957 with bevel strokes', ctx.__rects.some(r => r[0] === 0.5 && r[1] === 0.5 && r[2] === 959 && r[3] === 599 && r[4] === 'rgba(255,255,255,0.06)') && ctx.__rects.some(r => r[0] === 1.5 && r[1] === 1.5 && r[2] === 957 && r[3] === 597 && r[4] === 'rgba(0,0,0,0.22)'));
    canvasEl.__roughDrawCalls = 0;
    renderArena(ctx, world, -1, false);
    const d2 = world.asteroids[0]._roughDrawable;
    ok('asteroid._roughDrawable reused second frame (cache hit)', d1 === d2);
    ok('rough draw called again without new Drawable', canvasEl.__roughDrawCalls === 4);
    canvasEl.__roughDrawCalls = 0;
    world.asteroids = [{ x: 480, y: 300, r: 38, shape: Array(10).fill(1), angle: 0, spin: 0, size: 'L' }];
    renderArena(ctx, world, -1, false);
    ok('center asteroid draw count 1 seam copy', canvasEl.__roughDrawCalls === 1, String(canvasEl.__roughDrawCalls));
  }

  // Throttling >16× preserved: ArenaRenderer.frame throttles arena draws (behavioral)
  {
    const { ArenaRenderer } = await import('./js/render.js');
    const { canvasEl, ctx } = makeRoughMock();
    const ar = new ArenaRenderer({ arena: canvasEl });
    const world = { asteroids: [], agents: [], bullets: [] };
    for (let i = 0; i < 8; i++) ar.frame({ world, realDt: 0, speed: 17 });
    ok('ArenaRenderer throttles >16x: 2 draws in 8 frames', ctx.__fillRects === 2, String(ctx.__fillRects));
    for (let i = 0; i < 3; i++) ar.frame({ world, realDt: 0, speed: 10 });
    ok('ArenaRenderer draws every frame at speed<=16: 3 draws', ctx.__fillRects === 5, String(ctx.__fillRects));
  }

  // Logical size guard
  ok('logical arena still 960x600 after Rough lock', CONFIG.arena.width === 960 && CONFIG.arena.height === 600);
}
console.log(`\n${pass} passed, ${fail} failed`);
process.exit(fail ? 1 : 0);
