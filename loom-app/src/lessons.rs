//! LEÇONS — the browser's lessons and reference panel (`LESSONS` and `REF` in
//! `web/index.html`), as Rust data, plus the panel's search.
//!
//! The two tables at the bottom were generated from the web file (same text,
//! same order) and are committed as source: edit them here by hand.

/// One lesson: a numbered title, what it teaches, and code to load and play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lesson {
    /// e.g. `1 · Le rythme`.
    pub title: &'static str,
    /// One or two sentences of explanation.
    pub description: &'static str,
    /// A complete LOOM program (the « essayer » button loads it).
    pub code: &'static str,
}

/// One row of the reference table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefRow {
    /// The heading it is listed under, e.g. `Verbes (génératif)`.
    pub section: &'static str,
    /// The syntax, e.g. `| mutate N`.
    pub syntax: &'static str,
    /// What it does, in French.
    pub explanation: &'static str,
}

/// What the panel shows for a search: matching lessons and reference rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub lessons: Vec<&'static Lesson>,
    pub reference: Vec<&'static RefRow>,
}

/// Search both tabs at once. Like the web, a row matches when its text contains
/// the query; here case AND accents are ignored (`developpe` finds `développe`).
/// An empty query lists everything.
pub fn search(query: &str) -> Found {
    Found { lessons: search_lessons(query), reference: search_reference(query) }
}

/// Lessons whose title, description or code contain the query.
pub fn search_lessons(query: &str) -> Vec<&'static Lesson> {
    let q = normalize(query.trim());
    LESSONS.iter().filter(|l| q.is_empty() || normalize(&format!("{} {} {}", l.title, l.description, l.code)).contains(&q)).collect()
}

/// Reference rows whose section, syntax or explanation contain the query.
pub fn search_reference(query: &str) -> Vec<&'static RefRow> {
    let q = normalize(query.trim());
    REFERENCE
        .iter()
        .filter(|r| q.is_empty() || normalize(&format!("{} {} {}", r.section, r.syntax, r.explanation)).contains(&q))
        .collect()
}

/// Rows grouped under a heading each time the section changes, as the web draws them.
pub fn by_section<'a>(rows: &[&'a RefRow]) -> Vec<(&'a str, Vec<&'a RefRow>)> {
    let mut groups: Vec<(&str, Vec<&RefRow>)> = Vec::new();
    for row in rows {
        match groups.last_mut() {
            Some((section, list)) if *section == row.section => list.push(row),
            _ => groups.push((row.section, vec![row])),
        }
    }
    groups
}

/// Lowercase without accents (`É` → `e`, `ç` → `c`, `œ` → `oe`), to compare
/// what people type with the French text.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => out.push('a'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'î' | 'ï' | 'í' | 'ì' => out.push('i'),
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => out.push('o'),
            'ù' | 'û' | 'ü' | 'ú' => out.push('u'),
            'ÿ' => out.push('y'),
            'ç' => out.push('c'),
            'ñ' => out.push('n'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            _ => out.push(c),
        }
    }
    out
}

const fn l(title: &'static str, description: &'static str, code: &'static str) -> Lesson {
    Lesson { title, description, code }
}

const fn r(section: &'static str, syntax: &'static str, explanation: &'static str) -> RefRow {
    RefRow { section, syntax, explanation }
}

