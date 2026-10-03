// JS <-> Rust PARITY TEST
// ------------------------
// Proves the JS web engine and the shared Rust core (loom-core -> WASM) agree on
// the TIMING/onset of every voice, across the language features ported to Rust:
// generators, structure (@section / in), poly, jux, off, microtonal scales, pan,
// swing, groove, nudge, degrees, roman chords, modulation (key@section), ratchets,
// alternation, subsequences.
//
// It also proves the OFFLINE TEACHER ANALYSER agrees line-for-line: the web's
// `analyzePiece` (the long-tested JS fallback) vs the Rust `analyze_json` now
// shipped as the default — tonality, tuning, acoustics, harmony+cadence, form,
// the "why" and the novelty advice. Two fields are number-normalised (the float
// Plomp–Levelt consonance %, and the novelty score) and, on pieces with a
// `key@section` modulation, the novelty section is compared structurally-only:
// Rust fingerprints the RESOLVED notes (it hears the modulation), JS the abstract
// step tokens. Every other character is checked exactly.
//
// It loads the real JS engine from web/index.html (capture mode via
// window.__loom.scheduleBar / analyzePiece) and the Rust engine from the nodejs
// WASM build.
//
// Prereqs:  ./build-wasm.sh        (builds web/pkg AND tools/pkg-node)
// Run:      node tools/parity_test.js
//
// Compares DISTINCT onset times per voice per bar (chord-note multiplicity and
// the intentional JS-only randomness — degrade/humanize/feel — are out of scope;
// those are covered by loom-core's own unit tests).

const fs = require('fs');
const path = require('path');
const ROOT = path.join(__dirname, '..');

// --- load the Rust engine (nodejs WASM build) ---
const wasmPath = path.join(__dirname, 'pkg-node', 'loom_wasm.js');
if (!fs.existsSync(wasmPath)) {
  console.error('FAIL: ' + wasmPath + ' missing. Run ./build-wasm.sh first.');
  process.exit(1);
}
const W = require(wasmPath);

// --- stub DOM + Web Audio, then load the JS engine IIFE from web/index.html ---
const html = fs.readFileSync(path.join(ROOT, 'web', 'index.html'), 'utf8');
let code = null;
for (const block of html.match(/<script>\n([\s\S]*?)<\/script>/g) || []) {
  const inner = block.replace(/^<script>\n?/, '').replace(/<\/script>$/, '');
  if (inner.includes('initAudio')) { code = inner; break; }
}
if (!code) { console.error('FAIL: could not extract engine script'); process.exit(1); }

const noop = () => {};
const param = () => ({ value: 0, setValueAtTime: noop, linearRampToValueAtTime: noop,
  exponentialRampToValueAtTime: noop, setTargetAtTime: noop, cancelScheduledValues: noop });
const anode = () => ({ connect: () => anode(), disconnect: noop, start: noop, stop: noop,
  gain: param(), frequency: param(), detune: param(), Q: param(), delayTime: param(), playbackRate: param(),
  type: '', curve: null, oversample: '', buffer: null, getChannelData: () => new Float32Array(16) });
const ctxObj = { _t: 0, get currentTime() { return this._t; }, sampleRate: 44100, state: 'running',
  resume: noop, destination: anode(), createGain: anode, createOscillator: anode,
  createBiquadFilter: anode, createDelay: anode, createConvolver: anode, createWaveShaper: anode,
  createBufferSource: anode, createScriptProcessor: () => ({ ...anode(), onaudioprocess: null }),
  createAnalyser: () => ({ ...anode(), fftSize: 2048, frequencyBinCount: 1024, getByteTimeDomainData: noop, getByteFrequencyData: noop }),
  createDynamicsCompressor: () => ({ ...anode(), threshold: param(), knee: param(), ratio: param(), attack: param(), release: param() }),
  createStereoPanner: () => ({ ...anode(), pan: param() }),
  createBuffer: (ch, len) => ({ getChannelData: () => new Float32Array(len || 16) }),
  decodeAudioData: () => Promise.resolve({}) };
function AudioContext() { return ctxObj; }
function OfflineAudioContext(ch, length, rate) { return Object.assign({}, ctxObj, { length, sampleRate: rate || 44100,
  startRendering: () => Promise.resolve({ numberOfChannels: 2, length: length || 16, getChannelData: () => new Float32Array(length || 16) }) }); }
const URL = { createObjectURL: () => 'blob:stub', revokeObjectURL: noop };
if (typeof globalThis.Blob === 'undefined') globalThis.Blob = class { constructor(){} };

const els = {};
const makeEl = () => { const o = { value: '', textContent: '', innerHTML: '', className: '', dataset: {},
  style: { setProperty: noop }, classList: { add: noop, remove: noop, toggle: noop, contains: () => false },
  _h: {}, addEventListener: (e, fn) => { o._h[e] = fn; }, appendChild: noop, removeChild: noop, remove: noop,
  click: noop, select: noop, open: false, disabled: false, querySelectorAll: () => [], children: [], focus: noop }; return o; };
