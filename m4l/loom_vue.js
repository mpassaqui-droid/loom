// Loom : les visuels dans Live (v8ui, code embarqué dans le device).
//
// Deux modes, choisis par l'argument du v8ui :
//   mini  : dans le device, le piano-roll de la voix de la piste, la tête de
//           lecture du clip Loom, les notes qui s'allument quand elles sonnent.
//           Un clic ouvre la scène.
//   scene : une fenêtre, tout le morceau. Chaque voix a sa couleur (la même que
//           dans l'éditeur web) et tire des particules dans sa direction quand
//           elle joue, comme la scène de l'éditeur.
//
// Les données viennent du moteur (loom_device.js) par un Dict propre au device
// (« loomvue_<id du device> ») : version, voix, longueur, clip, pistes.
// Le temps vient du clip Loom de la piste (playing_position), donc de Live.

autowatch = 0;
outlets = 2;    // 0 : « open » vers la scène · 1 : diagnostic (port UDP du build de test)
mgraphics.init();
mgraphics.relative_coords = 0;
mgraphics.autofill = 0;

var MODE = (typeof jsarguments !== 'undefined' && jsarguments.length > 1) ? String(jsarguments[1]) : 'mini';
var BG = [0.043, 0.059, 0.043], FG = [0.616, 1.0, 0.541], DIM = [0.169, 0.42, 0.133];

var vue = { dict: null, version: null, data: null, clipApi: null, clipPath: '', pos: -1, joue: false };
var particules = [], eclats = {};
var diag = { images: 0, err: '' };

function fnv(s) {
    var h = 2166136261 >>> 0;
    for (var i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); }
    return h >>> 0;
}
function teinte(nom) { return fnv(nom || '') % 360; }

// hsl (h en degrés, s et l de 0 à 1) vers rgb 0..1
function hsl(h, s, l) {
    var c = (1 - Math.abs(2 * l - 1)) * s, x = c * (1 - Math.abs((h / 60) % 2 - 1)), m = l - c / 2, r, g, b;
    if (h < 60) { r = c; g = x; b = 0; } else if (h < 120) { r = x; g = c; b = 0; } else if (h < 180) { r = 0; g = c; b = x; }
    else if (h < 240) { r = 0; g = x; b = c; } else if (h < 300) { r = x; g = 0; b = c; } else { r = c; g = 0; b = x; }
    return [r + m, g + m, b + m];
}

// un Dict Max rend un tableau d'un seul élément comme une valeur seule
function tab(x) { return Array.isArray(x) ? x : (x === null || x === undefined || x === '' ? [] : [x]); }

function taille() {
    var r = box.rect;
    return [r[2] - r[0], r[3] - r[1]];
}

// ---- données ----

function nomDict() {
    if (vue.dict) return vue.dict;
    try {
        var id = Number(new LiveAPI('this_device').id);
        if (id > 0) vue.dict = 'loomvue_' + id;
    } catch (e) {}
    return vue.dict;
}

function lit() {
    var nom = nomDict();
    if (!nom) return;
    var d = new Dict(nom);
    var v = d.get('version');
    if (v === null || v === undefined || v === vue.version) return;
    vue.version = v;
    try { vue.data = JSON.parse(d.stringify()); } catch (e) { vue.data = null; return; }
    // notes à plat [hauteur, début, durée, vélocité, ...] → objets, triés par début
    vue.data.pistes = tab(vue.data.pistes);
    vue.data.sections = tab(vue.data.sections);
    vue.data.pistes.forEach(function (p) {
        var n = tab(p.n), out = [];
        for (var i = 0; i + 3 < n.length; i += 4) out.push({ p: n[i], b: n[i + 1], d: n[i + 2], v: n[i + 3] });
        p.notes = out;
        p.teinte = teinte(p.nom);
    });
    if (vue.data.clip !== vue.clipPath) {
        vue.clipPath = vue.data.clip || '';
        vue.clipApi = vue.clipPath ? new LiveAPI(vue.clipPath) : null;
    }
    vue.pos = -1;
}

