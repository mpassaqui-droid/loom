#!/usr/bin/env python3
"""Recopie les thèmes du web (const THEMES de web/index.html) en Rust pour le terminal et la fenêtre.
Une seule source : relancer après avoir changé un thème du web.
    python3 tools/gen_themes.py
"""
import os, re
RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
s = open(os.path.join(RACINE, 'web/index.html')).read()
i = s.index('const THEMES={')
j = s.index('\n  };', i)
lignes = re.findall(r'^\s*([a-z0-9_]+):\["(#[0-9a-fA-F]{6})","(#[0-9a-fA-F]{6})","(#[0-9a-fA-F]{6})","(#[0-9a-fA-F]{6})","(#[0-9a-fA-F]{6})","(#[0-9a-fA-F]{6})"', s[i:j], re.M)
out = ['//! Les thèmes du web (`const THEMES` de web/index.html), générés par tools/gen_themes.py.',
       '//! Ne pas éditer : changer le web puis relancer le script.',
       '//! Couleurs : fond, texte, atténué, lueur, ambre, cyan.', '',
       'pub const WEB_THEMES: &[(&str, [&str; 6])] = &[']
for m in lignes:
    out.append('    ("%s", ["%s"]),' % (m[0], '", "'.join(c.lower() for c in m[1:])))
out.append('];\n')
open(os.path.join(RACINE, 'loom-app/src/themes_web.rs'), 'w').write('\n'.join(out))
print(len(lignes), 'thèmes')