const getEl = id => (els[id] || (els[id] = makeEl()));
getEl('bpm').value = '120'; getEl('vol').value = '60'; getEl('swing').value = '0';
getEl('echo').value = '0'; getEl('space').value = '0';
const document = { getElementById: getEl, createElement: makeEl, querySelectorAll: () => [],
  documentElement: { style: { setProperty: noop } }, body: makeEl(), addEventListener: noop };
const window = { AudioContext, OfflineAudioContext };
const localStorage = { getItem: () => null, setItem: noop };
const navigator = { requestMIDIAccess: () => Promise.resolve({ outputs: new Map() }) };
const performance = { now: () => 0 };
const requestAnimationFrame = noop;
let timeouts = 0;
const setTimeout = (fn) => { if (timeouts++ > 4) return 0; return timeouts; };
const fetch = () => Promise.resolve({ arrayBuffer: () => Promise.resolve(new ArrayBuffer(8)) });

new Function('document', 'window', 'localStorage', 'requestAnimationFrame', 'setTimeout',
  'AudioContext', 'OfflineAudioContext', 'fetch', 'navigator', 'performance', 'URL', '"use strict";' + code)(
  document, window, localStorage, requestAnimationFrame, setTimeout, AudioContext, OfflineAudioContext, fetch, navigator, performance, URL);

const L = window.__loom;
if (!L || !L.scheduleBar) { console.error('FAIL: window.__loom.scheduleBar absent (old engine?)'); process.exit(1); }

// --- the comparison ---
const r3 = x => Math.round(x * 1000) / 1000;
const onsets = arr => { const m = {}; for (const e of arr) (m[e.voice] || (m[e.voice] = new Set())).add(r3(e.phase));
  const o = {}; for (const v in m) o[v] = [...m[v]].sort((a, b) => a - b); return o; };

const pieces = {
  'generators+structure': 'seed: 7\nbpm: 120\nsong: a*2 b*2\nkick: e(3,8)\nhat: ca(110,16)\nlead: walk(8) | in a\nmel@b: drunk(16)',
  'poly+jux+off+microtonal': "seed: 3\nkey: c 0 1.5 3 5 6.5 8 10\nfil: 1 4 6 3 5 1' | poly 13\nm: 1 2 3 | jux rev | off 2",
  'meter composé 2+2+3/8': 'seed: 4\nmeter: 2+2+3/8\nlead: 1 3 5 1\' 7 5 3 4\nkick: x . . x . . x .\nhat: x x x x x x x x',
  'poly m:n (triolet 3:2) sous métrique composée': 'seed: 4\nmeter: 2+2+3/8\nlead: 1 3 5 1\' 7 5 3 | poly 3:2\nkick: x . . x . . x| poly 2:3\nhat: x x x x x x x',
  'poly m:n (3:2) en 4/4': "seed: 4\nlead: 1 3 5 1' 7 5 3 | poly 3:2\nkick: x . . x| poly 2:3\nhat: x x x x x x",
  'swing+groove+nudge': 'groove: mpc\nh: x x x x x x x x | swing 50\nb: x x x x | shift 0.2',
  'degrees+roman+modulation': 'key: a minor\nkey@chorus: c minor\nsong: verse*2 chorus*2\nlead: 1 3 5 1\' 7 5 3 2\npad: I . IV . V . VI .',
  'ratchets+alternation+subseq': 'seed: 9\nm: 1 [2 3] <4/5> 1*3\nk: x*2 x x <x/.>',
  'degrade+chance (seeded drops)': 'seed: 11\nh: x x x x x x x x x x x x x x x x | degrade 40\np: 1 3 5 1\' | chance 0.6\nk: x x x x | degrade 25',
  'develop (motivic arc)': 'seed: 1\nkey: c major\nlead: 1 2 3 5 | develop\nhh: x . x x . x . x | develop',
  'markov (ordre 1, ordre-N, phrase apprise)': 'seed: 3\nkey: c major\nm1: 3 5 7 | markov\nm2: 3 5 7 | markov learn: 1 2 1 3 1 2 4\nm3: 3 5 7 1\' 5 | markov 2 learn: 1 2 1 3 1 2 4 1 2 5\nm4: 1 2 3 5 | markov 2',
  'contraintes solve (arc + règles)': 'seed: 3\nkey: c major\nm1: solve(12,target:1,ladder:2,range:1-5)\nm2: solve(16,target:5,ladder:3)\nm3: solve(16,target:1,ladder:2,range:1-7,arc)\nm4: solve(8,arc)',
  'rythmes du monde': 'kick: tresillo\nhat: bembe\nclap: son\nperc: baiao',
  'sieve (Xenakis)': 'kick: sieve(16,3:0,5:2)\nhat: sieve(13,2:0,3:1)\nperc: sieve(12,4:1,6:3)',
  '1/f pink': 'seed: 5\nkey: c minor\nmel: pink(16)\nmel2: 1f(13)',
  'contours arc/rise/fall': 'key: c major\nm1: arc(8)\nm2: rise(6)\nm3: fall(10)',
  'commentaires en ligne': 'key: c minor\nsong: a*2 b*2 # forme\nlead: 1 2 3 5 | saw # une ligne\nkick: x . x . # beat\nbass: 1 7# 1 . # garde le 7#',
  'gendy (Xenakis)': 'seed: 5\nkey: c minor\nm: gendy(16)\nm2: gendy(11)',
  'spectrum (Grisey)': 'm: spectrum(12,c1)\nm2: spectrum(8,e2) | bell',
  'tonnetz (neo-Riemann)': 'h: tonnetz(8,C,PLR) | saw\nh2: tonnetz(6,Am,LR) | sine',
  'hocket (tiling canon)': 'a: x . . x . . x . | hocket\nb: e(5,16) | hocket\nc: tresillo | hocket',
  'bus routing (no-op onsets)': 'room: fdn 0.85 4200\nbus: drums gain 0.8 lp 1500 reverb 0.3\nkick: x . x . | bus drums\nsnare: . . x . | bus drums\nlead: 1 3 5 1\' | saw | bus synths',
  'send returns (no-op onsets)': 'send: tails echo 4 0.5\nsend: airy reverb hall 0.85 4500\nsend: wash flange 0.4 0.6\nkick: x . x . | send tails 0.6\nsnare: . . x . | send tails 0.4\nlead: 1 3 5 1\' | send airy 0.7\npad: 1 5 3 7 | send wash 0.3 | send airy 0.2',
  'l-sys custom': "seed: 5\nlead: l-sys(16,1>1.2,2>1.3,5>3.1) | pluck | gain 0.1\nkick: x . . x . . x .",
  'xys xenakis (markov seedé)': 'seed: 3\nlead: xys(16,1,1>2:2|3:1,2>1:1,5>1.5:2,3>2:1) | sine | gain 0.1\nkick: e(3,8)',
  'bytebeat (aucune note, le reste joue)': "kick: x . x .\ncrunch: t: (t*5&t>>7)|(t*3&t>>10) | gain 0.4\nlead: 1 3 5 | saw",
  'evolve (génétique seedée)': 'seed: 3\nlead: evolve(16) | pluck | gain 0.1\nkick: x . . x . . x .',
};

