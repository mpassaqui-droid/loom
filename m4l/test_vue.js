// Les visuels du device simulés hors de Max (Node) : le moteur publie le Dict,
// les deux v8ui (mini et scène) le lisent, suivent la tête de lecture du clip,
// allument les notes et tirent les particules.
//   node m4l/test_vue.js
const fs = require('fs'), path = require('path'), os = require('os'), { execFileSync } = require('child_process');
const racine = path.join(__dirname, '..');
const tmp = path.join(os.tmpdir(), 'loom_device_vue.js');
execFileSync('python3', [path.join(__dirname, 'build_device.py'), '--code', tmp]);
let echecs = 0;
const ok = (c, m) => { console.log((c ? '✓ ' : '✗ ') + m); if (!c) echecs++; };

// un Dict Max : un seul élément = une valeur seule (comme Max)
const dicts = {};
function aplatit(x) {
  if (Array.isArray(x)) { const a = x.map(aplatit); return a.length === 1 ? a[0] : a; }
  if (x && typeof x === 'object') { const o = {}; for (const k in x) o[k] = aplatit(x[k]); return o; }
  return x;
}
function Dict(nom) { this.nom = nom; }
Dict.prototype.parse = function (s) { dicts[this.nom] = aplatit(JSON.parse(s)); };
Dict.prototype.get = function (k) { const d = dicts[this.nom]; return d && k in d ? d[k] : null; };
Dict.prototype.stringify = function () { return JSON.stringify(dicts[this.nom] || {}); };

const horloge = { pos: 0, joue: 1 };
function fabriqueLiveAPI(piste) {
  function LiveAPI(p) { this.unquotedpath = p; this.p = p; this.id = p === 'this_device' ? 42 : 7; }
  LiveAPI.prototype.get = function (k) {
    if (k === 'name') return this.p.indexOf('canonical_parent') >= 0 ? 'Loom ' + piste : '';
    if (k === 'is_playing') return horloge.joue;
    if (k === 'playing_position') return horloge.pos;
    return 0;
  };
  LiveAPI.prototype.getcount = function () { return 8; };
  LiveAPI.prototype.set = function () {};
  LiveAPI.prototype.call = function () {};
  return LiveAPI;
}

// 1. le moteur cuit autechre pour la piste « ph1 » et publie
new Function('fs', 'Dict', 'LiveAPI', `
  var outlet = function () {};
  var arrayfromargs = function (a) { return Array.prototype.slice.call(a); };
  function Task() { this.repeat = function () {}; }
  ${fs.readFileSync(tmp, 'utf8')}
  bang();
  code(fs.readFileSync(${JSON.stringify(path.join(racine, 'autechre.loom'))}, 'utf8'));
`)(fs, Dict, fabriqueLiveAPI('ph1'));
const pub = dicts.loomvue_42;
ok(pub && pub.voix === 'ph1' && pub.beats > 0 && Array.isArray(pub.pistes) && pub.pistes.length === 11, 'le moteur publie le Dict du device (voix, longueur, 11 pistes)');

// 2. les deux vues
function vue(mode, W, H) {
  const appels = {}, sorties = [];
  const mg = new Proxy({}, { get: (_, k) => (k === 'init' || typeof k !== 'string') ? () => {} : (...a) => { appels[k] = (appels[k] || 0) + 1; } });
  const env = {};
  new Function('env', 'mgraphics', 'Dict', 'LiveAPI', 'sorties', `
    var jsarguments = ['loom_vue.js', ${JSON.stringify(mode)}];
    var box = { rect: [0, 0, ${W}, ${H}] };
    var outlet = function () { sorties.push(Array.prototype.slice.call(arguments)); };
    function Task(f) { env.image = f; this.repeat = function () {}; }
    ${fs.readFileSync(path.join(__dirname, 'loom_vue.js'), 'utf8')}
    env.paint = paint; env.onclick = onclick;
    env.etat = function () { return { vue: vue, particules: particules, eclats: eclats }; };
  `)(env, mg, Dict, fabriqueLiveAPI('ph1'), sorties);
  return { env, appels, sorties };
}

const mini = vue('mini', 388, 96);
horloge.pos = 0; mini.env.image();
const p1 = mini.env.etat().vue.data.pistes.find(p => p.nom === 'ph1');
ok(p1 && p1.notes.length === pub.pistes.find(p => p.nom === 'ph1').n.length / 4, `mini : relit les ${p1 && p1.notes.length} notes de ph1`);
const vusMini = new Set();
for (let b = 0; b < pub.beats; b += 0.25) { horloge.pos = b; mini.env.image(); Object.keys(mini.env.etat().eclats).forEach(k => vusMini.add(k)); }
ok([...vusMini].join() === 'ph1', 'mini : seule la voix de la piste s\'allume');
mini.env.paint();
ok((mini.appels.rectangle || 0) > p1.notes.length, `mini : dessine les notes + tête de lecture (${mini.appels.rectangle} rectangles)`);
mini.env.onclick();
ok(mini.sorties.length === 1 && mini.sorties[0][1] === 'open', 'mini : un clic ouvre la scène');

const sc = vue('scene', 960, 540);
horloge.pos = 0; sc.env.image();
const vus = new Set();
for (let b = 0; b < pub.beats; b += 0.25) { horloge.pos = b; sc.env.image(); Object.keys(sc.env.etat().eclats).forEach(k => vus.add(k)); }
const e = sc.env.etat();
ok(vus.size >= 4 && e.particules.length > 0, `scène : ${vus.size} voix ont tiré, ${e.particules.length} particules`);
sc.env.paint();
ok((sc.appels.ellipse || 0) >= e.vue.data.pistes.length + e.particules.length, 'scène : voix + particules dessinées');
// retour au début de la boucle : les notes du temps 0 frappent encore
e.particules.length = 0; const avant = 0;
horloge.pos = pub.beats - 0.1; sc.env.image(); horloge.pos = 0.05; sc.env.image();
ok(sc.env.etat().particules.length > 0 && Object.keys(sc.env.etat().eclats).length > 0, `scène : le bouclage du clip refrappe (${avant} → ${sc.env.etat().particules.length} particules)`);
// clip arrêté : rien ne tire
horloge.joue = 0;
const sc2 = vue('scene', 960, 540);
for (let b = 0; b < 4; b += 0.25) { horloge.pos = b; sc2.env.image(); }
ok(sc2.env.etat().particules.length === 0, 'scène : clip arrêté, aucune particule');
// une seule section, une seule note : le Dict aplatit, la vue tient
new Dict('loomvue_42').parse(JSON.stringify({ version: 2, voix: 'x', beats: 4, mesure: 4, clip: 'c', sections: [{ nom: 'a', debut: 0 }], pistes: [{ nom: 'x', drum: 0, n: [60, 0, 1, 100] }] }));
const m2 = vue('mini', 388, 96);
horloge.joue = 1; horloge.pos = 0; m2.env.image(); m2.env.paint();
ok(m2.env.etat().vue.data.pistes[0].notes.length === 1, 'Dict aplati (1 piste, 1 section, 1 note) : relu sans erreur');

console.log(echecs ? `${echecs} échec(s)` : 'vue : tout passe');
process.exit(echecs ? 1 : 0);
