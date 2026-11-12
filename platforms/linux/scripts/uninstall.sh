#!/bin/sh
# Removes what install.sh put in place. Your data is left alone.
#
#   uninstall.sh [--prefix PREFIX] [todo|faves|notes|recipes|sticky|all]...
#
# PREFIX defaults to ~/.local; no app (or `all`) means every app. Safe to run in parallel.
set -eu
prefix=$HOME/.local
apps=
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix=${2:?--prefix needs a directory}; shift 2 ;;
    --prefix=*) prefix=${1#--prefix=}; shift ;;
    all) apps="$apps todo faves notes recipes sticky"; shift ;;
    todo|faves|notes|recipes|sticky) apps="$apps $1"; shift ;;
    *) echo "usage: uninstall.sh [--prefix PREFIX] [todo|faves|notes|recipes|sticky|all]..." >&2; exit 2 ;;
  esac
done
[ -n "$apps" ] || apps="todo faves notes recipes sticky"

for app in $apps; do
  case "$app" in
    todo) id=io.github.vazark.Todo ;;
    faves) id=io.github.vazark.Faves ;;
    notes) id=io.github.vazark.Notes ;;
    recipes) id=io.github.vazark.Recipes ;;
    sticky) id=io.github.vazark.Sticky ;;
  esac
  rm -f "$prefix/bin/$app-gui" "$prefix/bin/$app" \
    "$prefix/share/icons/hicolor/scalable/apps/$id.svg" \
    "$prefix/share/applications/$id.desktop"
  echo "Removed $app from $prefix."
done

refresh_caches() {
  if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$prefix/share/applications" || true
  fi
}
if [ -d "$prefix/share/applications" ]; then
  if command -v flock >/dev/null; then
    ( flock 9; refresh_caches ) 9> "$prefix/share/.install-caches.lock"
  else
    refresh_caches
  fi
fi