// Verbes Tidal (ply, linger, segment, range, loopAt). Ces pièces passent dans la
// parité d'onsets ci-dessous ET dans une parité plus stricte (onset + jeton joué),
// car range ne change que les hauteurs.
const tidal_pieces = {
  'tidal ply': "key: c minor\nkick: x . x . | ply 2\nhat: x x . x | ply 3\nlead: 1 [3 5] . 1'! | ply 2\nm: 1 2 3 4 | every 2 ply 4",
  'tidal linger': "key: d dorian\nlead: 1 2 3 4 5 6 7 1' | linger 0.25\nm2: 1 2 3 4 5 6 7 1' | linger 0.75\nm3: 1 2 3 4 5 6 7 1' | linger -0.25\nhat: x . x x . x x . | linger 0.5\nm4: 1 2 3 | linger 0.1",
  'tidal segment': "key: c major\nlead: 1 3 5 | segment 6\nm2: 1 2 3 4 5 | segment 3\nm3: 1 . 5 | segment 8\nk: x . x | segment 4\nm4: walk(8) | segment 5",
  'tidal range': "seed: 4\nkey: a minor\nlead: 1 2 3 4 5 | range 1 3\nm2: 1 . 3! c4 5? | range 1 1''\nm3: 1 3 5 | range 8 1\nm4: 4 4 | range 2, 5\nm5: walk(8) | range 3 6",
  'tidal combinés + loopAt': "seed: 2\nkey: e minor\nlead: 1 3 5 7 | range 1 5' | ply 2 | linger 0.5\nm2: 5 3 1 | segment 4 | rev\nbreak: x . . . | sample amen | loopAt 2",
};
Object.assign(pieces, tidal_pieces);

