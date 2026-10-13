#!/usr/bin/env node
// layout-check.mjs: find text that collides with other text or straddles a key object.
//
// The page registers boxes while it draws each frame and exposes them as window.LAYOUT:
//   [{ id: 'chat bubble', kind: 'text' | 'keep', x, y, w, h }, ...]   (screen pixels)
// `claim(id, kind, x, y, w, h)` in the starter does this from the current transform.
// Flags, per sampled frame:
//   text vs text overlapping at all
//   text vs keep overlapping partially (text fully inside a keep, e.g. a caption on a screen, is fine)
// usage: node layout-check.mjs page.html [--every 0.1] [--min 4]      exit 1 if anything collides
import { resolve } from 'node:path';
import { createRequire } from 'node:module';
async function loadChromium() {
  for (const name of ['playwright-core', 'playwright']) {
    try { return (await import(name)).chromium; } catch {}
    try { return createRequire(process.cwd() + '/')(name).chromium; } catch {}
  }
  throw new Error('playwright-core not found: run `npm i playwright-core` in this folder');
}
const args = process.argv.slice(2), file = args[0];
const opt = (n, d) => { const i = args.indexOf(n); return i < 0 ? d : Number(args[i + 1]); };
if (!file) { console.error('usage: node layout-check.mjs page.html [--every 0.1] [--min 4]'); process.exit(1); }
const EVERY = opt('--every', .1), MIN = opt('--min', 4);          // ignore overlaps thinner than MIN px

const chromium = await loadChromium();
let browser; try { browser = await chromium.launch({ channel: 'chrome' }); } catch { browser = await chromium.launch(); }
const page = await browser.newPage();
await page.goto('file://' + resolve(file) + '?render');
const { FRAMES, FPS } = await page.evaluate(() => ({ FRAMES: window.FRAMES, FPS: window.FPS }));
const frames = []; for (let t = 0; t * FPS < FRAMES; t += EVERY) frames.push(Math.round(t * FPS));
const layouts = await page.evaluate(fs => fs.map(f => { window.draw(f); return window.LAYOUT || null; }), frames);
await browser.close();
if (layouts.every(l => l === null)) { console.error('the page exposes no window.LAYOUT: call claim(...) for text and key objects while drawing'); process.exit(1); }

const inter = (a, b) => [Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)), Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y))];
const inside = (a, b) => a.x >= b.x - 1 && a.y >= b.y - 1 && a.x + a.w <= b.x + b.w + 1 && a.y + a.h <= b.y + b.h + 1;
const onScreen = a => a.x + a.w > 0 && a.y + a.h > 0 && a.x < 1e5 && a.w > 0 && a.h > 0;
const hits = new Map();                                              // "a × b" -> [times]
layouts.forEach((L, i) => {
  if (!L) return; const boxes = L.filter(onScreen);
  for (let p = 0; p < boxes.length; p++) for (let q = p + 1; q < boxes.length; q++) {
    const a = boxes[p], b = boxes[q]; if (a.kind !== 'text' && b.kind !== 'text') continue;
    const [ox, oy] = inter(a, b); if (ox < MIN || oy < MIN) continue;
    if (a.kind === 'text' && b.kind === 'keep' && inside(a, b)) continue;
    if (b.kind === 'text' && a.kind === 'keep' && inside(b, a)) continue;
    const key = `${a.id} × ${b.id}`; if (!hits.has(key)) hits.set(key, []); hits.get(key).push(frames[i] / FPS);
  }
});
if (!hits.size) { console.log(`CLEAN: no text collisions in ${frames.length} sampled frames (every ${EVERY}s).`); process.exit(0); }
const spans = ts => { const out = []; for (const t of ts) { const last = out[out.length - 1]; if (last && t - last[1] <= EVERY * 1.5) last[1] = t; else out.push([t, t]); } return out.map(([a, b]) => a === b ? `${a.toFixed(1)}s` : `${a.toFixed(1)}-${b.toFixed(1)}s`).join(', '); };
for (const [k, ts] of hits) console.log(`${k}: ${spans(ts)}`);
console.log(`\n${hits.size} collision(s). Move, resize or time-shift the text; render stills at those times to confirm.`);
process.exitCode = 1;
