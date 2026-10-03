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
// Messages : fichier <chemin> · code <texte> · editeur <texte> · voix <nom> · cuire · bang (init)
// Sorties  : 0 = texte d'état · 1 = OSC de contrôle (tests) · 2 = pattr chemin
//            3 = pattr code · 4 = l'éditeur (set <texte>)

autowatch = 0;
inlets = 1;
outlets = 5;

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
    dire(etat.fichier ? 'Loom prêt · ' + etat.fichier : 'Loom prêt · écris ton code, Tab pour jouer');
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

function anything() {
    // commandes OSC venues de udpreceive (build de test) : /loom/code, /loom/fichier, /loom/voix, /loom/editeur, /loom/cuire
    var m = messagename.replace(/^\/loom\//, '');
    var a = arrayfromargs(arguments);
    if (m === 'code') code.apply(this, a);
    else if (m === 'fichier') fichier.apply(this, a);
    else if (m === 'voix') voix.apply(this, a);
    else if (m === 'editeur') editeur.apply(this, a);
    else if (m === 'cuire') cuire();
}

function surveille() {
    if (!etat.pret) return;
    // un autre device Loom du set a joué du code : on le suit
    var d = new Dict(PROJET), v = d.get('version');
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
