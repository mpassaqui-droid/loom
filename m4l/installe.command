#!/bin/zsh
# Installe Loom pour Ableton Live sur ce Mac : double-clique ce fichier.
#  1. le device Loom dans ta User Library ;
#  2. la fenêtre Loom (la page web et son petit serveur local) dans
#     ~/Library/Application Support/Loom ;
#  3. un service qui lance ce serveur à chaque ouverture de session (127.0.0.1 seulement).
set -e
ICI="${0:A:h}"
LIB="$HOME/Music/Ableton/User Library/Presets/MIDI Effects/Max MIDI Effect"
APP="$HOME/Library/Application Support/Loom"
AGENT="$HOME/Library/LaunchAgents/com.loom.pont.plist"

mkdir -p "$LIB" "$APP"
cp "$ICI/Loom.amxd" "$LIB/Loom.amxd"
rm -rf "$APP/web" "$APP/pont"
cp -R "$ICI/web" "$APP/web"
mkdir -p "$APP/pont" && cp "$ICI/pont/loom_pont_ia.py" "$APP/pont/"
echo "✓ device installé : $LIB/Loom.amxd"

if [ ! -f "$AGENT" ] && lsof -iTCP:7410 -sTCP:LISTEN >/dev/null 2>&1; then
  echo "! le port 7410 est déjà pris par un autre programme : service non installé."
  exit 0
fi
launchctl bootout "gui/$(id -u)" "$AGENT" 2>/dev/null || true
cat > "$AGENT" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.loom.pont</string>
  <key>ProgramArguments</key><array>
    <string>/usr/bin/python3</string><string>$APP/pont/loom_pont_ia.py</string>
  </array>
  <key>EnvironmentVariables</key><dict>
    <key>PATH</key><string>$HOME/.local/bin:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>
  </dict>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>StandardOutPath</key><string>$HOME/Library/Logs/loom_pont.log</string>
  <key>StandardErrorPath</key><string>$HOME/Library/Logs/loom_pont.log</string>
</dict></plist>
PLIST
launchctl bootstrap "gui/$(id -u)" "$AGENT"
echo "✓ fenêtre Loom prête (service com.loom.pont). Ouvre Live, pose Loom sur une piste, clique « Ouvrir Loom »."
