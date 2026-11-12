# Organiser apps

Small Linux desktop apps for organising personal data, built on one shared engine. Each app is a
schema, a command-line frontend and an [iced](https://iced.rs) GUI styled with
[iced-adaptive-design](https://github.com/VaZark/iced-libadwaita) (GNOME-style, Wayland only).
Tags come from [weaver](https://github.com/VaZark/weaver).

| App                           | Status                                                                                            |
| ----------------------------- | ------------------------------------------------------------------------------------------------- |
| **Todo** (GTG, Jira, Planify) | Working: kanban board and list, nested tasks, epics, your own coloured statuses, Markdown notes, tags, search, CLI and GUI |
| **Notes** (Keep-style)         | Working: coloured cards or a list, Markdown, pinning, favourites, folders (with ratings) and their sections, archive and trash, CLI and GUI |
| **Recipes**                   | Working: ingredients (optional or required), ordered steps, photos, read mode, a log of what you cooked, a fridge with expiry dates, shopping lists from recipes and portions that you can copy into Todo, CLI and GUI |
| **Sticky Notes**              | Working: shows items of Notes, Todo and any compliant-app as coloured desktop stickies. It holds no data of its own, only a list of linked apps and flags per entry. Sticky Notes decides whether a sticky is a desktop window or only listed in its own window. GUI and CLI (`sticky`); it can start on login |

## Quick start

You need Rust (edition 2024, so a recent stable toolchain) and a Wayland session for the GUI.

Both dependencies are fetched from GitHub at pinned revisions, so a plain clone is enough:

```sh
cargo test --workspace            # run every test
cargo run -p todo-gui             # the Todo GUI
cargo run -p todo-cli -- --help   # the Todo command line (binary name: `todo`)
```

Each app keeps its own SQLite file at `$XDG_DATA_HOME/<app>/<app>.db` (for Todo,
`~/.local/share/todo/todo.db`). Pass `--db PATH` to either frontend to use another file, which is
handy for trying things out:

```sh
cargo run -p todo-cli -- --db /tmp/try.db add "Plan holiday" -t travel -s due=2026-12-01
cargo run -p todo-cli -- --db /tmp/try.db add "Book flights" -p 1
cargo run -p todo-cli -- --db /tmp/try.db tree
cargo run -p todo-gui -- --db /tmp/try.db
```

### Todo

A task has a title, details, a status (to do, in progress, done, won't do), a priority (lowest to
highest), start and due dates, tags, optional subtasks, and optionally an **epic**: a group of
tasks with its own dates, status and progress.

In the GUI there are two views of the same tasks, switched with the **board** icon in the header:

- The **kanban board** is the default: a column per status (to do, in progress, done), each a stack
  of cards in the style of Google Keep. A card is tinted with its epic's colour and shows the title,
  a snippet of the details, how many subtasks are done, `# tags`, priority, epic and due date. The
  arrows on a card move it to the next column, and pressing it opens its details. The sidebar lists
  your **epics** (with how many of their tasks are done) and your **tags** (with counts); click one
  to filter, and click it again to clear it.
- The **list** has no sidebar. Its header has a **flat list** toggle, a **calendar** toggle for
  the _current_ tasks, and the closed-tasks toggle. By default tasks are nested under their
  parents; **flat list** shows every task and subtask as an equal row, naming the parent of each
  subtask. **Current** shows the tasks and subtasks scheduled for today: those with a start or due
  date where today falls within them (a missing start counts as already started, a missing due
  date as never ending). Undated tasks and overdue ones are not current. Each task stands alone,
  always as a flat list, with the nearest deadline first.

In both views:

- The **circled-check** icon shows closed tasks. They are hidden by default; on the board it swaps
  the columns for the closed ones (done and won't do).
- The **search** icon (or `Ctrl+F`) turns the header into a search box, centred in the header.
  `Esc` closes it. In search mode a **filter** button appears at the end of the box. It opens a
  dialog (a full-screen sheet on a narrow window) to narrow the tasks by **epic** and by **tags**.
  Active filters stay visible and can each be removed with a click.
- Click a task to open its details: status, priority, epic, dates (with a calendar), a details
  text area, editable tags with suggestions, and subtasks. Every edit is saved as you make it. The
  details slide in from the right: beside the list, and over the board.
- In the quick-add box, `Buy milk #home due:tomorrow start:+3d` sets the title, a tag and dates.
  Dates accept `2026-12-01`, `today`, `tomorrow`, `+3d` or a weekday name. With an epic selected,
  new tasks go into it.

### Notes: folders, favourites and ratings

Notes can be filed in **folders** (each with a name, an icon, a colour and a description), and a
folder can have **sections**. The folders choose their own extras: **ratings** (up to five stars) on
or off. Every note can be a **favourite**. In All, Favorites, Archive and Trash the notes can be
grouped by folder, and a folder's page by section; the notes in no folder are under "Unassigned".

The rules hold everywhere, not just in the GUI. Switching ratings off for a folder hides its
ratings and keeps them for when it is back on. Deleting a folder keeps its notes (or deletes them
with it, if asked).

```sh
notes folder-new Films --rated --icon play         # prints the folder's id, say 1
notes add "Alien" -s body="Spooky."                # a note
notes move 3 1                                     # file note 3 in folder 1
notes rate 3 5                                     # five stars (the folder must rate)
notes fave 3                                       # mark a favourite
notes ls --favorites --sort rating                 # favourites, best rated first
notes folders                                      # folders with their counts
notes section-new 1 Classics --color '#f2d45c'     # a section of folder 1
notes import-faves                                 # copy what the old Faves app held
```


On the command line, the generic commands are the same for every app: `add list show edit rm mv
tag untag tags tag-new tag-parent tag-rm`. Todo adds `start done dismiss reopen tree current
epics epic-new epic-rm`. Every command takes `--json`.

```sh
todo epic-new "Holiday"                             # prints the new epic's id, say 1
todo add "Book flights" -t travel -s epic=1 -s priority=high -s due=2026-12-01
todo add "Pack" -p 2                                # -p makes it a subtask of task 2
todo start 2                                        # in progress
todo current                                        # start and due dates include today
todo tree --epic 1                                  # one epic's tasks, with subtasks
todo epics                                          # epics with progress
todo done 2                                         # closes the task and its open subtasks
todo list -t travel -w 'due<2026-12-01' --sort due  # filter by tag and field, then sort
todo list -q flights                                # full-text search
todo tree --all                                     # include done and won't-do tasks
```

## How it fits together

```
crates/
  organiser-core/       app identity (AppMeta) and XDG paths. No SQLite.
  organiser-engine/     the one engine: schema-validated items, hierarchy, tags, search, queries
  organiser-cli-kit/    a CLI generated from any app's schema
  organiser-ui/         shared GUI pieces on iad: Keep-style cards, kanban column, search, tag editor,
                        details dock, filter dialog, window shell
apps/<app>/
  schema/               the app: its item kinds and fields, plus its own rules (no UI)
  cli/                  `<app>`: a few lines on top of organiser-cli-kit
  gui/                  `<app>-gui`: views and state; every change goes through the schema crate
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

- **`Updating git repository` fails**: the first build downloads `weaver` and
  `iced-adaptive-design` from GitHub, so it needs network access.
- **SQLite errors about `fts5` or `json_extract`**: your system SQLite lacks those features.
  Build against the bundled copy: `cargo test -p organiser-engine --features bundled`.
- **The GUI does not start**: it needs a Wayland session. X11 is not supported by the design
  system.

## Licence

MIT.
