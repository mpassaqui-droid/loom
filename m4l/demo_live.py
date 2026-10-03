#!/usr/bin/env python3
"""Construit le set de démo de Loom dans le Live ouvert (AbletonOSC requis).

    ./target/release/loom bake odyssey.loom /tmp/odyssey.json
    python3 m4l/demo_live.py /tmp/odyssey.json

VIDE le set ouvert (pistes et scènes), puis : une piste « Loom voix » par voix,
un clip par section, une scène par section avec son tempo (tools/bake_live.py),
un instrument Live par piste et le device Loom devant. Ne lance pas la lecture.
Il reste à enregistrer le set (Fichier > Enregistrer le Live Set sous).
"""
import json, os, sys, time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'tools'))
from bake_live import Live, construit  # noqa: E402

# premier nom trouvé dans le navigateur de Live (correspondance exacte, sinon sous-chaîne)
INSTRUMENTS = {
    'kick': ['909 Core Kit', 'Core Kit', 'Kit'],
    'snare': ['909 Core Kit', 'Core Kit', 'Kit'],
    'hat': ['909 Core Kit', 'Core Kit', 'Kit'],
    'keys': ['Grand Piano', 'Piano', 'Electric'],
    'bass': ['Basic Bass', 'Bass', 'Drift'],
    'lead': ['Lead', 'Drift'],
    'pad': ['Pad', 'Wavetable'],
}


def charge(live, piste, noms):
    live.envoie('/live/view/set/selected_track', piste); time.sleep(0.3)
    for nom in noms:
        r = live.demande('/live/browser/load_instrument', nom, timeout=30.0)
        if r and r[0] != 'NOT_FOUND':
            time.sleep(0.8)
            return r[0]
    return None


def vide(live):
    n = live.demande('/live/song/get/num_tracks')[0]
    for i in range(n - 1, 0, -1):                     # Live garde au moins une piste
        live.envoie('/live/song/delete_track', i); time.sleep(0.15)
    s = live.demande('/live/song/get/num_scenes')[0]
    for i in range(s - 1, 0, -1):
        live.envoie('/live/song/delete_scene', i); time.sleep(0.1)
    live.envoie('/live/clip_slot/delete_clip', 0, 0); time.sleep(0.1)


if __name__ == '__main__':
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    baked = json.load(open(sys.argv[1]))
    live = Live()
    live.envoie('/live/song/stop_playing')
    vide(live)
    bilan = construit(baked, 'Loom', live)
    live.envoie('/live/song/delete_track', 0); time.sleep(0.4)  # la piste restée du set d'avant
    for i, piste in enumerate(baked['tracks']):
        inst = charge(live, i, INSTRUMENTS.get(piste['name'], ['Drift']))
        dev = charge(live, i, ['Loom.amxd', 'Loom'])
        noms = live.demande('/live/track/get/devices/name', i)[1:]
        print(f"{piste['name']:<8} instrument={inst} loom={dev} devices={list(noms)}")
    faux = [b for b in bilan if b[2] != b[3]]
    print(f'{len(bilan)} clips, {sum(b[2] for b in bilan)} notes, {len(faux)} écart(s)')
    sys.exit(1 if faux else 0)
