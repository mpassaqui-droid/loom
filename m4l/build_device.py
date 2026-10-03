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


def code():
    wasm = open(os.path.join(RACINE, 'web/pkg/loom_wasm_bg.wasm'), 'rb').read()
    return '\n'.join([
        open(os.path.join(ICI, 'loom_device.js')).read(),
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


def scene():
    # la fenêtre de la scène : un sous-patch avec le v8ui en mode « scene »
    W, H = 960.0, 540.0
    p = amxd.patcher([
        boite('obj-1', 'inlet', [10, 10, 30, 30], None, 0, 1, ['']),
        v8ui('obj-2', [0, 0, W, H], 'scene', [0, 0, W, H]),
    ], [], 'Loom · scène', largeur=W)
    p.update(rect=[120.0, 120.0, W, H], openrect=[0.0, 0.0, W, H], toolbarvisible=0, statusbarvisible=0,
             enablehscroll=0, enablevscroll=0, devicewidth=0.0)
    return p


def boite(id, classe, rect, texte=None, ins=1, outs=0, types=None, **attrs):
    b = {'id': id, 'maxclass': classe, 'numinlets': ins, 'numoutlets': outs, 'patching_rect': rect}
    if texte is not None:
        b['text'] = texte
    if types is not None:
        b['outlettype'] = types
    b.update(attrs)
    return b


def patch(test=False):
    boites = [
        boite('obj-1', 'newobj', [20, 20, 90, 22], 'live.thisdevice', 1, 3, ['bang', 'int', 'int']),
        boite('obj-2', 'v8.codebox', [20, 200, 420, 200], None, 1, 3, ['', '', ''],
              code=code(), filename='none', fontname='Menlo', fontsize=10.0, fontface=0,
              saved_object_attributes={'parameter_enable': 0}),
        boite('obj-3', 'live.drop', [140, 20, 160, 60], None, 1, 1, [''],
              presentation=1, presentation_rect=[6, 6, 150, 56],
              legend='Glisse un fichier .loom',
              saved_attribute_attributes={'valueof': {'parameter_longname': 'LOOM fichier',
                                                      'parameter_shortname': 'fichier',
                                                      'parameter_type': 4}},
              parameter_enable=1, varname='LOOM fichier'),
        boite('obj-4', 'newobj', [140, 100, 90, 22], 'prepend fichier', 1, 1, ['']),
        boite('obj-5', 'newobj', [260, 100, 110, 22], 'udpreceive 7402', 1, 1, ['']),
        boite('obj-6', 'newobj', [20, 440, 70, 22], 'prepend set', 1, 1, ['']),
        boite('obj-7', 'comment', [20, 480, 300, 40], 'Loom', 1, 0, None,
              presentation=1, presentation_rect=[162, 6, 230, 56], fontsize=11.0,
              linecount=3, textcolor=[0.9, 0.9, 0.9, 1.0]),
        boite('obj-8', 'newobj', [200, 440, 150, 22], 'udpsend 127.0.0.1 7401', 1, 0),
        # le chemin du fichier, sauvé avec le set (paramètre « blob », stocké seulement)
        boite('obj-11', 'newobj', [200, 480, 140, 22], 'pattr loomfichier', 1, 3, ['', '', ''],
              parameter_enable=1,
              saved_attribute_attributes={'valueof': {'parameter_longname': 'loomfichier',
                                                      'parameter_shortname': 'loomfichier',
                                                      'parameter_type': 3, 'parameter_invisible': 1}}),
        boite('obj-12', 'newobj', [200, 520, 100, 22], 'prepend restaure', 1, 1, ['']),
        # les visuels : piano-roll dans le device, un clic ouvre la scène
        v8ui('obj-13', [460, 200, 388, 96], 'mini', [6, 66, 388, 96]),
        boite('obj-14', 'newobj', [460, 310, 60, 22], 'pcontrol', 1, 1, ['']),
        boite('obj-15', 'newobj', [460, 340, 80, 22], 'p scène', 1, 0, None, patcher=scene()),
        boite('obj-16', 'newobj', [560, 310, 90, 22], 'prepend /loom/vue', 1, 1, ['']),
        boite('obj-9', 'newobj', [400, 20, 50, 22], 'midiin', 1, 1, ['int']),
        boite('obj-10', 'newobj', [400, 440, 50, 22], 'midiout', 1, 0),
    ]
    # obj-6 « prepend set » est inutile (le device envoie déjà « set ... ») : sortie 0 → commentaire direct
    lignes = [
        ('obj-1', 0, 'obj-2', 0),
        ('obj-3', 0, 'obj-4', 0), ('obj-4', 0, 'obj-2', 0),
        ('obj-5', 0, 'obj-2', 0),
        ('obj-2', 0, 'obj-7', 0),
        ('obj-2', 1, 'obj-8', 0),
        ('obj-9', 0, 'obj-10', 0),
        ('obj-2', 2, 'obj-11', 0), ('obj-11', 0, 'obj-12', 0), ('obj-12', 0, 'obj-2', 0),
        ('obj-13', 0, 'obj-14', 0), ('obj-14', 0, 'obj-15', 0),
        ('obj-13', 1, 'obj-16', 0), ('obj-16', 0, 'obj-8', 0),
    ]
    boites = [b for b in boites if b['id'] != 'obj-6']
    if not test:
        # le port UDP de contrôle n'existe que pour les tests : dans le device livré,
        # personne sur le réseau ne doit pouvoir charger un chemin ou du code
        hors = ('obj-5', 'obj-8', 'obj-16')
        boites = [b for b in boites if b['id'] not in hors]
        lignes = [l for l in lignes if l[0] not in hors and l[2] not in hors]
    return amxd.patcher(boites, lignes, 'Loom', largeur=400.0)


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
