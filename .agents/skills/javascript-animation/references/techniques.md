# Drawing techniques

Techniques, not presets. Each one is a way of making marks; the palette, scale and combination come from the subject (see "Derive the look" below). All snippets assume the helpers in `templates/starter.html` (`ctx`, `W`, `H`, `rng`, `hash2`, `clamp`, `lerp`, `ink`, `fill`, `tracePath`, `J`, `circle`, `rect`, `arc`, `resample`).

**How tested each one is** (be honest with the user about this):

| Technique | Status |
|---|---|
| 1. Ink line (hand-drawn) | Used across two full films |
| 2. Illustration shapes (picture-book) | Used across a full film |
| 3. Spot-color print (riso) | Renders correctly as a still; not yet used in a full film |
| 4. Single-line engraving | Renders correctly as a still; not yet used in a full film |
| 5. Words as the shape | Renders correctly as a still; not yet used in a full film |
| 6. Marker / gouache fill | Renders correctly as a still; not yet used in a full film |
| 7. Cinematic flat + blueprint | Used across a full film (the "fix one line" example) |


## Derive the look

Before drawing, write down what the subject is made of and let that pick the marks:
- a newborn's-vision explainer: black-and-white cards, so ink on paper, one warm accent;
- a baby's day for the family: soft, picture-book shapes, the actual colors of the nursery;
- a typhoon explainer: wind and water, so words-as-shape spirals, blues;
- a print-shop or zine topic: spot-color layers.

Two different subjects should not come out looking alike. Don't carry a palette over from the example or an earlier piece.

## 1. Ink line (in the starter)

`ink(pts, { w, col, seed, amp, passes })` strokes a path through two-frequency noise; with `LOOK.wobble` set, edges wobble like a pen and the path is stroked twice (a full line and a thinner, fainter second pass). With `wobble: 0` it is one steady line. `fill`, `hatch` (parallel strokes clipped to a shape), `withShadow` (the same shape offset a few px in a translucent ink) and the paper texture complete the look. Set `LOOK.boil: 4` so the wobble changes ~7.5x per second.

Tuning: `amp` 1-2.5 px (higher is sketchier); two passes for pen, one for marker; hatch `gap` 6-12 px, alpha 0.2-0.4.

## 2. Illustration shapes (in the starter)

`shape(pts, col, { shadow, shade, tex, lw, line })`: base fill, a darker rim toward the bottom-right (fill dark, then the shape again nudged up-left), a few translucent lighter brush strokes, and an outline in a darker tint of the fill (`tint(col, .6)`), never flat black. Add `blush(x, y, rx, ry)` for soft radial cheeks or glows. Keep the ink wobble low (`amp` ~1) so it reads as illustration, not sketch.

## 3. Spot-color print (risograph)

Draw each ink color on its own layer, knock it slightly out of register, grain it, then multiply. Overlaps produce new colors for free.

```js
function spotLayers(layers, { offset = 4, grain = .18, seed = 1 } = {}) {
  // layers: [{ color, draw: () => { /* fill shapes with any color; only the alpha is used */ } }, ...]
  const r = rng(seed);
  for (const L of layers) {
    const c = document.createElement('canvas'); c.width = W; c.height = H;
    const main = ctx; ctx = c.getContext('2d'); L.draw(); ctx = main;          // shape mask
    const x = c.getContext('2d');
    x.globalCompositeOperation = 'source-in'; x.fillStyle = L.color; x.fillRect(0, 0, W, H); // tint
    x.globalCompositeOperation = 'destination-out';                               // ink grain
    for (let i = 0; i < W * H / 60; i++) { x.globalAlpha = r() * grain; x.fillRect(r() * W, r() * H, 2, 2); }
    ctx.save(); ctx.globalCompositeOperation = 'multiply';
    ctx.drawImage(c, (r() - .5) * offset * 2, (r() - .5) * offset * 2);           // misregistration
    ctx.restore();
  }
}
```

Usage: `spotLayers([{ color: '#ff4f8b', draw: () => fill(circle(200, 230, 140), '#000') }, { color: '#1f9e9a', draw: () => ... }], { seed: 4 })`. Two or three inks; any fill color inside `draw` works (only its alpha is used). It allocates a full-size canvas per layer per frame: fine for rendering, heavy for live preview at 60 fps.

## 4. Single-line engraving

One continuous spiral whose thickness follows a brightness field you write in code (0 = dark, 1 = light). The line width is capped by the gap between turns, or everything fills in solid.

```js
function spiralShade(cx, cy, R, brightness, { turns = 40, col = INK } = {}) {
  const maxW = R / turns * .95; // never thicker than the gap between turns, or it all fills in
  ctx.save(); ctx.strokeStyle = col; ctx.lineCap = 'round';
  let prev = null;
  for (let a = 0; a < turns * Math.PI * 2; a += .04) {
    const r = R * a / (turns * Math.PI * 2), x = cx + Math.cos(a) * r, y = cy + Math.sin(a) * r;
    if (prev) { ctx.lineWidth = .3 + maxW * (1 - clamp(brightness(x, y))); ctx.beginPath(); ctx.moveTo(...prev); ctx.lineTo(x, y); ctx.stroke(); }
    prev = [x, y];
  }
  ctx.restore();
}
```

Example field, a sphere lit from the upper left:
`(x, y) => { const dx = (x - cx) / R, dy = (y - cy) / R, r2 = dx * dx + dy * dy; if (r2 > 1) return .96; return clamp(.1 + .9 * Math.max(0, -.45 * dx - .55 * dy + .7 * Math.sqrt(1 - r2))); }`

## 5. Words as the shape

Fill a region with a repeated phrase so the words are the picture, or set a phrase along a path.