// ── ANALYSER PARITY (offline teacher analysis) ─────────────────────────────
// The Rust analyser (loom-core::prof, now the web's default via analyze_json)
// must agree line-for-line with the JS analyzePiece (the long-tested fallback):
// tonality, tuning, acoustics, harmony+cadence, the "why", form, novelty.
const an_pieces = {
  'knowbase complète (majeur, pyth, ii-V-I, génératif, euclid)': 'key: c major\nprog: Dm G7 C\ntune: pyth\nkick: e(3,8) | every 4 rot 3\nlead: 1 3 5 6 | walk(3,7)\nsong: intro 4 verse 8',
  'morceau libre (novelty seul)': 'lead: c3 e3 g3',
  'microtonal personnalisé': 'key: d 0 1.5 3 5 6.5 8 10\nlead: 1 3 5',
  'maqâm + drone + bend/slide (pourquoi)': 'key: d hijaz\ndrone: 1 | saw\nlead: 1 2 3 5 | bend | slide',
  'euclid + swing + groove': 'groove: dilla\nkey: e minor\nhat: e(3,8) | swing 0.2\nkick: x x x x',
  'métrique composée 2+2+3/8': 'meter: 2+2+3/8\nkick: x . . x . . x .\nbell: 1 2 3',
  'métrique simple 7/8': 'meter: 7/8\nkick: x x x x x x x\nbell: 1 2 3',
  'accel (le pourquoi du tempo qui respire)': 'key: c major\naccel: 60 140\nlead: 1 3 5 1',
  'raga bhairav + forme': 'key: c bhairav\nsong: alaap 4 jor 4 jhala 4\ndrone: 1\nlead: 1 2 3 5 | slide',
  '19edo microtonal': 'key: c major\ntune: 19 edo\nlead: 1 3 5',
  'romans + résolution V-I sur G': 'key: g major\nprog: C G Am Em F C D7 G\nlead: 1 3 5 1 7 5 3 4',
  'blues (bend, blue note)': 'key: a blues\nlead: 1 3 4 5 | bend',
  'générative (solve + fib + walk)': 'seed: 3\nkey: c major\nlead: solve(12,target:1)\nmel: fib(8) | walk(4,9)',
  'modulation key@section': 'key: a minor\nkey@chorus: c minor\nsong: verse*2 chorus*2\nlead: 1 3 5 1 7 5 3 2',
  'gamelan pelog (pentatonique indonésienne)': 'key: d pelog\nkempul: x . . . x . . .\nlead: 1 3 5 1 3 5 1 3',
  'gamelan slendro + euclide': 'key: e slendro\nbonang: 1 2 3 5 6 | every 2 rev\nkendang: e(3,8)',
  'clave afro-cubaine 3-2': 'key: c major\nclave: x . x . x x . x\nguagua: x x . . x x . .',
  'samba batucada (surdo + repique)': 'key: d dorian\nsurdo: x . x x . x x .\nrepique: e(3,8) | every 4 rot 2',
  '17-EDO (accordage à 17 divisions)': 'key: f major\ntune: 17 edo\nlead: 1 3 5 6 1',
  'raga marwa (matinal, seconde augmentée 1·2·b3·#4)': 'key: d marwa\ndrone: 1\nlead: 1 5 7 6 b3 2 | slide',
  'attente résolue 7→1 (prog V→I)': 'key: c major\nprog: G7 C\nlead: 7 1',
  'attente suspendue sur la note de tête (7)': 'key: c major\nlead: 1 2 3 5 7',
  'mélodie étroite dans une 3ce + conseils': 'key: d minor\nlead: 1 2 3 1 2 3',
  'bytebeat + mélodie (la voix t: ne compte pas comme notes)': 'key: c minor\nkick: x . x .\ncrunch: t: (t*5&t>>7)|(t*3&t>>10) | gain 0.4\nlead: 1 3 5',
  'mode lydien (ethos — ambience par couleur)': 'key: f lydian\nlead: 1 2 3 4 5 6',
};
// force: the acoustic consonance % is a float measurement (Plomp–Levelt) —
// done with the piece's own tuniness now (the global `tuning` leak was fixed)
// but f64-in-JS vs f32-in-WASM can still wobble by a unit on some keys. Likewise
// the NOVELTY numbers: JS fingerprints the abstract step tokens while the Rust
// analyser fingerprints RESOLVED notes (it even hears a key@section modulation) —
// so score/variété/hauteurs/rythme % differ by construction on some pieces, not
// by bug. We normalise those NUMBERS and keep the ADVICE lines (the repeat-flags
// `⤷` and the décision bands) strictly exact. Everything deterministic (tonality,
// accordage labels, acoustique labels, harmonie+cadence, forme, the "why") is strict.
const an_norm = l => l
  .replace(/(consonance )\d+(\/100)/, '$1N$2')
  .replace(/^\[ nouveauté \] score \d+\/100.*$/, '[ nouveauté ] score N/100');
