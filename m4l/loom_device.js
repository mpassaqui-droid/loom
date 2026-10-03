// LOOM pour Live : le moteur loom-core (WASM) dans un device Max for Live.
//
// Glisse un fichier .loom sur le device : il cuit la voix de cette piste (celle
// qui porte le nom de la piste, « LOOM » en tête accepté) en notes MIDI, et les
// écrit dans un clip de la piste. Live joue le clip à l'échantillon près. Chaque
// enregistrement du fichier (éditeur LOOM, Ctrl+S) réécrit le clip : on code,
// Live joue. Le moteur est le même que l'appli LOOM (loom bake), au bit près.
//
// Messages : fichier <chemin> · code <texte> · voix <nom> · cuire · bang (init)
// Sorties  : 0 = texte d'état pour l'interface · 1 = OSC de contrôle (tests)

autowatch = 0;
inlets = 1;
outlets = 3;

var etat = { fichier: '', source: '', lu: null, voix: '', pret: false, tempoFait: false };
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
    dire(etat.fichier ? 'LOOM prêt · ' + etat.fichier : 'LOOM prêt · glisse un fichier .loom');
    if (!veille) {
        veille = new Task(surveille, this);
        veille.interval = 500;
        veille.repeat();
    }
    surveille();   // chemin restauré avec le set avant que le moteur soit prêt
}

function fichier() {
    var p = String(arrayfromargs(arguments).join(' '));
    // pattr rend 0 et live.drop « none » tant que rien n'a été choisi
    if (!p || p === 'none' || p === '0' || p === etat.fichier) return;
    // live.drop rend un chemin Max « Macintosh HD:/Users/... »
    etat.fichier = p.replace(/^[^:\/]+:\//, '/');
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

function voix() {
    etat.voix = arrayfromargs(arguments).join(' ');
    cuire();
}

function anything() {
    // commandes OSC venues de udpreceive : /loom/code, /loom/fichier, /loom/voix, /loom/cuire
    var m = messagename.replace(/^\/loom\//, '');
    var a = arrayfromargs(arguments);
    if (m === 'code') code.apply(this, a);
    else if (m === 'fichier') fichier.apply(this, a);
    else if (m === 'voix') voix.apply(this, a);
    else if (m === 'cuire') cuire();
}

function surveille() {
    if (!etat.fichier || !etat.pret) return;
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
    dire(choix.name + ' · ' + notes.length + ' notes · ' + cuit.bars + ' mesures · ' + (Date.now() - t0) + ' ms');
}

// Le clip de LOOM : celui qui porte « LOOM » dans le nom, sinon le premier emplacement vide.
function clipLoom(piste, longueur, voix) {
    var n = piste.api.getcount('clip_slots');
    var vide = -1;
    for (var s = 0; s < n; s++) {
        var slot = new LiveAPI(piste.api.unquotedpath + ' clip_slots ' + s);
        if (Number(slot.get('has_clip'))) {
            var c = new LiveAPI(slot.unquotedpath + ' clip');
            if (String(c.get('name')).indexOf('LOOM') === 0) {
                c.set('name', 'LOOM · ' + voix);
                return c;
            }
        } else if (vide < 0) vide = s;
    }
    if (vide < 0) { dire('aucun emplacement de clip libre sur la piste'); return null; }
    var libre = new LiveAPI(piste.api.unquotedpath + ' clip_slots ' + vide);
    libre.call('create_clip', longueur);
    var clip = new LiveAPI(libre.unquotedpath + ' clip');
    clip.set('name', 'LOOM · ' + voix);
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
