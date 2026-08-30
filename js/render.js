// Rendering: arena frame (ship under evaluation + its rays/flame), HUD, chart, network panel.
import { CONFIG } from './config.js';
import rough from 'roughjs';

// Singleton Rough generator — one instance for the lifetime of the page.
// Cached drawables per asteroid (asteroid._roughDrawable) mean zero per-frame
// alloc of Drawable; per-frame `rough.canvas(ctx.canvas).draw(drawable)` reuses
// the cached object via ctx.save/translate/restore after setupHiDPI transform.
// GC-safe at ×10000: only one generator, one drawable per asteroid, rc wrapper
// per frame is transient but tiny. Options match arcade-sketch Target2 bevel:
// hand-drawn hachure fill on warm grey, thin outline.
const _roughGen = rough.generator({ roughness: 1.2, bowing: 1 });

const W = CONFIG.arena.width;
const H = CONFIG.arena.height;
const FONT = "'Space Mono', 'Press Start 2P', monospace";

// Static starfield, precomputed once. Arcade variant keeps 120 dots, no CRT
// twinkle or nebula gradient — plain arc+fill stars, GC-free.
const stars = Array.from({ length: CONFIG.arena.starCount }, () => ({
  x: Math.random() * W,
  y: Math.random() * H,
  r: 0.5 + Math.random() * 1.2,
}));
// HiDPI setup: backing store = cssPixels * DPR (capped), style = cssPixels,
// context transform maps logical coords to device pixels.
// For arena: cssW = 960*scale, logicalW = 960 → transform = DPR*scale (fitted).
// For chart/net: cssW === logicalW → transform = DPR.
// Headless (no window) falls back to DPR=1 and becomes a no-op for determinism checks.
export function setupHiDPI(canvas, cssW, cssH, logicalW = cssW, logicalH = cssH) {
  const rawDpr = typeof window !== 'undefined' && window.devicePixelRatio ? window.devicePixelRatio : 1;
  const cap = CONFIG.render.dprCap ?? 2;
  const dpr = Math.min(rawDpr, cap);
  // Caller floors cssW/cssH (arena fitted); keep floor for backing too.
  const w = Math.max(1, Math.floor(cssW * dpr));
  const h = Math.max(1, Math.floor(cssH * dpr));
  // Setting width/height resets the context state — do it before setTransform.
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  // Style size is CSS pixels — browser composites backing → style.
  canvas.style.width = cssW + 'px';
  canvas.style.height = cssH + 'px';
  const ctx = canvas.getContext('2d');
  // logical → device: device = logical * (css/logical) * dpr
  const sx = dpr * (cssW / logicalW);
  const sy = dpr * (cssH / logicalH);
  // Guard division-by-zero (should never happen, logicalW/H >0)
  if (Number.isFinite(sx) && Number.isFinite(sy)) ctx.setTransform(sx, 0, 0, sy, 0, 0);
  // Exposed for tests/debugging.
  canvas._dpr = dpr;
  canvas._cssW = cssW;
  canvas._cssH = cssH;
  return { dpr, cssW, cssH, w, h };
}


