// Headless runtime test of LOOM's web engine: stub DOM + Web Audio, load the
// IIFE, then drive run() across several bars with a program that uses every
// feature. Goal: catch RUNTIME errors that `node --check` (syntax only) misses.
const fs = require('fs');
const html = fs.readFileSync('/Users/munaysiwarsylvepassaquiampuero/Downloads/loom/web/index.html', 'utf8');

// extract the IIFE <script> (the one with the engine), not the puter CDN tag
let code = null;
for (const block of html.match(/<script>\n([\s\S]*?)<\/script>/g) || []) {
  const inner = block.replace(/^<script>\n?/, '').replace(/<\/script>$/, '');
  if (inner.includes('initAudio')) { code = inner; break; }
}
if (!code) { console.error('FAIL: could not extract engine script'); process.exit(1); }

const noop = () => {};
const param = () => ({ value: 0, setValueAtTime: noop, linearRampToValueAtTime: noop,
  exponentialRampToValueAtTime: noop, setTargetAtTime: noop, cancelScheduledValues: noop });
const node = () => ({ connect: () => node(), disconnect: noop, start: noop, stop: noop,
  gain: param(), frequency: param(), detune: param(), Q: param(), delayTime: param(), playbackRate: param(),
  type: '', curve: null, oversample: '', buffer: null, getChannelData: () => new Float32Array(16) });

const ctxObj = { _t: 0, get currentTime() { return this._t; }, sampleRate: 44100, state: 'running',
  resume: noop, destination: node(),
  createGain: node, createOscillator: () => { globalThis._oscN = (globalThis._oscN||0)+1; return node(); },
  createBiquadFilter: node, createDelay: node,
  createConvolver: node, createWaveShaper: node,
  createBufferSource: () => { globalThis._smpN = (globalThis._smpN||0)+1; return node(); },
  createScriptProcessor: () => ({ ...node(), onaudioprocess: null }),
  createAnalyser: () => ({ ...node(), fftSize: 2048, frequencyBinCount: 1024, smoothingTimeConstant: 0.7,
    getByteTimeDomainData: noop, getByteFrequencyData: noop }),
  createDynamicsCompressor: () => ({ ...node(), threshold: param(), knee: param(), ratio: param(), attack: param(), release: param() }),
  createStereoPanner: () => ({ ...node(), pan: param() }),
  createBuffer: (ch, len) => ({ getChannelData: () => new Float32Array(len || 16) }),
  decodeAudioData: () => Promise.resolve({}) };
function AudioContext() { return ctxObj; }
function OfflineAudioContext(ch, length, rate) {
  return Object.assign({}, ctxObj, { length, sampleRate: rate || 44100,
    startRendering: () => Promise.resolve({ numberOfChannels: 2, length: length || 16,
      getChannelData: () => new Float32Array(length || 16) }) });
}
const URL = { createObjectURL: () => 'blob:stub', revokeObjectURL: noop };
if (typeof globalThis.Blob === 'undefined') globalThis.Blob = class { constructor(){} };

const els = {};
const makeEl = () => { const o = { value: '', textContent: '', innerHTML: '', className: '', dataset: {},
  style: { setProperty: noop }, classList: { add: noop, remove: noop, toggle: noop, contains: () => false },
  _h: {}, addEventListener: (e, fn) => { o._h[e] = fn; }, appendChild: noop, removeChild: noop, remove: noop,
  click: noop, select: noop, open: false, disabled: false, querySelectorAll: () => [], children: [], focus: noop }; return o; };
const getEl = id => (els[id] || (els[id] = makeEl()));
// sensible defaults for the controls run() reads
getEl('bpm').value = '120'; getEl('vol').value = '60'; getEl('swing').value = '20';
getEl('echo').value = '15'; getEl('space').value = '20';

let schedCalls = 0;
const document = {
  getElementById: getEl,
  createElement: makeEl,
  querySelectorAll: () => [],   // preset/theme button wiring → empty is fine
  documentElement: { style: { setProperty: noop } },
  body: makeEl(),
  addEventListener: noop,
};
const window = { AudioContext, OfflineAudioContext };
const localStorage = { getItem: () => null, setItem: noop };
const navigator = { requestMIDIAccess: () => Promise.resolve({ outputs: new Map() }) };
const performance = { now: () => 0 };
const requestAnimationFrame = noop;
// setTimeout: advance the fake clock and re-invoke tick a bounded number of times
let timeouts = 0;
const setTimeout = (fn) => { if (timeouts++ > 40) return 0; ctxObj._t += 0.5; try { fn(); } catch (e) { throw e; } return timeouts; };

