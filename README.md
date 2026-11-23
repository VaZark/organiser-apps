# Organiser apps

Small Linux desktop apps for organising personal data, built on one shared engine. Each app is a
schema, a command-line frontend and a Qt/[Kirigami](https://develop.kde.org/frameworks/kirigami/) GUI
that follows your KDE Plasma colour scheme and style (cxx-qt; Linux).
Tags come from [weaver](https://github.com/VaZark/weaver).

| App                           | Status                                                                                            |
| ----------------------------- | ------------------------------------------------------------------------------------------------- |
| **Todo** (GTG, Jira, Planify) | Working: kanban board and list, nested tasks, epics, your own coloured statuses, Markdown notes, tags, search, CLI and GUI |
| **Notes** (Keep-style)         | Working: coloured cards or a list, Markdown, pinning, favourites, sections (with ratings) nested to any depth, archive and trash, CLI and GUI |
| **Recipes**                   | Working: ingredients (optional or required), ordered steps, photos, read mode, a log of what you cooked, a fridge with expiry dates, shopping lists from recipes and portions that you can copy into Todo, CLI and GUI |
| **Sticky Notes**              | Working: shows items of Notes, Todo and any compliant-app as coloured desktop stickies. It holds no data of its own, only a list of linked apps and flags per entry. Sticky Notes decides whether a sticky is a desktop window or only listed in its own window. GUI and CLI (`sticky`); it can start on login |

## Quick start

You need Rust (edition 2024, so a recent stable toolchain) and a Wayland session for the GUI.

`weaver` is fetched from GitHub at a pinned revision; the GUIs also need the Qt 6 and KDE Frameworks 6 development files:

```sh
cargo test --workspace            # run every test
cargo run -p todo-kirigami        # the Todo GUI (needs Qt 6, Kirigami and KDE Frameworks 6)
cargo run -p todo-cli -- --help   # the Todo command line (binary name: `todo`)
```

Each app keeps its own SQLite file at `$XDG_DATA_HOME/<app>/<app>.db` (for Todo,
`~/.local/share/todo/todo.db`). Pass `--db PATH` to either frontend to use another file, which is
handy for trying things out:

```sh
cargo run -p todo-cli -- --db /tmp/try.db add "Plan holiday" -t travel -s due=2026-12-01
cargo run -p todo-cli -- --db /tmp/try.db add "Book flights" -p 1
cargo run -p todo-cli -- --db /tmp/try.db tree
cargo run -p todo-kirigami -- --db /tmp/try.db
```

### Look

The GUIs use Kirigami with the `org.kde.desktop` Qt Quick Controls style, so they follow your Plasma
colour scheme, icon theme and fonts. There is no app theme of their own.

## How it fits together

```
crates/
  organiser-core/       app identity (AppMeta) and XDG paths. No SQLite.
  organiser-engine/     the one engine: schema-validated items, hierarchy, tags, search, queries
  organiser-cli-kit/    a CLI generated from any app's schema
  organiser-qt/         shared Qt/Kirigami bootstrap, KDE integration (KI18n) and shared QML
  organiser-vault/      Markdown-folder storage shared by the apps
apps/<app>/
  schema/               the app: its item kinds and fields, plus its own rules (no UI)
  cli/                  `<app>`: a few lines on top of organiser-cli-kit
  kirigami/             `<app>-kirigami`: Qt-free `core.rs` (all behaviour), a thin cxx-qt `backend.rs`, and `qml/` that only draws
```

- **One engine, many apps.** An app declares a `Schema` (item kinds and typed fields). The engine
  stores items as rows with JSON fields, validated against that schema, in the app's own database.
  Nesting, tags, full-text search, filtering and sorting come with it.
- **Frontends are thin.** The CLI and GUI parse input, render output and call the engine or the
  app's schema crate. Business rules live below them, so anything the GUI does the CLI can do, and
  the tests cover the logic once.
- **Layers.** Inside each crate, code is split into `primitives` (pure rules and data),
  `components` (one concern each) and, for the UI, `widgets`, and a lower layer never imports a
  higher one. Tests enforce this. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Installing

Each app can be installed as a Flatpak or with a script, so it shows up in your desktop's app list
with its icon. See [platforms/README.md](platforms/README.md).

```sh
flatpak-builder --user --install --force-clean build-dir platforms/linux/flatpak/io.github.vazark.Todo.yml
flatpak run io.github.vazark.Todo
```

## Troubleshooting

- **`Updating git repository` fails**: the first build downloads `weaver`
  from GitHub, so it needs network access.
- **SQLite errors about `fts5` or `json_extract`**: your system SQLite lacks those features.
  Build against the bundled copy: `cargo test -p organiser-engine --features bundled`.
- **The GUI does not start**: it needs a Wayland session. X11 is not supported by the design
  system.

## Licence

MIT.
