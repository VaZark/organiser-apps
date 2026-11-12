# Packaging

Everything here is for Linux. The GUIs are Wayland only.

```
platforms/linux/
  icon/       one SVG per app (Todo, Faves, Notes, Recipes)
  flatpak/    a manifest, AppStream metainfo and a launcher per app
  scripts/    install.sh and uninstall.sh, for a plain install without Flatpak
```

The app ids are `io.github.vazark.Todo`, `.Faves`, `.Notes` and `.Recipes`. The `.desktop` file of
each is not written by hand: `<app>-gui --print-desktop-entry`
renders it from the app's metadata (`META` in its schema crate), so its name, comment, categories
and icon cannot drift from the app.

## Flatpak

You need `flatpak`, `flatpak-builder`, the Freedesktop 26.08 platform and SDK, and the Rust
extension:

```sh
flatpak install flathub org.freedesktop.Platform//26.08 org.freedesktop.Sdk//26.08 \
    org.freedesktop.Sdk.Extension.rust-stable//26.08
```

Build and install one app for your user, and run it:

```sh
flatpak-builder --user --install --force-clean build-dir platforms/linux/flatpak/io.github.vazark.Todo.yml
flatpak run io.github.vazark.Todo
```

(`Faves`, `Notes` or `Recipes` in place of `Todo` for the others.) To build without installing, leave out `--install`.

To build several at once, use the helper, which gives each app its own build and state directory
(sharing `build-dir` or `.flatpak-builder` between concurrent builds makes them clobber each other):

```sh
platforms/linux/scripts/flatpak-build.sh --install              # all four, two at a time
platforms/linux/scripts/flatpak-build.sh --jobs 4 Todo Notes    # chosen apps, up to four at once
```

Output is in `build-dir/<App>`, logs in `build-dir/<App>.log`.

- **Network.** The build downloads the Rust dependencies, including `weaver` and
  `iced-libadwaita` from GitHub at the revisions pinned in `Cargo.lock`, so it needs network
  access (`--share=network` in the manifest). Flathub itself builds offline, so publishing there
  would need the dependencies vendored with `flatpak-cargo-generator`.
- **Data.** The launcher points the app at your real data directory (`~/.local/share/todo`, `faves`,
  `notes`, `recipes`), so the Flatpak GUI and a command-line `todo` or `faves` installed on the
  host use the same database.
- **Permissions.** Wayland, the GPU, that one data directory, and read-only access to
  `~/.config/iced-adaptive-design` (the toolkit's desktop-wide settings). Nothing else.
- **SQLite** is built in: the apps need its full-text search and JSON support.
- Only the GUI is in the Flatpak. Install the command line with `cargo install --path apps/todo/cli`
  (or `faves`), or with the script below.

## Building all four on GitHub

The **Build Flatpaks** workflow (`.github/workflows/flatpak.yml`) is manual: run it from the
Actions tab. It builds each app as its own job, in parallel on separate runners (a new run on the same ref
cancels one still going), and uploads one `<App>.flatpak` bundle per app as a
run artifact. Install one with `flatpak install --user Notes.flatpak`.

## Without Flatpak

```sh
cargo build --release -p todo-gui -p todo-cli
platforms/linux/scripts/install.sh todo          # into ~/.local; --prefix DIR changes that
platforms/linux/scripts/install.sh               # every app, installed in parallel
platforms/linux/scripts/uninstall.sh todo notes  # removes them; your data stays
```

Several invocations can run at once: each writes only its own files, and the desktop and icon
caches are refreshed under a lock.

It installs the GUI, the command line, the icon and the `.desktop` file, so the desktop lists the
app. Make sure `~/.local/bin` is on your `PATH`.

## Checking the files

```sh
todo-gui --print-desktop-entry | desktop-file-validate /dev/stdin
appstreamcli validate --no-net platforms/linux/flatpak/io.github.vazark.Todo.metainfo.xml
```

`appstreamcli` reports one warning, a missing homepage URL, until the project has a public
repository to point to.
