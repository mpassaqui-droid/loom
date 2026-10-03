#!/usr/bin/env bash
# LOOM — install the native apps for the current user (no sudo).
#
#   ./install.sh              build + install `loom` (terminal) and `loom-gui` (window)
#   ./install.sh --uninstall  remove everything this script installed
#
# What it puts where:
#   ~/.local/bin/loom, ~/.local/bin/loom-gui          the apps
#   ~/.local/share/loom/examples/*.loom               the example songs
#   ~/.local/share/loom/samples/*.wav                 the sample bank (bd, amen, vox…)
#   ~/.local/share/applications/loom*.desktop         launchers (Super+Space menu)
#   ~/.local/share/icons/hicolor/scalable/apps/loom.svg
#   ~/.local/share/mime/packages/loom.xml             .loom files open in LOOM
#   ~/Music/loom/                                     your songs (created, never deleted)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN="$HOME/.local/bin"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
APPS="$DATA/applications"
ICON="$DATA/icons/hicolor/scalable/apps/loom.svg"
MIME="$DATA/mime/packages/loom.xml"
EXAMPLES="$DATA/loom/examples"
SAMPLES="$DATA/loom/samples"

say() { printf '\033[1;32m›\033[0m %s\n' "$*"; }

refresh_desktop() {
  command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" 2>/dev/null || true
  command -v update-mime-database >/dev/null && update-mime-database "$DATA/mime" 2>/dev/null || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$BIN/loom" "$BIN/loom-gui" "$APPS/loom.desktop" "$APPS/loom-gui.desktop" "$ICON" "$MIME"
  rm -rf "$DATA/loom"
  refresh_desktop
  say "LOOM désinstallé (tes morceaux dans ~/Music/loom sont conservés)."
  exit 0
fi

# Rust comes from mise on this machine; fall back to a cargo on PATH.
if command -v mise >/dev/null && mise which cargo >/dev/null 2>&1; then
  CARGO=(mise exec -- cargo)
elif command -v cargo >/dev/null; then
  CARGO=(cargo)
else
  echo "Rust est introuvable. Installe-le avec :  mise use -g rust" >&2
  exit 1
fi

say "Compilation (release) — la première fois prend quelques minutes…"
"${CARGO[@]}" build --release --manifest-path "$ROOT/Cargo.toml" -p loom-tui -p loom-gui --features loom-tui/libre

say "Installation des apps dans $BIN"
mkdir -p "$BIN" "$EXAMPLES" "$SAMPLES" "$APPS" "$(dirname "$ICON")" "$(dirname "$MIME")" "$HOME/Music/loom"
install -m 755 "$ROOT/target/release/loom" "$BIN/loom"
install -m 755 "$ROOT/target/release/loom-gui" "$BIN/loom-gui"
install -m 644 "$ROOT"/*.loom "$EXAMPLES/"
install -m 644 "$ROOT"/web/samples/*.wav "$SAMPLES/"
install -m 644 "$ROOT/packaging/loom.svg" "$ICON"

# On Omarchy, open the terminal app the Omarchy way (themed terminal,
# focus the existing LOOM window instead of opening a second one).
if command -v omarchy-launch-or-focus-tui >/dev/null; then
  TUI_EXEC="omarchy-launch-or-focus-tui $BIN/loom"
else
  TUI_EXEC="xdg-terminal-exec $BIN/loom"
fi

cat > "$APPS/loom.desktop" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=LOOM (terminal)
GenericName=Live-coding musical
Comment=Écrire de la musique en direct, dans le terminal
Exec=$TUI_EXEC
Icon=loom
Terminal=false
Categories=AudioVideo;Audio;Music;
Keywords=music;live;coding;synth;musique;
StartupNotify=true
EOF

cat > "$APPS/loom-gui.desktop" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=LOOM
GenericName=Live-coding musical
Comment=Écrire de la musique en direct
Exec=$BIN/loom-gui %f
Icon=loom
Terminal=false
Categories=AudioVideo;Audio;Music;
Keywords=music;live;coding;synth;musique;
MimeType=text/x-loom;
StartupWMClass=loom
StartupNotify=true
EOF

cat > "$MIME" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="text/x-loom">
    <comment>Morceau LOOM</comment>
    <sub-class-of type="text/plain"/>
    <glob pattern="*.loom"/>
  </mime-type>
</mime-info>
EOF

refresh_desktop
command -v xdg-mime >/dev/null && xdg-mime default loom-gui.desktop text/x-loom 2>/dev/null || true

say "Terminé !"
cat <<EOF

  Lance LOOM :
    • Super+Espace → « LOOM » (fenêtre) ou « LOOM (terminal) »
    • dans un terminal :  loom        ou  loom-gui
    • sans interface   :  loom play $EXAMPLES/techno.loom

  Tes morceaux vont dans ~/Music/loom. Les couleurs suivent ton thème Omarchy.
EOF
