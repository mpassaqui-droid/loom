// Loom : le moteur loom-core (WASM) dans un device Max for Live.
//
// Glisse un fichier .loom sur le device : il cuit la voix de cette piste (celle
// qui porte le nom de la piste, « Loom » en tête accepté) en notes MIDI, et les
// écrit dans un clip de la piste. Live joue le clip à l'échantillon près. Chaque
// enregistrement du fichier (éditeur Loom, Ctrl+S) réécrit le clip : on code,
// Live joue. Le moteur est le même que l'appli Loom (loom bake), au bit près.
//
// On peut aussi coder DANS Live : l'éditeur du device (Tab ou ▶ pour jouer).
// Le code est celui du morceau entier, partagé par tous les devices Loom du set
// (Dict « loom_projet ») : chaque piste recuit sa voix. Il est sauvé avec le set
// (pattr), et écrit dans le fichier .loom s'il y en a un de lié.
//
// L'IA (la même consigne SYS que l'éditeur web) : on décrit la musique, Claude
// écrit le code, le moteur le vérifie AVANT de l'appliquer (jamais de code cassé).
// La clé Claude reste sur ce Mac (~/Library/Preferences/loom_claude_key, dossier fermé aux
// autres comptes), jamais dans le set.
//
// Messages : fichier <chemin> · code <texte> · editeur <texte> · ia <demande> · cle <clé>
//            reponseia dictionary <nom> · lancer · arreter · voix <nom> · cuire · bang (init)
// Sorties  : 0 = texte d'état · 1 = OSC de contrôle (tests) · 2 = pattr chemin
//            3 = pattr code · 4 = l'éditeur (set <texte>) · 5 = maxurl · 6 = champ de la clé

autowatch = 0;
inlets = 1;
outlets = 7;

var MODELE_IA = 'claude-fable-5-1';
// le patcher du device, pris au chargement (« this » au niveau du script = l'objet Max)
var monPatcher = (typeof this !== 'undefined' && this && this.patcher) ? this.patcher : null;

var PROJET = 'loom_projet';
var etat = { fichier: '', source: '', lu: null, voix: '', pret: false, tempoFait: false, projet: null };
var veille = null;

function dire(t) {
    outlet(0, 'set', t);
    outlet(1, '/loom/etat', t);
}

function bang() {
    if (!etat.pret) {
        try {
            initSync({ module: base64EnOctets(LOOM_WASM) });
            etat.pret = true;
        } catch (e) {
            dire('moteur WASM : ' + e);
            return;
        }
    }
    dire(etat.fichier ? 'Loom prêt · ' + etat.fichier : 'Loom prêt · Ouvrir Loom pour écrire');
    outlet(1, '/loom/maison', maison() || '?');   // diagnostic (port de test seulement)
    // un « lancer » d'une session d'avant ne doit pas relancer ce device à son arrivée
    if (etat.lance === undefined) etat.lance = new Dict(PROJET).get('lancer');
    if (!veille) {
        veille = new Task(surveille, this);
        veille.interval = 500;
        veille.repeat();
    }
    surveille();   // chemin restauré avec le set avant que le moteur soit prêt
    if (etat.source && !etat.fichier) cuire();   // code restauré avec le set
}

