# Contributing

Thanks for helping. This page is what you need to build, test and extend the apps.

## Setup

1. Install a recent stable Rust toolchain (the workspace uses edition 2024).
2. Clone this repository. `weaver` and `iced-adaptive-design` are fetched from GitHub at the
   revisions pinned in the root `Cargo.toml`.
3. A Wayland session is needed to run the GUI, not to build it or run the tests.

### Changing weaver or iced-libadwaita alongside an app

Clone them next to this repository and tell Cargo to use them instead of the pinned revisions:

```sh
git clone https://github.com/VaZark/weaver ../weaver
git clone https://github.com/VaZark/iced-libadwaita ../iced-libadwaita
cp .cargo/config.toml.local-check .cargo/config.toml   # ignored by git
```

When your change there is pushed, bump the `rev` in the root `Cargo.toml` (`[workspace.dependencies]`)
and delete your local `.cargo/config.toml` to check that it builds against the pinned commit.

## The checks

Run these before you send a change. They are all fast.

```sh
cargo test --workspace                      # every test, including the layer rules
cargo clippy --workspace --all-targets      # should print no warnings
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
cargo run -p todo-gui -- --db /tmp/try.db
```

To render the window to a PNG (also how to check a layout without clicking around):

```sh
ICED_ADAPTIVE_DESIGN_SCREENSHOT=/tmp/wide.png cargo run -p todo-gui -- --db /tmp/try.db
TODO_GUI_SIZE=420x700 ICED_ADAPTIVE_DESIGN_SCREENSHOT=/tmp/narrow.png \
  cargo run -p todo-gui -- --db /tmp/try.db       # the narrow layout
```

**Do not type while a GUI is being captured.** A window opened for a screenshot takes keyboard
focus, often with a text field focused, and everything you type lands in it and is auto-saved. Run
screenshots against a copy of the database, never the seed: `rm -f /tmp/copy.db && cp seed/data/todo.db /tmp/copy.db`,
then pass `--db /tmp/copy.db` (`rm` first: `cp` may be aliased to prompt before overwriting).

Switches that put the Todo GUI into a state that otherwise needs a click (for screenshots; they do
nothing in normal use):

| Variable                          | Effect                                                    |
| --------------------------------- | --------------------------------------------------------- |
| `TODO_GUI_SIZE=WxH`               | window size                                               |
| `TODO_GUI_SELECT=<id>`            | open that task or epic in the details pane                |
| `TODO_GUI_SELECT_LATER=<id>`      | open it half a second after start, to catch the slide-in  |
| `TODO_GUI_ADD_TAG=1`              | with a task selected, show the inline tag entry           |
| `TODO_GUI_THEME=light\|dark`      | force the theme                                           |
| `TODO_GUI_SEARCH=<text>`          | start with the search open (empty text is fine)           |
| `TODO_GUI_FILTER=1`               | open the filter                                           |
| `TODO_GUI_NEW=1`                  | open the edit page on a new task                          |
| `TODO_GUI_PREFS=appearance\|tags\|statuses\|app` | open Preferences on that page                         |
| `TODO_GUI_PREVIEW=1`              | with a task selected, show its details drawn as Markdown  |
| `TODO_GUI_EPIC=<id>`, `TODO_GUI_TAG=a,b` | start with those filters on                        |
| `TODO_GUI_LIST=1`                 | start in the list (the board is the default view)         |
| `TODO_GUI_MENU_OPEN=1`            | with a narrow window, draw the header menu open           |
| `TODO_GUI_FLAT=1`                 | start in flat mode (list view)                            |
| `TODO_GUI_CURRENT=1`, `TODO_GUI_CLOSED=1` | start with Current, or closed tasks, on (on the board, the closed columns) |
| `ICED_ADAPTIVE_DESIGN_HOLD_ANIMATIONS=0.5` | freeze animations half way (needs a change that happens after start, such as `TODO_GUI_SELECT_LATER`) |
| `ICED_ADAPTIVE_DESIGN_SHOT_DELAY_MS=1200`  | wait this long before the screenshot             |