// Classic Asteroids seam behavior: an entity overlapping an arena edge is drawn
// again on the opposite side (up to 4 copies when straddling a corner).
export function seamCopies(x, y, r) {
  const xs = x - r < 0 ? [0, W] : x + r >= W ? [0, -W] : [0];
  const ys = y - r < 0 ? [0, H] : y + r >= H ? [0, -H] : [0];
  const out = [];
  for (const dx of xs) for (const dy of ys) out.push([dx, dy]);
  return out;
}
export function renderArena(ctx, world, shipIdx, showRays) {
  ctx.fillStyle = CONFIG.arena.background;
  ctx.fillRect(0, 0, W, H);

  // Arena inset highlight per prototype Target1 arcade-sketch: subtle bevel frame
  // inside the 960×600 canvas, preserves setupHiDPI transform (1 logical px == 1 line).
  ctx.strokeStyle = 'rgba(255,255,255,0.06)';
  ctx.lineWidth = 1;
  ctx.strokeRect(0.5, 0.5, W - 1, H - 1);
  ctx.strokeStyle = 'rgba(0,0,0,0.22)';
  ctx.strokeRect(1.5, 1.5, W - 3, H - 3);

  ctx.fillStyle = `rgba(255,255,255,${CONFIG.arena.starAlpha})`;
  for (const s of stars) {
    ctx.beginPath();
    ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
    ctx.fill();
  }

  // Asteroids — Rough.js ESM mandatory, no vanilla fallback.
  // Cache once per asteroid via _roughGen.polygon seeded by asteroid shape;
  // per-frame rc.draw(drawable) per seam copy via save/translate/restore.
  // Drawable is centered at (0,0); world position + seam offset + spin applied
  // via canvas transform so cache survives movement and rotation. Invalidated on
  // shatter implicitly — new asteroid objects get new _roughDrawable.
  const rc = rough.canvas(ctx.canvas);
  for (const a of world.asteroids) {
    if (!a._roughDrawable) {
      const n = a.shape.length;
      const verts = new Array(n);
      for (let i = 0; i < n; i++) {
        const ang = (i / n) * Math.PI * 2;
        const rr = a.r * a.shape[i];
        verts[i] = [Math.cos(ang) * rr, Math.sin(ang) * rr];
      }
      // Deterministic seed derived from the shape jitter only — stable per asteroid
      // and independent of first-render position/timing.
      const seed = ((Math.floor(a.shape[0] * 100000) ^ Math.floor(a.shape[1] * 100000)) >>> 0) || 1;
      a._roughDrawable = _roughGen.polygon(verts, {
        stroke: '#1a1a1a',
        fill: '#9aa4b0',
        fillStyle: 'hachure',
        roughness: 1.1,
        bowing: 1,
        strokeWidth: 1,
        seed,
      });
    }
  }
  for (const a of world.asteroids) {
    const d = a._roughDrawable;
    for (const [dx, dy] of seamCopies(a.x, a.y, a.r)) {
      ctx.save();
      ctx.translate(a.x + dx, a.y + dy);
      ctx.rotate(a.angle);
      rc.draw(d);
      ctx.restore();
    }
  }

  // Bullets: only the ship under evaluation.
  const leader = shipIdx >= 0 ? world.agents[shipIdx] : null;
  ctx.fillStyle = 'rgba(255,255,255,0.35)';
  if (leader) {
    for (const b of world.bullets) {
      if (b.owner !== leader) continue;
      for (const [dx, dy] of seamCopies(b.x, b.y, CONFIG.bullet.radius)) {
        ctx.beginPath();
        ctx.arc(b.x + dx, b.y + dy, CONFIG.bullet.radius, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  // Vision rays + ship: drawn once per seam copy of the ship, so the ship and
  // its rays read correctly on both sides of an edge (classic behavior).
  if (leader && leader.alive) {
    const offsets = CONFIG.sensors.rayOffsetsDeg;
    for (const [dx, dy] of seamCopies(leader.x, leader.y, 14)) {
      const sx = leader.x + dx;
      const sy = leader.y + dy;
      if (showRays && leader.inputs) {
        ctx.strokeStyle = 'rgba(255,90,90,0.30)';
        ctx.lineWidth = 1;
        for (let k = 0; k < offsets.length; k++) {
          const ang = leader.heading + offsets[k] * Math.PI / 180;
          const len = (1 - leader.inputs[k]) * CONFIG.sensors.range;
          if (len <= 0) continue;
          ctx.beginPath();
          ctx.moveTo(sx, sy);
          ctx.lineTo(sx + Math.cos(ang) * len, sy + Math.sin(ang) * len);
          ctx.stroke();
        }
      }
      ctx.save();
      ctx.translate(sx, sy);
      ctx.rotate(leader.heading);
      if (leader.thrusting) {
        ctx.fillStyle = 'rgba(255,160,60,0.8)';
        ctx.beginPath();
        ctx.moveTo(-8, -3);
        ctx.lineTo(-15, 0);
        ctx.lineTo(-8, 3);
        ctx.closePath();
        ctx.fill();
      }
      ctx.fillStyle = '#ffffff';
      ctx.beginPath();
      ctx.moveTo(12, 0);
      ctx.lineTo(-8, -7);
      ctx.lineTo(-8, 7);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
      // Ship inset highlight — plain 1px white circle arc (arcade-sketch, as prototype drew it)
      ctx.strokeStyle = '#ffffff';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.arc(sx, sy, 14, 0, Math.PI * 2);
      ctx.stroke();
    }
  }
}

export function renderOverlay(ctx, text) {
  ctx.fillStyle = 'rgba(5,8,15,0.78)';
  ctx.fillRect(0, H / 2 - 44, W, 88);
  ctx.fillStyle = '#e8ecf4';
  ctx.font = '26px ' + FONT;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, W / 2, H / 2);
  ctx.textAlign = 'left';
  ctx.textBaseline = 'alphabetic';
}

export function renderHUD(el, pop, world, brainIdx, info = {}) {
  const agent = world.agents[0];
  const h = pop.history[pop.history.length - 1];
  const best =
    pop.bestEver.gen > 0
      ? `${pop.bestEver.fitness.toFixed(1)} (gen ${pop.bestEver.gen})`
      : '—';
  const last = h ? `${h.best.toFixed(1)} avg ${h.avg.toFixed(1)}` : '—';
  // innerHTML: the tripped gate segment renders red. Dynamic parts are numbers only.
  el.innerHTML =
    (info.showcase ? '<span style="color:#ffb347">SHOWCASE</span> · ' : '') +
    `Gen ${pop.generation} · Brain ${Math.min(brainIdx + 1, pop.size)}/${pop.size}` +
    ` · t ${world.time.toFixed(1)}s · wave ${world.wave + 1}` +
    (agent && agent.alive ? '' : ' · dead') +
    ` · Best ever ${best} · Last gen best ${last}` +
    ` · seed ${info.seed}` +
    (info.gateTripped
      ? ` · <span style="color:#ff5a5a">GATE TRIPPED gen ${info.trippedGen}</span>`
      : ` · gate ${info.gateCount ?? 0}/15`);
}

export function renderChart(ctx, pop) {
  const { width: cw, height: ch, bestColor, avgColor, maxGens } = CONFIG.render.chart;
  ctx.clearRect(0, 0, cw, ch);
  const data = pop.history.slice(-maxGens);
  if (!data.length) return;

  let lo = Infinity;
  let hi = -Infinity;
  for (const d of data) {
    lo = Math.min(lo, d.best, d.avg);
    hi = Math.max(hi, d.best, d.avg);
  }
  if (hi - lo < 1e-9) hi = lo + 1;
  const pad = (hi - lo) * 0.1;
  lo -= pad;
  hi += pad;
  const x = (i) => 8 + (i / Math.max(1, data.length - 1)) * (cw - 16);
  const y = (v) => ch - 14 - ((v - lo) / (hi - lo)) * (ch - 26);

  const line = (key, color) => {
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    data.forEach((d, i) => (i === 0 ? ctx.moveTo(x(i), y(d[key])) : ctx.lineTo(x(i), y(d[key]))));
    ctx.stroke();
    // Latest-point marker (keeps single-point generations visible).
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(x(data.length - 1), y(data[data.length - 1][key]), 2.5, 0, Math.PI * 2);
    ctx.fill();
  };
  line('avg', avgColor);
  line('best', bestColor);

  const cur = data[data.length - 1];
  ctx.font = '11px ' + FONT;
  ctx.textAlign = 'left';
  ctx.fillStyle = bestColor;
  ctx.fillText(`best ${cur.best.toFixed(1)}`, 8, 12);
  ctx.textAlign = 'right';
  ctx.fillStyle = avgColor;
  ctx.fillText(`avg ${cur.avg.toFixed(1)}`, cw - 8, 12);
  ctx.textAlign = 'left';
}

export function renderNetwork(ctx, genome, network) {
  const { width: nw, height: nh, inX, hiddenXMin, hiddenXMax, outX, nodeRadius } =
    CONFIG.render.network;
  ctx.clearRect(0, 0, nw, nh);
  if (!genome) return;

  // --- Node positions.
  const pos = new Map();
  const byType = (type) =>
    [...genome.nodes.entries()].filter(([, n]) => n.type === type).map(([id]) => id).sort((a, b) => a - b);
  const spread = (ids, x, y0, y1) => {
    ids.forEach((id, i) =>
      pos.set(id, {
        x,
        y: ids.length === 1 ? (y0 + y1) / 2 : y0 + (i / (ids.length - 1)) * (y1 - y0),
      })
    );
  };
  spread(byType('input'), inX, 25, nh - 25);
  spread(byType('output'), outX, 60, nh - 60);

  const hiddens = byType('hidden');
  if (hiddens.length) {
    // Longest-path depth from inputs over enabled connections.
    const inConns = new Map(); // out id -> [in ids]
    for (const c of genome.connections.values()) {
      if (!c.enabled) continue;
      let l = inConns.get(c.out);
      if (!l) inConns.set(c.out, (l = []));
      l.push(c.in);
    }
    const memo = new Map();
    const depth = (id) => {
      if (memo.has(id)) return memo.get(id);
      let d = 0;
      const inc = inConns.get(id);
      if (inc) for (const p of inc) d = Math.max(d, depth(p) + 1);
      memo.set(id, d);
      return d;
    };
    const cols = new Map(); // depth -> [ids]
    let maxD = 1;
    for (const id of hiddens) {
      const d = Math.max(1, depth(id)); // orphan hidden nodes sit at the first column
      maxD = Math.max(maxD, d);
      let l = cols.get(d);
      if (!l) cols.set(d, (l = []));
      l.push(id);
    }
    for (const [d, ids] of cols) {
      // maxD === 1 (all hidden at depth 1) must not divide by zero -> NaN x silently draws nothing.
      const x =
        maxD <= 1
          ? hiddenXMin
          : hiddenXMin + ((d - 1) / (maxD - 1)) * (hiddenXMax - hiddenXMin);
      spread(ids, x, 30, nh - 30);
    }
  }

  // --- Edges (enabled only).
  ctx.globalAlpha = 0.6;
  for (const c of genome.connections.values()) {
    if (!c.enabled) continue;
    const A = pos.get(c.in);
    const B = pos.get(c.out);
    if (!A || !B) continue;
    ctx.strokeStyle = c.w >= 0 ? '#46e08a' : '#e05a46';
    ctx.lineWidth = Math.min(3, 0.5 + Math.abs(c.w));
    ctx.beginPath();
    ctx.moveTo(A.x, A.y);
    ctx.lineTo(B.x, B.y);
    ctx.stroke();
  }
  ctx.globalAlpha = 1;

  // --- Nodes, filled by last activation.
  for (const id of genome.nodes.keys()) {
    const P = pos.get(id);
    if (!P) continue;
    const v = network ? (network.lastActivations.get(id) ?? 0) : 0;
    const mag = Math.min(1, Math.abs(v));
    ctx.fillStyle = v >= 0 ? `rgba(60,220,120,${mag})` : `rgba(80,140,255,${mag})`;
    ctx.strokeStyle = '#888';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.arc(P.x, P.y, nodeRadius, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
  }

  // --- Labels: action names right of outputs, sensor group tags left of inputs.
  ctx.font = '9px ' + FONT;
  ctx.fillStyle = '#7f8ba3';
  ctx.textAlign = 'left';
  const outLabels = ['◀', '▶', 'THR', 'FIR', 'MEM'];
  CONFIG.nn.outputIds.forEach((id, i) => {
    const P = pos.get(id);
    if (P) ctx.fillText(outLabels[i], outX + 10, P.y + 3);
  });
  const groups = [
    [0, 'rays'], [9, 'vel'], [11, 'bias'], [12, 'thr'], [15, 'gun'],
    [16, 'lat'], [17, 't2'], [18, 'sz'], [19, 'prs'], [20, 'mem'],
  ];
  ctx.textAlign = 'right';
  for (const [id, label] of groups) {
    const P = pos.get(id);
    if (P) ctx.fillText(label, inX - 10, P.y + 3);
  }
  ctx.textAlign = 'left';
}

// Deep façade: one call per rAF owns canvas, HiDPI, stars, seamCopies, HUD, chart, net.
// Shallow functions above stay exported for verify / probes; this class is the
// concentrated interface. Deletion test: deleting it scatters HiDPI + throttling
// + starfield ownership back to every caller.
export class ArenaRenderer {
  constructor({ arena, hud, chart, net } = {}) {
    this.arena = arena ?? null;
    this.hud = hud ?? null;
    this.chart = chart ?? null;
    this.net = net ?? null;
    this.ctx = arena ? arena.getContext('2d') : null;
    this.chartCtx = chart ? chart.getContext('2d') : null;
    this.netCtx = net ? net.getContext('2d') : null;
    this.frameCount = 0;
    this.panelTimer = 0;
    this.layoutRaf = 0;
  }

  // Fitted HiDPI layout: arena backing = fitted CSS size * DPR (capped), chart/net fixed.
  applyHiDPIAndFit() {
    if (!this.arena || !this.chart || !this.net) return;
    const wrap = typeof document !== 'undefined' ? document.getElementById('arenaWrap') : null;
    if (!wrap) return;
    const scale = Math.min(
      wrap.clientWidth / CONFIG.arena.width,
      wrap.clientHeight / CONFIG.arena.height
    );
    const cssW = Math.floor(CONFIG.arena.width * scale);
    const cssH = Math.floor(CONFIG.arena.height * scale);
    setupHiDPI(this.arena, cssW, cssH, CONFIG.arena.width, CONFIG.arena.height);
    setupHiDPI(this.chart, CONFIG.render.chart.width, CONFIG.render.chart.height);
    setupHiDPI(this.net, CONFIG.render.network.width, CONFIG.render.network.height);
  }

  scheduleLayout() {
    if (this.layoutRaf) return;
    this.layoutRaf = requestAnimationFrame(() => {
      this.layoutRaf = 0;
      this.applyHiDPIAndFit();
    });
  }

  attachListeners() {
    this.applyHiDPIAndFit();
    if (typeof window !== 'undefined') {
      window.addEventListener('resize', () => this.scheduleLayout());
      if (window.matchMedia) {
        const watchDPR = () => {
          const mq = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
          const onChange = () => {
            this.scheduleLayout();
            if (mq.removeEventListener) mq.removeEventListener('change', onChange);
            else if (mq.removeListener) mq.removeListener(onChange);
            setTimeout(watchDPR, 0);
          };
          if (mq.addEventListener) mq.addEventListener('change', onChange);
          else if (mq.addListener) mq.addListener(onChange);
        };
        watchDPR();
      }
    }
  }

  // One call per rAF. Handles arena throttling (>16x every 4th frame) and HUD cadence.
  // opts: { world, pop, brainIdx, showRays, overlay, info, currentBrain, realDt, speed }
  frame(opts) {
    const { world, pop, brainIdx, showRays, overlay, info, currentBrain, realDt, speed } = opts;
    this.frameCount++;
    // Arena: throttle software rasterization at high speed.
    if (this.ctx && world) {
      if (speed <= 16 || this.frameCount % 4 === 0) {
        renderArena(this.ctx, world, 0, showRays);
        if (overlay) renderOverlay(this.ctx, overlay.text);
      }
    }
    this.panelTimer += realDt;
    if (this.panelTimer >= 1 / CONFIG.render.hudHz) {
      this.panelTimer = 0;
      if (this.hud && pop && world) renderHUD(this.hud, pop, world, brainIdx, info);
      if (this.chartCtx && pop) renderChart(this.chartCtx, pop);
      if (this.netCtx && currentBrain) renderNetwork(this.netCtx, currentBrain.genome, currentBrain.network);
    }
  }
}
