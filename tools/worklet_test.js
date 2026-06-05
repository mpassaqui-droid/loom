// Numerically verifies the LOOM AudioWorklet DSP processors.
globalThis.AudioWorkletProcessor = class {};
globalThis.sampleRate = 44100;
const P = {};
globalThis.registerProcessor = (n, c) => { P[n] = c; };
new Function(require('fs').readFileSync(__dirname + '/worklet_master.js', 'utf8'))();
let fail = 0;
const chk = (cond, msg) => { if (!cond) { fail++; console.error('  ✗ ' + msg); } };

// saturator bounds a loud signal
{ const s = new P['loom-sat'](), x = new Float32Array(128).fill(2), y = new Float32Array(128);
  s.process([[x]], [[y]], { drive: [0.5] }); chk(Math.max(...[...y].map(Math.abs)) < 1, 'saturator bounds output'); }
// limiter holds the ceiling
{ const l = new P['loom-lim'](), x = new Float32Array(128).fill(1.5), y = new Float32Array(128);
  for (let b = 0; b < 40; b++) l.process([[x]], [[y]], { ceiling: [0.97] });
  chk(Math.max(...[...y].map(Math.abs)) <= 0.98, 'limiter holds the ceiling'); }
// redux downsamples (a ramp -> few held levels)
{ const r = new P['loom-redux'](), x = new Float32Array(64), y = new Float32Array(64);
  for (let i = 0; i < 64; i++) x[i] = i / 64; r.process([[x]], [[y]], { reduce: [0.5] });
  chk(new Set([...y].map(v => v.toFixed(4))).size < 20, 'redux downsamples'); }
// reson rings after an impulse
{ const r = new P['loom-reson'](); let e = 0;
  for (let b = 0; b < 8; b++) { const x = new Float32Array(128), y = new Float32Array(128); if (!b) x[0] = 1;
    r.process([[x]], [[y]], { freq: [220], decay: [0.95] }); if (b >= 2) for (const v of y) e += v * v; }
  chk(e > 0.001, 'reson rings'); }
// flange modulates a DC input
{ const f = new P['loom-flange'](), x = new Float32Array(128).fill(0.5); let mn = 9, mx = -9;
  for (let b = 0; b < 20; b++) { const y = new Float32Array(128); f.process([[x]], [[y]], { rate: [5], depth: [0.8] });
    for (const v of y) { mn = Math.min(mn, v); mx = Math.max(mx, v); } }
  chk(mx - mn > 0.05, 'flange modulates'); }
{ const w = new P['loom-width'](), L = new Float32Array(4).fill(1), R = new Float32Array(4).fill(-1), oL = new Float32Array(4), oR = new Float32Array(4);
  w.process([[L,R]],[[oL,oR]],{width:[1.8]}); chk(Math.abs(oL[0]-oR[0]) > 2, 'width widens the side'); }
console.log((fail ? 'FAIL' : 'OK') + ': worklet DSP — 6 processors checked' + (fail ? ', ' + fail + ' failed' : ''));
process.exit(fail ? 1 : 0);