function fichier() {
    var p = String(arrayfromargs(arguments).join(' '));
    // pattr rend 0 et live.drop « none » tant que rien n'a été choisi
    if (!p || p === 'none' || p === '0' || p === etat.fichier) return;
    // live.drop rend un chemin Max « Macintosh HD:/Users/... »
    p = p.replace(/^[^:\/]+:\//, '/');
    // le chemin peut venir d'un set partagé (pattr) : seul un fichier .loom est lu ou écrit
    if (!/\.loom$/i.test(p)) { dire('pas un fichier .loom : ' + p); return; }
    etat.fichier = p;
    etat.lu = null;
    etat.tempoFait = false;
    outlet(2, etat.fichier);
    surveille();
}

function restaure() { fichier.apply(this, arguments); }

function code() {
    etat.source = arrayfromargs(arguments).join(' ').replace(/\\n/g, '\n');
    cuire();
}

// textedit (outputmode 1) : le texte arrive en un seul symbole, parfois précédé de « text »
function texteDe(args) {
    var a = arrayfromargs(args);
    if (a.length > 1 && a[0] === 'text') a.shift();
    return a.join(' ');
}

// l'éditeur du device : on joue ce code, on le partage avec les autres devices,
// on le sauve avec le set et dans le fichier lié
function editeur() {
    var t = texteDe(arguments);
    if (!t.replace(/\s/g, '')) return;
    etat.source = t;
    outlet(3, t);
    var d = new Dict(PROJET);
    etat.projet = Date.now();
    d.set('version', etat.projet);
    d.set('code', t);
    if (etat.fichier) ecritFichier(t);
    cuire();
}

// le code sauvé avec le set (pattr) : rendu à l'éditeur, joué quand le moteur est prêt
function restaurecode() {
    var t = texteDe(arguments);
    if (!t || t === '0' || etat.fichier) return;
    etat.source = t;
    outlet(4, 'set', t);
    if (etat.pret) cuire();
}

// ---- l'IA ----

function maison() {
    // le dossier de l'utilisateur, tiré du chemin du device (User Library ou projet)
    var p = String((monPatcher && monPatcher.filepath) || '');
    var m = /^(?:[^:\/]+:)?(\/Users\/[^\/]+)\//.exec(p);
    return m ? m[1] : '';
}

function fichierCle() { var h = maison(); return h ? h + '/Library/Preferences/loom_claude_key' : ''; }

var cleMemoire = '';

function lisCle() {
    if (cleMemoire) return cleMemoire;
    var chemin = fichierCle();
    if (!chemin) return '';
    var f = new File(chemin, 'read');
    if (!f.isopen) return '';
    var t = f.eof > 0 ? f.readstring(f.eof) : '';
    f.close();
    cleMemoire = String(t || '').replace(/\s/g, '');
    return cleMemoire;
}

function cle() {
    var k = texteDe(arguments).replace(/\s/g, '');
    if (!k) return;
    cleMemoire = k;
    var chemin = fichierCle();
    if (chemin) {
        var f = new File(chemin, 'write', 'TEXT');
        if (f.isopen) { f.eof = 0; f.writestring(k); f.close(); }
    }
    outlet(6, 'set', 'clé enregistrée sur ce Mac');
    dire('IA prête');
}

function idDevice() {
    try { var id = Number(new LiveAPI('this_device').id); return id > 0 ? id : 0; } catch (e) { return 0; }
}

function ia() {
    var demande = texteDe(arguments).replace(/^\s+|\s+$/g, '');
    if (!demande) return;
    var k = lisCle();
    if (!k) { dire('IA : colle ta clé Claude dans le champ « clé », puis Retour'); return; }
    var id = idDevice();
    var corps = JSON.stringify({
        model: MODELE_IA,
        max_tokens: 4000,
        system: SYS.join('\n'),
        messages: [{ role: 'user', content: 'Current LOOM code:\n' + etat.source + '\n\nRequest: ' + demande +
            '\n\nReturn the full updated LOOM program. Code only.' }],
    });
    // le corps part d'un fichier : un Dict Max changerait un tableau d'un élément en valeur seule
    var h = maison(), requete = h + '/Library/Caches/loom_ia_requete_' + id + '.json';
    var f = new File(requete, 'write', 'TEXT');
    if (!f.isopen) { dire('IA : écriture impossible de la requête'); return; }
    f.eof = 0; f.writestring(corps); f.close();
    var d = new Dict('loomia_req_' + id);
    d.clear();
    d.set('url', 'https://api.anthropic.com/v1/messages');
    d.set('http_method', 'post');
    d.set('filename_in', requete);
    d.set('headers', 'Content-Type: application/json', 'x-api-key: ' + k, 'anthropic-version: 2023-06-01');
    d.set('response_dict', 'loomia_rep_' + id);
    d.set('timeout', 120);
    d.set('parse_type', 'none');
    etat.iaEnCours = Date.now();
    dire('IA : je compose… (jusqu\'à une minute)');
    outlet(5, 'dictionary', 'loomia_req_' + id);
}

function reponseia() {
    var a = arrayfromargs(arguments), nom = a[a.length - 1];
    var d = new Dict(String(nom));
    if (/^loompont_/.test(String(nom))) return reponsepont(d);
    var corps = String(d.get('body') || '');
    var rep;
    try { rep = JSON.parse(corps); } catch (e) { dire('IA : réponse illisible (' + (d.get('http_status') || '?') + ')'); return; }
    if (rep.error) { dire('IA : ' + (rep.error.message || 'erreur')); return; }
    var texte = (rep.content || []).filter(function (b) { return b.type === 'text'; })
        .map(function (b) { return b.text; }).join('\n')
        .replace(/```[a-z]*\n?/gi, '').replace(/```/g, '').replace(/^\s+|\s+$/g, '');
    // le moteur valide AVANT d'appliquer : un code qui ne donne aucune voix ne remplace rien
    var ok = false;
    try { ok = texte && JSON.parse(bake_json(texte, 0)).tracks.length > 0; } catch (e) { ok = false; }
    if (!ok) { dire('IA : code refusé par le moteur, rien n\'a changé'); return; }
    outlet(4, 'set', texte);
    editeur('text', texte);
    dire('IA : nouveau code joué · ' + Math.round((Date.now() - etat.iaEnCours) / 1000) + ' s');
}

function ecritFichier(t) {
    if (!/\.loom$/i.test(etat.fichier)) return;
    var f = new File(etat.fichier, 'write', 'TEXT');
    if (!f.isopen) { dire('écriture impossible : ' + etat.fichier); return; }
    f.eof = 0;
    f.writestring(t);
    f.close();
    etat.lu = t;    // c'est nous : pas de recuisson en double au prochain tour
}

function voix() {
    etat.voix = arrayfromargs(arguments).join(' ');
    cuire();
}

// ---- la fenêtre Loom : le vrai Loom web, servi par le pont local ----
// Avant de charger la page, le device vérifie que c'est bien NOTRE pont qui écoute sur 7410 :
// il envoie un nombre neuf, le pont doit répondre sha256(jeton:nombre), et seul le pont connaît
// le jeton (fichier 0600). Un autre programme qui prendrait le port n'aurait ni la page ni le jeton.
var URL_PONT = 'http://127.0.0.1:7410';
var pont = { nonce: '', ok: false, jeton: '' };

function sha256(m) {
    var K = [], H = [], i, j;
    function premier(n) { for (var d = 2; d * d <= n; d++) if (n % d === 0) return false; return true; }
    function frac(x) { return ((x - Math.floor(x)) * 4294967296) | 0; }
    for (i = 2, j = 0; j < 64; i++) if (premier(i)) { if (j < 8) H[j] = frac(Math.pow(i, 1 / 2)); K[j++] = frac(Math.pow(i, 1 / 3)); }
    var o = unescape(encodeURIComponent(m)), l = o.length, w = [];
    for (i = 0; i < l; i++) w[i >> 2] |= o.charCodeAt(i) << (24 - (i % 4) * 8);
    w[l >> 2] |= 0x80 << (24 - (l % 4) * 8);
    w[((l + 8) >> 6) * 16 + 15] = l * 8;
    function r(x, n) { return (x >>> n) | (x << (32 - n)); }
    for (j = 0; j < w.length; j += 16) {
        var a = H.slice(0), W = [];
        for (i = 0; i < 64; i++) {
            if (i < 16) W[i] = w[j + i] | 0;
            else W[i] = (W[i - 16] + (r(W[i - 15], 7) ^ r(W[i - 15], 18) ^ (W[i - 15] >>> 3)) + W[i - 7] +
                (r(W[i - 2], 17) ^ r(W[i - 2], 19) ^ (W[i - 2] >>> 10))) | 0;
            var t1 = (a[7] + (r(a[4], 6) ^ r(a[4], 11) ^ r(a[4], 25)) + ((a[4] & a[5]) ^ (~a[4] & a[6])) + K[i] + W[i]) | 0;
            var t2 = ((r(a[0], 2) ^ r(a[0], 13) ^ r(a[0], 22)) + ((a[0] & a[1]) ^ (a[0] & a[2]) ^ (a[1] & a[2]))) | 0;
            a = [(t1 + t2) | 0, a[0], a[1], a[2], (a[3] + t1) | 0, a[4], a[5], a[6]];
        }
        for (i = 0; i < 8; i++) H[i] = (H[i] + a[i]) | 0;
    }
    return H.map(function (x) { return ('00000000' + (x >>> 0).toString(16)).slice(-8); }).join('');
}

function lisJetonPont() {
    var h = maison();
    if (!h) return '';
    var f = new File(h + '/Library/Preferences/loom_pont_jeton', 'read');
    if (!f.isopen) return '';
    var t = f.eof > 0 ? f.readstring(f.eof) : '';
    f.close();
    return String(t || '').replace(/\s/g, '');
}

function pageErreur(texte) {
    return 'data:text/html;charset=utf-8,' + encodeURIComponent('<body style="background:#0b0f0b;color:#9dff8a;' +
        'font:16px ui-monospace,monospace;padding:40px">LOOM<br><br>' + texte + '</body>');
}

// la fenêtre s'ouvre : on demande au pont de prouver qui il est
function ouvre() {
    var n = '';
    for (var i = 0; i < 4; i++) n += ('00000000' + Math.floor(Math.random() * 4294967296).toString(16)).slice(-8);
    pont.nonce = n; pont.ok = false; pont.jeton = '';
    var id = idDevice(), d = new Dict('loompont_req_' + id);
    d.clear();
    d.set('url', URL_PONT + '/preuve?n=' + n);
    d.set('http_method', 'get');
    d.set('response_dict', 'loompont_rep_' + id);
    d.set('timeout', 5);
    d.set('parse_type', 'none');
    outlet(5, 'dictionary', 'loompont_req_' + id);
}

function reponsepont(d) {
    var rep = null, j = lisJetonPont();
    try { rep = JSON.parse(String(d.get('body') || '')); } catch (e) { rep = null; }
    if (j && rep && pont.nonce && rep.preuve === sha256(j + ':' + pont.nonce)) {
        pont.ok = true; pont.jeton = j;
        outlet(4, 'url', URL_PONT + '/?live=1');
        dire('fenêtre Loom ouverte');
    } else {
        outlet(4, 'url', pageErreur(j ? 'Le pont Loom ne répond pas, ou ce n\'est pas lui.<br>Lance : python3 tools/loom_pont_ia.py<br>puis referme et rouvre cette fenêtre.'
            : 'Le pont Loom n\'a jamais été lancé sur ce Mac.<br>Lance : python3 tools/loom_pont_ia.py<br>puis referme et rouvre cette fenêtre.'));
        dire('pont Loom absent');
    }
    pont.nonce = '';
}

// ouvre la fenêtre Loom sans clic (build de test : /loom/fenetre)
function montreFenetre() {
    try { monPatcher.getnamed('loompcontrol').message('open'); } catch (e) { dire('fenêtre : ' + e); return; }
    ouvre();
}

// la page vient de se charger : elle reçoit le jeton du pont (seulement si le pont a prouvé
// qui il est) et le code du morceau
function pret() {
    if (pont.ok) outlet(4, 'jeton', pont.jeton);
    if (etat.source) outlet(4, 'set', etat.source);
}

function anything() {
    // commandes OSC venues de udpreceive (build de test) : /loom/code, /loom/fichier, /loom/voix, /loom/editeur, /loom/cuire
    var m = messagename.replace(/^\/loom\//, '');
    var a = arrayfromargs(arguments);
    if (m === 'code') code.apply(this, a);
    else if (m === 'fichier') fichier.apply(this, a);
    else if (m === 'voix') voix.apply(this, a);
    else if (m === 'editeur') editeur.apply(this, a);
    else if (m === 'ia') ia.apply(this, a);
    else if (m === 'lancer') lancer();
    else if (m === 'arreter') arreter();
    else if (m === 'cuire') cuire();
    else if (m === 'pret') pret();
    else if (m === 'ouvre') ouvre();
    else if (m === 'fenetre') montreFenetre();
}

// ---- le son : lancer les clips Loom de tout le set, ou tout arrêter ----

function lancer() {
    var d = new Dict(PROJET);
    etat.lance = Date.now();
    d.set('lancer', etat.lance);
    lanceMonClip();
    dire('▶ clips Loom lancés');
}

function lanceMonClip() {
    if (etat.clipPath) new LiveAPI(etat.clipPath).call('fire');
}

function arreter() {
    var set = new LiveAPI('live_set');
    set.call('stop_all_clips');
    set.call('stop_playing');
    dire('■ arrêt');
}

function surveille() {
    if (!etat.pret) return;
    // un autre device Loom du set a joué du code : on le suit
    var d = new Dict(PROJET), v = d.get('version');
    var l = d.get('lancer');
    if (l !== null && l !== undefined && l !== etat.lance) { etat.lance = l; lanceMonClip(); }
    if (v !== null && v !== undefined && v !== etat.projet) {
        etat.projet = v;
        var c = String(d.get('code') || '');
        if (c && c !== etat.source) {
            etat.source = c;
            outlet(3, c);
            outlet(4, 'set', c);
            cuire();
        }
    }
    if (!etat.fichier) return;
    var f = new File(etat.fichier, 'read');
    if (!f.isopen) {
        if (etat.lu !== false) dire('introuvable : ' + etat.fichier);   // une fois, pas toutes les 500 ms
        etat.lu = false;
        return;
    }
    var morceaux = [];
    while (f.position < f.eof) morceaux.push(f.readstring(Math.min(16000, f.eof - f.position)));
    f.close();
    // la date de modification n'est pas fiable dans Max : on compare le texte (fichiers courts)
    var texte = morceaux.join('');
    if (texte === etat.lu) return;
    etat.lu = texte;
    etat.source = texte;
    outlet(4, 'set', texte);
    cuire();
}

function nomPiste() {
    var piste = new LiveAPI('this_device canonical_parent');
    return { api: piste, nom: String(piste.get('name')) };
}

function cuire() {
    if (!etat.pret || !etat.source) return;
    var t0 = Date.now();
    var cuit;
    try {
        cuit = JSON.parse(bake_json(etat.source, 0));
    } catch (e) {
        // règle d'or du live coding : une erreur ne coupe jamais le son, l'ancien clip continue
        dire('erreur, l\'ancien clip continue : ' + e);
        return;
    }
    var piste = nomPiste();
    var voulu = etat.voix || piste.nom.replace(/^LOOM\s+/i, '');
    var choix = null;
    for (var i = 0; i < cuit.tracks.length; i++) if (cuit.tracks[i].name === voulu) choix = cuit.tracks[i];
    if (!choix) {
        var noms = cuit.tracks.map(function (t) { return t.name; }).join(' ');
        dire('aucune voix « ' + voulu + ' » · voix : ' + noms);
        return;
    }
    var longueur = cuit.bars * cuit.meter_beats;
    var clip = clipLoom(piste, longueur, choix.name);
    if (!clip) return;
    clip.call('remove_notes_extended', 0, 128, 0, Math.max(longueur, Number(clip.get('length'))));
    var notes = choix.notes.map(function (n) {
        return { pitch: n[0], start_time: n[1], duration: n[2], velocity: n[3] };
    });
    for (var k = 0; k < notes.length; k += 500) clip.call('add_new_notes', { notes: notes.slice(k, k + 500) });
    clip.set('loop_end', longueur);
    clip.set('end_marker', longueur);
    clip.set('loop_start', 0);
    clip.set('start_marker', 0);
    if (!etat.tempoFait) {
        new LiveAPI('live_set').set('tempo', cuit.bar_bpm[0]);
        etat.tempoFait = true;
    }
    etat.clipPath = String(clip.unquotedpath || '');
    publieVue(cuit, choix.name, longueur, clip);
    dire(choix.name + ' · ' + notes.length + ' notes · ' + cuit.bars + ' mesures · ' + (Date.now() - t0) + ' ms');
}

// Les visuels (loom_vue.js, deux v8ui) lisent ce Dict propre au device.
function publieVue(cuit, voix, longueur, clip) {
    var id = Number(new LiveAPI('this_device').id);
    if (!(id > 0)) return;
    var d = new Dict('loomvue_' + id);
    d.parse(JSON.stringify({
        version: Date.now(),
        voix: voix,
        beats: longueur,
        mesure: cuit.meter_beats,
        clip: String(clip.unquotedpath || ''),
        // pas de tableaux imbriqués dans un Dict Max : des objets
        sections: (cuit.sections || []).map(function (x) { return { nom: x[0], debut: x[1] }; }),
        pistes: cuit.tracks.map(function (t) {
            var n = [];
            t.notes.forEach(function (x) { n.push(x[0], x[1], x[2], x[3]); });
            return { nom: t.name, drum: t.drum ? 1 : 0, n: n };
        }),
    }));
}

// Le clip de Loom : celui dont le nom commence par « Loom », sinon le premier emplacement vide.
function clipLoom(piste, longueur, voix) {
    var n = piste.api.getcount('clip_slots');
    var vide = -1;
    for (var s = 0; s < n; s++) {
        var slot = new LiveAPI(piste.api.unquotedpath + ' clip_slots ' + s);
        if (Number(slot.get('has_clip'))) {
            var c = new LiveAPI(slot.unquotedpath + ' clip');
            if (/^loom\b/i.test(String(c.get('name')))) {
                c.set('name', 'Loom · ' + voix);
                return c;
            }
        } else if (vide < 0) vide = s;
    }
    if (vide < 0) {
        // piste pleine (un set cuit par sections) : une scène de plus, en bas, pour le clip Loom
        new LiveAPI('live_set').call('create_scene', -1);
        vide = piste.api.getcount('clip_slots') - 1;
        if (vide < n) { dire('aucun emplacement de clip libre sur la piste'); return null; }
    }
    var libre = new LiveAPI(piste.api.unquotedpath + ' clip_slots ' + vide);
    libre.call('create_clip', longueur);
    var clip = new LiveAPI(libre.unquotedpath + ' clip');
    clip.set('name', 'Loom · ' + voix);
    clip.set('looping', 1);
    return clip;
}

// ---- outils : Max n'a ni TextEncoder ni TextDecoder ni atob ----

function base64EnOctets(b64) {
    var tab = new Uint8Array(256);
    var alpha = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    for (var i = 0; i < 64; i++) tab[alpha.charCodeAt(i)] = i;
    var n = b64.length, pad = b64[n - 1] === '=' ? (b64[n - 2] === '=' ? 2 : 1) : 0;
    var out = new Uint8Array(n / 4 * 3 - pad), o = 0;
    for (var j = 0; j < n; j += 4) {
        var v = (tab[b64.charCodeAt(j)] << 18) | (tab[b64.charCodeAt(j + 1)] << 12) | (tab[b64.charCodeAt(j + 2)] << 6) | tab[b64.charCodeAt(j + 3)];
        if (o < out.length) out[o++] = v >> 16 & 255;
        if (o < out.length) out[o++] = v >> 8 & 255;
        if (o < out.length) out[o++] = v & 255;
    }
    return out;
}

function TextEncoder() {}
TextEncoder.prototype.encode = function (s) {
    var out = [];
    for (var i = 0; i < s.length; i++) {
        var c = s.codePointAt(i);
        if (c > 0xffff) i++;
        if (c < 0x80) out.push(c);
        else if (c < 0x800) out.push(0xc0 | c >> 6, 0x80 | c & 63);
        else if (c < 0x10000) out.push(0xe0 | c >> 12, 0x80 | c >> 6 & 63, 0x80 | c & 63);
        else out.push(0xf0 | c >> 18, 0x80 | c >> 12 & 63, 0x80 | c >> 6 & 63, 0x80 | c & 63);
    }
    return new Uint8Array(out);
};
TextEncoder.prototype.encodeInto = function (s, vue) {
    var b = this.encode(s);
    vue.set(b.subarray(0, vue.length));
    return { read: s.length, written: Math.min(b.length, vue.length) };
};

function TextDecoder() {}
TextDecoder.prototype.decode = function (o) {
    if (!o) return '';
    var s = '', i = 0;
    while (i < o.length) {
        var c = o[i++];
        if (c >= 0xf0) c = (c & 7) << 18 | (o[i++] & 63) << 12 | (o[i++] & 63) << 6 | o[i++] & 63;
        else if (c >= 0xe0) c = (c & 15) << 12 | (o[i++] & 63) << 6 | o[i++] & 63;
        else if (c >= 0xc0) c = (c & 31) << 6 | o[i++] & 63;
        s += String.fromCodePoint(c);
    }
    return s;
};
