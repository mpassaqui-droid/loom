#!/usr/bin/env python3
"""Assemble le produit « Loom » dans dist/Loom/ (+ zip).

    ./build-wasm.sh && python3 m4l/package.py

Contenu : le device, la fenêtre Loom (web/ + le pont local), l'installeur
(installe.command), le mode d'emploi, le script de cuisson complète (bake_live.py)
et quelques morceaux d'exemple. Rien n'est publié.
"""
import os, shutil, subprocess, sys

ICI = os.path.dirname(os.path.abspath(__file__))
RACINE = os.path.dirname(ICI)
DIST = os.path.join(RACINE, 'dist', 'Loom')
EXEMPLES = ['odyssey.loom', 'autechre.loom', 'techno.loom', 'gamelan.loom', 'raga.loom', 'h_dub.loom', 'tools/gigue.loom']

subprocess.run([sys.executable, os.path.join(ICI, 'build_device.py')], check=True)
shutil.rmtree(DIST, ignore_errors=True)
os.makedirs(os.path.join(DIST, 'exemples'))
shutil.copy(os.path.join(ICI, 'LOOM.amxd'), os.path.join(DIST, 'Loom.amxd'))
shutil.copy(os.path.join(ICI, 'LISEZ-MOI.md'), DIST)
shutil.copy(os.path.join(RACINE, 'tools', 'bake_live.py'), DIST)
# la fenêtre Loom : la page web (sans les fichiers de travail) et son pont local
shutil.copytree(os.path.join(RACINE, 'web'), os.path.join(DIST, 'web'),
                ignore=shutil.ignore_patterns('.DS_Store', '*.d.ts', 'package.json', '.gitignore'))
os.makedirs(os.path.join(DIST, 'pont'))
shutil.copy(os.path.join(RACINE, 'tools', 'loom_pont_ia.py'), os.path.join(DIST, 'pont'))
shutil.copy(os.path.join(ICI, 'installe.command'), DIST)
os.chmod(os.path.join(DIST, 'installe.command'), 0o755)
for e in EXEMPLES:
    shutil.copy(os.path.join(RACINE, e), os.path.join(DIST, 'exemples', os.path.basename(e)))
# le set de démo (m4l/demo_live.py, puis enregistré dans Live sous m4l/demo/), s'il existe
DEMO = os.path.join(ICI, 'demo')
if os.path.isdir(DEMO):
    shutil.copytree(DEMO, os.path.join(DIST, 'demo'), ignore=shutil.ignore_patterns('Backup', '.DS_Store'))
archive = shutil.make_archive(DIST, 'zip', os.path.dirname(DIST), os.path.basename(DIST))
taille = sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(DIST) for f in fs)
print(f'{DIST} ({taille // 1024} Ko) · {archive} ({os.path.getsize(archive) // 1024} Ko)')
