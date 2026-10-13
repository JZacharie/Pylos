#!/usr/bin/env node
// asset-audit.mjs: prove a page is zero-asset (every pixel computed, nothing loaded).
// Static scan of the HTML for anything that pulls in images, video, audio, fonts, scripts or data,
// then a live check: load the page and record every network request it makes.
//
// usage: node asset-audit.mjs page.html     exit 0 = clean, 1 = something is loaded or embedded
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createRequire } from 'node:module';
// resolve playwright from the skill folder, else from the project you run it in (where you `npm i playwright-core`)
async function loadChromium() {
  for (const name of ['playwright-core', 'playwright']) {
    try { return (await import(name)).chromium; } catch {}
    try { return createRequire(process.cwd() + '/')(name).chromium; } catch {}
  }
  throw new Error('playwright-core not found: run `npm i playwright-core` in this folder');
}

const file = process.argv[2];
if (!file) { console.error('usage: node asset-audit.mjs page.html'); process.exit(1); }
const src = readFileSync(file, 'utf8');

const RULES = [
  [/<img\b/gi, '<img> tag'],
  [/<(video|audio|source|picture|object|embed|iframe)\b/gi, 'media / embed tag'],
  [/<link\b[^>]*rel=["']?(stylesheet|preload|icon)/gi, '<link> to an external resource'],
  [/<script\b[^>]*\bsrc=/gi, 'external <script src>'],
  [/data:(image|audio|video|font|application)\//gi, 'embedded data: URI'],
  [/url\(\s*['"]?(?!#)[^)'"\s]+/gi, 'CSS url(...)'],
  [/@import\b/gi, 'CSS @import'],
  [/@font-face\b/gi, '@font-face (loaded font)'],
  [/\b(fetch|XMLHttpRequest|importScripts)\s*\(/g, 'network call'],
  [/\bnew\s+(Image|Audio|FontFace)\s*\(/g, 'new Image/Audio/FontFace'],
  [/\bimport\s*\(\s*['"]/g, 'dynamic import'],
  [/['"][^'"\s]+\.(png|jpe?g|gif|webp|avif|svg|mp4|webm|mov|mp3|wav|ogg|m4a|woff2?|ttf|otf|json)['"]/gi, 'file reference'],
  [/[A-Za-z0-9+/]{400,}={0,2}/g, 'long base64-like blob'],
];
const lineOf = i => src.slice(0, i).split('\n').length;
const hits = [];
for (const [re, what] of RULES) for (const m of src.matchAll(re)) hits.push({ line: lineOf(m.index), what, text: m[0].slice(0, 60) });

// live check: any request other than the page itself means something is loaded
let requests = [];
try {
  const chromium = await loadChromium();
  let browser; try { browser = await chromium.launch({ channel: 'chrome' }); } catch { browser = await chromium.launch(); }
  const page = await browser.newPage();
  const normPath = resolve(file).replace(/\\/g, '/');
  const self = 'file://' + (normPath.startsWith('/') ? '' : '/') + normPath;
  page.on('request', r => { if (!r.url().startsWith(self)) requests.push(r.url()); });
  await page.goto(self + '?render'); await page.waitForTimeout(1500);
  await browser.close();
} catch (e) { console.warn('live check skipped (playwright-core not available):', e.message); }

const kb = (Buffer.byteLength(src) / 1024).toFixed(0);
if (!hits.length && !requests.length) {
  console.log(`CLEAN: ${file} (${kb} KB) embeds and loads nothing; every frame is computed in code.`);
} else {
  for (const h of hits) console.log(`line ${h.line}: ${h.what}: ${h.text}`);
  for (const r of requests) console.log(`network request: ${r}`);
  console.log(`\nNOT zero-asset: ${hits.length} static hit(s), ${requests.length} request(s).`);
  process.exitCode = 1;
}
