// Unit tests for LOOM's pure language functions (loads the engine, reads window.__loom).
const fs = require('fs');
const html = fs.readFileSync('/Users/munaysiwarsylvepassaquiampuero/Downloads/loom/web/index.html', 'utf8');
let code = null;
for (const block of html.match(/<script>\n([\s\S]*?)<\/script>/g) || []) {
  const inner = block.replace(/^<script>\n?/, '').replace(/<\/script>$/, '');
  if (inner.includes('initAudio')) { code = inner; break; }
}
if (!code) { console.error('FAIL: no engine'); process.exit(1); }

const noop = () => {};
const param = () => ({ value: 0, setValueAtTime: noop, linearRampToValueAtTime: noop, exponentialRampToValueAtTime: noop, setTargetAtTime: noop, cancelScheduledValues: noop });
const node = () => ({ connect: () => node(), disconnect: noop, start: noop, stop: noop, gain: param(), frequency: param(), detune: param(), Q: param(), delayTime: param(), playbackRate: param(), type: '', curve: null, getChannelData: () => new Float32Array(16) });
const ctxObj = { _t: 0, get currentTime(){return this._t;}, sampleRate: 44100, state: 'running', resume: noop, destination: node(),
  createGain: node, createOscillator: node, createBiquadFilter: node, createDelay: node, createConvolver: node, createWaveShaper: node, createBufferSource: node,
  createScriptProcessor: () => ({ ...node(), onaudioprocess: null }), createAnalyser: () => ({ ...node(), fftSize: 2048, frequencyBinCount: 1024, getByteTimeDomainData: noop, getByteFrequencyData: noop }),
  createDynamicsCompressor: () => ({ ...node(), threshold: param(), knee: param(), ratio: param(), attack: param(), release: param() }), createStereoPanner: () => ({ ...node(), pan: param() }),
  createBuffer: (ch, len) => ({ duration: (len||16)/44100, getChannelData: () => new Float32Array(len || 16) }), decodeAudioData: () => Promise.resolve({}) };
function AudioContext(){return ctxObj;}
function OfflineAudioContext(){return ctxObj;}
const makeEl = () => { const o = { value:'', textContent:'', innerHTML:'', className:'', dataset:{}, style:{setProperty:noop}, classList:{add:noop,remove:noop,toggle:noop,contains:()=>false}, _h:{}, addEventListener:(e,fn)=>{o._h[e]=fn;}, appendChild:noop, removeChild:noop, remove:noop, click:noop, select:noop, open:false, disabled:false, querySelectorAll:()=>[], children:[], focus:noop }; return o; };
const els = {}; const getEl = id => (els[id] || (els[id] = makeEl()));
getEl('bpm').value='120'; getEl('vol').value='60'; getEl('swing').value='20'; getEl('echo').value='15'; getEl('space').value='20';
const document = { getElementById:getEl, createElement:makeEl, querySelectorAll:()=>[], documentElement:{style:{setProperty:noop}}, body:makeEl(), addEventListener:noop };
const window = { AudioContext, OfflineAudioContext };
const localStorage = { getItem:()=>null, setItem:noop };
const navigator = { requestMIDIAccess: () => Promise.resolve({ outputs:new Map() }) };
const performance = { now:()=>0 }; const requestAnimationFrame = noop;
const setTimeout = () => 0;
const URL = { createObjectURL:()=>'blob:', revokeObjectURL:noop };
const fetch = () => Promise.resolve({ arrayBuffer:()=>Promise.resolve(new ArrayBuffer(8)) });
const run = new Function('document','window','localStorage','requestAnimationFrame','setTimeout','AudioContext','OfflineAudioContext','fetch','navigator','performance','URL','"use strict";'+code);
run(document, window, localStorage, requestAnimationFrame, setTimeout, AudioContext, OfflineAudioContext, fetch, navigator, performance, URL);

const L = window.__loom;
if (!L) { console.error('FAIL: window.__loom not exposed'); process.exit(1); }

let pass = 0, fail = 0;
const eq = (got, want, msg) => { const g = JSON.stringify(got), w = JSON.stringify(want); if (g === w) pass++; else { fail++; console.error('  ✗ ' + msg + ' — got ' + g + ' want ' + w); } };

// transposeDeg (diatonic transpose, octave wrap, marks/mods preserved)
eq(L.transposeDeg('1', 2), '3', 'transpose 1+2');
eq(L.transposeDeg('7', 2), "2'", 'transpose 7+2 wraps up');
eq(L.transposeDeg('1', 7), "1'", 'transpose octave');
eq(L.transposeDeg('1', -2), '6,', 'transpose down wraps');
eq(L.transposeDeg("1'", 2), "3'", 'transpose keeps octave mark');
eq(L.transposeDeg('3?40', 2), '5?40', 'transpose keeps prob mod');
eq(L.transposeDeg('c3', 2), 'c3', 'transpose leaves notes alone');

// tokenMods [base, prob, ratchet, accent, rampDir]
eq(L.tokenMods('x'), ['x', null, 1, 1, 0], 'tokenMods plain');
eq(L.tokenMods('x!'), ['x', null, 1, 1.4, 0], 'tokenMods accent');
eq(L.tokenMods('x*4>'), ['x', null, 4, 1, 1], 'tokenMods crescendo roll');
eq(L.tokenMods('x?60'), ['x', 0.6, 1, 1, 0], 'tokenMods probability');
eq(L.tokenMods('<1/3>'), ['<1/3>', null, 1, 1, 0], 'tokenMods leaves alternation intact');