/// Every lesson, in the web's order (57).
#[rustfmt::skip]
pub const LESSONS: &[Lesson] = &[
    l("1 · Le rythme",
      "x = frappe, . = silence. kick snare hat clap sont des percussions. Chaque ligne remplit une mesure.",
      "kick:  x . . . x . . .\nsnare: . . x . . . x .\nhat:   x x x x x x x x"),
    l("2 · Les mélodies (notes)",
      "Toute autre voix joue des notes : lettre a-g, # ou b, octave 1-5. . = silence.",
      "lead: c3 e3 g3 c4 g3 e3 c3 ."),
    l("3 · Tonalité + degrés",
      "Déclare une key, puis écris en degrés 1-7 (1=tonique). La gamme donne les bonnes notes.",
      "key:  c minor\nbass: 1 . 3 . 5 . 3 . | oct 2\nlead: 1 2 3 5 3 2 1 ."),
    l("4 · Les gammes",
      "Change juste la gamme et la même mélodie change de couleur. Essaie dorian, phrygian, hijaz...",
      "key:  e phrygian\nlead: 1 2 3 4 5 4 3 2 | pulse"),
    l("5 · Octaves & altérations",
      "' monte d'une octave, , descend. Altère un degré en suffixe : 7# (hausser), 3b 5b (baisser).",
      "key:  c minor\nlead: 1 5, 1' 7# 5 3 1 . | pulse"),
    l("6 · Les accords",
      "Une MAJUSCULE joue un accord entier : Cm7 F G7 Am Dsus4. Minuscule = note simple.",
      "keys: Cm7 . Fm7 . Gm7 . Cm7 . | sine | gain 0.3"),
    l("7 · Accords romains",
      "Avec une key : I ii iii IV V vi vii = accords diatoniques. Ajoute 7 pour la septième.",
      "key:  c major\nkeys: I . . . vi . . . IV . . . V7 . . . | sine | gain 0.25"),
    l("8 · Arpèges",
      "| arp up (down/updown) égrène un accord note par note. Séquence techno/synthwave.",
      "key: a minor\narp: Am . F . C . G . | arp up | saw | gain 0.2"),
    l("9 · Les verbes",
      "Après | : rev (inverse), fast N, slow N, every N <verbe>. On les enchaîne.",
      "hat:  x x x x | fast 2\nbass: c2 e2 g2 c3 | every 4 rev | saw"),
    l("10 · Rythmes euclidiens",
      "e(k,n) répartit k frappes sur n pas. e(k,n,rot) tourne. Magique pour les grooves.",
      "kick: e(4,16)\nsnare: e(2,16,4)\nhat:  e(7,16) | gain 0.16"),
    l("11 · Probabilité + seed",
      "? = la note joue parfois (x? ~50%, x?70 = 70%). seed: N rend l'aléatoire reproductible.",
      "seed: 5\nkick: x . . . x . . .\nhat:  x?80 x?40 x x?60 x?80 x?40 x x?50 | gain 0.16"),
    l("12 · Rolls (ratchets)",
      "*N répète un token N fois dans son pas : roulements et fills.",
      "kick:  x . . . x . . . x . . . x*2 x*4 . .\nsnare: . . . . x . . . . . . . x . x*3 ."),
    l("13 · Le son (filtre + enveloppe)",
      "lp/hp = filtre (+ q résonance). env A D S R = enveloppe (attaque déclin sustain release).",
      "key: c minor\npad: Cm7 . . . Ab . . . | saw | lp 800 q 6 | env 0.4 0.3 0.7 1.2 | gain 0.16"),
    l("14 · Synthé gras (super-saw + drive)",
      "unison N empile des oscillateurs désaccordés, detune l'écart, drive sature, lfo fait bouger.",
      "key: f minor\nsaw: 1 3 5 1' 5 3 4 2 | saw | unison 5 | detune 18 | lp 1400 q 5 | lfo cutoff 0.3 0.6 | gain 0.13"),
    l("15 · Sidechain (pump)",
      "duck: N (0-100) : tout le mix « pompe » à chaque kick. Le groove techno/house.",
      "duck: 70\nkey: a minor\nkick: x . . . x . . . x . . . x . . .\npad:  Am7 . . . . . . . F . . . . . . . | saw | lp 1100 | gain 0.18"),
    l("16 · Structure de morceau",
      "song: liste les sections (nom*barres). Tague les voix avec | in <sections>. Regarde la console.",
      "song: intro*2 verse*4 chorus*4 outro*2\nkey:  a minor\nkick: x . . . x . . .\nhat:  x x x x x x x x | gain 0.14 | in verse chorus\narp:  Am . F . C . G . | arp up | saw | gain 0.18 | in chorus\npad:  Am . . . | sine | gain 0.2"),
    l("17 · Gammes du monde",
      "Au-delà de major/minor : des gammes de toutes les traditions. Change juste la key.",
      "key:  d hijaz\nlead: 1 2 3 4 5 4 3 2 1 . | pluck 0.7 | reverb 0.4 | gain 0.15\ndrone: 1 . . . 5, . . . | bow | gain 0.12"),
    l("18 · Microtonal (quarts de ton)",
      "Les maqâms arabes ont de VRAIS quarts de ton : le 2e degré de bayati tombe entre les touches.",
      "key:  d bayati\nney:  1 2 3 4 3 2 1 7, 1 . | reed | slide | reverb 0.5 | gain 0.14\ndrone: 1 . . . . . . . | bow | gain 0.12"),
    l("19 · Accords (qualités + slash)",
      "MAJUSCULE = accord entier. Après / la note va à la basse (renversement).",
      "keys: Cm7 . Fm9 . Gm7 . Cm7/Eb . | sine | env 0.02 0.2 0.6 0.4 | reverb 0.4 | gain 0.16"),
    l("20 · Progression + accord-suivi (^)",
      "prog pose l'harmonie. ^ joue l'accord courant ; ^1 sa fondamentale (pour la basse).",
      "key:  a minor\nprog: Am F C G\nbass: ^1 . . . 5, . ^1 . | sine | oct 2 | gain 0.3\npad:  ^ . . . | saw | unison 4 | lp 1200 | gain 0.15"),
    l("21 · Verbes stéréo (jux/chunk/off)",
      "jux = version transformée à droite. chunk = varie 1/N par mesure. off = écho décalé.",
      "key:  a minor\narp:  1 2 3 5 3 2 | jux rev | pluck 0.7 | gain 0.12\nbass: 1 5, 1' 5 | chunk 4 rev | saw | oct 2 | gain 0.13"),
    l("22 · Sous-séquences & stacks",
      "[a b] subdivise un pas (joue plus vite). ; empile plusieurs lignes sur UNE voix.",
      "key:  c minor\nbleep: 1 [3 5] 1' [5 3] | pluck 0.7 | gain 0.14\nharm:  1 . 5 . ; 3 . 7 . | sine | gain 0.12"),
    l("23 · Aléatoire & alternance",
      "{a b c} tire au hasard (reproductible via seed). <a/b> alterne une version par mesure.",
      "seed:  4\nkey:   a minor\nlead:  {1 3 5} . <5/7> . {3 5 1'} . 6? . | pluck 0.7 | delay 0.3 | gain 0.14"),
    l("24 · Générateurs (automates/suites)",
      "ca(rule,n) = automate de Wolfram qui évolue chaque mesure. morse(n)/fib(n) = suites auto-similaires.",
      "kick: x . . . x . . .\nhat:  ca(90,16) | sample hh | gain 0.14\nperc: fib(16) | sample perc | gain 0.16"),
    l("25 · Polymétrie (phasing Reich)",
      "| poly N : la voix tourne à N pas/mesure. Deux N différents -> elles dérivent l'une contre l'autre.",
      "key: c minor\nph1: 1 5 4 3 5 6 | poly 13 | pluck 0.7 | pan -0.3 | gain 0.12\nph2: 1 5 4 3 5 6 | poly 16 | pluck 0.7 | pan 0.3 | gain 0.12"),
    l("26 · FM & tables d'ondes",
      "fm RATIO INDEX = cloches/e-piano/basses métalliques. wt = oscillateur à table d'ondes.",
      "key:   d minor\nepiano: 1 3 5 3 | sine | fm 1 2 | reverb 0.4 | gain 0.12\norgue:  1 . 5 . | wt organ | gain 0.1"),
    l("27 · Modèles physiques",
      "pluck (corde pincée), bow (frottée), reed (anche), bell (cloche modale) : du vrai instrument.",
      "key:  a minor\nharp: 1 2 3 5 3 2 | pluck 0.7 | reverb 0.4 | gain 0.13\nbell: 1' . . . 3' . . . | bell | reverb 0.6 | gain 0.1"),
    l("28 · Synthèse West-coast",
      "fold (replie l'onde), sync (lead qui balaye), ring (modulation en anneau métallique).",
      "key:  c minor\nlead: 1 . 1' . 5 . 3 . | sync 3 | lp 2400 | gain 0.1\nweird: 1 . 3 . | saw | ring 140 | gain 0.08"),
    l("29 · Voix de synthèse",
      "vowel = filtre de formants (aah/eeh). vocode = la synthèse PARLE avec un sample voix.",
      "key:   a minor\nchoir: 1 . . . 5, . . . | saw | unison 4 | vowel a | reverb 0.6 | gate 1.6 | gain 0.1\nrobot: 1 . 3 . 5 . | saw | unison 3 | vocode vox | gain 0.1"),
    l("30 · Mouvement (LFO & automation)",
      "lfo fait bouger un paramètre. lp 300..4000 = rampe (filtre qui s'ouvre). lp sine RATE MIN MAX = signal qui ondule.",
      "key:  f minor\nwub:  1 1 1 1 1 1 1 1 | saw | lp sine 0.5 300 2000 | gain 0.12\nopen: 1 . . . | saw | unison 4 | lp 300..4000 | over 4 | gain 0.1"),
    l("31 · Expression (slide/bend/wow/orbit)",
      "slide = portamento (303). bend = la note glisse vers la hauteur. wow = pleurage. orbit = tourne autour de toi.",
      "key:  a minor\nacid: 1 5, 1' 5 4 1 5 3 | saw | oct 2 | slide | lp 600 q 8 | gain 0.14\nwarp: 1 . . . | sine | bend -12 0.1 | oct 2 | gain 0.2"),
    l("32 · Effets lo-fi & espace",
      "crush (bitcrush IDM), redux (lo-fi numérique), chorus/phaser (largeur), comp (punch).",
      "key:  a minor\nstab: Am . . . F . . . | saw | unison 3 | crush 5 | chorus 0.4 | gain 0.12"),
    l("33 · Morceaux longs (@section)",
      "nom@section: donne un motif différent par section -> le morceau évolue dans le temps.",
      "song: a*4 b*4\nkey:  a minor\nbass: 1 . 5, . | sine | oct 2 | gain 0.3\nbass@b: 1 3 5 3 1 5 3 1 | saw | oct 2 | drive 0.3 | gain 0.16"),
    l("34 · Accordage (tune)",
      "tune change l'intonation : just (ratios purs), pyth (Pythagore), 19edo (19 notes par octave)...",
      "key:  c major\ntune: just\nkeys: I . IV . V . I . | sine | gain 0.2"),
    l("35 · Samples du monde",
      "Des sons échantillonnés repitchés sur les degrés + percussions du monde.",
      "key:   c minor\ntabla: e(7,16) | sample tabla | gain 0.4\nsitar: 1 . 2 3 . 4 5 . | sample sitar | gain 0.4"),
    l("36 · Réverbe à convolution (espaces)",
      "| ir pose un VRAI espace acoustique sur une voix : hall room plate spring cathedral (+ mix 0-1). Plus réaliste que reverb N.",
      "key:  a minor\nprog: Am F C G\npad:  ^ . . . | saw | unison 3 | ir hall 0.4 | gain 0.1\nharp: 1 2 3 5 3 2 | pluck 0.7 | ir plate 0.5 | gain 0.12\nbell: 1' . . . 5 . . . | bell | ir cathedral 0.5 | gain 0.1"),
    l("37 · Gammes perso & liaisons",
      "scale: 0 2 3 5 7 9 10 invente N'IMPORTE quelle gamme (microtonal ok). Le _ prolonge la note précédente (notes tenues).",
      "key:  d 0 1.5 3 5 7 8 10\nlead: 1 _ _ 3 . 5 _ 2 1 _ 7, . 1 _ _ . | pluck 0.7 | reverb 0.4 | gain 0.13\ndrone: 1 . . . . . . . | bow | gain 0.1"),
    l("38 · Voicings & Markov",
      "| voicing aère un accord (open/drop2/wide). | markov régénère une mélodie d'après ses propres enchaînements.",
      "key:  c major\nseed: 4\nkeys: Cmaj7 . Fmaj7 . G7 . Em7 . | voicing open | sine | reverb 0.3 | gain 0.14\nlead: 1 2 3 5 6 5 3 2 | markov | pluck 0.7 | delay 0.2 | gain 0.1"),
    l("39 · Mesures impaires & tempo qui glisse",
      "meter: 7/8 met la barre en temps impairs. accel: A B fait glisser le tempo de A à B (montée/ralenti).",
      "meter: 7/8\naccel: 100 130\nkey:  e phrygian\nlead: 1 2 3 4 5 4 3 | pluck 0.7 | gain 0.13\nkick: x . . x . x . | sample bd | gain 0.4"),
    l("40 · Modulation & voice-leading",
      "key@section change de tonalité pour une section (vraie modulation). | voicing lead enchaîne les accords en douceur.",
      "song: verse*4 chorus*4\nkey:  a minor\nkey@chorus: c minor\nprog: Am F C G\nkeys: ^ . . . | voicing lead | sine | reverb 0.3 | gain 0.15\nbass: ^1 . . . | sine | oct 2 | gain 0.2"),
    l("41 · Groove & dynamiques",
      "groove: dilla donne le swing « bancal » du hip-hop à tout. | cresc fait monter la vélocité (crescendo), | dim la fait descendre.",
      "groove: dilla\nkey:  f minor\nkick: x . . . x . . . | sample bd | gain 0.5\nhat:  x x x x x x x x | sample hh | gain 0.1\nfill: 1 2 3 4 5 6 7 1' | cresc | pluck 0.7 | gain 0.12"),
    l("42 · Verbes de motif & tempéraments",
      "| invert renverse la mélodie (miroir). | sequence la répète transposée. tune: werck = tempérament baroque.",
      "key:  c major\ntune: werck\nlead: 1 2 3 5 3 2 | invert | pluck 0.7 | reverb 0.3 | gain 0.12\nseq:  1 2 3 | sequence 2 | sine | gain 0.1"),
    l("43 · Sampler : chop, reverse, region",
      "| chop N découpe une boucle en N tranches (une par coup). | reverse la lit à l'envers. | begin/end ne jouent qu'une partie.",
      "loop: x . x . x . x . | sample amen | chop 8 | gain 0.3\nrev:  x . . . x . . . | sample amen | reverse | gain 0.2\nbit:  . . x . . . x . | sample break | begin 0.5 | end 0.75 | gain 0.2"),
    l("44 · Gammes du monde & autopan",
      "De nouvelles gammes (ragas, maqamat, Okinawa…). | autopan balaie la voix de gauche à droite.",
      "key:  c bhairavi\nlead: 1 2 3 5 4 3 2 1 | pluck 0.7 | gain 0.13\npad:  1 . . . 5 . . . | sine | autopan 0.4 | reverb 0.5 | gate 1.6 | gain 0.08"),
    l("45 · Progressions toutes prêtes",
      "prog: auto <style> écrit la suite d'accords pour toi (pop, jazz, sad, epic…). Les voix la suivent avec ^.",
      "key:  a minor\nprog: auto sad\nkeys: ^ . . . | voicing lead | saw | unison 3 | lp 1400 | reverb 0.5 | gate 1.6 | gain 0.1\nbass: ^1 . . . 5, . . . | sine | oct 2 | gain 0.18"),
    l("46 · Image stéréo (haas)",
      "| haas élargit énormément une voix dans l'espace (effet Haas : un mini-délai d'un côté).",
      "key:  c major\nprog: auto pop\npad:  ^ . . . | saw | unison 4 | haas 0.7 | lp 1300 | reverb 0.5 | gate 1.8 | gain 0.08\nlead: ^1 ^2 ^3 ^5 | pluck 0.7 | gain 0.1"),
    l("47 · Synthèse additive & orgue",
      "| partials dessine l'amplitude de chaque harmonique. | organ = orgue à tirettes Hammond (9 chiffres 0-8).",
      "key:  c major\nclar: 1 3 5 3 | partials 1 0 0.5 0 0.33 0 0.25 | lp 3000 | gain 0.12\norg:  I . IV . V . I . | organ 888650000 | reverb 0.3 | gain 0.1"),
    l("48 · Modes d'arpège",
      "Au-delà de up/down : converge (de l'extérieur vers le centre), diverge, thumb et pinky (rebond).",
      "key:  a minor\narp:  Am7 . . . Fmaj7 . . . | arp converge | pluck 0.7 | delay 0.2 | gain 0.12\narp2: C . . . G . . . | arp pinky | pluck 0.6 | gain 0.1"),
    l("49 · Variation générative (mutate/stutter)",
      "| mutate fait ÉVOLUER le motif chaque mesure (ça ne se répète jamais pareil). | stutter répète le début (beat-repeat).",
      "seed: 4\nkey:  c minor\nlead: 1 2 3 5 6 5 3 2 | mutate 30 | pluck 0.7 | delay 0.2 | gain 0.12\nbeat: x . x . x . x . | stutter 4 | sample bd | gain 0.4"),
    l("50 · Caractère cassette (tape)",
      "| tape N en un seul réglage : pleurage de bande + saturation + souffle. La nostalgie lo-fi instantanée.",
      "key:  f major\nprog: auto ballad\nkeys: ^ . . . | wt warm | tape 0.6 | reverb 0.4 | gate 1.6 | gain 0.1\nbass: ^1 . . . | sine | oct 2 | gain 0.16"),
    l("51 · Harmoniser une voix",
      "| harmonize N ajoute une harmonie parallèle à N demi-tons : 7=quinte (power chord), 4=tierce, 12=octave.",
      "key:  e minor\nriff: 1 . 1 . 4 . 5 . | saw | harmonize 7 | drive 0.3 | lp 1800 | gain 0.1\nlead: 1 2 3 5 3 2 | pluck 0.7 | harmonize 4 0.4 | gain 0.1"),
    l("52 · Progressions toutes faites (suite)",
      "prog: auto cinematic / lofi / edm / house — et prog: auto random tire une suite différente à chaque RUN.",
      "key:  a minor\nprog: auto cinematic\nkeys: ^ . . . | voicing lead | saw | unison 3 | lp 1300 | reverb 0.5 | gate 1.6 | gain 0.1\nbass: ^1 . . . | sine | oct 2 | gain 0.18"),
    l("53 · Arrangement : chance",
      "| chance N : une voix entière ne joue qu'une partie des mesures (au hasard, seedé). Pour des fills/contre-chants qui vont et viennent.",
      "seed: 2\nkey:  a minor\nkick: x . . . x . . . | sample bd | gain 0.5\nperc: . . x . . . x . | sample perc | chance 0.6 | gain 0.2\nfill: . . . . . . x*4 | sample sd | chance 0.4 | gain 0.3"),
    l("54 · Trance gate",
      "| tgate N hache une nappe tenue en N pulsations par mesure — le son « pompé » de la trance/EDM.",
      "key:  a minor\nprog: auto edm\npad:  ^ . . . | saw | unison 4 | tgate 8 | lp 1500 | reverb 0.4 | gate 1.8 | gain 0.1\nkick: x . . . x . . . | sample bd | gain 0.5"),
    l("55 · LFO : formes & cibles",
      "Le LFO module cutoff/pitch/gain/pan/reso, avec une forme (sine/tri/saw/square). square sur cutoff = le wobble dubstep.",
      "key:  c minor\nwob:  1 1 1 1 1 1 1 1 | saw | lp 800 q 8 | lfo cutoff 2 0.7 square | gain 0.12\nvib:  1 . 3 . 5 . 3 . | sine | lfo pitch 6 0.3 tri | reverb 0.3 | gain 0.1\npn:   1 . . . 5 . . . | tri | lfo pan 1 0.9 | reverb 0.3 | gain 0.1"),
    l("56 · Délai synchronisé (echo) & double LFO",
      "| echo DIV cale des échos sur le tempo (6=pointé, 8=croche). | lfo2 = un 2e LFO pour moduler deux choses à la fois.",
      "key:  a minor\nbpm:  120\nlead: 1 . . . 5 . . . | pluck 0.7 | echo 6 0.5 | reverb 0.3 | gain 0.12\nwob:  1 1 1 1 | saw | lp 700 q 9 | lfo cutoff 2 0.8 square | lfo2 pitch 5 0.2 | gain 0.1"),
    l("57 · Macros (def / $) & walk",
      "def NOM: … crée un motif réutilisable, rappelé par $NOM. walk(n) = une mélodie en marche aléatoire (maths).",
      "key: a minor\ndef riff: 1 2 3 5 3 2 1 .\nlead: $riff $riff | pluck 0.7 | gain 0.12\ngen:  walk(16) | sine | fm 1 2 | delay 0.2 | gain 0.1\nkick: x . . . x . . . | sample bd | gain 0.4"),
];

