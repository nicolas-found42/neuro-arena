// verify-context.mjs — Documentation seam for CONTEXT.md
// Verifies CONTEXT-FORMAT altitude: 1–2 sentence "what it IS", project-specific only,
// Avoid lists, no spec tables, grounded without bloating.
// Usage: node verify-context.mjs [--fixture path]
// Exit 1 on any failure, 0 on all green.
//
// Seam: documentation layer — reads CONTEXT.md as text and asserts invariants,
// mirroring verify.mjs style but for glossary format (no js/ behavior changes).

import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

const target = process.argv.includes('--fixture')
  ? resolve(process.argv[process.argv.indexOf('--fixture') + 1])
  : resolve('CONTEXT.md');

if (!existsSync(target)) {
  console.error(`FAIL  CONTEXT.md not found at ${target}`);
  process.exit(1);
}

const raw = readFileSync(target, 'utf8');

let pass = 0, fail = 0;
const ok = (name, cond, detail = '') => {
  if (cond) { pass++; console.log(`  ok  ${name}`); }
  else { fail++; console.log(`FAIL  ${name} ${detail}`); }
};

// Helpers
const countSentences = (text) => {
  // Trim Avoid clause already stripped; count sentence terminators.
  // Split on . ! ? followed by space or end; filter fragments with letters.
  const fragments = text.trim().split(/(?<=[.!?])\s+/).filter(s => /[A-Za-z0-9]/.test(s));
  // Also handle definition that is single sentence without trailing space split
  // If definition ends without period (should not), count as 1
  if (fragments.length === 0 && text.trim().length > 0) return 1;
  return fragments.length;
};

const entries = [];
// Parse entries: **Term**: definition _Avoid_: list
// Allow multiline definitions; term line starts with **
const entryRegex = /^\*\*([^*]+)\*\*:\s*(.+?)\s*_Avoid_:\s*(.+)$/gm;
let m;
while ((m = entryRegex.exec(raw)) !== null) {
  const term = m[1].trim();
  const def = m[2].trim();
  const avoid = m[3].trim();
  entries.push({ term, def, avoid, raw: m[0] });
}

const termSet = new Set(entries.map(e => e.term));

