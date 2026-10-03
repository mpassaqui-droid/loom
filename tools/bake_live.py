#!/usr/bin/env python3
"""Cuire un morceau LOOM dans Ableton Live (via AbletonOSC, ports 11000/11001).

    ./target/release/loom bake morceau.loom /tmp/m.json
    python3 tools/bake_live.py /tmp/m.json [--prefixe LOOM]

Ce que ça construit, sans jamais lancer la lecture :
  - une piste MIDI par voix LOOM, nommée « LOOM nom » ;
  - une scène par section du `song:`, nommée « section 120 BPM » : lancer la
    scène règle le tempo de Live (convention de Live 12), et chaque piste y a
    un clip avec les notes de la section ;
  - sans `song:`, une seule scène avec tout le morceau ;
  - l'accordage (`tune:`) écrit dans User Library/Tunings, à glisser sur le set.
Puis relit chaque clip et vérifie que le nombre de notes est exact.

Python standard uniquement (client OSC minimal intégré).
"""
import json, os, socket, struct, sys, time

HOTE, ENVOI, RETOUR = '127.0.0.1', 11000, 11001


def _pad(b):
    return b + b'\0' * (4 - len(b) % 4)


def _encode(addr, args):
    tags, data = ',', b''
    for a in args:
        if isinstance(a, bool):
            a = int(a)
        if isinstance(a, int):
            tags += 'i'; data += struct.pack('>i', a)
        elif isinstance(a, float):
            tags += 'f'; data += struct.pack('>f', a)
        else:
            tags += 's'; data += _pad(str(a).encode())
    return _pad(addr.encode()) + _pad(tags.encode()) + data


def _decode(pkt):
    def chaine(i):
        j = pkt.index(b'\0', i)
        return pkt[i:j].decode(), (j // 4 + 1) * 4
    addr, i = chaine(0)
    tags, i = chaine(i)
    out = []
    for t in tags[1:]:
        if t == 'i':
            out.append(struct.unpack('>i', pkt[i:i + 4])[0]); i += 4
        elif t == 'f':
            out.append(struct.unpack('>f', pkt[i:i + 4])[0]); i += 4
        elif t == 's':
            s, i = chaine(i); out.append(s)
        elif t in 'TF':
            out.append(t == 'T')
    return addr, out


class Live:
    def __init__(self):
        self.tx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.rx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.rx.bind((HOTE, RETOUR))

    def envoie(self, addr, *args):
        self.tx.sendto(_encode(addr, args), (HOTE, ENVOI))

    def demande(self, addr, *args, timeout=5.0):
        self.rx.settimeout(0.0)
        try:
            while True:
                self.rx.recvfrom(65536)
        except (BlockingIOError, socket.timeout):
            pass
        self.envoie(addr, *args)
        fin = time.time() + timeout
        while time.time() < fin:
            self.rx.settimeout(max(0.01, fin - time.time()))
            try:
                a, out = _decode(self.rx.recvfrom(65536)[0])
            except socket.timeout:
                break
            if a == addr:
                return out
        raise TimeoutError(f'Live ne répond pas à {addr} (AbletonOSC chargé ?)')


def construit(baked, prefixe='LOOM', live=None):
    live = live or Live()
    live.demande('/live/song/get/tempo')                      # Live et AbletonOSC sont là
    beats = baked['meter_beats']
    sections = baked['sections'] or [['morceau', 0, baked['bars']]]
    noms = list(live.demande('/live/song/get/track_names'))
    base = len(noms)
    n_scenes = live.demande('/live/song/get/num_scenes')[0]
    while n_scenes < len(sections):
        live.envoie('/live/song/create_scene', -1); time.sleep(0.2); n_scenes += 1
    for s, (nom, debut, long) in enumerate(sections):
        bpm = baked['bar_bpm'][debut]
        live.envoie('/live/scene/set/name', s, f'{nom} {bpm:g} BPM')
    live.envoie('/live/song/set/tempo', float(baked['bar_bpm'][0]))
    bilan = []
    for k, piste in enumerate(baked['tracks']):
        i = base + k
        live.envoie('/live/song/create_midi_track', -1); time.sleep(0.4)
        live.envoie('/live/track/set/name', i, f"{prefixe} {piste['name']}")
        for s, (nom, debut, long) in enumerate(sections):
            b0, b1 = debut * beats, (debut + long) * beats
            notes = [n for n in piste['notes'] if b0 - 1e-6 <= n[1] < b1 - 1e-6]
            if not notes:
                continue
            live.envoie('/live/clip_slot/create_clip', i, s, float(b1 - b0)); time.sleep(0.25)
            live.envoie('/live/clip/set/name', i, s, f"{piste['name']} · {nom}")
            for j in range(0, len(notes), 100):
                flat = []
                for p, t, d, v in notes[j:j + 100]:
                    flat += [int(p), float(t - b0), float(min(d, b1 - t)), int(v), 0]
                live.envoie('/live/clip/add/notes', i, s, *flat); time.sleep(0.05)
            time.sleep(0.2)
            lu = live.demande('/live/clip/get/notes', i, s)
            recu = (len(lu) - 2) // 5
            bilan.append((piste['name'], nom, len(notes), recu))
    if baked.get('ascl'):
        dossier = os.path.expanduser('~/Music/Ableton/User Library/Tunings')
        os.makedirs(dossier, exist_ok=True)
        titre = baked['ascl'].splitlines()[0].lstrip('! ').strip()
        with open(os.path.join(dossier, titre), 'w') as f:
            f.write(baked['ascl'])
        print('accordage écrit :', os.path.join(dossier, titre), '(à glisser sur le set)')
    return bilan


if __name__ == '__main__':
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    prefixe = sys.argv[sys.argv.index('--prefixe') + 1] if '--prefixe' in sys.argv else 'LOOM'
    bilan = construit(json.load(open(sys.argv[1])), prefixe)
    faux = [b for b in bilan if b[2] != b[3]]
    for nom, sec, attendu, recu in bilan:
        print(f'{nom:<14} {sec:<10} {recu:>5}/{attendu}')
    total = sum(b[2] for b in bilan)
    print(f'{len(bilan)} clips, {total} notes, {len(faux)} écart(s)')
    sys.exit(1 if faux else 0)