// tokenizeWithCols keeps groups + records columns
const tk = L.tokenizeWithCols('x [a b] {c d}', 0);
eq(tk.map(t=>t.text), ['x', '[a b]', '{c d}'], 'tokenize keeps [] and {} groups');
eq(tk[1].col, 2, 'tokenize column of [a b]');

// parseProg with durations
eq(L.parseProg('prog: Am*2 F G'), ['Am','Am','F','G'], 'prog durations');
// parseTempoMap
eq(L.parseTempoMap('tempo: intro 120 drop 140'), {intro:120, drop:140}, 'tempo map');
eq(L.parseTempoMap('tempo: 130'), {}, 'tempo single = no map');

// rampF crescendo/decrescendo
eq([0,1,2,3].map(j=>+L.rampF(j,4,1).toFixed(2)), [0.4,0.6,0.8,1], 'rampF crescendo');
eq([0,1,2,3].map(j=>+L.rampF(j,4,-1).toFixed(2)), [1,0.8,0.6,0.4], 'rampF decrescendo');

// parseChord (returns midi notes)
const cm = L.parseChord('Cm7', 3);
eq(Array.isArray(cm) && cm.length, 4, 'parseChord Cm7 has 4 notes');

// degreeToMidi against a key (equal tuning)
L.setTuning('equal');
const key = L.parseKey('key: c minor');
eq(L.degreeToMidi('1', key) % 12, 0, 'degree 1 in C = C');
eq(L.degreeToMidi('3', key) % 12, 3, 'degree 3 in C minor = Eb');

// just intonation: the MAJOR third (5/4, in C major degree 3 = E) is flatter than equal
const cmaj = L.parseKey('key: c major');
L.setTuning('just');
const jiThird = L.degreeToMidi('3', cmaj);
L.setTuning('equal');
const etThird = L.degreeToMidi('3', cmaj);
eq(jiThird < etThird, true, 'just intonation major 3rd is flatter than equal');
L.setTuning('equal');

// midiToToken (hum-to-melody pitch snapping) against C major — round-trip from the tonic midi
const cmaj2 = L.parseKey('key: c major'); L.setKey(cmaj2);
const baseC = L.degreeToMidi('1', cmaj2);
eq(L.midiToToken(baseC), '1', 'midiToToken tonic -> degree 1');
eq(L.midiToToken(baseC + 4), '3', 'midiToToken +major third -> 3');
eq(L.midiToToken(baseC + 7), '5', 'midiToToken +fifth -> 5');
eq(L.midiToToken(baseC + 12), "1'", 'midiToToken +octave -> 1 up');

// odd meters: meter N/D -> bar length in quarter-note beats
eq(L.parseMeter('meter: 7/8'), 3.5, 'meter 7/8 = 3.5 beats');
eq(L.parseMeter('meter: 5/4'), 5, 'meter 5/4 = 5 beats');
eq(L.parseMeter('bpm: 120'), 4, 'no meter -> 4/4 default');

// modulation: key@SECTION resolves that section in its own key
const km = L.parseKeyMap('key@chorus: c minor\nkey@verse: a minor');
eq(km.chorus.tonic, 0, 'key@chorus tonic = C');
eq(km.verse.tonic, 9, 'key@verse tonic = A');
L.setTuning('equal');
eq(L.degreeToMidi('1', km.chorus) % 12, 0, 'degree 1 in chorus(Cm) = C');
eq(L.degreeToMidi('1', km.verse) % 12, 9, 'degree 1 in verse(Am) = A');

// accelerando / ritardando
eq(L.parseAccel('accel: 100 130'), {a:100,b:130,bars:0}, 'accel 100->130');
eq(L.parseAccel('accel: 140 90 16'), {a:140,b:90,bars:16}, 'ritard 140->90 over 16 bars');
eq(L.parseAccel('bpm: 120'), null, 'no accel directive -> null');

// historical temperaments: Werckmeister/Kirnberger differ from equal
const cmajT = L.parseKey('key: c major');
L.setTuning('werck'); const wT = L.degreeToMidi('3', cmajT);
L.setTuning('equal'); const eT = L.degreeToMidi('3', cmajT);
eq(wT !== eT, true, 'werckmeister 3rd differs from equal');

// share-by-URL round-trips (unicode-safe)
const sng = 'key: a minor\nlead: 1 2 3 | pluck  # é à ç';
eq(L.decShare(L.encShare(sng)), sng, 'share URL round-trips unicode');

// MIDI export produces a valid Standard MIDI File
const mid = L.buildMidiBytes('key: c major\nlead: 1 2 3 5 6 5 3 2 | pluck\nkick: x . x . | sample bd');
eq(String.fromCharCode(mid[0],mid[1],mid[2],mid[3]), 'MThd', 'MIDI file starts with MThd');
eq(String.fromCharCode(mid[14],mid[15],mid[16],mid[17]), 'MTrk', 'MIDI has a track chunk');
let _non=0; for(let i=22;i<mid.length;i++) if((mid[i]&0xf0)===0x90 && mid[i+2]>0) _non++;
eq(_non > 5, true, 'MIDI has note-on events');

console.log((fail===0 ? 'OK' : 'FAIL') + ': ' + pass + ' passed, ' + fail + ' failed');
process.exit(fail ? 1 : 0);
