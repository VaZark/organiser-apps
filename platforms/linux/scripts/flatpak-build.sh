#!/bin/sh
# Builds the Flatpaks in parallel, each with its own build directory and flatpak-builder state
# directory (a shared one makes concurrent builds clobber each other).
#
#   flatpak-build.sh [--install] [--jobs N] [Todo|Faves|Notes|Recipes|Sticky|all]...
#
# No app (or `all`) means every app. Output goes to build-dir/<App>; logs to build-dir/<App>.log.
# --jobs caps concurrent builds (default: 2, each one runs a full cargo build).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
install_flag=
jobs=2
apps=
while [ $# -gt 0 ]; do
  case "$1" in
    --install) install_flag=--install; shift ;;
    --jobs) jobs=${2:?--jobs needs a number}; shift 2 ;;
    --jobs=*) jobs=${1#--jobs=}; shift ;;
    all) apps="$apps Todo Faves Notes Recipes Sticky"; shift ;;
    Todo|Faves|Notes|Recipes|Sticky) apps="$apps $1"; shift ;;
    *) echo "usage: flatpak-build.sh [--install] [--jobs N] [Todo|Faves|Notes|Recipes|Sticky|all]..." >&2; exit 2 ;;
  esac
done
[ -n "$apps" ] || apps="Todo Faves Notes Recipes Sticky"
apps=$(printf '%s\n' $apps | awk '!seen[$0]++')
cd "$root"
mkdir -p build-dir .flatpak-builder

build_one() {
  app=$1
  # --install writes to the user's flatpak repo, which flatpak serialises itself.
  flatpak-builder --user $install_flag --force-clean \
    --state-dir=".flatpak-builder/$app" "build-dir/$app" \
    "platforms/linux/flatpak/io.github.vazark.$app.yml" > "build-dir/$app.log" 2>&1
}

pids=
running=0
failed=
for app in $apps; do
  if [ "$running" -ge "$jobs" ]; then
    # Wait for any one to finish (the oldest, for simplicity) before starting the next.
    set -- $pids; oldest=$1; shift; pids="$*"
    wait "${oldest%%:*}" || failed="$failed ${oldest#*:}"
    running=$((running - 1))
  fi
  echo "Building $app (log: build-dir/$app.log)"
  build_one "$app" &
  pids="$pids $!:$app"
  running=$((running + 1))
done
for p in $pids; do wait "${p%%:*}" || failed="$failed ${p#*:}"; done

if [ -n "$failed" ]; then
  echo "Failed:$failed (see build-dir/<App>.log)" >&2
  exit 1
fi
echo "Built:$(printf ' %s' $apps)"
