// Valide la structure du web/ : chaque script inline de web/index.html se
// parse, le manifest PWA est du JSON valide, le service worker se parse.
// Utilisé par la CI (.github/workflows/ci.yml) et localement (node tools/check_web.js).
'use strict';
const fs = require('fs');
const path = require('path');
const root = path.join(__dirname, '..');
let ok = true;
const fail = (m) => { ok = false; console.error('✗ ' + m); };
const chk = (name, cond, extra) => {
  if (cond) console.log('✓ ' + name);
  else fail(name + (extra ? ' — ' + extra : ''));
};

const htmlPath = path.join(root, 'web', 'index.html');
const html = fs.readFileSync(htmlPath, 'utf8');

// 1) script principal (le MOTEUR) — le premier bloc <script> sans attribut.
const first = html.indexOf('<script>');
if (first < 0) { fail('aucun <script> principal'); }
else {
  const end = html.indexOf('</script>', first);
  new Function(html.slice(first + 8, end) + ';return true;');
  chk('script principal (moteur JS) se parse', true);
}

// 2) script PWA (enregistrement du service worker) — après le commentaire PWA.
const p = html.indexOf('<!-- PWA :');
if (p < 0) { fail('bloc PWA (enregistrement SW) introuvable'); }
else {
  const je = html.indexOf('</script>', p);
  const blk = html.slice(html.indexOf('<script>', p) + 8, je);
  new Function(blk + ';return true;');
  chk('script PWA se parse', true);
}

// 3) manifest.webmanifest : JSON valide, icônes présentes côté disque.
const mfPath = path.join(root, 'web', 'manifest.webmanifest');
const mf = JSON.parse(fs.readFileSync(mfPath, 'utf8'));
const iconOk = Array.isArray(mf.icons) && mf.icons.every((ic) => {
  const p = path.join(root, 'web', ic.src);
  return fs.existsSync(p);
});
chk('manifest.webmanifest : JSON valide + icônes présentes', !!mf.name && !!mf.start_url && iconOk);

// 4) service worker : se parse (new Function = compilation seule, rien ne tourne).
new Function(fs.readFileSync(path.join(root, 'web', 'sw.js'), 'utf8') + ';return true;');
chk('sw.js se parse', true);

process.exit(ok ? 0 : 1);
