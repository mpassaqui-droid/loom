// Le device LOOM simulé hors de Max (Node) : mêmes globales que Max v8
// (outlet, Task, LiveAPI, File), sans TextEncoder/TextDecoder comme dans Max.
// Vérifie que la cuisson écrite dans le clip = `loom bake` pour la voix de la piste.
//   node m4l/test_device.js
const fs = require('fs'), path = require('path'), os = require('os'), { execFileSync } = require('child_process');
const racine = path.join(__dirname, '..');
const tmp = path.join(os.tmpdir(), 'loom_device_test.js');
execFileSync('python3', [path.join(__dirname, 'build_device.py'), '--code', tmp]);
const ref = JSON.parse(execFileSync('node',
  ['-e', `process.stdout.write(require(${JSON.stringify(path.join(racine, 'tools/pkg-node/loom_wasm.js'))}).bake_json(require('fs').readFileSync(${JSON.stringify(path.join(racine, 'autechre.loom'))},'utf8'),0))`]).toString());
// les Dict de Max : nommés, partagés par tous les devices (comme dans Live)
const dicts = {};
globalThis.Dict = function (nom) { this.nom = nom; dicts[nom] = dicts[nom] || {}; };
Dict.prototype.get = function (k) { return k in dicts[this.nom] ? dicts[this.nom][k] : null; };
Dict.prototype.set = function (k, v) { dicts[this.nom][k] = v; };
Dict.prototype.parse = function (j) { dicts[this.nom] = JSON.parse(j); };
let echecs = 0;
const ok = (c, m) => { console.log((c ? '✓ ' : '✗ ') + m); if (!c) echecs++; };
for (const voix of ['ph1', 'kick', 'pad']) {
  const sorties = [], notes = [];
  const sandbox = new Function('fs', 'sorties', 'notes', 'piste', `
    var outlet = function () { sorties.push(Array.prototype.slice.call(arguments)); };
    var arrayfromargs = function (a) { return Array.prototype.slice.call(a); };
    function Task() { this.repeat = function () {}; }
    function LiveAPI(p) { this.unquotedpath = p; this.p = p; }
    LiveAPI.prototype.get = function (k) {
      if (k === 'name') return this.p.indexOf('canonical_parent') >= 0 ? 'Loom ' + piste : '';
      return 0;
    };
    LiveAPI.prototype.getcount = function () { return 8; };
    LiveAPI.prototype.set = function () {};
    LiveAPI.prototype.call = function (m, a) { if (m === 'add_new_notes') a.notes.forEach(function (n) { notes.push(n); }); };
    ${fs.readFileSync(tmp, 'utf8')}
    bang();
    code(fs.readFileSync(${JSON.stringify(path.join(racine, 'autechre.loom'))}, 'utf8'));
  `);
  sandbox(fs, sorties, notes, voix);
  const attendu = ref.tracks.find(t => t.name === voix).notes;
  const pareil = notes.length === attendu.length && notes.every((n, i) => n.pitch === attendu[i][0] && Math.abs(n.start_time - attendu[i][1]) < 1e-9);
  ok(pareil, `${voix} : ${notes.length}/${attendu.length} notes identiques à loom bake`);
}
const sorties = [];
new Function('sorties', `
  var outlet = function () { sorties.push(Array.prototype.slice.call(arguments)); };
  var arrayfromargs = function (a) { return Array.prototype.slice.call(a); };
  function Task() { this.repeat = function () {}; }
  function LiveAPI() {} LiveAPI.prototype.get = function () { return 'LOOM inconnue'; };
  ${fs.readFileSync(tmp, 'utf8')}
  bang(); code('kick: x . x .');
`)(sorties);
ok(String(sorties[sorties.length - 1]).includes('aucune voix'), 'voix absente : message clair, aucun clip touché');
// pattr vide (0) et fichier introuvable : un seul message, aucun spam
const s2 = [];
new Function('s2', `
  var outlet = function () { s2.push(Array.prototype.slice.call(arguments)); };
  var arrayfromargs = function (a) { return Array.prototype.slice.call(a); };
  function Task() { this.repeat = function () {}; }
  function File() { this.isopen = false; }
  ${fs.readFileSync(tmp, 'utf8')}
  restaure(0); bang(); fichier('/nulle/part.loom'); surveille(); surveille(); surveille();
`)(s2);
const etats = s2.filter(a => a[1] === '/loom/etat').map(a => a[2]);
ok(!etats.some(e => e.includes(': 0')) && etats.filter(e => e.startsWith('introuvable')).length === 1, 'pattr vide ignoré, « introuvable » dit une seule fois');
// coder dans Live : l'éditeur du device A joue, le device B (autre piste) suit
function device(piste, sorties, notes) {
  const env = {};
  new Function('fs', 'env', 'sorties', 'notes', 'piste', `
    var outlet = function () { sorties.push(Array.prototype.slice.call(arguments)); };
    var arrayfromargs = function (a) { return Array.prototype.slice.call(a); };
    function Task() { this.repeat = function () {}; }
    function LiveAPI(p) { this.unquotedpath = p; this.p = p; this.id = 0; }
    LiveAPI.prototype.get = function (k) { return k === 'name' && this.p.indexOf('canonical_parent') >= 0 ? 'Loom ' + piste : 0; };
    LiveAPI.prototype.getcount = function () { return 8; };
    LiveAPI.prototype.set = function () {};
    LiveAPI.prototype.call = function (m, a) { if (m === 'add_new_notes') a.notes.forEach(function (n) { notes.push(n); }); };
    ${fs.readFileSync(tmp, 'utf8')}
    env.bang = bang; env.editeur = editeur; env.surveille = surveille; env.restaurecode = restaurecode;
  `)(fs, env, sorties, notes, piste);
  return env;
}
const morceau = 'bpm: 120\nkick: x . x .\nhat: x x x x x x x x';
const sa = [], na = [], sb = [], nb = [];
const A = device('kick', sa, na), B = device('hat', sb, nb);
A.bang(); B.bang();
A.editeur('text', morceau);
ok(na.length === 16 && sa.some(o => o[0] === 3 && o[1] === morceau), `éditeur A (kick) : ${na.length} notes cuites, code envoyé au pattr du set`);
B.surveille();
ok(nb.length === 64 && sb.some(o => o[0] === 4 && o[1] === 'set' && o[2] === morceau), `device B (hat) suit le code partagé : ${nb.length} notes, son éditeur affiche le code`);
const nb2 = nb.length; B.surveille();
ok(nb.length === nb2, 'B ne recuit pas deux fois la même version');
A.editeur('text', '   ');
ok(na.length === 16, 'éditeur vide : rien ne change, la musique continue');
const sc = [], nc = [];
const C = device('kick', sc, nc);
C.restaurecode(0); C.restaurecode(morceau); C.bang();
ok(nc.length === 16 && sc.some(o => o[0] === 4 && o[2] === morceau), 'code restauré avec le set : éditeur rempli, clip recuit au chargement');
// piste pleine : le device ajoute une scène et y écrit son clip
for (const k in dicts) delete dicts[k];
{
  const sorties = [], notes = [], appels = [];
  let slots = 3;
  new Function('fs', 'sorties', 'notes', 'appels', 'st', `
    var outlet = function () { sorties.push(Array.prototype.slice.call(arguments)); };
    var arrayfromargs = function (a) { return Array.prototype.slice.call(a); };
    function Task() { this.repeat = function () {}; }
    function LiveAPI(p) { this.unquotedpath = p; this.p = p; this.id = 0; }
    LiveAPI.prototype.get = function (k) {
      if (k === 'name') return /canonical_parent$/.test(this.p) ? 'Loom kick' : 'pad · intro';
      if (k === 'has_clip') return /clip_slots (\\d+)$/.exec(this.p)[1] < 3 ? 1 : 0;
      return 0;
    };
    LiveAPI.prototype.getcount = function () { return st.slots; };
    LiveAPI.prototype.set = function () {};
    LiveAPI.prototype.call = function (m, a) {
      appels.push(this.p + ' ' + m);
      if (m === 'create_scene') st.slots++;
      if (m === 'add_new_notes') a.notes.forEach(function (n) { notes.push(n); });
    };
    ${fs.readFileSync(tmp, 'utf8')}
    bang(); code('kick: x . x .');
  `)(fs, sorties, notes, appels, { get slots() { return slots; }, set slots(v) { slots = v; } });
  ok(appels.some(a => a.endsWith("create_scene")) && appels.some(a => a.includes('clip_slots 3 create_clip')) && notes.length === 16,
    `piste pleine : une scène ajoutée, clip écrit dans le nouvel emplacement (${notes.length} notes)`);
}
console.log(echecs ? `${echecs} échec(s)` : 'device : tout passe');
process.exit(echecs ? 1 : 0);
