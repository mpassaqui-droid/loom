#!/usr/bin/env python3
"""Test du pont IA avec un faux `claude` (aucun appel réel) : python3 tools/loom_pont_ia_test.py"""
import json, os, stat, sys, tempfile, threading, urllib.request, urllib.error

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import loom_pont_ia as pont  # noqa: E402

echecs = 0


def ok(c, m):
    global echecs
    print(('✓ ' if c else '✗ ') + m)
    echecs += 0 if c else 1


faux = os.path.join(tempfile.mkdtemp(), 'claude')
with open(faux, 'w') as f:
    f.write('#!/usr/bin/env python3\n'
            'import sys, os, json\n'
            'a = sys.argv[1:]\n'
            'i = a.index("--tools")\n'
            'print(json.dumps({"tools": a[i + 1], "cle": "ANTHROPIC_API_KEY" in os.environ,'
            ' "cwd_vide": os.listdir(".") == [], "stdin": sys.stdin.read(),'
            ' "systeme": a[a.index("--system-prompt") + 1], "modele": a[a.index("--model") + 1]}))\n')
os.chmod(faux, 0o755)

os.environ['ANTHROPIC_API_KEY'] = 'sk-ne-doit-pas-passer'
pont.JETON_FICHIER = os.path.join(tempfile.mkdtemp(), 'loom_pont_jeton')   # jamais le vrai jeton du Mac
s = pont.serveur(faux, port=0)
threading.Thread(target=s.serve_forever, daemon=True).start()
url = f'http://127.0.0.1:{s.server_port}/v1/messages'
jeton = open(pont.JETON_FICHIER).read().strip()


def post(corps, j=jeton, chemin=url):
    data = corps if isinstance(corps, bytes) else json.dumps(corps).encode()
    req = urllib.request.Request(chemin, data=data, method='POST',
                                 headers={'Content-Type': 'application/json', 'X-Loom-Jeton': j})
    try:
        r = urllib.request.urlopen(req, timeout=10)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read())


mode = stat.S_IMODE(os.stat(pont.JETON_FICHIER).st_mode)
ok(mode == 0o600 and len(jeton) >= 32, f'jeton {len(jeton)} car., fichier en {oct(mode)}')
requete = {'model': 'claude-fable-5-1', 'system': 'CONSIGNE', 'max_tokens': 10,
           'messages': [{'role': 'user', 'content': 'Current LOOM code:\nkick: x\n\nRequest: plus'}]}
code, rep = post(requete, j='mauvais')
ok(code == 403, 'mauvais jeton : 403')
code, rep = post(requete)
dedans = json.loads(rep['content'][0]['text']) if code == 200 else {}
ok(code == 200 and rep['content'][0]['type'] == 'text', 'bon jeton : réponse au format de l\'API Messages')
ok(dedans.get('tools') == '' and not dedans.get('cle') and dedans.get('cwd_vide'),
   'claude -p sans aucun outil, sans ANTHROPIC_API_KEY, dans un dossier vide')
ok(dedans.get('systeme') == 'CONSIGNE' and dedans.get('modele') == 'claude-fable-5-1' and 'kick: x' in dedans.get('stdin', ''),
   'consigne, modèle et demande transmis')
code, _ = post(b'x' * (pont.TAILLE_MAX + 1))
ok(code == 413, 'requête trop grosse : 413')
code, _ = post(requete, chemin=url.replace('/v1/messages', '/autre'))
ok(code == 404, 'autre chemin : 404')
def get(chemin, hote=None):
    req = urllib.request.Request(url.replace('/v1/messages', chemin), headers={'Host': hote} if hote else {})
    try:
        r = urllib.request.urlopen(req, timeout=10)
        return r.status, r.headers.get('Content-Type'), r.read()
    except urllib.error.HTTPError as e:
        return e.code, '', e.read()


code, typ, corps = get('/')
ok(code == 200 and b'LOOM' in corps and 'html' in typ, 'sert le vrai Loom (web/index.html)')
code, typ, _ = get('/pkg/loom_wasm_bg.wasm')
ok(code == 200 and typ == 'application/wasm', 'sert le moteur WASM en application/wasm')
code, _, _ = get('/../tools/loom_pont_ia.py')
code2, _, _ = get('/%2e%2e/tools/loom_pont_ia.py')
ok(code == 404 and code2 == 404, 'rien hors de web/ (../ refusé)')
code, _, _ = get('/jeton')
ok(code == 404, 'le jeton ne sort jamais par HTTP (/jeton : 404)')
import hashlib  # noqa: E402
code, _, corps = get('/preuve?n=0123456789abcdef0123456789abcdef')
ok(code == 200 and json.loads(corps)['preuve'] == hashlib.sha256(f'{jeton}:0123456789abcdef0123456789abcdef'.encode()).hexdigest(),
   'preuve = sha256(jeton:nombre), sans livrer le jeton')
code, _, _ = get('/preuve?n=zz')
ok(code == 400, 'preuve sans nombre valable : 400')
code, _, _ = get('/preuve?n=0123456789abcdef', hote='evil.example:7410')
ok(code == 403, 'autre hôte (DNS rebinding) : 403')
s2 = pont.serveur(faux, port=0)
ok(open(pont.JETON_FICHIER).read().strip() != jeton, 'jeton neuf à chaque lancement')
s2.server_close()
s.shutdown()
print(f'{echecs} échec(s)' if echecs else 'pont : tout passe')
sys.exit(1 if echecs else 0)
