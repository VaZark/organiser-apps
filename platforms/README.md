# Packaging

Everything here is for Linux. Both GUIs are Wayland only.

```
platforms/linux/
  icon/       one SVG per app: io.github.vazark.Todo.svg, io.github.vazark.Faves.svg
  flatpak/    a manifest, AppStream metainfo and a launcher per app
  scripts/    install.sh and uninstall.sh, for a plain install without Flatpak
```

The app ids are `io.github.vazark.Todo` and `io.github.vazark.Faves`. The `.desktop` file of each
is not written by hand: `todo-gui --print-desktop-entry` and `faves-gui --print-desktop-entry`
render it from the app's metadata (`META` in its schema crate), so its name, comment, categories
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

(`Faves` in place of `Todo` for the other.) To build without installing, leave out `--install`.

- **Network.** The build downloads the Rust dependencies, including `weaver` and
  `iced-libadwaita` from GitHub at the revisions pinned in `Cargo.lock`, so it needs network
  access (`--share=network` in the manifest). Flathub itself builds offline, so publishing there
  would need the dependencies vendored with `flatpak-cargo-generator`.
- **Data.** The launcher points the app at your real data directory (`~/.local/share/todo`,
  `~/.local/share/faves`), so the Flatpak GUI and a command-line `todo` or `faves` installed on the
  host use the same database.
- **Permissions.** Wayland, the GPU, that one data directory, and read-only access to
  `~/.config/iced-adaptive-design` (the toolkit's desktop-wide settings). Nothing else.
- **SQLite** is built in: the apps need its full-text search and JSON support.
- Only the GUI is in the Flatpak. Install the command line with `cargo install --path apps/todo/cli`
  (or `faves`), or with the script below.

## Without Flatpak

```sh
cargo build --release -p todo-gui -p todo-cli
platforms/linux/scripts/install.sh todo          # into ~/.local; a second argument is a prefix
platforms/linux/scripts/uninstall.sh todo        # removes it; your data stays
```

It installs the GUI, the command line, the icon and the `.desktop` file, so the desktop lists the
app. Make sure `~/.local/bin` is on your `PATH`.

## Checking the files

```sh
todo-gui --print-desktop-entry | desktop-file-validate /dev/stdin
appstreamcli validate --no-net platforms/linux/flatpak/io.github.vazark.Todo.metainfo.xml
```

`appstreamcli` reports one warning, a missing homepage URL, until the project has a public
repository to point to.
