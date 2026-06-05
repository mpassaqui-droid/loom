// Numerically verifies the master worklet DSP (saturator bounds, limiter ceiling).
let SatCls, LimCls;
globalThis.AudioWorkletProcessor = class {};
globalThis.registerProcessor = (n, c) => { if (n === 'loom-sat') SatCls = c; else LimCls = c; };
const s = require('fs').readFileSync(__dirname + '/worklet_master.js', 'utf8');
new Function(s)();
let fail = 0;
const sat = new SatCls(), x = new Float32Array(128).fill(2), y = new Float32Array(128);
sat.process([[x]], [[y]], { drive: [0.5] });
const sm = Math.max(...[...y].map(Math.abs));
if (!(sm < 1)) { fail++; console.error('saturator not bounded:', sm); }
const lim = new LimCls(), xi = new Float32Array(128).fill(1.5), yo = new Float32Array(128);
for (let b = 0; b < 40; b++) lim.process([[xi]], [[yo]], { ceiling: [0.97] });
const pk = Math.max(...[...yo].map(Math.abs));
if (!(pk <= 0.98)) { fail++; console.error('limiter over ceiling:', pk); }
console.log((fail ? 'FAIL' : 'OK') + ': worklet DSP (saturator bound=' + sm.toFixed(3) + ', limiter peak=' + pk.toFixed(3) + ')');
process.exit(fail ? 1 : 0);
