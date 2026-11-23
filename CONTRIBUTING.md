# Contributing

Thanks for helping. This page is what you need to build, test and extend the apps.

## Setup

1. Install a recent stable Rust toolchain (the workspace uses edition 2024).
2. Clone this repository. `weaver` is fetched from GitHub at the
   revisions pinned in the root `Cargo.toml`.
3. The GUIs need Qt 6, Kirigami, Kirigami Addons and KDE Frameworks 6 (KI18n, Syntax Highlighting) with their development files, and a Plasma/Wayland session to run; the tests and CLIs do not.

### Changing weaver alongside an app

Clone them next to this repository and tell Cargo to use them instead of the pinned revisions:

```sh
git clone https://github.com/VaZark/weaver ../weaver
cp .cargo/config.toml.local-check .cargo/config.toml   # ignored by git
```

When your change there is pushed, bump the `rev` in the root `Cargo.toml` (`[workspace.dependencies]`)
and delete your local `.cargo/config.toml` to check that it builds against the pinned commit.

## Working with an agent

`AGENTS.md` is the entry point: conventions, invariants and the skills in `.agents/skills`. Work is tracked one task per file in `TODO/<epic>/` (Todo app vault format; point Todo at it with `todo vault set TODO`), and `AGENTS_TODO_AND_STATUS.md` holds the active batch, validation and handoff.

## The checks

Run these before you send a change. They are all fast.

```sh
cargo test --workspace                      # every test, including the layer rules
cargo clippy --workspace --all-targets -- -D warnings   # should print no warnings
cargo fmt --all                             # formatting
```

Useful variations:

```sh
cargo test -p organiser-engine              # one crate
cargo test -p todo-schema -- closing        # tests whose name contains "closing"
cargo test -p organiser-engine --features bundled   # use the bundled SQLite (needs FTS5 + JSON1)
```

### Trying the GUI without touching your data

```sh
cargo run -p todo-kirigami -- --db /tmp/try.db
```

To render a page to a PNG (also how to check a layout without clicking around), run on the real
session (offscreen hangs) and set `QT_FORCE_STDERR_LOGGING=1` to see QML errors:

```sh
QT_FORCE_STDERR_LOGGING=1 cargo run -p todo-kirigami -- --db /tmp/copy.db \
  --page board --screenshot /tmp/board.png
```

**Do not type while a GUI is being captured.** The window takes keyboard focus and anything typed
is auto-saved. Run screenshots against a copy of the database, never the seed
(`rm -f /tmp/copy.db && cp seed/data/todo.db /tmp/copy.db`; `rm` first, `cp` may prompt). Dev flags
are listed in each app's `qml/Main.qml`.

## Packaging

`platforms/` holds the Flatpak manifests, AppStream metainfo, icons and install scripts; see
[platforms/README.md](platforms/README.md). When you add an app, add its icon, manifest, metainfo
and launcher there, and check the `.desktop` text and the metainfo with `desktop-file-validate`
and `appstreamcli validate --no-net`. Flatpak builds ignore `.cargo/`, `seed/` and `target/`.

## The rules of the codebase

**Behaviour lives below the frontends.** The CLI and GUI parse input, render output and call down.
If you are writing a rule (what closing a task does, what a valid URL is), it belongs in the
engine or the app's schema crate, with a test there, not in a view or a command handler.

**Layers.** Each crate is split into layers, and a layer imports only the ones below it:

| Crate              | Layers, lowest first                                              |
| ------------------ | ----------------------------------------------------------------- |
| `organiser-engine` | `primitives` (pure, no SQLite) → `components` (one SQLite concern each) → `db` (the `Db` facade) |
| `organiser-cli-kit`, `todo-schema` | `primitives` (pure) → `components`                |

Each crate has a `tests/layers.rs` that fails if you break this, so a failing layer test means
code is in the wrong layer, not that the test is wrong. When unsure: if it needs a database, it is
not a primitive; if it draws something, it is not in the engine.

**Rules that must hold for every frontend live in the engine.** A field of type
**Gated fields** work the same way: `Field::new("rating", …).gated_by_parent("list", "rated")`
exists only while the list it belongs to has `rated` switched on, and the engine removes it when the
switch goes off. A `FieldType::Reference("epic")` is checked by the engine on every write (the target must exist and
be an epic) and is removed when its target is deleted, so the CLI, the GUI and any future frontend
get it for free. Do not re-check such things in a helper that a frontend can bypass.

**Schemas validate, the engine stores.** Fields are JSON on an item and are checked against the
app's `Schema` on every write. Never write SQL for a field name that did not come from the schema;
the query planner (`organiser-engine/src/primitives/plan.rs`) resolves names against it and binds
every value.

**Migrations only append.** Database upgrades are in
`organiser-engine/src/components/migrate.rs`, versioned by `PRAGMA user_version`. Never edit an
existing migration, because real users will have it applied. Indexes on schema fields are rebuilt
from the schema on every open, so changing which fields are `indexed` needs no migration.

**Tests.** Engine and schema logic is tested against `Db::open_in_memory`. Add a test with the
change, and a test for the failure case, not only the happy path. GUI behaviour goes in the
app's Qt-free `kirigami/src/core.rs` so it is tested without a window; QML only draws.

## Adding an app

Using Notes as the example:

1. Create `apps/notes/{schema,cli,kirigami}` and add them to `members` in the root `Cargo.toml`, plus a
   `notes-schema` entry in `[workspace.dependencies]`.
2. **Schema.** In `notes-schema`, define `META` (id like `io.github.vazark.Notes`, slug, name,
   categories) and a `static SCHEMA` listing the kinds and their fields. Use
   `Field::new("body", FieldType::Markdown).searchable()`, `.indexed()` for fields you filter or
   sort by, and `.default_to("…")` for defaults. Put the app's own rules in `primitives/` (pure) and
   `components/` (needs a `Db`), as `apps/todo/schema` does.
3. **CLI.** `main.rs` is one call: `organiser_cli_kit::run(&META, &SCHEMA, &organiser_cli_kit::Plain)`.
   Pass your own `Extension` for app-specific subcommands (see `apps/todo/cli`).
4. **GUI.** Copy the shape of `apps/todo/kirigami`: `core.rs` (no Qt) holds the behaviour and its
   tests, `backend.rs` is a thin cxx-qt object that calls it and republishes JSON, and `qml/` only
   draws, using idiomatic Kirigami (actions, page stack, FormCard) and `organiser-qt`'s shared QML.
5. Test the schema crate (`tests/behaviour.rs`) and keep its `tests/layers.rs`.

## Style

- `cargo fmt --all` and no clippy warnings.
- Comments explain *why*, not what. Public items get a one-line doc comment.
- Keep views free of logic. A view reads state and produces messages.
- Error messages are for people: say what was wrong and what was expected.

## Commits

Small, focused commits with an imperative subject ("Add due-date filter"). Mention what changed
and why in the body when it is not obvious.