```js
function textFill(pts, phrase, { size = 18, col = INK, lead = 1.25, drift = 0 } = {}) {
  const xs = pts.map(p => p[0]), ys = pts.map(p => p[1]);
  ctx.save(); tracePath(pts, true); ctx.clip();
  ctx.font = `700 ${size}px ui-monospace, monospace`; ctx.fillStyle = col; ctx.textBaseline = 'top';
  const unit = phrase + '  ', uw = ctx.measureText(unit).width;
  for (let y = Math.min(...ys), row = 0; y < Math.max(...ys); y += size * lead, row++) {
    const x0 = Math.min(...xs) - uw + ((row * uw * .37 + drift * (row % 2 ? 1 : -1)) % uw + uw) % uw;
    for (let x = x0; x < Math.max(...xs); x += uw) ctx.fillText(unit, x, y);
  }
  ctx.restore();
}
// Or set a phrase along any path (a spiral, a wave, an orbit).
function textOnPath(pts, text, { size = 22, col = INK, start = 0 } = {}) {
  ctx.save(); ctx.font = `700 ${size}px ui-monospace, monospace`; ctx.fillStyle = col; ctx.textBaseline = 'middle';
  let d = start, i = 0, acc = 0;
  for (const ch of text) {
    const w = ctx.measureText(ch).width;
    while (i < pts.length - 1 && acc + Math.hypot(pts[i + 1][0] - pts[i][0], pts[i + 1][1] - pts[i][1]) < d) { acc += Math.hypot(pts[i + 1][0] - pts[i][0], pts[i + 1][1] - pts[i][1]); i++; }
    if (i >= pts.length - 1) break;
    const [x0, y0] = pts[i], [x1, y1] = pts[i + 1], seg = Math.hypot(x1 - x0, y1 - y0), k = (d - acc) / seg;
    ctx.save(); ctx.translate(lerp(x0, x1, k), lerp(y0, y1, k)); ctx.rotate(Math.atan2(y1 - y0, x1 - x0)); ctx.fillText(ch, 0, 0); ctx.restore();
    d += w;
  }
  ctx.restore();
}
```

Good for explainers: the label and the thing become one ("warm water" rows make the sea; "air spins inward" makes the spiral).

## 6. Marker / gouache fill

Overlapping broad strokes in one direction, each a slightly different tint, semi-opaque, rounded ends.

```js
function markerFill(pts, color, { angle = -.5, width = 26, seed = 3, alpha = .82, vary = 14 } = {}) {
  const r = rng(seed), xs = pts.map(p => p[0]), ys = pts.map(p => p[1]);
  const cx = (Math.min(...xs) + Math.max(...xs)) / 2, cy = (Math.min(...ys) + Math.max(...ys)) / 2;
  const R = Math.hypot(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys)) / 2;
  const base = [1, 3, 5].map(i => parseInt(color.substr(i, 2), 16));
  ctx.save(); tracePath(J(pts, seed, 2), true); ctx.clip();
  ctx.translate(cx, cy); ctx.rotate(angle); ctx.lineCap = 'round';
  for (let y = -R; y < R; y += width * .62) {
    const v = (r() - .5) * vary;
    ctx.strokeStyle = `rgb(${base.map(c => clamp(c + v, 0, 255) | 0).join(',')})`;
    ctx.globalAlpha = alpha; ctx.lineWidth = width * (.85 + r() * .3);
    ctx.beginPath(); ctx.moveTo(-R - 10, y + (r() - .5) * 6); ctx.lineTo(R + 10, y + (r() - .5) * 6); ctx.stroke();
  }
  ctx.restore();
}
```

## No house style

Pick the look from the subject and the audience; the techniques below are ingredients, not presets.

## 7. Cinematic flat + blueprint (in the starter)

`flatMass(pts, col)` fills a shape with a shadow side and no outline. `guide(pts, t0, t)` draws a dashed construction outline that fades as the real thing appears at `t0` (show structures being "drawn up" before they fill in). `glow(x, y, r, [r,g,b], a)` is a soft radial light. Keep line widths constant on screen under zoom by multiplying by `PX`. Grain comes from the paper texture multiplied over the frame.

## Depth without 3D (a road, a river, a corridor)

Put a horizon at `HZ` and a vanishing x. Give each moving thing a depth `d` in 0..1 that loops with time (`d = (k / N + t * speed) % 1`); its screen row is `y = HZ + (H - HZ) * d * d` and its scale `lerp(.08, 1.6, d * d)`. Stripes across the path at those rows rush toward the viewer; objects placed at the path edge (± half-width at that row) grow as they pass. The squared depth is what makes it read as perspective. Used for a stroller-eye view down a park path.

## Canvas gotchas

- **Build gradients after the transform they live in.** `createRadialGradient(x, y, …)` then `translate(x, y)` moves the gradient off by (x, y) again, and the fill silently shows nothing. Translate/scale first, then create the gradient at (0, 0).
- **`ctx.filter` blur and saturate** work in headless Chrome and in the render; they are the cheapest way to show "how someone else sees it" (see `drawLayer`).
- **Monospace fonts lack many glyphs** (`≈`, arrows, CJK) and swap them silently. Check the stills; use a font stack that has the characters.
- **`-x ** y` is a syntax error** in JS; write `-(x ** y)`.

## Motion building blocks (in the starter)

- `drawOn(pts, p)`: reveal a stroke from 0 to 1 (a line being drawn).
- `label(text, x, y, { p })`: a paper-tape tag that types on; its box grows with the typed text.
- `drawLayer(fn, filter, clip)`: draw into an offscreen layer and composite with a CSS filter (`blur()`, `saturate()`, `contrast()`) and optional clip, e.g. a before/after slider.
- Easing: `ease`, `easeOut`, `back` (overshoot pop), `prog(t, a, b)` for a 0-1 ramp between two times.
