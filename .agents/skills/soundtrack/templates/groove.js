// groove.js: a beat-grid score with sections, a break, and a synthesized sound for every action.
// Paste into the page's <script> (before the live-preview block), then:
//   window.SCORE = ac => buildGroove(ac, { dur: DUR, bpm: BPM, sections: SECTIONS, events: EVENTS });
// The picture reads the same BPM / SECTIONS / EVENTS, so sound and image can't drift apart.
//
// sections: [[fromBar, toBar, layers], ...]  layers from:
//   'pad' 'padSoft' 'kick2' 'kick4' 'hat8' 'hat16' 'clap' 'bassHalf' 'bass8' 'lead' 'pluck' 'crash'
//   A section with [] layers is a break: silence (the SFX still play).
// events:   [[seconds, sfxName, arg?], ...]  sfxName from the FX table below.
// kit:      the instrument set, chosen per piece: 'electro' | 'acoustic' | 'keys' | 'percussion'
// harmony:  a mood from HARMONY below, or your own chords (one per bar, MIDI notes); key: semitone transpose.
// Choose kit, harmony, key and bpm for THIS piece; don't reuse the last piece's choices.
// sfxGain: level of the sound effects against the music (lower it if effects drown the downbeats).
// Layering rule learned the hard way: bring layers in one or two per bar. Kick + bass entering
// together after a quiet intro jumped +11 dB and read as a startle; staggered, it was +3.5 dB.
const HARMONY = {
  bright:  [[60, 64, 67], [65, 69, 72], [67, 71, 74], [60, 64, 67]],          // I IV V I: plain, sunny
  wistful: [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]],          // vi IV I V: pop, bittersweet
  dreamy:  [[60, 64, 67, 71], [62, 66, 69, 72], [60, 64, 67, 71], [55, 59, 62, 66]], // lydian maj7 colour: floating, wonder
  tense:   [[57, 60, 64], [53, 57, 60], [52, 56, 59], [57, 60, 64]],          // harmonic minor, i VI V i: suspense, mystery
  folk:    [[62, 66, 69], [67, 71, 74], [62, 66, 69], [57, 61, 64]],          // I IV I V in D: warm, open
  blues:   [[60, 64, 67, 70], [65, 69, 72, 75], [60, 64, 67, 70], [67, 71, 74, 77]], // dominant 7ths: cheeky, swaggering
};
function buildGroove(ac, { dur, bpm = 120, sections, events = [], kit = 'electro', harmony = 'wistful', key = 0, gain = .9, sfxGain = .9 } = {}) {
  const chords = (Array.isArray(harmony) ? harmony : HARMONY[harmony]).map(c => c.map(m => m + key));
  const T0 = ac.currentTime + (ac instanceof OfflineAudioContext ? 0 : .05), BEAT = 60 / bpm;
  const hz = m => 440 * Math.pow(2, (m - 69) / 12);
  const out = ac.createDynamicsCompressor(); out.threshold.value = -12; out.ratio.value = 3; out.connect(ac.destination);
  const master = ac.createGain(); master.gain.value = gain; master.connect(out);
  const M = ac.createGain(), X = ac.createGain(); M.connect(master); X.connect(master); X.gain.value = sfxGain;
  const rev = ac.createConvolver(), wet = ac.createGain(); wet.gain.value = .22; rev.connect(wet); wet.connect(master); M.connect(rev); X.connect(rev);
  { const n = Math.round(ac.sampleRate * 1.6), ir = ac.createBuffer(2, n, ac.sampleRate);          // seeded room: identical every render
    for (let c = 0; c < 2; c++) { const d = ir.getChannelData(c); let s = 99 + c; for (let i = 0; i < n; i++) { s = (s * 1103515245 + 12345) & 0x7fffffff; d[i] = (s / 0x7fffffff * 2 - 1) * Math.pow(1 - i / n, 3); } } rev.buffer = ir; }
  const noise = (() => { const n = ac.sampleRate, buf = ac.createBuffer(1, n, n), d = buf.getChannelData(0); let s = 7; for (let i = 0; i < n; i++) { s = (s * 1103515245 + 12345) & 0x7fffffff; d[i] = s / 0x7fffffff * 2 - 1; } return buf; })();
  const env = (g, t, a, peak, decay) => { g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(peak, t + a); g.gain.setTargetAtTime(0, t + a, decay); };
  function osc(bus, t, type, f, peak, a, decay, f2, glide = .05, pan = 0) {
    const o = ac.createOscillator(), g = ac.createGain(), p = ac.createStereoPanner(); o.type = type; o.frequency.setValueAtTime(f, t);
    if (f2) o.frequency.exponentialRampToValueAtTime(f2, t + glide); p.pan.value = pan; env(g, t, a, peak, decay); o.connect(g); g.connect(p); p.connect(bus); o.start(t); o.stop(t + a + decay * 7);
  }
  function hiss(bus, t, type, f, q, peak, a, decay, f2, glide = .1, pan = 0) {
    const s = ac.createBufferSource(), fl = ac.createBiquadFilter(), g = ac.createGain(), p = ac.createStereoPanner(); s.buffer = noise;
    fl.type = type; fl.frequency.setValueAtTime(f, t); if (f2) fl.frequency.exponentialRampToValueAtTime(f2, t + glide); fl.Q.value = q; p.pan.value = pan;
    env(g, t, a, peak, decay); s.connect(fl); fl.connect(g); g.connect(p); p.connect(bus); s.start(t, (t * 7.3) % .5); s.stop(t + a + decay * 7);
  }
  // instruments, per kit (same roles, different voices)
  const soft = { electro: 1, acoustic: .7, keys: .45, percussion: .8 }[kit];
  const kick = (t, a = .9) => kit === 'percussion' ? osc(M, t, 'sine', 130, a * .6, .002, .14, 90, .12)                       // low tom
    : osc(M, t, 'sine', kit === 'electro' ? 150 : 110, a * soft, .002, kit === 'electro' ? .12 : .09, kit === 'electro' ? 42 : 55, .1);
  const clap = (t, v = 1) => kit === 'electro' ? (hiss(M, t, 'bandpass', 1500, .9, .5 * v, .002, .06), hiss(M, t + .012, 'bandpass', 1700, .9, .35 * v, .002, .08))
    : kit === 'acoustic' ? (hiss(M, t, 'bandpass', 1100, 3, .35 * v, .001, .02), osc(M, t, 'triangle', 420, .15 * v, .001, .03))              // rimshot
    : kit === 'keys' ? hiss(M, t, 'lowpass', 3000, .7, .18 * v, .01, .12)                                                            // brush
    : (osc(M, t, 'sine', 900, .3 * v, .001, .03), osc(M, t, 'sine', 1400, .15 * v, .001, .02));                                     // woodblock
  const hat = (t, a = .12) => kit === 'electro' ? hiss(M, t, 'highpass', 7500, .7, a, .001, .025, null, 0, .3)
    : hiss(M, t, 'bandpass', 6000, 1.2, a * (kit === 'keys' ? .4 : .7), .01, .045, null, 0, .3);                                    // shaker
  const crash = t => hiss(M, t, 'highpass', 5000, .5, .35 * soft, .002, .6);
  const bass = (t, m, len) => { const o = ac.createOscillator(), f = ac.createBiquadFilter(), g = ac.createGain();
    o.type = kit === 'electro' ? 'sawtooth' : kit === 'acoustic' ? 'triangle' : 'sine'; o.frequency.value = hz(m);
    f.type = 'lowpass'; f.frequency.value = kit === 'electro' ? 520 : 900; const pk = kit === 'percussion' ? .3 : .22;
    g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(pk, t + .01); g.gain.setTargetAtTime(0, t + (kit === 'percussion' ? .05 : len * .7), kit === 'percussion' ? .08 : .05);
    o.connect(f); f.connect(g); g.connect(M); o.start(t); o.stop(t + len + .4); };
  const pluck = (t, m, a = .12, pan = 0) => {
    if (kit === 'keys') { osc(M, t, 'sine', hz(m), a * 1.1, .004, .5, null, 0, pan); osc(M, t, 'sine', hz(m) * 2.01, a * .35, .002, .18, null, 0, pan); osc(M, t, 'sine', hz(m) * 3.98, a * .12, .001, .06, null, 0, pan); return; }   // electric piano
    if (kit === 'percussion') { osc(M, t, 'sine', hz(m), a * 1.2, .001, .12, null, 0, pan); osc(M, t, 'sine', hz(m) * 4, a * .3, .001, .03, null, 0, pan); return; }                                  // marimba
    osc(M, t, 'triangle', hz(m), a, .003, kit === 'acoustic' ? .32 : .18, null, 0, pan); osc(M, t, 'sine', hz(m + 12), a * .3, .003, .08, null, 0, pan); };                                             // pluck / nylon
  const pad = (t, notes, len, a = .045) => notes.forEach((m, i) => { const o = ac.createOscillator(), g = ac.createGain(), f = ac.createBiquadFilter();
    o.type = kit === 'keys' ? 'sawtooth' : 'sine'; o.frequency.value = hz(m); o.detune.value = (i - 1) * (kit === 'keys' ? 9 : 6);           // keys: a string section
    f.type = 'lowpass'; f.frequency.value = kit === 'keys' ? 1400 : 8000; const pk = kit === 'keys' ? a * .5 : a, at = kit === 'keys' ? .7 : .4;
    g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(pk, t + at); g.gain.setValueAtTime(pk, t + len - .3); g.gain.linearRampToValueAtTime(0, t + len + .2);
    o.connect(f); f.connect(g); g.connect(M); o.start(t); o.stop(t + len + .3); });
  const bars = Math.ceil(dur / (4 * BEAT)); let prevL = [];
  for (let bar = 0; bar < bars; bar++) {
    const sec = sections.find(([a, e]) => bar >= a && bar < e); if (!sec) { prevL = []; continue; }
    const L = sec[2], t0 = T0 + bar * 4 * BEAT, ch = chords[bar % chords.length], first = bar === sec[0];
    // a drum layer's first bar comes in at half strength, so it arrives instead of jumping out
    const fresh = n => L.includes(n) && !prevL.includes(n) && bar > 0, kv = (fresh('kick4') || fresh('kick2')) && !prevL.some(x => x.startsWith('kick')) ? .5 : 1, cv = fresh('clap') ? .6 : 1;
    if (L.includes('pad')) pad(t0, ch.map(m => m + 12), 4 * BEAT); if (L.includes('padSoft')) pad(t0, ch.map(m => m + 12), 4 * BEAT, .035);
    if (L.includes('crash') && first) crash(t0);
    for (let s = 0; s < 16; s++) { const t = t0 + s * BEAT / 4, on = s % 4 === 0, beat = s / 4;
      if (L.includes('kick4') && on) kick(t, .9 * kv); if (L.includes('kick2') && (s === 0 || s === 8)) kick(t, .6 * kv);
      if (L.includes('clap') && (s === 4 || s === 12)) clap(t, cv);
      if (L.includes('hat8') && s % 2 === 0 && !on) hat(t); if (L.includes('hat16') && !on) hat(t, s % 2 ? .07 : .12);
      if (L.includes('bassHalf') && (s === 0 || s === 8)) bass(t, ch[0] - 24, 2 * BEAT);
      if (L.includes('bass8') && s % 2 === 0) bass(t, ch[0] - 24 + (s % 8 === 6 ? 12 : 0), BEAT / 2);
      if (L.includes('lead')) pluck(t, [ch[0], ch[1], ch[2], ch[1] + 12][s % 4] + 12, .07, s % 2 ? .3 : -.3);
      if (L.includes('pluck') && on) pluck(t, [ch[2], ch[1], ch[0], ch[1]][beat] + 12, .1);
    }
    prevL = L;
  }
  // sound effects: one per visible action
  const FX = {
    click: t => hiss(X, t, 'highpass', 3000, .7, .09, .001, .012), clack: t => { hiss(X, t, 'bandpass', 2200, 1, .25, .001, .03); osc(X, t, 'sine', 120, .25, .002, .05); },
    switch: t => hiss(X, t, 'bandpass', 3000, 2, .2, .001, .015),
    whooshIn: t => hiss(X, t, 'bandpass', 300, 1.2, .3, .35, .25, 3500, .45), whooshOut: t => hiss(X, t, 'bandpass', 3500, 1.2, .25, .3, .25, 300, .45),
    whoosh: t => hiss(X, t, 'bandpass', 500, 1, .22, .15, .15, 2500, .3),
    boing: t => { osc(X, t, 'sine', 520, .3, .005, .18, 180, .25); osc(X, t + .02, 'sine', 260, .15, .005, .12); },
    clank: t => { osc(X, t, 'triangle', 880, .22, .001, .06); osc(X, t, 'triangle', 1318, .12, .001, .05); hiss(X, t, 'bandpass', 4000, 3, .15, .001, .02); },
    ding: t => { osc(X, t, 'sine', 1568, .22, .002, .35); osc(X, t, 'sine', 2349, .08, .002, .2); },
    pop: t => osc(X, t, 'sine', 500, .28, .002, .06, 900, .05), popLow: (t, i = 0) => osc(X, t, 'sine', 220 + i * 40, .3, .002, .08, 420 + i * 60, .06, i % 2 ? .4 : -.4),
    tick: t => { hiss(X, t, 'bandpass', 2600, 4, .22, .001, .02); osc(X, t, 'triangle', 620, .12, .001, .03, 300, .04); },
    swish: t => hiss(X, t, 'bandpass', 1200, 2, .18, .03, .08, 2500, .12), thud: t => { osc(X, t, 'sine', 110, .5, .002, .12, 40, .12); hiss(X, t, 'lowpass', 400, 1, .2, .002, .06); },
    clonk: t => { osc(X, t, 'triangle', 330, .25, .001, .07); osc(X, t, 'triangle', 495, .12, .001, .05); },
    creak: t => osc(X, t, 'sawtooth', 90, .05, .02, .12, 70, .2),
    blip: (t, i = 0) => osc(X, t, 'square', 880 + i * 110, .06, .002, .04, null, 0, (i - 2.5) * .15),
    zip: (t, i = 0) => osc(X, t, 'sawtooth', 200 + i * 45, .06, .004, .08, 700 + i * 90, .12, i % 2 ? .3 : -.3),
    flag: t => { hiss(X, t, 'bandpass', 1800, 1.5, .2, .01, .06); osc(X, t + .05, 'sine', 1319, .2, .002, .3); osc(X, t + .12, 'sine', 1760, .2, .002, .4); },
    alarm: t => { osc(X, t, 'square', 1760, .07, .002, .05); osc(X, t + .12, 'square', 1760, .07, .002, .05); },
    step: t => { osc(X, t, 'sine', 140, .12, .002, .04, 90, .04); hiss(X, t, 'lowpass', 800, 1, .05, .002, .02); },
    notify: t => { osc(X, t, 'sine', 1319, .25, .002, .25); osc(X, t + .1, 'sine', 1976, .22, .002, .35); },
    drip: t => osc(X, t, 'sine', 1400, .18, .002, .08, 700, .08),
  };
  for (const [t, name, arg] of events) { if (!FX[name]) throw new Error(`groove: unknown sfx "${name}"`); FX[name](T0 + t, arg); }
  master.gain.setValueAtTime(gain, T0 + dur - 1); master.gain.linearRampToValueAtTime(0, T0 + dur);
}
