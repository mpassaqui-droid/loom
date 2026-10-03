#!/usr/bin/env python3
"""Fabrique le device Max for Live « LOOM.amxd » (MIDI Effect).

    ./build-wasm.sh && python3 m4l/build_device.py [--installe] [--test]
    node m4l/test_device.js       (le device simulé hors de Max : cuisson = loom bake)

Tout tient dans un seul fichier : le code du device (m4l/loom_device.js), la colle
wasm-bindgen (web/pkg/loom_wasm.js, débarrassée de ses `export` et de
`import.meta`) et le moteur loom-core en WASM, encodé en base64, dans un
[v8.codebox]. --installe copie le device dans la User Library de Live
(Presets/MIDI Effects/Max MIDI Effect), là où le navigateur de Live le voit.
--test ajoute le port UDP de contrôle (7402 entrée, 7401 sortie), jamais livré.
"""
import base64, os, re, shutil, sys

ICI = os.path.dirname(os.path.abspath(__file__))
RACINE = os.path.dirname(ICI)
sys.path.insert(0, ICI)
import amxd  # noqa: E402

SORTIE = os.path.join(ICI, 'LOOM.amxd')
BIBLIO = os.path.expanduser('~/Music/Ableton/User Library/Presets/MIDI Effects/Max MIDI Effect')


def colle():
    js = open(os.path.join(RACINE, 'web/pkg/loom_wasm.js')).read()
    js = js[:js.index('async function __wbg_init')]          # import.meta et fetch : pas dans Max
    js = re.sub(r'^export (function|class|const|let)', r'\1', js, flags=re.M)
    js = re.sub(r'^export \{[^}]*\};?\s*$', '', js, flags=re.M)
    assert 'import.meta' not in js and not re.search(r'^\s*export\b', js, re.M)
    return js


def consigne_ia():
    # la consigne SYS de l'éditeur web, telle quelle (une seule source)
    s = open(os.path.join(RACINE, 'web/index.html')).read()
    i = s.index('const SYS=[') + len('const SYS=')
    d, k, q, esc = 0, i, None, False
    while True:
        c = s[k]
        if q:
            if esc: esc = False
            elif c == '\\': esc = True
            elif c == q: q = None
        elif c in '"\'`': q = c
        elif c == '[': d += 1
        elif c == ']':
            d -= 1
            if d == 0: break
        k += 1
    return 'var SYS = ' + s[i:k + 1] + ';'


def code():
    wasm = open(os.path.join(RACINE, 'web/pkg/loom_wasm_bg.wasm'), 'rb').read()
    return '\n'.join([
        open(os.path.join(ICI, 'loom_device.js')).read(),
        '// ---- consigne de l\'IA, tirée de web/index.html (générée) ----',
        consigne_ia(),
        '// ---- colle wasm-bindgen (générée, ne pas éditer) ----',
        colle(),
        'var LOOM_WASM = "' + base64.b64encode(wasm).decode() + '";',
        '',
    ])


def vue_code():
    return open(os.path.join(ICI, 'loom_vue.js')).read()


def v8ui(id, rect, mode, presentation_rect=None):
    b = boite(id, 'v8ui', rect, None, 1, 2, ['', ''], parameter_enable=0, filename='loom_vue.js', embed=1,
              jsarguments=[mode],
              textfile={'filename': 'loom_vue.js', 'flags': 0, 'embed': 1, 'autowatch': 0, 'text': vue_code()})
    if presentation_rect:
        b.update(presentation=1, presentation_rect=presentation_rect)
    return b


def boite(id, classe, rect, texte=None, ins=1, outs=0, types=None, **attrs):
    b = {'id': id, 'maxclass': classe, 'numinlets': ins, 'numoutlets': outs, 'patching_rect': rect}
    if texte is not None:
        b['text'] = texte
    if types is not None:
        b['outlettype'] = types
    b.update(attrs)
    return b




def bouton(id, rect, texte, nom):
    return boite(id, 'live.text', rect, None, 1, 2, ['', ''],
                 presentation=1, presentation_rect=rect, mode=0, text=texte, texton=texte,
                 parameter_enable=1, varname=nom,
                 saved_attribute_attributes={'valueof': {'parameter_longname': nom, 'parameter_shortname': nom,
                                                         'parameter_type': 2, 'parameter_enum': ['val1', 'val2'],
                                                         'parameter_mmax': 1}})


URL_LOOM = 'http://127.0.0.1:7410/?live=1'


