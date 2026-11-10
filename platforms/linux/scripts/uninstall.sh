#!/bin/sh
# Removes what install.sh put in place. Your data is left alone.
#
#   uninstall.sh todo|faves [PREFIX]   (PREFIX defaults to ~/.local)
set -eu
app=${1:?usage: uninstall.sh todo|faves [PREFIX]}
prefix=${2:-$HOME/.local}
case "$app" in
  todo) id=io.github.vazark.Todo ;;
  faves) id=io.github.vazark.Faves ;;
  *) echo "unknown app: $app (todo or faves)" >&2; exit 2 ;;
esac
rm -f "$prefix/bin/$app-gui" "$prefix/bin/$app" \
  "$prefix/share/icons/hicolor/scalable/apps/$id.svg" \
  "$prefix/share/applications/$id.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" || true
echo "Removed $app from $prefix."