let an_total = 0, an_bad = 0;
for (const [name, src] of Object.entries(an_pieces)) {
  let js, rs;
  try { js = L.analyzePiece(src); } catch (e) { js = ['JS-ERR: ' + e.message]; }
  try { rs = W.analyze_json(src); } catch (e) { rs = ['RUST-ERR: ' + e.message]; }
  // with a key@section modulation both engines hear different bars (Rust resolves the
  // section's notes, JS keeps the step tokens) — their novelty sections are not
  // line-comparable by design, so compare the deterministic locks only.
  if (/key@/.test(src)) { js = js.filter(l => !l.startsWith('[ nouveauté ]') && !l.startsWith('   ⤷')); rs = rs.filter(l => !l.startsWith('[ nouveauté ]') && !l.startsWith('   ⤷')); }
  if (js.length !== rs.length || js.some((l, i) => an_norm(l) !== an_norm(rs[i]))) {
    an_bad++;
    if (an_bad <= 12) {
      console.log(`\n  ANALYSE DIFF [${name}]`);
      const n = Math.max(js.length, rs.length);
      for (let i = 0; i < n; i++) {
        const a = i < js.length ? js[i] : '<none>', b = i < rs.length ? rs[i] : '<none>';
        if (an_norm(a) !== an_norm(b)) { console.log(`    JS  : ${a}\n    Rust: ${b}  (acoustique % normalisé)`); }
      }
    }
  }
  an_total++;
}
const an_perfect = an_bad === 0;

// ── RENDER PARITY (P6-1 unified engine: deterministic mono PCM DSP) ──────────
// Both DSP paths consume the IDENTICAL Rust-scheduled event stream
// (schedule_bar_json, extended with `dur` + the resolved v1 `synth` subset) and
// the same `bar_seconds` — so this compares ONLY the DSP (waveform oscillators,
// unison/detune/sub/double, state-variable filter static-or-swept, ADSR x gate),
// never the scheduler (already proven bit-for-bit above).
// Rust computes in f32; the JS mirror frounds every feedback/phase state, so the
// samples should agree to ~1e-6..1e-3 (a Math.sin/tan/pow last-ulp is possible).
const rd_pieces = {
  'sine + env ADSR': 'key: c major\nlead: 1 3 5 1 5 3 1 | sine | env 0.01 0.15 0.8 0.3',
  'square + lp balayé': 'key: a minor\nlead: 1 5 3 1 | square | lp 300..4000 q 3 | env 0.01 0.2 0.6 0.1',
  'saw + hp q élevé': 'key: c major\nlead: 1 2 3 5 3 2 | saw | hp 500 q 4',
  'supersaw (unison 7)': 'key: e minor\nlead: 1 3 5 1 | supersaw',
  'unison 4 + detune 25 + bp': 'key: d major\nlead: 1 5 3 1 | saw | unison 4 | detune 25 | bp 1200 q 1.5',
  'sub + double octaves': 'key: f minor\nlead: 1 3 5 1 | tri | sub 0.6 | double 0.25',
  'notch + gate court': 'key: c major\nlead: 1 1 1 1 | saw | notch 800 q 1 | gate 0.5 | env 0.005 0.05 0.5 0.05',
  'pelog microtonal': 'key: d pelog\nlead: 1 3 5 1 | saw | lp 900 q 1.2',
  'drums ignorés + mélodie': 'kick: x . x .\nhat: x x x x\nkey: c major\nlead: 1 3 5 1 | sine | env 0.01 0.1 0.8 0.05',
};
const RQ_BARS = 2, RQ_SR = 44100;
let rd_total = 0, rd_bad = 0, rd_worst = 0;
for (const [name, src] of Object.entries(rd_pieces)) {
  try {
    const evsArr = [], barSecs = [];
    for (let b = 0; b < RQ_BARS; b++) { evsArr.push(JSON.parse(W.schedule_bar_json(src, b))); barSecs.push(W.bar_seconds(src, b)); }
    const js = L.renderPCM(evsArr, barSecs, RQ_SR);
    const rs = W.render_json(src, RQ_BARS, RQ_SR);
    rd_total++;
    if (js.length !== rs.length) { rd_bad++; console.log(`  RENDER LEN DIFF [${name}] JS ${js.length} Rust ${rs.length}`); continue; }
    let worst = 0, loose = 0;
    for (let i = 0; i < rs.length; i++) {
      const d = Math.abs(rs[i] - js[i]);
      if (d > worst) worst = d;
      if (d > 5e-3) loose++;
    }
    if (worst > rd_worst) rd_worst = worst;
    if (worst > 5e-3 || loose > rs.length * 0.0001) {
      rd_bad++;
      if (rd_bad <= 8) console.log(`  RENDER DIFF [${name}] worst ${worst.toExponential(2)} loose=${loose}/${rs.length}`);
    }
  } catch (e) { rd_bad++; rd_total++; console.log(`  RENDER ERR [${name}] ${e.message}`); }
}
console.log(`\nRENDER parity (P6-1 DSP, JS renderPCM <-> Rust render_json, échantillon par échantillon): ${rd_total - rd_bad}/${rd_total} pièces  worst diff ${rd_worst.toExponential(2)}  ${rd_bad === 0 ? '✅' : '⚠️ ' + rd_bad + ' diffs'}`);

