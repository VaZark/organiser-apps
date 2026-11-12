# organiser-sticky: registering with Sticky Notes

Sticky Notes shows items of other apps as desktop stickies. It has no app compiled in: an app
**registers** (says what it is) and the person **links** it (the switch). Both are files under
`$XDG_CONFIG_HOME/organiser/` (`~/.config/organiser/`), so any program in any language can do them.

## Register

Either run `sticky register --id org.example.Tasks --name "Example Tasks" --db /path/tasks.db
--kind task=notes [--link]`, or write `sticky.d/<app id>.toml` yourself:

```toml
[[link]]
app_id = "org.example.Tasks"     # letters, digits . _ -
name = "Example Tasks"
slug = "tasks"
db_path = "/home/me/.local/share/tasks/tasks.db"
[[link.kinds]]
kind = "task"                    # which items can be stickies
body = "notes"                   # the field holding their text (letters, digits, _)
```

A registration is only an offer; the app is switched off until linked. Re-register to move the
database; the switch is kept. `sticky link|unlink|unregister APP_ID`, `apps`, `items` and `drop` do the rest (the `sticky` command line, over the same engine as the GUI).
Rust apps in this repo use `organiser_sticky::store::publish(&SPEC)` and the Preferences row
`organiser_ui::widgets::sticky_link`, which write the same files.

## The data contract

The database is SQLite in the layout `organiser-engine` makes (an `items` table with `kind`, `title`,
`fields` JSON, `archived`, `updated`, and the `items_fts` search table).

**The app decides what is a sticky.** An item of a registered kind is a sticky while its JSON field
`sticky` is `true`; it may also give a starting colour in `sticky_color` (`#rrggbb`). Sticky Notes
only *reads* those. It never sets or clears `sticky`, and never deletes or archives an item. It does
write back the item's title and its text field, so editing a sticky edits the item.

**Sticky Notes decides how it shows.** Whether a sticky is a window on the desktop (`show_on_desktop`,
true by default) or only listed in Sticky Notes' own window, and the colour picked in Sticky, are
kept in `sticky-local.toml` beside the registry, never in the app's database. That state is dropped
when the app stops flagging the item, and when the app is unregistered. A database that is not in
the engine's layout is refused without being touched.
