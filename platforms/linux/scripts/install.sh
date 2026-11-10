#!/bin/sh
# Installs an app so the desktop (GNOME Resources, the shell) shows it as an app: the GUI binary,
# the command-line tool, the icon and the .desktop file.
#
#   install.sh todo|faves [PREFIX]     (PREFIX defaults to ~/.local)
#
# Run `cargo build --release -p <app>-gui -p <app>-cli` first.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
app=${1:?usage: install.sh todo|faves [PREFIX]}
prefix=${2:-$HOME/.local}
case "$app" in
  todo) id=io.github.vazark.Todo ;;
  faves) id=io.github.vazark.Faves ;;
  *) echo "unknown app: $app (todo or faves)" >&2; exit 2 ;;
esac
gui=$root/target/release/$app-gui
cli=$root/target/release/$app

install -Dm755 "$gui" "$prefix/bin/$app-gui"
[ -x "$cli" ] && install -Dm755 "$cli" "$prefix/bin/$app"
install -Dm644 "$here/../icon/$id.svg" "$prefix/share/icons/hicolor/scalable/apps/$id.svg"
mkdir -p "$prefix/share/applications"
"$gui" --print-desktop-entry > "$prefix/share/applications/$id.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qt "$prefix/share/icons/hicolor" || true
echo "Installed $app to $prefix. Make sure $prefix/bin is on PATH."