Faves has the same kind of switches: `FAVES_GUI_SIZE=WxH`, `FAVES_GUI_LIST=<id>`,
`FAVES_GUI_SELECT=<entry id>`, `FAVES_GUI_SELECT_LATER=<entry id>` (open it half a second after start, to catch the details fading in), `FAVES_GUI_FAVORITES=1`, `FAVES_GUI_COMPACT=1`, `FAVES_GUI_SORT_OPEN=1` (the sort popover open), `FAVES_GUI_MENU_OPEN=1` (the narrow header menu open) and `FAVES_GUI_MENU_PAGE=1` (starting on its Sort page), the dialogs
(`FAVES_GUI_LISTS=1`, `FAVES_GUI_NEW=1` (the edit page on a new entry), `FAVES_GUI_PREVIEW=1`, `FAVES_GUI_PREFS=appearance|tags|app`, `FAVES_GUI_FILTER=1`, `FAVES_GUI_SETTINGS=1`), filters
(`FAVES_GUI_FAV_ONLY=1`, `FAVES_GUI_MIN_RATING=<n>`, `FAVES_GUI_TAG=a,b`), `FAVES_GUI_SEARCH=<text>`
and `FAVES_GUI_THEME=light|dark`.

Notes: `NOTES_GUI_SIZE=WxH`, `NOTES_GUI_FILTER=1`, `NOTES_GUI_PREFS=appearance|tags|app`, `NOTES_GUI_TAG_EDIT=<tag>` (a tag's editor), `NOTES_GUI_PALETTE_EDIT=1` (the palette editor), `NOTES_GUI_NEW=1` (the edit page on a new note), `NOTES_GUI_SELECT=<id>`, `NOTES_GUI_PREVIEW=1` (the selected note drawn as Markdown),
`NOTES_GUI_SCOPE=archive|trash`, `NOTES_GUI_TAG=a,b`, `NOTES_GUI_SEARCH=<text>` and `NOTES_GUI_THEME=light|dark`.

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
| `organiser-ui`     | `primitives` (no iced) → `components` → `widgets`; `shell` is for `main` |
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
change, and a test for the failure case, not only the happy path. GUI state that can be separated
from drawing goes in `organiser-ui`'s `primitives` so it can be tested without a window.

## Adding an app

Using Notes as the example:

1. Create `apps/notes/{schema,cli,gui}` and add them to `members` in the root `Cargo.toml`, plus a
   `notes-schema` entry in `[workspace.dependencies]`.
2. **Schema.** In `notes-schema`, define `META` (id like `io.github.vazark.Notes`, slug, name,
   categories) and a `static SCHEMA` listing the kinds and their fields. Use
   `Field::new("body", FieldType::Markdown).searchable()`, `.indexed()` for fields you filter or
   sort by, and `.default_to("…")` for defaults. Put the app's own rules in `primitives/` (pure) and
   `components/` (needs a `Db`), as `apps/todo/schema` does.
3. **CLI.** `main.rs` is one call: `organiser_cli_kit::run(&META, &SCHEMA, &organiser_cli_kit::Plain)`.
   Pass your own `Extension` for app-specific subcommands (see `apps/todo/cli`).
4. **GUI.** Copy the shape of `apps/todo/gui`: `main.rs` calls `organiser_ui::shell`, `app.rs`
   holds state, messages and `update`, `view/` holds one file per area of the window.
5. Test the schema crate (`tests/behaviour.rs`) and keep its `tests/layers.rs`.

## Style

- `cargo fmt --all` and no clippy warnings.
- Comments explain *why*, not what. Public items get a one-line doc comment.
- Keep views free of logic. A view reads state and produces messages.
- Error messages are for people: say what was wrong and what was expected.

## Commits

Small, focused commits with an imperative subject ("Add due-date filter"). Mention what changed
and why in the body when it is not obvious.
