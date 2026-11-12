#!/bin/sh
# Builds the Flatpaks in parallel, each with its own build directory and flatpak-builder state
# directory (a shared one makes concurrent builds clobber each other).
#
#   flatpak-build.sh [--install] [--jobs N] [--cache DIR | --no-cache] [Todo|Faves|Notes|Recipes|Sticky|all]...
#
# No app (or `all`) means every app. Output goes to build-dir/<App>; logs to build-dir/<App>.log.
# --jobs caps concurrent builds (default: 2).
#
# Cache: a build is normally from scratch (cargo's home and target/ live in the sandbox's temporary
# space), so every app recompiles iced and wgpu. By default the cargo home (downloads) and target/
# (compiled dependencies) are kept in .flatpak-cache/ (gitignored) and shared by every app, so only
# the first build is slow. --cache DIR puts them elsewhere; --no-cache builds from scratch as the
# manifests alone do (what CI does). Concurrent builds take turns on cargo's own lock.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
install_flag=
jobs=2
cache=
no_cache=
apps=
while [ $# -gt 0 ]; do
  case "$1" in
    --install) install_flag=--install; shift ;;
    --jobs) jobs=${2:?--jobs needs a number}; shift 2 ;;
    --jobs=*) jobs=${1#--jobs=}; shift ;;
    --cache) cache=${2:?--cache needs a directory}; shift 2 ;;
    --cache=*) cache=${1#--cache=}; shift ;;
    --no-cache) no_cache=1; shift ;;
    all) apps="$apps Todo Faves Notes Recipes Sticky"; shift ;;
    Todo|Faves|Notes|Recipes|Sticky) apps="$apps $1"; shift ;;
    *) echo "usage: flatpak-build.sh [--install] [--jobs N] [--cache DIR | --no-cache] [Todo|Faves|Notes|Recipes|Sticky|all]..." >&2; exit 2 ;;
  esac
done
[ -n "$apps" ] || apps="Todo Faves Notes Recipes Sticky"
apps=$(printf '%s\n' $apps | awk '!seen[$0]++')
cd "$root"
mkdir -p build-dir .flatpak-builder
if [ -z "$no_cache" ]; then
  [ -n "$cache" ] || cache=$root/.flatpak-cache
  mkdir -p "$cache/cargo" "$cache/target"
  cache=$(cd "$cache" && pwd)
fi

# The manifest to build: as it is, or, with the cache, a copy that mounts the cache directory into
# the sandbox and points cargo at it. The copy lives in build-dir/ (so its source path is made
# absolute), and the install steps read the binaries from the cache's target/.
manifest_for() {
  app=$1
  src="platforms/linux/flatpak/io.github.vazark.$app.yml"
  if [ -n "$no_cache" ]; then echo "$src"; return; fi
  out="build-dir/io.github.vazark.$app.yml"
  sed \
    -e "s|^\( *\)- --share=network\$|&\n\1- --filesystem=$cache|" \
    -e "s|^\( *\)CARGO_HOME: .*\$|\1CARGO_HOME: $cache/cargo\n\1CARGO_TARGET_DIR: $cache/target|" \
    -e "s|target/release/|$cache/target/release/|g" \
    -e "s|path: ../../..\$|path: $root|" \
    "$src" > "$out"
  echo "$out"
}

build_one() {
  app=$1
  # --install writes to the user's flatpak repo, which flatpak serialises itself.
  flatpak-builder --user $install_flag --force-clean \
    --state-dir=".flatpak-builder/$app" "build-dir/$app" \
    "$(manifest_for "$app")" > "build-dir/$app.log" 2>&1
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