// ── RENDER2 PARITY (P6-2 full pipeline: envois + master) ──────────────────────
// Same Rust event stream + the resolved `mix_json` through BOTH engines: dry +
// note inserts (| echo/flange/phaser) + sends (| reverb/| delay) feeding the
// master reverb + ping-pong + duck + warmth sat + lookahead limiter. Rust f32 vs
// JS Math.fround op-for-op; the exponentials/pows may wobble by a last ulp, so
// the loose gate (5e-3) stays generous — the WAV export is compared byte-exact.
const rf_pieces = {
  'echo tempo + feedback': 'key: c major\nlead: 1 3 5 1 | sine | echo 2 0.45 | env 0.01 0.05 0.9 0.02',
  'flange balayé': 'key: c major\nlead: 1 3 5 1 | saw | flange 0.5 0.8 | env 0.01 0.05 0.8 0.02',
  'phaser 4 allpass': 'key: a minor\nlead: 1 5 3 1 | saw | phaser 0.7 | env 0.01 0.05 0.8 0.02',
  'reverb send + room fdn': 'room: fdn 0.86 3600\nkey: c major\nlead: 1 3 5 1 | saw | reverb 0.8 | env 0.01 0.05 0.9 0.02',
  'reverb schroeder par défaut': 'key: c major\nlead: 1 3 5 1 | saw | reverb 0.5 | env 0.01 0.05 0.9 0.02',
  'delay send → ping-pong': 'key: c major\nlead: 1 3 5 1 | sine | delay 0.5 | env 0.01 0.05 0.9 0.02',
  'duck pump sur kick': 'duck: 0.7\nkick: x x x x\nkey: c major\nlead: 1 1 1 1 | saw | gain 1.4 | env 0.02 0.1 0.9 0.05',
  'warmth 0.9 sur saw fort': 'warmth: 0.9\nkey: c minor\nlead: 1 3 1 5 3 | saw | gain 1.6 | env 0.01 0.1 0.9 0.05',
  'pile complète: fdn+room+echo+phaser+duck+warmth': 'duck: 0.6\nwarmth: 0.5\nroom: fdn 0.84 4200\nkick: x . x .\nkey: e minor\nlead: 1 3 5 1 | saw | reverb 0.4 | delay 0.3 | echo 2 0.3 | phaser 0.4 | unison 2 | detune 12',
  'drums seuls (silence attendu)': 'kick: x . x .\nhat: x x x x',
  // P6-3A sources riches + inserts de note (sans drums)
  'fm ratio 2 index 3 feedback': 'key: c major\nlead: 1 3 5 1 | saw | fm 2 3 0.5 | env 0.01 0.05 0.9 0.02',
  'wt organ + partials': 'key: c major\nlead: 1 3 | wt organ | env 0.01 0.05 0.9 0.02\npad: 5 1 | sine | partials 1 0.5 0.3 0.2 | env 0.01 0.05 0.9 0.02',
  'pwm + sync en unison': 'key: a minor\nlead: 1 5 3 1 | square | pwm 0.6 | unison 3 | detune 10 | env 0.01 0.05 0.8 0.02\nbass: 1 . 5 . | saw | sync 3 | env 0.01 0.05 0.8 0.02',
  'noise air + am + ring': 'key: c major\nlead: 1 3 5 1 | saw | noise 0.4 | am 6 | env 0.01 0.05 0.9 0.02\npad: 5 . 3 . | sine | ring 220 | env 0.01 0.05 0.9 0.02',
  'drive fold crush redux': 'key: e minor\nlead: 1 3 5 1 | saw | drive 0.7 | fold 0.5 | crush 6 | redux 0.3 | lp 2400 q 2 | env 0.01 0.05 0.9 0.02',
  'bell modal + echo': 'key: c major\nlead: 1 5 3 1 | sine | bell | echo 2 0.3 | env 0.001 0.2 0.6 0.3',
};
let rd2_total = 0, rd2_bad = 0, rd2_worst = 0, wav_bad = 0;
for (const [name, src] of Object.entries(rf_pieces)) {
  try {
    const evsArr = [], barSecs = [];
    for (let b = 0; b < RQ_BARS; b++) { evsArr.push(JSON.parse(W.schedule_bar_json(src, b))); barSecs.push(W.bar_seconds(src, b)); }
    const mix = JSON.parse(W.mix_json(src));
    const js = L.renderFX(evsArr, barSecs, RQ_SR, mix);
    const rs = W.render_fx_json(src, RQ_BARS, RQ_SR);
    rd2_total++;
    if (js.length !== rs.length) { rd2_bad++; console.log(`  RENDER2 LEN DIFF [${name}] JS ${js.length} Rust ${rs.length}`); continue; }
    let worst = 0, loose = 0;
    for (let i = 0; i < rs.length; i++) {
      const d = Math.abs(rs[i] - js[i]);
      if (d > worst) worst = d;
      if (d > 5e-3) loose++;
    }
    if (worst > rd2_worst) rd2_worst = worst;
    if (worst > 5e-3 || loose > rs.length * 0.0001) {
      rd2_bad++;
      if (rd2_bad <= 8) console.log(`  RENDER2 DIFF [${name}] worst ${worst.toExponential(2)} loose=${loose}/${rs.length}`);
    }
    // WAV export: identical thread/Riff header + 16-bit PCM. The warm/saturator
    // chain ends through tanh/exp (f32-libm on Rust, f64-libm→Math.fround on JS)
    // whose rare last-ulp difference can shift ONE int16 step at a quantization
    // boundary — so a file FAILS only if any sample differs by more than 1 step.
    const jw = L.wavBytes(js, RQ_SR), rw = W.render_wav(src, RQ_BARS, RQ_SR);
    if (jw.length !== rw.length) { wav_bad++; console.log(`  WAV LEN DIFF [${name}] JS ${jw.length} Rust ${rw.length}`); continue; }
    const dvj = new DataView(jw.buffer), dvr = new DataView(rw.buffer);
    let off1 = 0, over1 = 0;
    const ns = (rw.length - 44) / 2;
    for (let k = 0; k < ns; k++) {
      const a = dvj.getInt16(44 + k * 2, true), b = dvr.getInt16(44 + k * 2, true);
      const d = Math.abs(a - b);
      if (d === 1) off1++; else if (d > 1) over1++;
    }
    let hdrSame = true;
    for (let i = 0; i < 44 && hdrSame; i++) hdrSame = jw[i] === rw[i];
    if (!hdrSame) { wav_bad++; console.log(`  WAV HEADER DIFF [${name}]`); continue; }
    if (over1 > 0) { wav_bad++; console.log(`  WAV PCM DIFF [${name}] ${over1} samples >1 step (off-by-1=${off1})`); }
  } catch (e) { rd2_bad++; rd2_total++; console.log(`  RENDER2 ERR [${name}] ${e.message}`); }
}
console.log(`\nRENDER2 parity (P6-2 pipeline, JS renderFX <-> Rust render_fx_json): ${rd2_total - rd2_bad}/${rd2_total} pièces  worst diff ${rd2_worst.toExponential(2)}  ${rd2_bad === 0 ? '✅' : '⚠️ ' + rd2_bad + ' diffs'}`);
console.log(`WAV parity (JS wavBytes <-> Rust render_wav, toleré ≤1 step int16): ${rd2_total - wav_bad}/${rd2_total} fichiers  ${wav_bad === 0 ? '✅' : '⚠️ ' + wav_bad + ' diffs'}`);

