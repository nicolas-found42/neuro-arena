// dev/smoke.mjs — direct puppeteer-core smoke for polished arcade-sketch build
// - launches Chrome headless:new, goto seed URL, collects pageErrors + CDN 200s + arena pixelAvg
import puppeteer from 'puppeteer-core';

const CDN_KEYS = ['roughjs','normalize','98.css','nes.css','hint','spinkit','open-props','tabler','monaspace'];
const URL = 'http://127.0.0.1:8899/index.html?seed=1409546675';

const browser = await puppeteer.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: 'new',
  args: ['--no-sandbox','--disable-setuid-sandbox','--disable-gpu']
});

const page = await browser.newPage();
const pageErrors = [];
const cdnStatuses = {};

page.on('pageerror', e => pageErrors.push(String(e?.message || e)));
page.on('response', r => {
  const u = r.url().toLowerCase();
  for (const k of CDN_KEYS) if (u.includes(k.toLowerCase())) cdnStatuses[k] = r.status();
});

await page.goto(URL, { waitUntil: 'networkidle2' });
await new Promise(r => setTimeout(r, 1100));

const pixelAvg = await page.evaluate(() => {
  const c = document.getElementById('arena');
  if (!c) return null;
  const ctx = c.getContext('2d');
  const { data } = ctx.getImageData(0, 0, c.width, c.height);
  let sum = 0;
  for (let i = 0; i < data.length; i += 4) sum += (data[i] + data[i+1] + data[i+2]) / 3;
  return sum / (data.length / 4);
});

const result = { url: URL, pageErrors, cdnStatuses, pixelAvg };
console.log(JSON.stringify(result, null, 2));
await browser.close();