// 1. File structure — title, intro, Language header, clusters
ok('CONTEXT.md has title', /^# Neuroevolution Asteroids/m.test(raw));
ok('CONTEXT.md has ## Language', /^## Language/m.test(raw));
ok('Has Arena & Simulation cluster', /### Arena & Simulation/.test(raw));
ok('Has Evolution cluster', /### Evolution/.test(raw));
ok('Has Presentation cluster', /### Presentation/.test(raw));
ok('No Sensing & Control cluster (Sensor Ray folds into Arena)', !/### Sensing & Control/.test(raw));

// 2. Required terms present (19 tight + Sensor Ray = 20)
const required = [
  'Arena', 'World', 'Ship', 'Agent', 'Asteroid', 'Wave', 'Episode', 'Seam Copies', 'Sensor Ray',
  'Genome', 'Network', 'Population', 'Generation', 'Species', 'Fitness', 'Competence Gate', 'Innovation Tracker',
  'HUD', 'Chart', 'Arcade Sketch',
];
for (const t of required) {
  ok(`term present: ${t}`, termSet.has(t));
}
ok(`term count is 20 (19+Sensor Ray)`, entries.length === 20, `got ${entries.length}: ${[...termSet].join(', ')}`);

// 3. No banned standalone spec terms (Physics Step, Sensors, Memory Loop, Seed as headers)
const bannedHeaders = ['Physics Step', 'Sensors', 'Memory Loop', 'Seed', 'Brain', 'Physics', 'Behavior Descriptor', 'Novelty Archive'];
for (const b of bannedHeaders) {
  // Exact header match: **Banned**:
  ok(`no banned header: ${b}`, !new RegExp(`^\\*\\*${b}\\*\\*:\\s`, 'm').test(raw), `found **${b}**`);
}
// Special: Sensors as standalone is banned, but "Sensor Ray" is allowed — ensure not confused
ok('Sensor Ray present, not Sensors header', termSet.has('Sensor Ray') && !termSet.has('Sensors'));

// 4. Every entry is 1–2 sentences of "what it IS"
for (const e of entries) {
  const n = countSentences(e.def);
  ok(`1–2 sentences: ${e.term} (${n})`, n >= 1 && n <= 2, `"${e.def}" => ${n} sentences`);
}

// 5. Every entry has Avoid list (parsed) and Avoid list is opinionated (contains at least one synonym)
for (const e of entries) {
  ok(`Avoid list present: ${e.term}`, e.avoid.length > 0, JSON.stringify(e));
  ok(`Avoid list non-empty synonyms: ${e.term}`, e.avoid.split(',').filter(s => s.trim()).length >= 1);
}

// 6. No spec tables or code-snippet bloat
ok('no markdown tables', !/^\s*\|.*\|.*$/m.test(raw), 'found | table row');
ok('no 21-element Sensors table', !/21[\s-]*element/i.test(raw));
ok('no js file paths in glossary', !/\bjs\//.test(raw));
ok('no import/code snippet', !/```/.test(raw) && !/\bimport\s+rough\b/.test(raw));
ok('no physics-step recipe enumeration', !/0\.7.*radius|damping.*speed/i.test(raw));
ok('no RNG algorithm detail', !/xorshift|mulberry|rng.*algorithm/i.test(raw));

// 7. Project-specific altitude — definitions mention domain concepts, not generic timeouts/errors
ok('no generic timeout/error language', !/\btimeout\b|\berror types?\b/i.test(raw));

// 8. ADR alignment — wording respects locked decisions
const world = entries.find(e => e.term === 'World');
ok('World steps one Agent through one Episode (ADR-0002 solo)', world && /one Agent.*one Episode/.test(world.def));
const arcade = entries.find(e => e.term === 'Arcade Sketch');
ok('Arcade Sketch locked no runtime variant switcher (ADR-0001)', arcade && /Locked.*no runtime variant switcher/.test(arcade.def));
const fitness = entries.find(e => e.term === 'Fitness');
ok('Fitness shaped selection score (ADR-0003)', fitness && /Shaped selection score/.test(fitness.def));
const gate = entries.find(e => e.term === 'Competence Gate');
ok('Competence Gate raw skill metrics in HUD (ADR-0003)', gate && /Raw skill metrics.*HUD/.test(gate.def));
const arena = entries.find(e => e.term === 'Arena');
ok('Arena 960×600 toroidal playfield', arena && /960×600.*toroidal playfield/.test(arena.def));
const sensorRay = entries.find(e => e.term === 'Sensor Ray');
ok('Sensor Ray toroidal fixed to Ship heading nearest Asteroid distance', sensorRay && /toroidal.*fixed to.*Ship.*heading.*nearest Asteroid/.test(sensorRay.def));
const asteroid = entries.find(e => e.term === 'Asteroid');
ok('Asteroid wraps via Seam Copies and splits', asteroid && /Seam Copies/.test(asteroid.def) && /splits/.test(asteroid.def));
const hud = entries.find(e => e.term === 'HUD');
ok('HUD shows Generation, Wave, gate state', hud && /Generation/.test(hud.def) && /Wave/.test(hud.def));

// 9. Grounding covers 12 modules conceptually (no file paths, but terms map)
// We assert coverage by term existence, not file content:
// physics -> Arena/World/Asteroid/Seam Copies, sensors -> Sensor Ray, neat/population/evaluation -> Genome/Network/Innovation Tracker/Species/Fitness/Competence Gate,
// game/evolution/rng/render/main -> World/Episode/Wave/Ship/Agent/Arcade Sketch/HUD/Chart
ok('physics grounded via Arena/World/Asteroid/Seam Copies', ['Arena','World','Asteroid','Seam Copies'].every(t=>termSet.has(t)));
ok('sensors grounded via Sensor Ray', termSet.has('Sensor Ray'));
ok('neat grounded via Genome/Network/Innovation Tracker/Species', ['Genome','Network','Innovation Tracker','Species'].every(t=>termSet.has(t)));
ok('evaluation grounded via Fitness/Competence Gate', termSet.has('Fitness') && termSet.has('Competence Gate'));
ok('game/evolution grounded via World/Episode/Wave/Population/Generation', ['World','Episode','Wave','Population','Generation'].every(t=>termSet.has(t)));
ok('render/main grounded via HUD/Chart/Arcade Sketch', ['HUD','Chart','Arcade Sketch'].every(t=>termSet.has(t)));

// 10. Each definition starts with capital "The" or "One" or noun phrase — lightweight "what it IS" check
for (const e of entries) {
  const startsWithArticle = /^(The|One|Reusable|Extra|Compatibility|Shaped|Raw|Ordered|Overlay|Fitness|Locked|Toroidal|Evolvable|Feedforward|The fixed)/.test(e.def);
  // Sensor Ray starts with The, Wave with One, World with Reusable, etc. — allow any capital start
  ok(`definition capital start: ${e.term}`, /^[A-Z]/.test(e.def), `"${e.def.slice(0,40)}"`);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail) {
  console.log('\nHint: CONTEXT.md should be tight 1–2 sentence per term, project-specific only.');
  console.log('Banned headers: Physics Step, Sensors (use Sensor Ray), Memory Loop, Seed.');
  console.log('Keep clusters Arena & Simulation / Evolution / Presentation; fold grounding as clause.');
}
process.exit(fail ? 1 : 0);
