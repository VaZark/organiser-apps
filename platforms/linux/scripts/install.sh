#!/bin/sh
# Installs apps so the desktop (GNOME Resources, the shell) shows them as apps: the GUI binary,
# the command-line tool, the icon and the .desktop file.
#
#   install.sh [--prefix PREFIX] [todo|notes|recipes|sticky|all]...
#
# PREFIX defaults to ~/.local; no app (or `all`) means every app. Apps are installed in parallel,
# and the desktop database and icon cache are refreshed once at the end, so several invocations
# (one per app, from a script or a CI matrix) are safe too: each writes only its own files, and the
# caches are refreshed under a lock.
#
# Run `cargo build --release -p <app>-gui -p <app>-cli` first.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
prefix=$HOME/.local
apps=
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix=${2:?--prefix needs a directory}; shift 2 ;;
    --prefix=*) prefix=${1#--prefix=}; shift ;;
    all) apps="$apps todo notes recipes sticky"; shift ;;
    todo|notes|recipes|sticky) apps="$apps $1"; shift ;;
    *) echo "usage: install.sh [--prefix PREFIX] [todo|notes|recipes|sticky|all]..." >&2; exit 2 ;;
  esac
done
[ -n "$apps" ] || apps="todo notes recipes sticky"

install_one() {
  app=$1
  case "$app" in
    todo) id=io.github.vazark.Todo ;;
    notes) id=io.github.vazark.Notes ;;
    recipes) id=io.github.vazark.Recipes ;;
    sticky) id=io.github.vazark.Sticky ;;
  esac
  gui=$root/target/release/$app-gui
  cli=$root/target/release/$app
  install -Dm755 "$gui" "$prefix/bin/$app-gui"
  if [ -x "$cli" ]; then install -Dm755 "$cli" "$prefix/bin/$app"; fi
  install -Dm644 "$here/../icon/$id.svg" "$prefix/share/icons/hicolor/scalable/apps/$id.svg"
  mkdir -p "$prefix/share/applications"
  # Write beside the target and rename, so nothing ever sees a half-written entry.
  tmp=$(mktemp "$prefix/share/applications/.$id.XXXXXX")
  "$gui" --print-desktop-entry > "$tmp"
  chmod 644 "$tmp"
  mv -f "$tmp" "$prefix/share/applications/$id.desktop"
  echo "Installed $app to $prefix."
}

# Each app in its own background job; collect every failure rather than stopping at the first.
pids=
for app in $(printf '%s\n' $apps | awk '!seen[$0]++'); do
  install_one "$app" &
  pids="$pids $!"
done
status=0
for pid in $pids; do wait "$pid" || status=1; done
[ "$status" -eq 0 ] || { echo "some installs failed" >&2; exit 1; }

refresh_caches() {
  if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$prefix/share/applications" || true
  fi
  if command -v gtk-update-icon-cache >/dev/null; then
    gtk-update-icon-cache -qt "$prefix/share/icons/hicolor" || true
  fi
}
mkdir -p "$prefix/share"
if command -v flock >/dev/null; then
  ( flock 9; refresh_caches ) 9> "$prefix/share/.install-caches.lock"
else
  refresh_caches
fi
echo "Make sure $prefix/bin is on PATH."
