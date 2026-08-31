// dev/smoke.mjs — direct puppeteer-core smoke for arcade-sketch verification gate
// Launch Chrome headless:new, goto seed URL, collect pageErrors + CDN 200* + CORS * + arena pixelAvg.
// Usage: node dev/smoke.mjs  (requires http://127.0.0.1:8899 and Chrome)
import puppeteer from 'puppeteer-core';
import { existsSync } from 'node:fs';

const CDN_KEYS = ['roughjs', 'normalize', '98.css', 'nes.css', 'hint', 'spinkit', 'open-props', 'tabler', 'monaspace'];
const URL = 'http://127.0.0.1:8899/index.html?seed=1409546675';
const CANDIDATES = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium-browser',
  '/usr/bin/chromium',
];
const executablePath = CANDIDATES.find((p) => existsSync(p)) ?? CANDIDATES[0];

const browser = await puppeteer.launch({
  executablePath,
  headless: 'new',
  args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
});

const page = await browser.newPage();
await page.setViewport({ width: 1280, height: 800, deviceScaleFactor: 1 });

const pageErrors = [];
const consoleErrors = [];
const cdnStatuses = {};
const cdnCors = {};

page.on('pageerror', (e) => pageErrors.push(String(e?.message ?? e)));
page.on('console', (m) => {
  if (m.type() === 'error') consoleErrors.push(m.text());
});
page.on('response', (r) => {
  const u = r.url().toLowerCase();
  for (const k of CDN_KEYS) if (u.includes(k.toLowerCase())) {
    cdnStatuses[k] = r.status();
    cdnCors[k] = r.headers()['access-control-allow-origin'] ?? null;
  }
});

await page.goto(URL, { waitUntil: 'networkidle2', timeout: 20000 });
await new Promise((r) => setTimeout(r, 1100));

const pixelAvg = await page.evaluate(() => {
  const c = document.getElementById('arena');
  if (!c) return null;
  const ctx = c.getContext('2d');
  const { data } = ctx.getImageData(0, 0, c.width, c.height);
  let s = 0;
  for (let i = 0; i < data.length; i += 4) s += (data[i] + data[i + 1] + data[i + 2]) / 3;
  return s / (data.length / 4);
});

// Element screenshots (~20 lines each) — reviewer can see CRT/glass/donut without launching browser locally
for (const sel of ['#arena', '#hud', '#chart', '#net']) {
  try {
    const el = await page.$(sel);
    if (el) await el.screenshot({ path: `/tmp/smoke_${sel.slice(1)}.png` });
  } catch {}
}

const result = { url: URL, pageErrors, consoleErrors, cdnStatuses, cdnCors, pixelAvg };
console.log(JSON.stringify(result, null, 2));

const missing = CDN_KEYS.filter((k) => cdnStatuses[k] !== 200);
// favicon 404 is the only expected console error; Chrome reports it as
// "Failed to load resource: the server responded with a status of 404 (File not found)"
// without the favicon URL in the console text, so allow any single 404.
const faviconOnly =
  consoleErrors.length === 0 ||
  (consoleErrors.length === 1 && /404|favicon/i.test(consoleErrors[0]));
const ok = pageErrors.length === 0 && missing.length === 0 && pixelAvg !== null && faviconOnly;

if (!ok) {
  console.error(`[smoke] FAIL missing CDN ${missing.join(',') || 'none'} pageErrors ${pageErrors.length} faviconOnly ${faviconOnly} pixelAvg ${pixelAvg}`);
}
await browser.close();
process.exit(ok ? 0 : 1);