def fenetre():
    """La fenêtre Loom, comme l'interface d'un plugin : le VRAI Loom (web/index.html), le même
    que sur le web, servi par le pont local (tools/loom_pont_ia.py) dans le navigateur de Max.
    Elle parle au moteur par send/receive propres au device (« --- » devient l'id du device) :
    la page envoie « editeur <code> », « lancer », « arreter », « pret » ; le moteur lui renvoie
    le code du morceau par « code <texte> »."""
    W, H = 1280.0, 800.0
    bx = [
        boite('w-1', 'inlet', [1400, 10, 30, 30], None, 0, 1, ['']),
        boite('w-2', 'jweb', [0, 0, W, H], None, 1, 1, [''], presentation=1, presentation_rect=[0, 0, W, H],
              url=URL_LOOM, rendermode=0),
        boite('w-3', 'newobj', [1400, 60, 200, 22], 'route editeur lancer arreter pret', 1, 5, ['', '', '', '', '']),
        boite('w-4', 'newobj', [1400, 90, 100, 22], 'prepend editeur', 1, 1, ['']),
        boite('w-5', 'newobj', [1400, 120, 90, 22], 's ---loomin', 1, 0),
        boite('w-6', 'message', [1500, 90, 50, 22], 'lancer', 2, 1, ['']),
        boite('w-7', 'message', [1560, 90, 50, 22], 'arreter', 2, 1, ['']),
        boite('w-8', 'message', [1620, 90, 40, 22], 'pret', 2, 1, ['']),
        boite('w-9', 'newobj', [1400, 160, 90, 22], 'r ---editeur', 0, 1, ['']),
        boite('w-10', 'newobj', [1400, 190, 70, 22], 'route set', 1, 2, ['', '']),
        boite('w-11', 'newobj', [1400, 220, 90, 22], 'prepend code', 1, 1, ['']),
        # le pont pas lancé : on recharge la page à chaque ouverture de la fenêtre
        boite('w-12', 'message', [1500, 220, 200, 22], 'url ' + URL_LOOM, 2, 1, ['']),
        boite('w-13', 'newobj', [1400, 40, 30, 22], 't b', 1, 1, ['bang']),
    ]
    li = [
        ('w-2', 0, 'w-3', 0), ('w-3', 0, 'w-4', 0), ('w-4', 0, 'w-5', 0),
        ('w-3', 1, 'w-6', 0), ('w-6', 0, 'w-5', 0),
        ('w-3', 2, 'w-7', 0), ('w-7', 0, 'w-5', 0),
        ('w-3', 3, 'w-8', 0), ('w-8', 0, 'w-5', 0),
        ('w-9', 0, 'w-10', 0), ('w-10', 0, 'w-11', 0), ('w-11', 0, 'w-2', 0),
        ('w-1', 0, 'w-13', 0), ('w-13', 0, 'w-12', 0), ('w-12', 0, 'w-2', 0),
    ]
    p = amxd.patcher(bx, li, 'Loom', largeur=W)
    p.update(rect=[60.0, 60.0, W, H], openrect=[0.0, 0.0, W, H], toolbarvisible=0, statusbarvisible=0,
             enablehscroll=0, enablevscroll=0, devicewidth=0.0, openinpresentation=1,
             bgcolor=[0.03, 0.04, 0.03, 1.0], editing_bgcolor=[0.03, 0.04, 0.03, 1.0])
    return p


