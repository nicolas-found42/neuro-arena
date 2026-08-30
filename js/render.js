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

// Static starfield, precomputed once. Enriched to 3-layer parallax twinkle:
// each star carries depth z 0.3–1.5 and a twinkle phase so the same 120-star
// budget gains motion without per-frame allocation or texture thrash.
const stars = Array.from({ length: CONFIG.arena.starCount }, () => ({
  x: Math.random() * W,
  y: Math.random() * H,
  r: 0.5 + Math.random() * 1.2,
  z: 0.3 + Math.random() * 1.2,
  phase: Math.random() * Math.PI * 2,
}));

// ── Offscreen dithered nebula at 1/4 res — one drawImage per frame, globalAlpha 0.12 ──
// Sin-noise plasma via Math.sin LUT into ImageData (MDN createImageData/putImageData)
// derived from effectgames/CanvasCycle palette rotation article + phoboslab/plasma.js
// inspiration (MIT). anttihirvonen/demoscene-starter-kits is no LICENSE (license:null,
// no plasma.c at root, processing/*.pde + common/ only) — discovery inspiration only.
// Rendered once offscreen, composited with drawImage. GC-safe, respects Seam Copies/setupHiDPI.
let _nebulaCanvas = null;
let _palettePhase = 0;
function _ensureNebula() {
  if (_nebulaCanvas) return _nebulaCanvas;
  if (typeof document === 'undefined') return null;
  const nw = Math.floor(W / 4);
  const nh = Math.floor(H / 4);
  const c = document.createElement('canvas');
  c.width = nw;
  c.height = nh;
  const nctx = c.getContext('2d');
  if (!nctx) return null;
  const id = nctx.createImageData(nw, nh);
  // Bayer 4×4 dither matrix for 1-bit dither feel
  const bayer = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
  for (let y = 0; y < nh; y++) {
    for (let x = 0; x < nw; x++) {
      const i = (y * nw + x) * 4;
      // sin LUT plasma 30-line pattern: sum of sines
      const v = Math.sin(x * 0.08) + Math.sin(y * 0.05) + Math.sin((x + y) * 0.04) + Math.sin(Math.hypot(x - nw / 2, y - nh / 2) * 0.06);
      const t = (v + 4) / 8;
      const b = bayer[((y & 3) * 4) + (x & 3)] / 16;
      const d = Math.max(0, Math.min(1, t + (b - 0.5) * 0.12));
      // Lospec-like palette: deep teal → muted purple → warm grey (copy-paste hex, 0KB)
      const r = Math.floor(11 + d * 55);
      const g = Math.floor(14 + d * 28);
      const bch = Math.floor(20 + d * 65);
      id.data[i] = r;
      id.data[i + 1] = g;
      id.data[i + 2] = bch;
      id.data[i + 3] = Math.floor(22 + d * 38);
    }
  }
  nctx.putImageData(id, 0, 0);
  const grad = nctx.createRadialGradient(nw * 0.22, nh * 0.68, 0, nw * 0.22, nh * 0.68, nw * 0.95);
  grad.addColorStop(0, 'rgba(70,18,85,0.22)');
  grad.addColorStop(1, 'rgba(11,14,20,0)');
  nctx.fillStyle = grad;
  nctx.fillRect(0, 0, nw, nh);
  _nebulaCanvas = c;
  return c;
}
function _drawNebula(ctx) {
  const neb = _ensureNebula();
  if (!neb) return;
  // palette cycling: subtle hue rotation over time (0KB pattern, no per-pixel cost)
  const t = typeof performance !== 'undefined' ? performance.now() * 0.001 : 0;
  _palettePhase = (t * 6) % 360;
  ctx.save();
  ctx.globalAlpha = 0.12;
  // use filter for palette rotation if available (composes with lighter, no allocation)
  const prevFilter = ctx.filter;
  try { ctx.filter = `hue-rotate(${_palettePhase * 0.25}deg) saturate(1.15)`; } catch {}
  ctx.drawImage(neb, 0, 0, W, H);
  try { ctx.filter = prevFilter || 'none'; } catch {}
  ctx.restore();
}
// Exposed for probes only via facade — not a public seam; palette cycling helper
function _cyclePalette(dt) {
  _palettePhase = (_palettePhase + dt * 6) % 360;
}

