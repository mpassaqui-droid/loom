#!/usr/bin/env python3
"""Assemble le produit « LOOM pour Live » dans dist/LOOM-pour-Live/ (+ zip).

    ./build-wasm.sh && python3 m4l/package.py

Contenu : le device, le mode d'emploi FR/EN, le script de cuisson complète
(bake_live.py) et quelques morceaux d'exemple. Rien n'est publié.
"""
import os, shutil, subprocess, sys

ICI = os.path.dirname(os.path.abspath(__file__))
RACINE = os.path.dirname(ICI)
DIST = os.path.join(RACINE, 'dist', 'LOOM-pour-Live')
EXEMPLES = ['autechre.loom', 'techno.loom', 'gamelan.loom', 'raga.loom', 'h_dub.loom', 'tools/gigue.loom']

subprocess.run([sys.executable, os.path.join(ICI, 'build_device.py')], check=True)
shutil.rmtree(DIST, ignore_errors=True)
os.makedirs(os.path.join(DIST, 'exemples'))
shutil.copy(os.path.join(ICI, 'LOOM.amxd'), os.path.join(DIST, 'LOOM pour Live.amxd'))
shutil.copy(os.path.join(ICI, 'LISEZ-MOI.md'), DIST)
shutil.copy(os.path.join(RACINE, 'tools', 'bake_live.py'), DIST)
for e in EXEMPLES:
    shutil.copy(os.path.join(RACINE, e), os.path.join(DIST, 'exemples', os.path.basename(e)))
archive = shutil.make_archive(DIST, 'zip', os.path.dirname(DIST), os.path.basename(DIST))
taille = sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(DIST) for f in fs)
print(f'{DIST} ({taille // 1024} Ko) · {archive} ({os.path.getsize(archive) // 1024} Ko)')