let total = 0, bad = 0;
for (const [name, src] of Object.entries(pieces)) {
  for (let bar = 0; bar < 4; bar++) {
    const js = onsets(L.scheduleBar(src, bar));
    const rs = onsets(JSON.parse(W.schedule_bar_json(src, bar)));
    for (const v of new Set([...Object.keys(js), ...Object.keys(rs)])) {
      total++;
      const a = (js[v] || []).join(','), b = (rs[v] || []).join(',');
      if (a !== b) { bad++; if (bad <= 12) console.log(`  DIFF [${name} bar${bar} ${v}]\n    JS  : ${a}\n    Rust: ${b}`); }
    }
  }
}
// Parité stricte des verbes Tidal : chaque (onset, jeton joué) par voix et mesure.
// Hauteur comparée en MIDI : Rust donne la fréquence résolue, JS le jeton brut
// (degré ou nom de note) qu'on résout avec la tonalité de la pièce.
const NOTE = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };
const jsMidi = (tok, key, drum) => {
  if (drum) return 'x';
  const base = String(tok).replace(/[?*!].*$/, '');
  if (base === 'x') return 48;
  if (/^[1-7]/.test(base) && key) return L.degreeToMidi(base, key);
  const m = /^([a-g])([#b]?)(-?\d+)$/.exec(base);
  return m ? 12 * (+m[3] + 1) + NOTE[m[1]] + (m[2] === '#' ? 1 : m[2] === 'b' ? -1 : 0) : '?' + base;
};
const rsMidi = e => e.is_drum ? 'x' : e.token === 'x' ? 48 : e.freq == null ? '?' + e.token : Math.round(69 + 12 * Math.log2(e.freq / 440));
const notesOf = (arr, pitch) => { const m = {}; for (const e of arr) (m[e.voice] || (m[e.voice] = [])).push(r3(e.phase) + ':' + pitch(e));
  for (const v in m) m[v].sort(); return m; };
let td_total = 0, td_bad = 0;
for (const [name, src] of Object.entries(tidal_pieces)) {
  const key = L.parseKey(src);
  for (let bar = 0; bar < 4; bar++) {
    const js = notesOf(L.scheduleBar(src, bar), e => jsMidi(e.tok, key, e.drum));
    const rs = notesOf(JSON.parse(W.schedule_bar_json(src, bar)), rsMidi);
    for (const v of new Set([...Object.keys(js), ...Object.keys(rs)])) {
      td_total++;
      const a = (js[v] || []).join(' '), b = (rs[v] || []).join(' ');
      if (a !== b) { td_bad++; if (td_bad <= 8) console.log(`  TIDAL DIFF [${name} bar${bar} ${v}]\n    JS  : ${a}\n    Rust: ${b}`); }
    }
  }
}
// ── BYTEBEAT parity (B3) : l'évaluateur d'expressions et le signal ──────────
// Même expression, mêmes t : valeur entière identique (ou erreur des deux côtés),
// puis le PCM échantillonné-bloqué bit pour bit (Float32), à 44,1 et 48 kHz.
const bb_exprs = ['t', 't*(42&t>>10)', 't*((t>>12|t>>8)&63&t>>4)', '(t*5&t>>7)|(t*3&t>>10)',
  't/(t>>8)', 't%0', '-t>>3', '~t^t<<3', 't*t*t', '2147483647+t', '0xffffffff&t', 't>>33',
  '(t<4096)*t+(t>=4096)*(t>>2)', 't==8000', 't!=0', '1+2*3-4/2%3', '-(-t)', '(0-2147483647-1)/(0-1)',
  '', '1+', '(t', 'tt', 'x', 't 2', '1||2', '99999999999', '0x', '12ab'];
const bb_ts = [0, 1, 255, 1024, 4096, 8000, 12345, 65535, 1000000, 2147483647, -5];
let bb_total = 0, bb_bad = 0;
for (const ex of bb_exprs) {
  let ce = null; try { ce = L.exprCompile(ex, ['t']); } catch (_) {}
  for (const t of bb_ts) {
    bb_total++;
    const js = ce ? String(L.exprEval(ce, [t])) : 'ERR';
    const rs = W.expr_eval(ex, 't', Int32Array.from([t]));
    if (js !== rs) { bb_bad++; if (bb_bad <= 8) console.log(`  BYTEBEAT EXPR DIFF [${ex} t=${t}] JS ${js} Rust ${rs}`); }
  }
}
for (const [ex, start, len, sr] of [['(t*5&t>>7)|(t*3&t>>10)', 0, 4410, 44100], ['t*((t>>12|t>>8)&63&t>>4)', 123457, 9600, 48000],
  ['t*(42&t>>10)', 44100 * 7, 22050, 44100], ['t', 5, 100, 16000]]) {
  bb_total++;
  const js = L.bytebeatFill(L.exprCompile(ex, ['t']), start, len, sr), rs = W.bytebeat_pcm(ex, start, len, sr);
  let same = js.length === rs.length;
  for (let k = 0; same && k < rs.length; k++) same = Object.is(js[k], rs[k]);
  if (!same) { bb_bad++; console.log(`  BYTEBEAT PCM DIFF [${ex} @${start} sr ${sr}]`); }
}
// la coupe expression | verbes et le parse de la voix des deux côtés : même
// expression retenue (évaluée en t=777), aucun événement côté Rust.
for (const line of ['crunch: t: (t*5&t>>7)|(t*3&t>>10) | gain 0.4 | pan -0.5', 'b: t:t|t>>8|lp 900', 'c: t: (t | gain) | gain 0.3', 'bad: t: t*(2+']) {
  bb_total++;
  const jv = L.parse(line)[0];
  const jsrc = jv ? String(L.exprEval(jv.bytebeat, [777])) : 'ERR';
  const sp = L.bytebeatSplit(line.slice(line.indexOf('t:') + 2));
  const rsrc = W.expr_eval(sp[0], 't', Int32Array.from([777]));
  const rv = JSON.parse(W.schedule_bar_json(line + '\nkick: x', 0)).filter(e => e.voice !== 'kick');
  if (rv.length !== 0 || rsrc !== jsrc) { bb_bad++; console.log(`  BYTEBEAT PARSE DIFF [${line}] JS ${jsrc} Rust ${rsrc} events ${rv.length}`); }
}
console.log(`\nBYTEBEAT parity (expressions + PCM + coupe, JS <-> Rust): ${bb_total - bb_bad}/${bb_total}  ${bb_bad === 0 ? '✅' : '⚠️ ' + bb_bad + ' diffs'}`);

console.log(`\nANALYSE parity (JS analyzePiece <-> Rust analyze_json, ligne à ligne): ${an_total - an_bad}/${an_total}  ${an_perfect ? '✅' : '⚠️ ' + an_bad + ' diffs'}`);
console.log(`\nTIDAL parity (ply/linger/segment/range/loopAt, onset + jeton): ${td_total - td_bad}/${td_total} voice×bar  ${td_bad === 0 ? '✅' : '⚠️ ' + td_bad + ' diffs'}`);
console.log(`\nPARITY (JS <-> Rust onset timing): ${total - bad}/${total} voice×bar  ${bad === 0 ? '✅ PERFECT' : '⚠️ ' + bad + ' diffs'}`);
process.exit((bad === 0 && td_bad === 0 && bb_bad === 0 && an_perfect && rd_bad === 0 && rd2_bad === 0 && wav_bad === 0) ? 0 : 2);
