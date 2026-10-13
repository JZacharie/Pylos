// score.js: a code-synthesized soundtrack that lives INSIDE the animation page (Web Audio API).
// Paste it into the page's <script>, then set:
//     window.SCORE = ac => buildScore(ac, { dur: DUR, cues: window.CUES });
// render.mjs renders it offline (OfflineAudioContext -> WAV) and muxes it into the MP4;
// the page's live preview plays the same score on click. No samples, no libraries.
//
// The music FOLLOWS THE PICTURE: a tempo map gives every scene a whole number of bars, so each cut
// lands on a downbeat; each scene walks home to the dominant (G) so the cut arrives on the tonic (C).
// The edit is marked by harmony, not volume. A scene only one bar long is just the dominant (G) leading
// into the next cut; for many short scenes, lower `bar` so each gets two or more bars. Options: bar (target bar length s, ~90 bpm = 2.67),
// key (semitone transpose), tail (seconds of final resolving chord), gain (master level), chime (a faint bell on each cut).
function buildScore(ac, { dur, cues = [], bar = 2.67, key = 0, tail = 4, gain = .8, chime = false } = {}) {
  const T0 = ac.currentTime + (ac instanceof OfflineAudioContext ? 0 : .05), TAU = Math.PI * 2; // live: 50ms scheduling headroom
  const hz = m => 440 * Math.pow(2, (m + key - 69) / 12);
  const CUES = cues.filter(c => c > 0 && c < dur).sort((a, b) => a - b);
  const TAIL = Math.min(tail, dur / 3);

  // master chain: dry + generated-impulse reverb -> compressor -> fades -> out
  const master = ac.createGain(), comp = ac.createDynamicsCompressor(), wet = ac.createGain(), rev = ac.createConvolver();
  comp.threshold.value = -14; comp.ratio.value = 3; wet.gain.value = .32;
  const irLen = Math.round(ac.sampleRate * 2.2), ir = ac.createBuffer(2, irLen, ac.sampleRate);
  for (let ch = 0; ch < 2; ch++) { // deterministic noise tail (seeded, so every render is identical)
    const d = ir.getChannelData(ch); let s = 1234 + ch * 777;
    for (let i = 0; i < irLen; i++) { s = (s * 1103515245 + 12345) & 0x7fffffff; d[i] = (s / 0x7fffffff * 2 - 1) * Math.pow(1 - i / irLen, 3); }
  }
  rev.buffer = ir;
  const bus = ac.createGain(); bus.connect(comp); bus.connect(rev); rev.connect(wet); wet.connect(comp);
  comp.connect(master); master.connect(ac.destination);
  master.gain.setValueAtTime(0, T0); master.gain.linearRampToValueAtTime(gain, T0 + .3);
  master.gain.setValueAtTime(gain, T0 + dur - 1.5); master.gain.linearRampToValueAtTime(0, T0 + dur);

  // one partial: sine (or other wave) with its own attack/decay envelope
  function partial(t, f, amp, attack, decay, pan = 0, type = 'sine', len) {
    const o = ac.createOscillator(), g = ac.createGain(), p = ac.createStereoPanner();
    o.type = type; o.frequency.value = f; p.pan.value = pan;
    const s = T0 + t, end = s + (len || decay * 6);
    g.gain.setValueAtTime(0, s); g.gain.linearRampToValueAtTime(amp, s + attack);
    if (len) { g.gain.setValueAtTime(amp, end - decay); g.gain.linearRampToValueAtTime(0, end); }
    else g.gain.setTargetAtTime(0, s + attack, decay);
    o.connect(g); g.connect(p); p.connect(bus); o.start(s); o.stop(end + .05);
  }
  const musicBox = (t, f, a, pan) => { partial(t, f, a, .004, .9, pan); partial(t, 2 * f, a * .35, .004, .35, pan); partial(t, 4.03 * f, a * .12, .004, .12, pan); };
  const pad = (t, f, a, len) => { partial(t, f, a * .4, .9, .9, -.2, 'sine', len); partial(t, f * 1.004, a * .4, .9, .9, .2, 'sine', len); partial(t, f * 2.002, a * .2, .9, .9, 0, 'sine', len); };
  const bass = (t, f, a) => { partial(t, f, a, .01, 1.4); partial(t, 2 * f, a * .2, .01, 1.4); };
  const softChime = (t, f, a, pan) => partial(t, f, a, .02, 1.4, pan); // 20ms attack: never startles

  const CH = { C: [60, 64, 67], Am: [57, 60, 64], F: [53, 57, 60], G: [55, 59, 62], Dm: [50, 53, 57] };
  const progression = bars => {
    if (bars === 1) return ['G'];
    const cycle = ['C', 'Am', 'F', 'G', 'C', 'Dm', 'F', 'G'];
    const p = Array.from({ length: bars }, (_, i) => cycle[i % cycle.length]);
    p[bars - 1] = 'G'; if (bars >= 3 && p[bars - 2] === 'G') p[bars - 2] = 'F';
    return p;
  };
  const bounds = [0, ...CUES, dur - TAIL].filter((v, i, a) => i === 0 || v > a[i - 1] + .5);
  const PATTERN = [0, 1, 2, 1, 3, 2, 1, 2]; // 8 eighth notes; 3 = root an octave up
  for (let s = 0; s < bounds.length - 1; s++) {
    const a = bounds[s], b = bounds[s + 1], bars = Math.max(1, Math.round((b - a) / bar)), BAR = (b - a) / bars;
    progression(bars).forEach((name, bi) => {
      const t0 = a + bi * BAR, ch = CH[name], onCut = s > 0 && bi === 0; // the cut is marked by the chord change alone
      pad(t0, hz(ch[0] - 12), .05, BAR + .9);
      pad(t0, hz(ch[2] - 12), .035, BAR + .9);
      bass(t0, hz(ch[0] - 24), onCut ? .1 : .14);
      PATTERN.forEach((k, i) => {
        if (s === 0 && bi === 0 && i % 2 && bars > 1) return; // sparse opening bar (only if more bars follow)
        musicBox(t0 + i * BAR / 8, hz((k === 3 ? ch[0] + 12 : ch[k]) + 12), .085 * (i === 0 && !onCut ? 1.15 : 1), i % 2 ? .35 : -.35);
      });
    });
  }
  if (chime) CUES.forEach((c, i) => softChime(c, hz(88 + [0, 7, 4, 9, 12][i % 5]), .025, i % 2 ? .5 : -.5));   // optional sparkle on cuts
  const end = bounds[bounds.length - 1]; // ending: resolve on C; the roll starts exactly on the boundary
  bass(end, hz(48), .14); pad(end, hz(48), .06, TAIL);
  [60, 64, 67, 72].forEach((m, i) => musicBox(end + i * .08, hz(m + 12), .075, (i - 1.5) * .25));
  return bounds.map((b, i) => i && 240 / ((b - bounds[i - 1]) / Math.max(1, Math.round((b - bounds[i - 1]) / bar)))).slice(1); // bpm per scene, for sanity checks
}
