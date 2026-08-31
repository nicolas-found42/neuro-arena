// dev/cdp_inspect.mjs — direct CDP fallback for the browser verification gate
// Tries puppeteer.connect({browserURL:'http://127.0.0.1:9222'}) first; if that harness
// is unavailable, launches Chrome with --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-inspect
// then reconnects. Same probes as dev/smoke.mjs plus element screenshots.
// Mirrors the chrome-devtools MCP protocol (new_page/list_console_messages/list_network_requests/evaluate_script/take_screenshot).
// Usage: node dev/cdp_inspect.mjs   (or: chrome --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-inspect & node dev/cdp_inspect.mjs)
import puppeteer from 'puppeteer-core';
import { existsSync } from 'node:fs';

const URL = 'http://127.0.0.1:8899/index.html?seed=1409546675';
const CDN_KEYS = ['roughjs', 'normalize', '98.css', 'nes.css', 'hint', 'spinkit', 'open-props', 'tabler', 'monaspace'];
const CANDIDATES = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium-browser',
  '/usr/bin/chromium',
];
const executablePath = CANDIDATES.find((p) => existsSync(p)) ?? CANDIDATES[0];

let browser;
let launched = false;
try {
  browser = await puppeteer.connect({ browserURL: 'http://127.0.0.1:9222' });
  console.log('[cdp] connected to existing http://127.0.0.1:9222');
} catch {
  browser = await puppeteer.launch({
    executablePath,
    headless: 'new',
    args: [
      '--remote-debugging-port=9222',
      '--user-data-dir=/tmp/chrome-inspect',
      '--no-sandbox',
      '--disable-setuid-sandbox',
      '--disable-gpu',
      '--disable-dev-shm-usage',
    ],
  });
  launched = true;
  console.log('[cdp] launched Chrome with --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-inspect');
  try {
    const probe = await puppeteer.connect({ browserURL: 'http://127.0.0.1:9222' });
    await probe.disconnect();
    console.log('[cdp] puppeteer.connect verified');
  } catch {}
}

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
await new Promise((r) => setTimeout(r, 1300));

const pixelAvg = await page.evaluate(() => {
  const c = document.getElementById('arena');
  if (!c) return null;
  const ctx = c.getContext('2d');
  const { data } = ctx.getImageData(0, 0, c.width, c.height);
  let s = 0;
  for (let i = 0; i < data.length; i += 4) s += (data[i] + data[i + 1] + data[i + 2]) / 3;
  return s / (data.length / 4);
});

// Element screenshots for #arena/#hud/#chart/#net (~20 lines each)
const shots = {};
for (const sel of ['#arena', '#hud', '#chart', '#net']) {
  try {
    const el = await page.$(sel);
    if (el) {
      const path = `/tmp/cdp_${sel.slice(1)}.png`;
      await el.screenshot({ path });
      shots[sel] = path;
    }
  } catch (e) {
    shots[sel] = `error: ${e?.message ?? e}`;
  }
}

const result = { url: URL, pageErrors, consoleErrors, cdnStatuses, cdnCors, pixelAvg, shots, mode: launched ? 'launched:9222' : 'connected:9222' };
console.log(JSON.stringify(result, null, 2));

const missing = CDN_KEYS.filter((k) => cdnStatuses[k] !== 200);
const faviconOnly =
  consoleErrors.length === 0 ||
  (consoleErrors.length === 1 && /404|favicon/i.test(consoleErrors[0]));
const ok = pageErrors.length === 0 && missing.length === 0 && pixelAvg !== null && faviconOnly;

if (!ok) console.error(`[cdp] FAIL missing ${missing.join(',') || 'none'} pageErrors ${pageErrors.length} faviconOnly ${faviconOnly} pixelAvg ${pixelAvg}`);

// Keep remote Chrome alive when we only connected; close when we launched
if (launched) await browser.close();
else await browser.disconnect();

process.exit(ok ? 0 : 1);