// ── Pooled FX: debris, shock rings, screen shake ──
// Module-private pooled arrays spliced on life<=0 — GC-safe at 10,000×,
// lighter+shadowBlur for additive bloom, single drawImage/composite wrappers.
const _particles = [];
const _rings = [];
let _shakeT = 0;
let _shakeAmp = 0;
let _prevAstCount = null;
let _prevAlive = null;
function _emitDebris(x, y, n = 9) {
  for (let i = 0; i < n; i++) {
    const ang = (i / n) * Math.PI * 2 + Math.random() * 0.5;
    const sp = 18 + Math.random() * 86;
    _particles.push({
      x, y,
      vx: Math.cos(ang) * sp,
      vy: Math.sin(ang) * sp,
      life: 0.42 + Math.random() * 0.18,
      r: 1.1 + Math.random() * 1.8,
      color: i % 2 ? 'rgba(255,180,80,0.95)' : 'rgba(160,210,255,0.85)',
    });
  }
}
function _emitRing(x, y) {
  for (let k = 0; k < 2; k++) {
    _rings.push({ x: x + (Math.random() - 0.5) * 6, y: y + (Math.random() - 0.5) * 6, life: 0.35 - k * 0.08, maxLife: 0.35 - k * 0.08 });
  }
}
function _triggerShake() {
  _shakeT = 0;
  _shakeAmp = 6;
}
function _updateFx(dt, world) {
  // detect shatter / death behind the facade
  if (world) {
    const curCount = world.asteroids.length;
    if (_prevAstCount !== null && curCount < _prevAstCount) {
      const ax = world.asteroids[0] ? world.asteroids[0].x : W / 2;
      const ay = world.asteroids[0] ? world.asteroids[0].y : H / 2;
      const cx = (Math.random() * W * 0.5 + W * 0.25);
      const cy = (Math.random() * H * 0.5 + H * 0.25);
      // emit near a random current asteroid or center if field cleared
      const ex = curCount ? ax : cx;
      const ey = curCount ? ay : cy;
      _emitDebris(ex, ey, 10);
      _emitRing(ex, ey);
      _triggerShake();
    }
    _prevAstCount = curCount;
    const curAlive = world.agents && world.agents[0] ? !!world.agents[0].alive : null;
    if (_prevAlive === true && curAlive === false) {
      const a = world.agents[0];
      _emitDebris(a.x, a.y, 14);
      _emitRing(a.x, a.y);
      _triggerShake();
    }
    if (curAlive !== null) _prevAlive = curAlive;
  }
  // advance shake clock 6*exp(-t/0.12)
  if (_shakeAmp > 0) {
    _shakeT += dt;
    const amp = _shakeAmp * Math.exp(-_shakeT / 0.12);
    if (amp < 0.08) { _shakeAmp = 0; _shakeT = 0; }
  }
  for (let i = _particles.length - 1; i >= 0; i--) {
    const p = _particles[i];
    p.life -= dt;
    if (p.life <= 0) { _particles.splice(i, 1); continue; }
    p.x += p.vx * dt;
    p.y += p.vy * dt;
    p.vx *= 0.985;
    p.vy *= 0.985;
    // toroidal wrap for debris without allocating
    if (p.x < 0) p.x += W; else if (p.x >= W) p.x -= W;
    if (p.y < 0) p.y += H; else if (p.y >= H) p.y -= H;
  }
  for (let i = _rings.length - 1; i >= 0; i--) {
    const r = _rings[i];
    r.life -= dt;
    if (r.life <= 0) _rings.splice(i, 1);
  }
  _cyclePalette(dt);
}
function _shakeOffset() {
  if (_shakeAmp <= 0) return null;
  const amp = _shakeAmp * Math.exp(-_shakeT / 0.12);
  if (amp < 0.08) return null;
  return { x: (Math.random() - 0.5) * amp * 2, y: (Math.random() - 0.5) * amp * 2 };
}
function _drawParticles(ctx) {
  if (!_particles.length) return;
  ctx.save();
  ctx.globalCompositeOperation = 'lighter';
  ctx.shadowBlur = 8;
  ctx.shadowColor = 'rgba(255,170,60,0.85)';
  for (const p of _particles) {
    const a = Math.max(0, p.life / 0.6);
    ctx.globalAlpha = a;
    ctx.fillStyle = p.color;
    // seam copies for pooled debris so shatter reads across edge
    for (const [dx, dy] of seamCopies(p.x, p.y, p.r + 2)) {
      ctx.beginPath();
      ctx.arc(p.x + dx, p.y + dy, p.r, 0, Math.PI * 2);
      ctx.fill();
    }
  }
  ctx.restore();
}
function _drawRings(ctx) {
  if (!_rings.length) return;
  ctx.save();
  for (const r of _rings) {
    const prog = 1 - r.life / r.maxLife;
    const rad = prog * 38;
    const alpha = (1 - prog) * 0.55;
    ctx.globalAlpha = alpha;
    ctx.strokeStyle = prog < 0.5 ? 'rgba(90,220,255,0.9)' : 'rgba(255,190,90,0.75)';
    ctx.lineWidth = prog < 0.35 ? 2 : 1.25;
    const copies = seamCopies(r.x, r.y, rad + 2);
    for (const [dx, dy] of copies) {
      ctx.beginPath();
      ctx.arc(r.x + dx, r.y + dy, rad, 0, Math.PI * 2);
      ctx.stroke();
    }
    // second arc 1–2 arcs signal per spec
    if (prog > 0.25) {
      ctx.globalAlpha = alpha * 0.45;
      ctx.lineWidth = 1;
      for (const [dx, dy] of copies) {
        ctx.beginPath();
        ctx.arc(r.x + dx, r.y + dy, rad * 0.62, 0, Math.PI * 2);
        ctx.stroke();
      }
    }
  }
  ctx.restore();
}