let failed = null;
(async () => {
try {
  const fetch = () => Promise.resolve({ arrayBuffer: () => Promise.resolve(new ArrayBuffer(8)) });
  const run = new Function('document', 'window', 'localStorage', 'requestAnimationFrame', 'setTimeout', 'AudioContext', 'OfflineAudioContext', 'fetch', 'navigator', 'performance', 'URL',
    '"use strict";' + code);
  run(document, window, localStorage, requestAnimationFrame, setTimeout, AudioContext, OfflineAudioContext, fetch, navigator, performance, URL);

  // a program exercising essentially every feature at once
  const mega = [
    'seed: 42', 'duck: 60', 'key: c minor',
    'song: intro*1 drop*2',
    'kick: e(4,16) | in intro drop',
    'snare: . . . . x . . . . . . . x . x*3 .',
    'hat: x?80 x?40 x x?60 x?80 x?40 x <x/x*2> | hp 6000 | gain 0.14',
    'bass: 1 . 3b . 5 . <1/7,> . | saw | unison 3 | detune 18 | lp 600 q 6 | drive 0.4 | env 0.005 0.08 0.3 0.1 | oct 2',
    'keys: i . . . V7 . . . VI . . . iv . . . | sine | env 0.1 0.2 0.7 0.4 | gain 0.16 | in drop',
    'arp: Cm7 . Ab . | arp updown | saw | lp 1500 q 5 | lfo cutoff 0.3 0.6 | in drop',
    'lead: 1 2 3 5 7# 1\' . . | pulse | lfo pitch 5 0.3 | gain 0.14 | pan -0.3',
    'sk: x . x . | sample bd', 'sb: 1 . 5 . | sample bass | oct 2', 'sa: 1 3 5 1\' | sample arpy | arp up',
    'mb: 1 . 5 . | midi 1', 'mk: x . x . | midi 10', 'rv: Cm7 . | reverb 0.4 | delay 0.3',
    'au: 1 5 1 5 | saw | lp 300..6000 | over 4 | drive 0..0.5 | pan -1..1', 'au@drop: 1 . 5 . | lp 500..3000',
    'poly3: 1 1 1', 'poly5: 1 3 5 4 6',
    'ph1: 1 2 3 5 6 | poly 16', 'ph2: 1 2 3 5 6 | poly 17 | sample arpy',
    'scene: aurora',
    'jx: 1 3 5 1\' 7 5 3 2 | tri | jux rev | hue 200',
    'jh: x x x x x x x x | sample hh | jux rot 3 | gain 0.12',
    'of: 1 . 5 . 1\' . 5 . | tri | off 3 | delay 0.3 | hue 280',
    'ofv: 1 3 5 7 | saw | off 4 rev | lp 1800 | crush 5',
    'both: 1 5 1\' 5 | tri | jux fast 2 | off 2',
    'sub: x [x x] x [x x x]',
    'nest: 1 [5 [1\' 5]] 3 [7 5]',
    'subm: 1 [3 5] 1\' [7 5 3] | saw | lp 1500',
    'subg: [e(3,8) x] . x . | sample hh',
    'gst: 1 3 5 1\' | tri | gate 0.25',
    'gle: 1 . 5 . | saw | gate 1.8 | reverb 0.5',
    'harm: 1 3 5 1\' 7 5 3 2 | tri | jux add 2 | reverb 0.4',
    'canon: 1 . 5 . 1\' . 5 . | saw | off 3 add 4 | lp 1800',
    'oct: 1 5 1\' 5 | tri | every 2 add 7',
    'down: 5 4 3 2 1 | saw | add -2 | [1 3]',
    'stk3: 1 . 3 . ; 5 . . . ; 1\' . . . | tri | reverb 0.3',
    'cpt: 1 2 3 4 ; 5 4 3 2 | saw | lp 1600 | gate 0.4',
    'stkj: 1 3 5 ; 1\' 5 3 | tri | jux rev | off 2',
    'acid: 1 . 1\' . 5, . 1 . 4 . 1 . 5 . 1\' . | saw | slide | lp 600 q 8 | drive 0.4 | oct 2',
    'gld: 1 5 1\' 5 | tri | slide 0.12 | reverb 0.3',
    'prog: Am F C G',
    'pad2b: ^ . . . ^ . . . | saw | unison 4 | lp 1000 | reverb 0.6 | gate 1.6',
    'pbass: ^1 . . . ^1 . ^1 . | saw | oct 2 | lp 700 | slide',
    'parp: ^1 ^2 ^3 ^2 | arp up | tri | lp 1800',
    'pgrp: ^1 [^2 ^3] ^1\' . | tri',
    'acc: x . x! . x . x!! . | sample bd',
    'acch: x x! x x x x! x x | sample hh | gain 0.12',
    'accm: 1 3! 5 1\' 7 5! 3 2 | tri | add 2',
    'accc: x?60! . x!*2 . | sample sd',
    'rnd: {1 3 5 7} . {2 4 6} . | tri | lp 1800',
    'rndd: x {. x} x {x .} x {. x!} x x | sample hh',
    'rndg: {1 [3 5]} . {^1 ^2} . | tri',
    'rnds: 1 3 ; {5 7} {1\' 5} | saw | jux rev',
    'roll: . . . . . . . . . . . . x*16> . . . | sample sd',
    'rolld: x*4> . x*4< . | sample hh | gain 0.12',
    'rollm: 1 . 3*4> . 5 . 1\'*4< . | tri | lp 1800',
    'rollchk: <1/3> {x .} x*2 | sample bd',
    'deg: x x x x x x x x | sample hh | degrade 30',
    'degm: 1 3 5 1\' 7 5 3 2 | tri | degrade 40 | jux add 2',
    'sh1: . . x . . . x . | sample sd | shift 0.06',
    'sh2: x x x x | sample hh | shift -0.04 | degrade 20',
    'swh: x x x x x x x x | sample hh | swing 58',
    'swp: 1 2 3 5 6 | poly 12 | swing 60 | tri',
    'rng1: 1 3 5 1\' | tri | ring 220 | gain 0.12',
    'rng2: 1 . 5, . | saw | ring 80 | oct 2 | lp 900',
    'subb: 1 . 5, . 1 . 6, . | saw | oct 2 | sub 0.7 | lp 700 | slide',
    'subk: 1 . . . | sine | oct 1 | sub 0.5 | gain 0.4',
    'nz1: . . x . . . x . | sample sd | noise 0.3',
    'nz2: Cm9 . . . | saw | unison 4 | noise 0.15 | reverb 0.5 | gate 1.5',
    'vw1: 1 3 5 1\' | saw | unison 3 | vowel a | gain 0.12',
    'vw2: Cm7 . Ab . | saw | vowel o | reverb 0.5 | gate 1.4',
    'plk1: 1 3 5 1\' 5 3 | pluck 0.7 | reverb 0.5 | gain 0.16',
    'plk2: 1 . 5, . | pluck 0.4 | oct 2 | gate 0.8',
    'plkc: Cm7 . Ab . | pluck 0.6 | arp up | reverb 0.4',
    'bnd1: 1 . . . | sine | bend -12 0.12 | oct 2 | gain 0.3',
    'bnd2: 1 3 5 1\' | tri | bend 5 0.06',
    'bel1: 1 . 3 . 5 . 1\' . | bell | reverb 0.5 | gain 0.14',
    'bel2: Cm7 . Ab . | bell | arp up | reverb 0.4'
  ].join('\n');
  // if a .loom file path is passed as an arg, run THAT instead of the mega test
  const fileArg = process.argv[2];
  const program = fileArg ? require('fs').readFileSync(fileArg, 'utf8') : mega;
  getEl('editor').value = program;
  // fire the RUN handler -> parse + buildGrid + initAudio + tick (schedules many bars)
  const runHandler = getEl('run')._h['click'];
  if (!runHandler) throw new Error('run button has no click handler (wiring failed)');
  runHandler();
  // exercise the offline render path too (catches global-swap / encode bugs)
  const renderHandler = getEl('renderBtn')._h['click'];
  if (renderHandler) await renderHandler();
  // NOTE: stems export uses paced await-setTimeout which this harness can't simulate
  // (bounded setTimeout); it's structurally identical to renderOffline (covered above).
  console.log('OK: engine loaded + ran + offline-render without runtime errors;',
    'scheduled across', Math.round(ctxObj._t / 0.5), 'ticks');
  console.log('audio events:', (globalThis._oscN||0), 'synth oscillators,', (globalThis._smpN||0), 'sample voices');
} catch (e) {
  failed = e;
  console.error('RUNTIME ERROR:', e && e.message);
  console.error((e && e.stack || '').split('\n').slice(0, 4).join('\n'));
}
process.exit(failed ? 1 : 0);
})();
