"""Écrit un device Max for Live (.amxd) à partir d'un patcher Max (dict JSON).

Format observé sur les devices d'usine de Live 12 : en-tête « ampf », type de
device sur 4 octets (mmmm = MIDI Effect, aaaa = Audio Effect, iiii = Instrument),
bloc « meta » vide, puis « ptch » + taille + le patcher JSON.
"""
import json, struct

TYPES = {'midi': b'mmmm', 'audio': b'aaaa', 'instrument': b'iiii'}


def ecrit(chemin, patcher, genre='midi'):
    corps = json.dumps({'patcher': patcher}, indent=1, ensure_ascii=False).encode('utf-8') + b'\n\0'
    with open(chemin, 'wb') as f:
        f.write(b'ampf' + struct.pack('<I', 4) + TYPES[genre])
        f.write(b'meta' + struct.pack('<I', 4) + b'\0\0\0\0')
        f.write(b'ptch' + struct.pack('<I', len(corps)) + corps)


def lit(chemin):
    b = open(chemin, 'rb').read()
    assert b[:4] == b'ampf', 'pas un .amxd'
    i = b.index(b'ptch')
    n = struct.unpack('<I', b[i + 4:i + 8])[0]
    return b[8:12], json.loads(b[i + 8:i + 8 + n].rstrip(b'\0\n').decode('utf-8'))['patcher']


def patcher(boites, lignes, titre, largeur=400.0):
    return {
        'fileversion': 1,
        'appversion': {'major': 9, 'minor': 1, 'revision': 5, 'architecture': 'x64', 'modernui': 1},
        'classnamespace': 'box', 'rect': [100.0, 100.0, 900.0, 600.0],
        'openrect': [0.0, 0.0, largeur, 169.0], 'openinpresentation': 1,
        'default_fontsize': 10.0, 'default_fontface': 0, 'default_fontname': 'Arial Bold',
        'gridonopen': 1, 'gridsize': [8.0, 8.0], 'gridsnaponopen': 1, 'objectsnaponopen': 1,
        'statusbarvisible': 2, 'toolbarvisible': 1, 'devicewidth': largeur,
        'description': '', 'digest': '', 'tags': '', 'style': '', 'subpatcher_template': '',
        'title': titre, 'boxes': [{'box': b} for b in boites],
        'lines': [{'patchline': {'source': [a, o], 'destination': [b, i]}} for a, o, b, i in lignes],
        'dependency_cache': [], 'autosave': 0,
    }
