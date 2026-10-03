#!/usr/bin/env python3
"""Pont local du device Loom : sert le VRAI Loom (web/) à la fenêtre du device,
et fait passer VIBE par `claude -p` (l'abonnement, pas l'API).

    python3 tools/loom_pont_ia.py            # http://127.0.0.1:7410/

Le device Loom, s'il n'a pas de clé API, envoie sa requête ici (même format que
l'API Messages d'Anthropic) ; le pont lance `claude -p` SANS AUCUN OUTIL, sans
MCP, sans réglages, et rend la réponse au même format. Pour vendre Loom, on
passe à la clé API : le device n'a rien d'autre à changer.

Sécurité :
- n'écoute que 127.0.0.1 ;
- jeton secret dans ~/Library/Preferences/loom_pont_jeton (dossier fermé aux
  autres comptes), envoyé par le device dans l'en-tête X-Loom-Jeton : une page
  web ou un autre compte ne peut pas s'en servir ;
- `claude -p --tools ""` : le modèle ne peut ni lire, ni écrire, ni exécuter ;
- ANTHROPIC_API_KEY retiré de l'environnement : c'est l'abonnement qui paie.
"""
import hmac, json, mimetypes, os, secrets, subprocess, sys, tempfile, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HOTE, PORT = '127.0.0.1', int(os.environ.get('LOOM_PONT_PORT', '7410'))
JETON_FICHIER = os.path.expanduser('~/Library/Preferences/loom_pont_jeton')
TAILLE_MAX = 512 * 1024
WEB = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'web')
HOTES_OK = {f'127.0.0.1:{PORT}', f'localhost:{PORT}'}
DELAI = 180
VERROU = threading.Lock()          # une composition à la fois


def jeton():
    if os.path.exists(JETON_FICHIER):
        t = open(JETON_FICHIER).read().strip()
        if len(t) >= 32:
            return t
    t = secrets.token_hex(24)
    fd = os.open(JETON_FICHIER, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, 'w') as f:
        f.write(t)
    return t


def compose(requete, executable='claude'):
    """Requête au format API Messages -> texte de la réponse de `claude -p`."""
    systeme = str(requete.get('system') or '')
    messages = requete.get('messages') or []
    demande = '\n\n'.join(str(m.get('content', '')) for m in messages if m.get('role') == 'user')
    if not demande.strip():
        raise ValueError('requête vide')
    modele = str(requete.get('model') or 'opus')
    env = {k: v for k, v in os.environ.items() if k != 'ANTHROPIC_API_KEY'}
    commande = [executable, '-p', '--tools', '', '--strict-mcp-config', '--disable-slash-commands',
                '--no-session-persistence', '--setting-sources', '', '--model', modele,
                '--system-prompt', systeme]
    with tempfile.TemporaryDirectory() as vide:     # aucun fichier du Mac à portée
        r = subprocess.run(commande, input=demande, capture_output=True, text=True,
                           timeout=DELAI, cwd=vide, env=env)
    if r.returncode != 0:
        raise RuntimeError((r.stderr or r.stdout or 'claude -p a échoué').strip()[:300])
    return r.stdout.strip()


class Pont(BaseHTTPRequestHandler):
    attendu = ''
    executable = 'claude'
    hotes = HOTES_OK

    def _rend(self, code, corps):
        b = json.dumps(corps, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def _hote_ok(self):
        # contre le « DNS rebinding » : une page d'un autre site qui se ferait passer pour 127.0.0.1
        return self.headers.get('Host', '') in self.hotes

    def do_POST(self):
        if not self._hote_ok():
            return self._rend(403, {'error': {'message': 'hôte refusé'}})
        if self.path != '/v1/messages':
            return self._rend(404, {'error': {'message': 'chemin inconnu'}})
        recu = self.headers.get('X-Loom-Jeton', '')
        if not hmac.compare_digest(recu, self.attendu):
            return self._rend(403, {'error': {'message': 'jeton du pont refusé'}})
        n = int(self.headers.get('Content-Length') or 0)
        if n <= 0 or n > TAILLE_MAX:
            return self._rend(413, {'error': {'message': 'requête trop grosse ou vide'}})
        try:
            requete = json.loads(self.rfile.read(n))
        except ValueError:
            return self._rend(400, {'error': {'message': 'JSON illisible'}})
        if not VERROU.acquire(blocking=False):
            return self._rend(429, {'error': {'message': 'une composition est déjà en cours'}})
        try:
            texte = compose(requete, self.executable)
            return self._rend(200, {'content': [{'type': 'text', 'text': texte}]})
        except subprocess.TimeoutExpired:
            return self._rend(504, {'error': {'message': f'claude -p trop long (> {DELAI} s)'}})
        except Exception as e:
            return self._rend(500, {'error': {'message': str(e)}})
        finally:
            VERROU.release()

    def do_GET(self):
        if not self._hote_ok():
            return self._rend(403, {'error': {'message': 'hôte refusé'}})
        chemin = self.path.split('?')[0].split('#')[0]
        if chemin == '/jeton':
            # même origine seulement : sans en-tête CORS, une autre page ne peut pas lire la réponse
            return self._rend(200, {'jeton': self.attendu})
        if chemin == '/':
            chemin = '/index.html'
        vrai = os.path.realpath(os.path.join(WEB, chemin.lstrip('/')))
        if not vrai.startswith(os.path.realpath(WEB) + os.sep) or not os.path.isfile(vrai):
            return self._rend(404, {'error': {'message': 'introuvable'}})
        b = open(vrai, 'rb').read()
        type_ = mimetypes.guess_type(vrai)[0] or 'application/octet-stream'
        if vrai.endswith('.wasm'):
            type_ = 'application/wasm'
        self.send_response(200)
        self.send_header('Content-Type', type_)
        self.send_header('Content-Length', str(len(b)))
        self.send_header('Cache-Control', 'no-store')
        self.end_headers()
        self.wfile.write(b)

    def log_message(self, fmt, *args):
        sys.stderr.write('loom-pont · ' + (fmt % args) + '\n')


def serveur(executable='claude', port=PORT):
    Pont.attendu = jeton()
    Pont.executable = executable
    s = ThreadingHTTPServer((HOTE, port), Pont)
    Pont.hotes = {f'127.0.0.1:{s.server_port}', f'localhost:{s.server_port}'}
    return s


if __name__ == '__main__':
    s = serveur()
    print(f'loom-pont · http://{HOTE}:{s.server_port} · jeton dans {JETON_FICHIER}', flush=True)
    s.serve_forever()
