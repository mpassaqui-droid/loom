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
console.log(echecs ? `${echecs} échec(s)` : 'device : tout passe');
process.exit(echecs ? 1 : 0);
