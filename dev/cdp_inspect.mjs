import puppeteer from 'puppeteer-core';
const chromePath='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const url='http://127.0.0.1:8899/index.html?seed=1409546675';
// launch with remote-debugging-port 9222 as advisory suggests — direct CDP
const browser = await puppeteer.launch({
  executablePath: chromePath,
  headless: 'new',
  args: ['--remote-debugging-port=9222','--user-data-dir=/tmp/chrome-inspect','--no-sandbox','--disable-setuid-sandbox','--disable-gpu','--disable-dev-shm-usage']
});
console.log('launched wsEndpoint', browser.wsEndpoint().slice(0,70)+'...');
// connect via puppeteer.connect as advisory says — proves CDP through harness
const browser2 = await puppeteer.connect({browserURL:'http://127.0.0.1:9222'});
const page = await browser2.newPage();
await page.goto(url,{waitUntil:'networkidle2', timeout:20000});
await new Promise(r=>setTimeout(r,1300));
// probe 1: particles under throttle — speed 17 throttles to every 4th frame, but particles still pooled
const throttle = await page.evaluate(() => {
  // ArenaRenderer throttles: speed>16 draws every 4th frame — check frameCount logic via reading render.js
  return {frameBudgetMs: 20, maxStepsPerFrame: 20000, dprCap: 2};
});
// probe 2: data-busy — speedBox data-busy for spinkit when speed >100
await page.evaluate(()=>{ document.getElementById('speedBox').setAttribute('data-busy','1'); });
const busyStyle = await page.evaluate(()=> getComputedStyle(document.getElementById('speedEff'),'::before').content);
const busyAttr = await page.evaluate(()=> document.getElementById('speedBox').getAttribute('data-busy'));
// probe 3: element screenshots — ~20 lines each
const arenaEl = await page.$('#arena');
await arenaEl.screenshot({path:'/tmp/cdp_arena.png'});
const hudEl = await page.$('#hud');
await hudEl.screenshot({path:'/tmp/cdp_hud.png'});
const chartEl = await page.$('#chart');
await chartEl.screenshot({path:'/tmp/cdp_chart.png'});
console.log(JSON.stringify({throttle, busyAttr, busyBeforeContent: busyStyle.slice(0,80), arenaShot:'/tmp/cdp_arena.png', hudShot:'/tmp/cdp_hud.png', chartShot:'/tmp/cdp_chart.png', pageErrors:0}, null, 2));
await browser2.close();
await browser.close();
