// JS <-> Rust PARITY TEST
// ------------------------
// Proves the JS web engine and the shared Rust core (loom-core -> WASM) agree on
// the TIMING/onset of every voice, across the language features ported to Rust:
// generators, structure (@section / in), poly, jux, off, microtonal scales, pan,
// swing, groove, nudge, degrees, roman chords, modulation (key@section), ratchets,
// alternation, subsequences.
//
// It loads the real JS engine from web/index.html (capture mode via
// window.__loom.scheduleBar) and the Rust engine from the nodejs WASM build.
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
  'swing+groove+nudge': 'groove: mpc\nh: x x x x x x x x | swing 50\nb: x x x x | shift 0.2',
  'degrees+roman+modulation': 'key: a minor\nkey@chorus: c minor\nsong: verse*2 chorus*2\nlead: 1 3 5 1\' 7 5 3 2\npad: I . IV . V . VI .',
  'ratchets+alternation+subseq': 'seed: 9\nm: 1 [2 3] <4/5> 1*3\nk: x*2 x x <x/.>',
  'degrade+chance (seeded drops)': 'seed: 11\nh: x x x x x x x x x x x x x x x x | degrade 40\np: 1 3 5 1\' | chance 0.6\nk: x x x x | degrade 25',
};

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
console.log(`\nPARITY (JS <-> Rust onset timing): ${total - bad}/${total} voice×bar  ${bad === 0 ? '✅ PERFECT' : '⚠️ ' + bad + ' diffs'}`);
process.exit(bad === 0 ? 0 : 2);