// HiDPI setup: backing store = cssPixels * DPR (capped), style = cssPixels,
// context transform maps logical coords to device pixels.
// For arena: cssW = 960*scale, logicalW = 960 → transform = DPR*scale (fitted).
// For chart/net: cssW === logicalW → transform = DPR.
// Headless (no window) falls back to DPR=1 and becomes a no-op for determinism checks.
export function setupHiDPI(canvas, cssW, cssH, logicalW = cssW, logicalH = cssH) {
  const rawDpr = typeof window !== 'undefined' && window.devicePixelRatio ? window.devicePixelRatio : 1;
  const cap = CONFIG.render.dprCap ?? 2;
  const dpr = Math.min(rawDpr, cap);
  const w = Math.max(1, Math.floor(cssW * dpr));
  const h = Math.max(1, Math.floor(cssH * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  canvas.style.width = cssW + 'px';
  canvas.style.height = cssH + 'px';
  const ctx = canvas.getContext('2d');
  const sx = dpr * (cssW / logicalW);
  const sy = dpr * (cssH / logicalH);
  if (Number.isFinite(sx) && Number.isFinite(sy)) ctx.setTransform(sx, 0, 0, sy, 0, 0);
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
  // Private FX tick: GC-safe, single-frame pooling, respects 10,000× budget.
  _updateFx(CONFIG.dt, world);
  const shake = _shakeOffset();
  let didShake = false;
  if (shake) {
    ctx.save();
    ctx.translate(shake.x, shake.y);
    didShake = true;
  }

  ctx.fillStyle = CONFIG.arena.background;
  ctx.fillRect(0, 0, W, H);

  // Dithered nebula offscreen at 1/4 res composited globalAlpha 0.12 (one drawImage per frame)
  _drawNebula(ctx);

  // Arena inset highlight per prototype Target1 arcade-sketch: subtle bevel frame
  ctx.strokeStyle = 'rgba(255,255,255,0.06)';
  ctx.lineWidth = 1;
  ctx.strokeRect(0.5, 0.5, W - 1, H - 1);
  ctx.strokeStyle = 'rgba(0,0,0,0.22)';
  ctx.strokeRect(1.5, 1.5, W - 3, H - 3);

  // 3-layer parallax twinkle starfield: z 0.3–1.5 drift + sine twinkle, no allocation
  {
    const tSec = typeof performance !== 'undefined' && performance.now ? performance.now() * 0.001 : 0;
    for (const s of stars) {
      const tw = 0.62 + 0.38 * Math.sin(s.phase + tSec * (0.7 + s.z * 0.42));
      const alpha = CONFIG.arena.starAlpha * tw * (0.55 + s.z * 0.32);
      ctx.fillStyle = `rgba(255,255,255,${alpha.toFixed(3)})`;
      const driftX = (tSec * 8 * s.z) % W;
      const driftY = (tSec * 2.2 * s.z) % H;
      const x = (s.x + driftX) % W;
      const y = (s.y + driftY) % H;
      ctx.beginPath();
      ctx.arc(x, y, s.r * (0.75 + s.z * 0.45), 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // Asteroids — Rough.js ESM mandatory, no vanilla fallback.
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
    // Cache crater arcs deterministically per asteroid (2–3 arcs inside hachure)
    if (!a._craters) {
      const s0 = Math.floor(a.shape[0] * 100000) >>> 0;
      const s1 = Math.floor(a.shape[2 % a.shape.length] * 100000) >>> 0;
      const s2 = Math.floor(a.shape[4 % a.shape.length] * 100000) >>> 0;
      const base = (s0 ^ (s1 << 5) ^ (s2 << 11)) >>> 0;
      const nCraters = 2 + (base & 1);
      const cr = [];
      for (let k = 0; k < nCraters; k++) {
        const bits = (base >>> (k * 6)) & 0xff;
        const ang = (bits / 255) * Math.PI * 2;
        const radFrac = 0.32 + ((base >>> (k * 3 + 1)) & 0x7) / 18;
        const rr = a.r * 0.18 * (0.78 + ((base >>> (k * 4)) & 0x3) / 7);
        cr.push({ ang, radFrac, rr, highlight: k % 2 === 0 });
      }
      a._craters = cr;
    }
  }
  for (const a of world.asteroids) {
    const d = a._roughDrawable;
    const craters = a._craters;
    for (const [dx, dy] of seamCopies(a.x, a.y, a.r)) {
      ctx.save();
      ctx.translate(a.x + dx, a.y + dy);
      ctx.rotate(a.angle);
      rc.draw(d);
      // Cratered material over hachure: inner arcs with highlight/shadow
      for (const c of craters) {
        const cx = Math.cos(c.ang) * a.r * c.radFrac;
        const cy = Math.sin(c.ang) * a.r * c.radFrac;
        ctx.fillStyle = c.highlight ? 'rgba(255,255,255,0.09)' : 'rgba(0,0,0,0.32)';
        ctx.beginPath();
        ctx.arc(cx, cy, c.rr, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = c.highlight ? 'rgba(255,255,255,0.07)' : 'rgba(0,0,0,0.24)';
        ctx.lineWidth = 0.85;
        ctx.beginPath();
        ctx.arc(cx, cy, c.rr, 0, Math.PI * 2);
        ctx.stroke();
        // second smaller rim for depth
        ctx.fillStyle = c.highlight ? 'rgba(255,255,255,0.045)' : 'rgba(0,0,0,0.16)';
        ctx.beginPath();
        ctx.arc(cx + c.rr * 0.22, cy - c.rr * 0.18, c.rr * 0.42, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.restore();
    }
  }

  // Bullets: only the ship under evaluation — lighter bloom + chromatic fringe
  const leader = shipIdx >= 0 ? world.agents[shipIdx] : null;
  if (leader) {
    // bullet bloom pass: lighter composite, shadowBlur
    if (world.bullets.length) {
      ctx.save();
      ctx.globalCompositeOperation = 'lighter';
      ctx.shadowBlur = 8;
      ctx.shadowColor = 'rgba(120,200,255,0.75)';
      ctx.fillStyle = 'rgba(255,255,255,0.42)';
      for (const b of world.bullets) {
        if (b.owner !== leader) continue;
        for (const [dx, dy] of seamCopies(b.x, b.y, CONFIG.bullet.radius)) {
          ctx.beginPath();
          ctx.arc(b.x + dx, b.y + dy, CONFIG.bullet.radius, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      ctx.restore();
      // chromatic fringe for bullets: triple shadow at ±1 px offsets
      ctx.save();
      ctx.globalCompositeOperation = 'lighter';
      ctx.globalAlpha = 0.16;
      ctx.fillStyle = 'rgba(255,60,120,0.9)';
      for (const b of world.bullets) {
        if (b.owner !== leader) continue;
        for (const [dx, dy] of seamCopies(b.x, b.y, CONFIG.bullet.radius)) {
          ctx.beginPath();
          ctx.arc(b.x + dx + 1, b.y + dy, CONFIG.bullet.radius * 0.92, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      ctx.fillStyle = 'rgba(60,255,255,0.9)';
      for (const b of world.bullets) {
        if (b.owner !== leader) continue;
        for (const [dx, dy] of seamCopies(b.x, b.y, CONFIG.bullet.radius)) {
          ctx.beginPath();
          ctx.arc(b.x + dx - 1, b.y + dy, CONFIG.bullet.radius * 0.92, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      ctx.restore();
    }
  }

  // Vision rays + ship: drawn once per seam copy of the ship
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
        // Radial-gradient engine flame with shadowBlur flicker — P3 pattern
        const flick = 0.86 + Math.sin((typeof performance !== 'undefined' ? performance.now() : 0) * 0.022 + sx * 0.009) * 0.14;
        const grad = ctx.createRadialGradient(-11, 0, 0.5, -13, 0, 12);
        grad.addColorStop(0, `rgba(255,247,160,${(0.96 * flick).toFixed(3)})`);
        grad.addColorStop(0.32, `rgba(255,140,43,${(0.88 * flick).toFixed(3)})`);
        grad.addColorStop(0.66, `rgba(255,70,20,${(0.55 * flick).toFixed(3)})`);
        grad.addColorStop(1, 'rgba(255,40,0,0)');
        ctx.shadowBlur = 11 * flick;
        ctx.shadowColor = 'rgba(255,110,30,0.85)';
        ctx.fillStyle = grad;
        ctx.beginPath();
        ctx.moveTo(-7, -4);
        ctx.lineTo(-16, 0);
        ctx.lineTo(-7, 4);
        ctx.closePath();
        ctx.fill();
        ctx.shadowBlur = 0;
      }
      // Ship hull with lighter bloom
      ctx.save();
      ctx.globalCompositeOperation = 'lighter';
      ctx.shadowBlur = 8;
      ctx.shadowColor = 'rgba(57,208,255,0.56)';
      ctx.fillStyle = '#ffffff';
      ctx.beginPath();
      ctx.moveTo(12, 0);
      ctx.lineTo(-8, -7);
      ctx.lineTo(-8, 7);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
      // Chromatic aberration triple-shadow fringe for ship: +1 / -1 offsets in lighter
      ctx.save();
      ctx.globalCompositeOperation = 'lighter';
      ctx.globalAlpha = 0.18;
      ctx.fillStyle = '#f08';
      ctx.beginPath();
      ctx.moveTo(13, 0);
      ctx.lineTo(-7, -7);
      ctx.lineTo(-7, 7);
      ctx.closePath();
      ctx.fill();
      ctx.fillStyle = '#0ff';
      ctx.beginPath();
      ctx.moveTo(11, 0);
      ctx.lineTo(-9, -7);
      ctx.lineTo(-9, 7);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
      // crisp white on top after bloom
      ctx.fillStyle = '#ffffff';
      ctx.beginPath();
      ctx.moveTo(12, 0);
      ctx.lineTo(-8, -7);
      ctx.lineTo(-8, 7);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
      // Ship inset highlight — plain 1px white circle arc (arcade-sketch)
      ctx.strokeStyle = '#ffffff';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.arc(sx, sy, 14, 0, Math.PI * 2);
      ctx.stroke();
    }
  }

  // Pooled explosion debris under lighter+shadowBlur + shock rings 1–2 arcs 0.35s decay
  _drawParticles(ctx);
  _drawRings(ctx);

  if (didShake) ctx.restore();
}

export function renderOverlay(ctx, text) {
  // 1-bit Bayer dither on pause overlay (P11) — subtle blueprint feel without WebGL
  ctx.fillStyle = 'rgba(5,8,15,0.78)';
  ctx.fillRect(0, H / 2 - 44, W, 88);
  try {
    const id = ctx.getImageData(0, H / 2 - 44, W, 88);
    const bayer = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    for (let y = 0; y < 88; y++) {
      for (let x = 0; x < W; x++) {
        const i = (y * W + x) * 4;
        const thr = bayer[((y & 3) * 4) + (x & 3)] / 16;
        if (id.data[i + 3] > 20 && ((id.data[i] + id.data[i + 1] + id.data[i + 2]) / 3) < thr * 255 * 0.6) {
          id.data[i + 3] = Math.floor(id.data[i + 3] * 0.72);
        }
      }
    }
    ctx.putImageData(id, 0, H / 2 - 44);
  } catch {}
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
  const pct = h ? Math.min(1, Math.max(0, (h.best - (h.avg * 0.72)) / (Math.max(1, h.best) * 1.05))) : 0;
  const deg = Math.round(pct * 360);
  // conic-gradient fitness donut + tabular-nums + MONO density — glass handled in CSS
  el.innerHTML =
    `<span class="hud-donut" style="--pct:${deg}deg" aria-hidden="true"></span>` +
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
    const inConns = new Map();
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
    const cols = new Map();
    let maxD = 1;
    for (const id of hiddens) {
      const d = Math.max(1, depth(id));
      maxD = Math.max(maxD, d);
      let l = cols.get(d);
      if (!l) cols.set(d, (l = []));
      l.push(id);
    }
    for (const [d, ids] of cols) {
      const x =
        maxD <= 1
          ? hiddenXMin
          : hiddenXMin + ((d - 1) / (maxD - 1)) * (hiddenXMax - hiddenXMin);
      spread(ids, x, 30, nh - 30);
    }
  }

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
  frame(opts) {
    const { world, pop, brainIdx, showRays, overlay, info, currentBrain, realDt, speed } = opts;
    this.frameCount++;
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