def patch(test=False):
    boites = [
        boite('obj-1', 'newobj', [20, 20, 90, 22], 'live.thisdevice', 1, 3, ['bang', 'int', 'int']),
        boite('obj-2', 'v8.codebox', [20, 200, 420, 200], None, 1, 7, [''] * 7,
              code=code(), filename='none', fontname='Menlo', fontsize=10.0, fontface=0,
              saved_object_attributes={'parameter_enable': 0}),
        # la face du device : ouvrir Loom, le fichier (optionnel), l'état, le piano-roll
        bouton('obj-19', [6, 6, 150, 40], 'Ouvrir Loom', 'ouvrir'),
        boite('obj-20', 'newobj', [460, 280, 30, 22], 't b', 1, 1, ['bang']),
        boite('obj-23', 'message', [460, 300, 40, 22], 'open', 2, 1, ['']),
        boite('obj-3', 'live.drop', [140, 20, 160, 60], None, 1, 1, [''],
              presentation=1, presentation_rect=[6, 52, 150, 30],
              legend='fichier .loom (optionnel)',
              saved_attribute_attributes={'valueof': {'parameter_longname': 'LOOM fichier',
                                                      'parameter_shortname': 'fichier',
                                                      'parameter_type': 4}},
              parameter_enable=1, varname='LOOM fichier'),
        boite('obj-4', 'newobj', [140, 100, 90, 22], 'prepend fichier', 1, 1, ['']),
        boite('obj-5', 'newobj', [260, 100, 110, 22], 'udpreceive 7402', 1, 1, ['']),
        boite('obj-7', 'comment', [20, 480, 300, 40], 'Loom', 1, 0, None,
              presentation=1, presentation_rect=[6, 88, 150, 75], fontsize=9.0,
              linecount=5, textcolor=[0.9, 0.9, 0.9, 1.0]),
        boite('obj-8', 'newobj', [200, 440, 150, 22], 'udpsend 127.0.0.1 7401', 1, 0),
        # le chemin du fichier, sauvé avec le set (paramètre « blob », stocké seulement)
        boite('obj-11', 'newobj', [200, 480, 140, 22], 'pattr loomfichier', 1, 3, ['', '', ''],
              parameter_enable=1,
              saved_attribute_attributes={'valueof': {'parameter_longname': 'loomfichier',
                                                      'parameter_shortname': 'loomfichier',
                                                      'parameter_type': 3, 'parameter_invisible': 1}}),
        boite('obj-12', 'newobj', [200, 520, 100, 22], 'prepend restaure', 1, 1, ['']),
        v8ui('obj-13', [460, 200, 300, 157], 'mini', [162, 6, 300, 157]),
        # le code, sauvé avec le set
        boite('obj-21', 'newobj', [200, 600, 140, 22], 'pattr loomcode', 1, 3, ['', '', ''],
              parameter_enable=1,
              saved_attribute_attributes={'valueof': {'parameter_longname': 'loomcode',
                                                      'parameter_shortname': 'loomcode',
                                                      'parameter_type': 3, 'parameter_invisible': 1}}),
        boite('obj-22', 'newobj', [200, 640, 110, 22], 'prepend restaurecode', 1, 1, ['']),
        boite('obj-14', 'newobj', [460, 330, 60, 22], 'pcontrol', 1, 1, ['']),
        boite('obj-15', 'newobj', [460, 360, 80, 22], 'p Loom', 1, 0, None, patcher=fenetre()),
        boite('obj-16', 'newobj', [560, 310, 90, 22], 'prepend /loom/vue', 1, 1, ['']),
        # les liens avec la fenêtre (send/receive propres au device)
        boite('obj-24', 'newobj', [20, 420, 80, 22], 's ---etat', 1, 0),
        boite('obj-25', 'newobj', [20, 700, 90, 22], 's ---editeur', 1, 0),
        boite('obj-26', 'newobj', [20, 160, 90, 22], 'r ---loomin', 0, 1, ['']),
        boite('obj-27', 'newobj', [20, 740, 70, 22], 's ---cle', 1, 0),
        # l'IA : la requête HTTP part d'ici, la réponse revient au moteur
        boite('obj-28', 'newobj', [460, 420, 60, 22], 'maxurl', 1, 2, ['', '']),
        boite('obj-29', 'newobj', [460, 450, 110, 22], 'prepend reponseia', 1, 1, ['']),
        boite('obj-9', 'newobj', [400, 20, 50, 22], 'midiin', 1, 1, ['int']),
        boite('obj-10', 'newobj', [400, 440, 50, 22], 'midiout', 1, 0),
    ]
    lignes = [
        ('obj-1', 0, 'obj-2', 0),
        ('obj-3', 0, 'obj-4', 0), ('obj-4', 0, 'obj-2', 0),
        ('obj-5', 0, 'obj-2', 0),
        ('obj-2', 0, 'obj-7', 0), ('obj-2', 0, 'obj-24', 0),
        ('obj-2', 1, 'obj-8', 0),
        ('obj-9', 0, 'obj-10', 0),
        ('obj-2', 2, 'obj-11', 0), ('obj-11', 0, 'obj-12', 0), ('obj-12', 0, 'obj-2', 0),
        ('obj-19', 0, 'obj-20', 0), ('obj-20', 0, 'obj-23', 0), ('obj-23', 0, 'obj-14', 0),
        ('obj-13', 0, 'obj-14', 0), ('obj-14', 0, 'obj-15', 0), ('obj-23', 0, 'obj-15', 0),
        ('obj-2', 3, 'obj-21', 0), ('obj-21', 0, 'obj-22', 0), ('obj-22', 0, 'obj-2', 0),
        ('obj-2', 4, 'obj-25', 0),
        ('obj-26', 0, 'obj-2', 0),
        ('obj-2', 5, 'obj-28', 0), ('obj-28', 0, 'obj-29', 0), ('obj-29', 0, 'obj-2', 0),
        ('obj-2', 6, 'obj-27', 0),
        ('obj-13', 1, 'obj-16', 0), ('obj-16', 0, 'obj-8', 0),
    ]
    if not test:
        # le port UDP de contrôle n'existe que pour les tests : dans le device livré,
        # personne sur le réseau ne doit pouvoir charger un chemin ou du code
        hors = ('obj-5', 'obj-8', 'obj-16')
        boites = [b for b in boites if b['id'] not in hors]
        lignes = [l for l in lignes if l[0] not in hors and l[2] not in hors]
    return amxd.patcher(boites, lignes, 'Loom', largeur=468.0)


if __name__ == '__main__':
    if '--code' in sys.argv:                      # le JS seul, pour m4l/test_device.js
        open(sys.argv[sys.argv.index('--code') + 1], 'w').write(code())
        sys.exit(0)
    amxd.ecrit(SORTIE, patch('--test' in sys.argv), 'midi')
    print(SORTIE, os.path.getsize(SORTIE) // 1024, 'Ko')
    if '--installe' in sys.argv:
        os.makedirs(BIBLIO, exist_ok=True)
        shutil.copy(SORTIE, os.path.join(BIBLIO, 'Loom.amxd'))
        print('installé :', os.path.join(BIBLIO, 'Loom.amxd'))
