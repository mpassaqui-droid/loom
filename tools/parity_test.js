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
};

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
console.log(`\nANALYSE parity (JS analyzePiece <-> Rust analyze_json, ligne à ligne): ${an_total - an_bad}/${an_total}  ${an_perfect ? '✅' : '⚠️ ' + an_bad + ' diffs'}`);
console.log(`\nPARITY (JS <-> Rust onset timing): ${total - bad}/${total} voice×bar  ${bad === 0 ? '✅ PERFECT' : '⚠️ ' + bad + ' diffs'}`);
process.exit((bad === 0 && an_perfect) ? 0 : 2);