function avance() {
    var avant = vue.pos;
    vue.joue = false;
    if (vue.clipApi) {
        try {
            // le clip reste « en lecture » quand le transport de Live est arrêté
            if (!vue.setApi) vue.setApi = new LiveAPI('live_set');
            vue.joue = Number(vue.setApi.get('is_playing')) === 1 && Number(vue.clipApi.get('is_playing')) === 1;
            if (vue.joue) vue.pos = Number(vue.clipApi.get('playing_position'));
        } catch (e) { vue.joue = false; }
    }
    if (vue.joue && avant >= 0 && vue.pos !== avant) frappes(avant, vue.pos);
}

// les notes dont le début tombe dans ]avant, apres] (avec retour au début de la boucle)
function frappes(avant, apres) {
    if (!vue.data) return;
    var L = vue.data.beats || 0;
    var dedans = apres >= avant
        ? function (b) { return b > avant && b <= apres; }
        : function (b) { return b > avant || b <= apres; };
    (vue.data.pistes || []).forEach(function (p) {
        if (MODE === 'mini' && p.nom !== vue.data.voix) return;
        var g = 0;
        for (var i = 0; i < p.notes.length; i++) if (dedans(p.notes[i].b % (L || Infinity))) g = Math.max(g, p.notes[i].v / 127);
        if (g > 0) { eclats[p.nom] = 1; if (MODE === 'scene') rafale(p.teinte, g); }
    });
}

// la rafale de la scène web (emitVoiceBurst) : départ du centre, vers l'angle de la teinte
function rafale(h, g) {
    var t = taille(), W = t[0], H = t[1], n = 4 + Math.floor(g * 10), base = h / 360 * 6.2831853,
        spd = Math.max(W, H) * (0.006 + g * 0.012), col = hsl(h, 0.9, 0.62);
    for (var k = 0; k < n; k++) {
        var a = base + (Math.random() - 0.5) * 1.1, s = spd * (0.5 + Math.random());
        particules.push({ x: W / 2, y: H / 2, vx: Math.cos(a) * s, vy: Math.sin(a) * s, vie: 1, col: col });
    }
    if (particules.length > 1500) particules.splice(0, particules.length - 1500);
}

// ---- dessin ----

function fond(W, H) {
    mgraphics.set_source_rgba(BG[0], BG[1], BG[2], 1);
    mgraphics.rectangle(0, 0, W, H);
    mgraphics.fill();
}

function texte(t, x, y, taillePolice, col, alpha) {
    mgraphics.select_font_face('Menlo');
    mgraphics.set_font_size(taillePolice);
    mgraphics.set_source_rgba(col[0], col[1], col[2], alpha === undefined ? 1 : alpha);
    mgraphics.move_to(x, y);
    mgraphics.show_text(String(t));
}

function peintMini(W, H) {
    fond(W, H);
    var d = vue.data, p = null;
    if (d) (d.pistes || []).forEach(function (q) { if (q.nom === d.voix) p = q; });
    if (!p || !p.notes.length || !d.beats) {
        texte('Loom · glisse un fichier .loom', 8, H / 2 + 4, 10, DIM);
        return;
    }
    var lo = 127, hi = 0;
    p.notes.forEach(function (n) { lo = Math.min(lo, n.p); hi = Math.max(hi, n.p); });
    var range = Math.max(7, hi - lo + 1), haut = 14, rh = (H - haut - 2) / range, L = d.beats;
    var vif = hsl(p.teinte, 0.9, 0.62), terne = hsl(p.teinte, 0.6, 0.38);
    for (var i = 0; i < p.notes.length; i++) {
        var n = p.notes[i], x = n.b / L * W, w = Math.max(1.5, n.d / L * W - 0.5);
        var y = haut + (hi - n.p + (range - (hi - lo + 1)) / 2) * rh;
        var allume = vue.joue && vue.pos >= n.b && vue.pos < n.b + n.d;
        var c = allume ? vif : terne;
        mgraphics.set_source_rgba(c[0], c[1], c[2], allume ? 1 : 0.85);
        mgraphics.rectangle(x, y, w, Math.max(1.5, rh - 0.5));
        mgraphics.fill();
    }
    // mesures
    var mesure = d.mesure || 4;
    mgraphics.set_source_rgba(DIM[0], DIM[1], DIM[2], 0.5);
    for (var b = mesure; b < L; b += mesure) { mgraphics.rectangle(b / L * W, haut, 0.5, H - haut); mgraphics.fill(); }
    if (vue.joue) {
        mgraphics.set_source_rgba(FG[0], FG[1], FG[2], 1);
        mgraphics.rectangle((vue.pos % L) / L * W, haut, 1, H - haut);
        mgraphics.fill();
    }
    var e = eclats[p.nom] || 0;
    texte(p.nom + ' · ' + p.notes.length + ' notes · clic : scène', 6, 10, 9, e > 0.05 ? vif : FG, 0.6 + 0.4 * e);
}