/// The reference table, grouped by section in the web's order (163 rows).
#[rustfmt::skip]
pub const REFERENCE: &[RefRow] = &[
    r("Bases", "name: motif | verbe | timbre", "une voix par ligne ; le motif remplit une mesure"),
    r("Bases", "x  .", "frappe / silence (batterie : kick snare hat clap)"),
    r("Bases", "# ...", "commentaire (ligne ignorée)"),
    r("Notes & degrés", "c3 eb2 g4", "note absolue (a-g, # ou b, octave 1-5)"),
    r("Notes & degrés", "key: c minor", "déclare la tonalité → active les degrés"),
    r("Notes & degrés", "1 2 3 4 5 6 7", "degrés de la gamme (1 = tonique)"),
    r("Notes & degrés", "1'   5,", "octave : ' monte, , descend"),
    r("Notes & degrés", "7#  3b  5b", "altération chromatique (en suffixe)"),
    r("Gammes", "scale: dorian", "major minor dorian phrygian lydian mixolydian locrian · pentatonic minpent · harmonic_minor melodic_minor whole_tone blues chromatic"),
    r("Gammes du monde", "+ ragas/maqamat", "bhairavi malkauns ahirbhairav nikriz athar_kurd ryukyu pelog_selisir istrian tritone harmonic_major lydian_minor hindu"),
    r("Gammes du monde", "key: d hijaz", "hijaz hijazkar nahawand kurd ajam bhairav yaman todi marwa purvi slendro pelog hirajoshi insen iwato egyptian"),
    r("Microtonal", "key: d bayati", "maqâm à vrais quarts de ton : bayati rast saba sikah huzam jiharkah"),
    r("Gammes", "scale: 0 2 3 5 7 9 10", "gamme PERSONNALISÉE : demi-tons depuis la tonique (microtonal ok : 0 1.5 3 5 7 8 10)"),
    r("Microtonal", "3+50   c3-30", "bend une note de N cents (+ aigu / - grave)"),
    r("Accords", "Cm7 F G7 Am", "MAJUSCULE = accord. Qualités : 5 maj m dim aug sus2 sus4 6 m6 6/9 7 maj7 m7 mmaj7 m7b5 dim7 7b5 7#5 7b9 7#9 7#11 add9 add11 9 maj9 m9 11 m11 13 maj13 m13"),
    r("Accords", "Cm7/G   C/E", "renversement / slash : la note après / passe à la basse"),
    r("Accords", "I ii iii IV V vi vii (+7)", "chiffres romains = accords diatoniques de la key"),
    r("Accords", "| arp up", "arpège : up / down / updown / downup / converge / diverge / thumb / pinky"),
    r("Accords", "| voicing open|drop2|drop3|shell|quartal|wide|lead", "ré-harmonise un accord : open/drop2/wide = aéré ; lead = voice-leading (proche de l'accord précédent)"),
    r("Verbes", "| rev", "inverse le motif"),
    r("Verbes", "| fast N    | slow N", "accélère N× / joue une mesure sur N"),
    r("Verbes", "| every N <verbe>", "applique un verbe une mesure sur N"),
    r("Verbes", "| jux <verbe>", "stéréo : original à gauche, version transformée (ex. rev) à droite"),
    r("Verbes", "| chunk N <verbe>", "applique le verbe à 1/N du motif, en tournant chaque mesure (variation évolutive, Tidal)"),
    r("Verbes", "| off N <verbe>", "couche décalée de N pas, plus douce (canon/écho) ; verbe optionnel"),
    r("Verbes", "| add N", "transpose les degrés de N pas de gamme (ex. jux add 2 = harmonie en tierce à droite)"),
    r("Son", "| semi N", "transposition chromatique de la voix (demi-tons, -36..36) — layers quintes/octaves détunées"),
    r("Motif", "[a b ...]", "sous-séquence : subdivise un pas (ex. x [x x] = 2e pas joué 2× plus vite) ; s'imbrique"),
    r("Motif", "a b ; c d", "stack : plusieurs lignes simultanées sur UNE voix (séparées par ;) — polyphonie/contrepoint"),
    r("Motif", "x!  x!!", "accent : ! augmente la vélocité de la note (!=fort, !!=plus fort) — groove/backbeat"),
    r("Motif", "{a b c}", "choix ALÉATOIRE : tire une alternative au hasard à chaque fois (déterministe via seed)"),
    r("Motif", "1 _ _ 3", "liaison : _ PROLONGE la note précédente sur le pas (notes tenues, phrasé legato)"),
    r("Motif", "x*8>  x*8<", "roll : *N> monte en intensité (crescendo), *N< descend (decrescendo) — montées/fills"),
    r("Effets & mix", "| degrade N", "raréfie : enlève N% des coups au hasard (seeded) — variation/espace"),
    r("Son", "| shift N", "microtiming : décale la voix de N pas (-0.5..0.5). + = en retard (laid-back), - = en avance (push)"),
    r("Son", "| swing N", "swing par voix (0-100) : retarde les pas impairs — ex. hats swingués sur un kick droit"),
    r("Son", "| ring FREQ", "modulation en anneau (Hz) : timbres métalliques/robotiques/cloches inharmoniques"),
    r("Worklet", "| reson FREQ [DECAY]", "résonateur en peigne accordé (Hz) — anneau métallique/Karplus (AudioWorklet)"),
    r("Worklet", "| flange RATE [DEPTH]", "flanger (LFO sur délai court + feedback) — swoosh d'avion (AudioWorklet)"),
    r("Worklet", "| stereo N", "widener stéréo M/S (0-2) — élargit l'image (mono passe tel quel) (AudioWorklet)"),
    r("Son", "| am FREQ", "modulation d'amplitude (Hz) : trémolo audio / bandes latérales — plus doux que ring"),
    r("Son & filtre", "| bp/notch N", "filtre passe-bande / réjecteur (Hz, + q) — en plus de lp/hp ; voix nasales, wah"),
    r("Son & filtre", "| fenv N", "enveloppe de filtre (0-1) : la coupure suit une ADSR — sons acid/plucky/wow"),
    r("Son", "| sub N", "oscillateur sub (sinus une octave en dessous, 0-1) : du poids/corps — idéal basses"),
    r("Son", "| harmonize N [niv]", "couche d'HARMONIE parallèle à N demi-tons (7=quinte, 4=tierce, 12=octave, -12=octave basse)"),
    r("Son", "| double N", "couche octave au-dessus (0-1) : présence/brillance (leads, plucks, claviers)"),
    r("Son", "| noise N", "couche de bruit filtré (0-1) : air/souffle/texture — caisses, nappes, hats"),
    r("Son", "| vowel a/e/i/o/u", "filtre de formants : timbre vocal/chœur (« aah/eeh/ooh ») — superbe sur saw/nappes"),
    r("Son", "| vocode SAMPLE", "VOCODEUR à bandes : la voix « parle » avec le sample modulateur (vox/breath/yeah)"),
    r("Son", "| pluck N", "corde pincée Karplus-Strong (modélisation physique) : guitare/harpe/koto. N=brillance/sustain 0-1"),
    r("Son", "| bow", "corde FROTTÉE (Karplus-Strong soutenu) : violon/violoncelle/nappe organique"),
    r("Son", "| reed", "bois/anche (guide d'onde à réflexion inversée) : clarinette/hautbois, creux et boisé"),
    r("Son", "| sync N", "hard-sync : lead synchronisé qui balaye (N = ratio max) — son agressif rave/acid"),
    r("Son", "| bend N [T]", "pitch-bend : la note démarre N demi-tons plus loin et glisse vers la hauteur en T s (kick drop, 808)"),
    r("Son", "| bell", "synthèse modale : banc de partiels inharmoniques amortis = cloche / métal / gamelan frappé"),
    r("Son", "| spread N", "largeur stéréo de l'unison (0-1) : étale les voix détunées en stéréo (supersaw large)"),
    r("Son", "| supersaw", "macro : saw + unison 7 + detune + spread (gros lead/nappe large instantané)"),
    r("Son", "| fold N", "wavefolding (0-1, Buchla/West-coast) : replie le signal → harmoniques riches et mouvants"),
    r("Son", "| grain N", "synthèse granulaire (0-1) : nuage de micro-grains (densité/dispersion) — textures, clouds"),
    r("Effets & mix", "| phaser N", "phaser (0-1) : filtres all-pass balayés par un LFO → encoches mouvantes (psyché, nappes)"),
    r("Effets & mix", "| comp N", "compression par voix (0-1) : transitoires domptés, punch & glue (basses, drums, bus)"),
    r("Théorie", "| chord NAME", "harmoniseur : chaque note devient un accord (maj min sus dim aug fifth oct maj7 min7 dom7 add9)"),
    r("Théorie", "tune: werck|kirn|meantone", "tempéraments historiques : Werckmeister III, Kirnberger III, méantone (1/4 de comma)"),
    r("Théorie", "tune: just|pyth|Nedo|equal", "accordage : juste intonation / Pythagore / N-EDO (ex. 19edo, 31edo) /tempérament égal"),
    r("Tempo", "bpm: 128", "fixe le tempo du morceau (BPM, dans le fichier) — sinon le bouton BPM"),
    r("Tempo", "tempo: intro 120 drop 140", "tempo PAR SECTION (paires nom/bpm) — accélère/ralentit selon le morceau"),
    r("Tempo", "meter: 7/8", "MESURE impaire : N/D temps par barre (7/8, 5/4, 3/4…) — change la longueur de la barre"),
    r("Tempo", "accel: 120 160 [bars]", "ACCELERANDO / ritardando : le tempo glisse de A à B (sur tout le morceau, ou sur N mesures)"),
    r("Mix", "warmth: 0.5", "saturation chaleur du master (0-1, AudioWorklet sur-échantillonné) — repli auto si indispo"),
    r("Vidéo", "film: vertical|square|wide", "format du bouton FILM : 9:16 (réseaux) / 1:1 / paysage (défaut)"),
    r("Vidéo", "title: Mon morceau", "carte-titre en fondu au début du FILM (sinon le nom du projet, ou LOOM)"),
    r("Théorie", "prog: Am F C G", "progression d'accords, un accord par mesure (cycle)"),
    r("Théorie", "prog: auto <style>", "progression prête : pop jazz sad minor epic andalusian pachelbel blues folk ballad gospel doowop cinematic lofi edm house · random"),
    r("Effets & mix", "| haas N", "élargisseur stéréo (effet Haas) : un mini-délai d'un côté → image très large (0-1)"),
    r("Théorie", "^  /  ^N", "^ = l'accord courant de la prog ; ^N = sa Nième note (^1 = fondamentale, pour la basse)"),
    r("Verbes", "| palindrome", "endroit une mesure, envers la suivante"),
    r("Verbes (proba)", "| sometimes <verbe>", "applique un verbe au hasard (often 75% · rarely 25% · someby P)"),
    r("Verbes", "| rot N    | iter N", "rotation fixe / décalage d'un pas par mesure"),
    r("Génératif", "e(k,n)   e(k,n,rot)", "rythme euclidien (k frappes sur n pas)"),
    r("Génératif (maths)", "ca(rule,n)", "automate cellulaire (Wolfram) qui évolue chaque mesure — rules 90 30 110 18 150"),
    r("Génératif (maths)", "morse(n)   fib(n)", "suites de Thue-Morse / mot de Fibonacci comme rythme (auto-similaire)"),
    r("Génératif (maths)", "| poly N", "vraie polymétrie : la voix tourne à N pas/mesure et dérive (phasing à la Reich)"),
    r("Génératif (maths)", "| markov", "régénère la mélodie d'après ses propres transitions note→note (chaîne de Markov, seedée)"),
    r("Génératif (maths)", "walk(n)", "mélodie en MARCHE ALÉATOIRE sur les degrés (n notes, seedée) — évolue à chaque mesure"),
    r("Génératif (maths)", "lsys(n)  drunk(n)", "L-système (mélodie auto-similaire) · marche d'ivrogne (densité rythmique qui dérive)"),
    r("Génératif (maths)", "lsys(n)  l-sys(n,…)", "L-système auto-similaire · forme personnalisée : l-sys(n,axiome,1>1.2,5>3.1) (règles `sym>prod`, points entre degrés, sans règle = terminal)"),
    r("Génératif (maths)", "xys(n,racine,…)", "MARCHE DE MARKOV à la Xenakis : xys(16,1,1>2:2|3:1,2>1:1) — chaque note tire sa transition 1>a:p|b:q par son poids :p (seedée, évolue chaque mesure)"),
    r("Génératif (maths)", "evolve(n[,gen][,pop])", "MÉLODIE GÉNÉTIQUE : une population concourt (contour lisse + tonique/quinte + variété), élite + crossover + mutation 20% — seedée, évolue chaque mesure"),
    r("Génératif (maths)", "prime(n)  bin(n)", "frappes sur les positions premières · les chiffres binaires de n comme rythme (musique des nombres)"),
    r("Génératif (maths)", "pi(n)  golden(n)", "les décimales de π comme mélodie · rythme du nombre d'or (Beatty, quasi-périodique)"),
    r("Génératif (maths)", "collatz(n)  recaman(n)", "suite de Collatz (rythme) · suite de Recamán (mélodie aux sauts surprenants)"),
    r("Génératif (maths)", "logistic(n) cantor(n) trib(n)", "carte logistique (chaos→mélodie) · ensemble de Cantor (rythme fractal) · mot Tribonacci"),
    r("Génératif (maths)", "perlin(n)", "mélodie au bruit lisse (contour organique, fluide — au lieu des sauts d'une marche aléatoire)"),
    r("Structure", "loop: section", "ne boucle qu'une seule section du morceau (looping live d'une partie)"),
    r("Verbes", "| mirror   | accent N   | thin N", "reflet · accentue 1 pas/N · ne garde qu'1 pas/N (clairsemé)"),
    r("Aide", "?mot  (dans VIBE)", "recherche instantanée hors-ligne : ?dorian, ?reverb, ?euclid… explique le terme"),
    r("Théorie", "analyse auto", "au RUN, la console affiche l'analyse de ta progression en chiffres romains (ex. i VI III VII)"),
    r("Verbes", "| shuffle", "mélange (seedé) l'ordre des pas du motif"),
    r("Exporter", ".loom  /  📁", "télécharger le morceau en fichier .loom · ouvrir un fichier .loom"),
    r("Structure (long)", "scene@section: julia", "le VISUEL change avec la section du morceau (la scène suit la structure)"),
    r("Motif", "def nom: …   $nom", "MACRO : définis un fragment réutilisable, rappelle-le avec $nom n'importe où"),
    r("Génératif", "x?    x?70", "probabilité (~50% / 70%)"),
    r("Génératif", "| chance N", "la VOIX entière ne joue que N% des mesures (0-1) — variation d'arrangement (seedé)"),
    r("Effets & mix", "| tgate N", "trance gate : hache le son en N pulsations rythmiques par mesure (nappes pulsées)"),
    r("Génératif", "seed: 42", "rend l'aléatoire reproductible"),
    r("Génératif", "x*3", "ratchet / roll (N frappes dans un pas)"),
    r("Génératif", "<a/b/c>", "variation A/B : une alternative par mesure"),
    r("Structure", "song: intro*4 drop*8", "arrangement (nom*mesures ; boucle)"),
    r("Structure", "| in verse chorus", "la voix ne joue que dans ces sections"),
    r("Structure (long)", "nom@section: motif", "pattern différent par section → morceaux longs"),
    r("Structure (long)", "key@section: c minor", "MODULATION : cette section résout ses degrés/accords dans sa propre tonalité"),
    r("Son", "| saw sine tri pulse square", "forme d'onde"),
    r("Son", "| gain N", "volume de la voix (0-1)"),
    r("Son", "| oct N", "octave par défaut des degrés"),
    r("Son", "| lp N   | hp N   (q R)", "filtre passe-bas / passe-haut (+ résonance)"),
    r("Son", "| env A D S R", "enveloppe : attaque déclin sustain(0-1) release"),
    r("Son", "| unison N   | detune C", "super-saw (oscillateurs désaccordés)"),
    r("Son (FM)", "| fm RATIO INDEX", "synthèse FM : cloches, e-piano, basses grasses, métalliques (index 1-6)"),
    r("Son (wavetable)", "| wt organ|warm|rich|soft", "oscillateur à table d'ondes (timbre plus riche)"),
    r("Son (additif)", "| partials 1 0 0.5 0 0.3", "synthèse ADDITIVE : dessine l'amplitude de chaque harmonique (1=fond., 2=octave…)"),
    r("Son (additif)", "| organ 888650000", "orgue à TIRETTES (Hammond) : 9 chiffres 0-8 = niveau de chaque harmonique"),
    r("Son", "| drive X", "distorsion / chaleur (0-1)"),
    r("Son", "| lfo cutoff|pitch|gain|pan|reso RATE DEPTH [forme]", "modulation ; forme = sine/tri/saw/square (mouvement, vibrato, trémolo, wobble)"),
    r("Son", "| lfo2 …", "DEUXIÈME LFO indépendant (ex. wobble du filtre + vibrato en même temps)"),
    r("Automation", "| lp 300..6000   (| over N)", "rampe d'un paramètre dans le temps (filtre qui s'ouvre, riser)"),
    r("Automation", "| drive 0..0.6   | pan -1..1", "tout param A..B évolue sur la section (ou N mesures)"),
    r("Signaux", "| lp sine 0.5 300 4000", "un param ondule : WAVE(sine saw tri square rand) RATE(cycles/mesure) MIN MAX"),
    r("Effets & mix", "| pan P", "panoramique (-1 gauche … 1 droite)"),
    r("Effets & mix", "| reverb N   | delay N", "sends d'effet par voix (0-1)"),
    r("Effets & mix", "| echo DIV [FB]", "délai SYNCHRONISÉ au tempo : DIV échos par mesure (4=noire, 8=croche), FB=réinjection — dub/IDM"),
    r("Effets & mix", "| ir hall|room|plate|spring|cathedral [mix]", "réverbe à CONVOLUTION : un espace réel synthétisé, en insert par voix (mix 0-1, déf. 0.35)"),
    r("Effets & mix", "| eq L M H", "EQ 3 bandes par voix en dB (grave/médium/aigu)"),
    r("Effets & mix", "| chorus N", "chorus stéréo (0-1) — élargit/épaissit (nappes, leads)"),
    r("Effets & mix", "| crush BITS", "bitcrush 1-8 : grain lo-fi / IDM (plus bas = plus sale)"),
    r("Effets & mix", "| redux N", "réduction de fréquence d'échantillonnage (0-1, AudioWorklet) — lo-fi/digital crunch (≠ crush)"),
    r("Effets & mix", "duck: N", "sidechain : tout pompe sur le kick"),
    r("Groove", "feel: N", "humanise le timing/vélocité (0-100) — ça respire au lieu d'être mécanique"),
    r("Samples (drums)", "| sample bd", "bd sd hh cp kick2 kick909 clubkick clap2 oh rim cow perc tom tom2 tom3 ride tabla"),
    r("Sampler", "| chop N   | reverse", "beat-slice : joue 1/N de la boucle à chaque coup · reverse = lit le sample à l'envers"),
    r("Sampler", "| begin B   | end E", "ne joue que la région [B..E] du sample (0-1) — chops manuels, FX"),
    r("Effets & mix", "| autopan RATE", "panoramique automatique : un LFO balaie la voix de gauche à droite (Hz)"),
    r("Samples (mélo)", "| sample sitar", "bass bass1 arpy gtr casio sax sitar juno pad2 hoover stab bleep sid voodoo (repitchés)"),
    r("Samples (loops/voix)", "| sample amen", "breaks: amen break amencut · voix/texture: vox yeah breath wood metal glass bottle"),
    r("Samples", "| root NOTE", "accorde un sample mélodique à l'oreille"),
    r("Mes sons", "panneau « mes sons »", "dépose tes fichiers audio → | sample <ton_nom> (mémorisés)"),
    r("MIDI", "| midi CH", "envoie la voix en MIDI (drums → canal 10)"),
    r("Exporter", "REC   /   RENDER", "enregistrer en live / bouncer tout le morceau en WAV"),
    r("Exporter", "MIDI   /   LIEN", "télécharger un fichier .mid (pour ton DAW) · copier un lien de partage (le morceau est dans l'URL)"),
    r("Groove", "groove: dilla|mpc|swing16|push|laidback", "groove nommé : microtiming (le swing de Dilla, le MPC…) appliqué à tout"),
    r("Groove", "| cresc   | dim", "crescendo / diminuendo : la vélocité monte / descend au fil de la mesure"),
    r("Verbes", "| invert   | sequence N", "invert = mélodie en miroir ; sequence = motif suivi d'une copie transposée"),
    r("Verbes (génératif)", "| mutate N   | stutter N", "mutate = fait évoluer le motif chaque mesure (N% de changement) ; stutter = beat-repeat (boucle le 1/N)"),
    r("Son", "| tape N", "macro lo-fi cassette : wow + saturation + souffle en un seul réglage (0-1)"),
    r("IA & voix", "VIBE   /   🎤", "décris (ou parle) → l'IA écrit le code LOOM"),
    r("Apparence", "look ▸   /   🎲", "48 thèmes de terminal, mémorisés"),
    r("Son", "| gate N", "longueur des notes (0.05-4) : <1 staccato, >1 legato/tenu"),
    r("Son", "| slide N", "portamento : glisse depuis la note précédente (303). N = durée du glissé en s (déf. 0.06)"),
    r("Son", "| flam", "note de grâce douce juste avant chaque coup (fla de batterie, ornement)"),
    r("Son", "| strum N", "accord égrené : décale les notes d'un accord dans le temps (N s, guitare/harpe)"),
    r("Son", "| wow N", "wow & flutter : pleurage de bande/vinyle (0-1) — caractère vintage, lo-fi"),
    r("Son", "| orbit RATE", "spatialisation : la voix tourne autour de l'auditeur (Hz) — mouvement stéréo + distance"),
    r("Groove", "| humanize N", "humanisation par voix (0-100) : micro-décalages timing/vélocité, indépendant du feel global"),
    r("Live", "| mute  /  | solo", "coupe une voix / isole une voix (mixage en direct)"),
    r("Visuel (SCÈNE)", "scene: radial|tunnel|bars|starfield|kaleido|terrain|spiral|aurora|metaballs|flow|julia|plasma|mandelbrot|network|lissajous|waterfall|torus|fountain|helix|moire|fire|sunburst|matrix|orbits|globe|wave|harmono|clifford|cymatic|lorenz|dejong|rossler|superf|hopalong|rose|spiro|gumowski|tinker|ikeda|phyllo|reaction|truchet|warp|ship|julia4|maurer|epicycle|lissa3d|flower|honeycomb|cube|tesseract|knot|wormhole|mobius|pendulum|radialbars|henon|thomas|aizawa|fern|chen|voronoi|sierpinski|dadras|halvorsen|guilloche|lightning|metatron|tracker|chladni|string|spectro|scope|faraday|ripple|grid|petals|dna|mandala|spectrum3d|ribbon|fireworks|vortex|lattice|tiles|starsphere|bubbles|comet|shader|gtunnel|gfractal|gwarp|gtorus|gkaleid|gaurora|gcells|gwater|gmandel|nebula|scan|rings|checker", "choisit le visualiseur plein écran (touche N pour changer)"),
    r("Visuel (SCÈNE)", "| viz burst|ring|beam|flash|shape · | hue N", "apparence/couleur d'une voix dans la scène"),
    r("Mes morceaux", "SAVE / LOAD / DEL", "bibliothèque locale + code de partage LOOM1:"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_complete() {
        assert_eq!(LESSONS.len(), 57);
        assert_eq!(REFERENCE.len(), 166);
        for (i, l) in LESSONS.iter().enumerate() {
            assert!(l.title.starts_with(&format!("{} · ", i + 1)), "{}", l.title);
            assert!(!l.description.is_empty() && !l.code.is_empty());
        }
        assert!(REFERENCE.iter().any(|r| r.section == "Verbes (génératif)" && r.syntax.contains("mutate")));
        assert!(REFERENCE.iter().all(|r| !r.section.is_empty() && !r.syntax.is_empty() && !r.explanation.is_empty()));
    }

    #[test]
    fn every_lesson_parses_with_a_voice() {
        let broken: Vec<&str> = LESSONS
            .iter()
            .filter(|l| loom_core::Program::parse(l.code).voices.is_empty() || crate::vibe::validate(l.code).is_err())
            .map(|l| l.title)
            .collect();
        assert!(broken.is_empty(), "lessons that don't parse: {broken:?}");
    }

    #[test]
    fn search_ignores_case_and_accents() {
        assert_eq!(search("").lessons.len(), LESSONS.len());
        let found = search("MUTATE");
        assert!(found.lessons.iter().any(|l| l.title.starts_with("49 ")));
        assert!(found.reference.iter().any(|r| r.section == "Verbes (génératif)"));
        assert!(!search_lessons("generative").is_empty(), "accents ignored");
        assert!(search("zzzz introuvable").lessons.is_empty());
        assert_eq!(normalize("Élève Œuvre ç"), "eleve oeuvre c");
    }

    #[test]
    fn reference_groups_follow_the_order() {
        let all = search_reference("");
        let groups = by_section(&all);
        assert_eq!(groups[0].0, "Bases");
        assert_eq!(groups.iter().map(|g| g.1.len()).sum::<usize>(), REFERENCE.len());
    }
}