function peintScene(W, H) {
    fond(W, H);
    var d = vue.data;
    if (!d || !(d.pistes || []).length) {
        texte('Loom · aucun morceau cuit', 20, H / 2, 16, DIM);
        return;
    }
    var cx = W / 2, cy = H / 2, R = Math.min(W, H) * 0.38;
    // les voix, chacune à l'angle de sa couleur (là où partent ses particules)
    d.pistes.forEach(function (p) {
        var a = p.teinte / 360 * 6.2831853, x = cx + Math.cos(a) * R, y = cy + Math.sin(a) * R,
            e = eclats[p.nom] || 0, c = hsl(p.teinte, 0.9, 0.45 + 0.25 * e), r = 5 + 9 * e;
        mgraphics.set_source_rgba(c[0], c[1], c[2], 0.35 + 0.65 * e);
        mgraphics.ellipse(x - r, y - r, 2 * r, 2 * r);
        mgraphics.fill();
        texte(p.nom, x + (Math.cos(a) >= 0 ? 12 : -12 - 7 * p.nom.length), y + 4, 11, c, 0.5 + 0.5 * e);
    });
    for (var i = 0; i < particules.length; i++) {
        var q = particules[i], rr = 1.5 + 3 * q.vie;
        mgraphics.set_source_rgba(q.col[0], q.col[1], q.col[2], q.vie);
        mgraphics.ellipse(q.x - rr, q.y - rr, 2 * rr, 2 * rr);
        mgraphics.fill();
    }
    // le morceau : sections et tête de lecture
    var L = d.beats || 1, y0 = H - 18;
    mgraphics.set_source_rgba(DIM[0], DIM[1], DIM[2], 0.6);
    mgraphics.rectangle(10, y0, W - 20, 4);
    mgraphics.fill();
    (d.sections || []).forEach(function (s) {
        var x = 10 + s.debut * (d.mesure || 4) / L * (W - 20);
        mgraphics.set_source_rgba(FG[0], FG[1], FG[2], 0.5);
        mgraphics.rectangle(x, y0 - 4, 1, 12);
        mgraphics.fill();
        texte(s.nom, x + 3, y0 - 6, 9, DIM);
    });
    if (vue.joue) {
        mgraphics.set_source_rgba(FG[0], FG[1], FG[2], 1);
        mgraphics.rectangle(10 + (vue.pos % L) / L * (W - 20) - 1, y0 - 6, 3, 16);
        mgraphics.fill();
    }
    texte('LOOM', 16, 26, 16, FG);
    texte(vue.joue ? 'temps ' + (vue.pos % L).toFixed(1) + ' / ' + L : 'lance un clip Loom', 16, 44, 10, DIM);
}

function paint() {
    var t = taille();
    if (MODE === 'scene') peintScene(t[0], t[1]); else peintMini(t[0], t[1]);
}

// ---- animation ----

function image() {
    diag.images++;
    try { lit(); } catch (e) { diag.err = 'lit: ' + e; }
    try { avance(); } catch (e) { diag.err = 'avance: ' + e; }
    if (diag.images % 30 === 1) {
        var d = vue.data;
        outlet(1, MODE + ' images=' + diag.images + ' dict=' + vue.dict + ' version=' + vue.version +
            ' pistes=' + (d ? (d.pistes || []).length : -1) + ' voix=' + (d ? d.voix : '') +
            ' joue=' + vue.joue + ' pos=' + vue.pos + ' err=' + diag.err);
    }
    for (var k in eclats) { eclats[k] *= 0.86; if (eclats[k] < 0.02) delete eclats[k]; }
    for (var i = particules.length - 1; i >= 0; i--) {
        var q = particules[i];
        q.x += q.vx; q.y += q.vy; q.vx *= 0.985; q.vy *= 0.985; q.vie -= 0.025;
        if (q.vie <= 0) particules.splice(i, 1);
    }
    mgraphics.redraw();
}

function onclick() { if (MODE === 'mini') outlet(0, 'open'); }

var anime = new Task(image, this);
anime.interval = 33;
anime.repeat();
