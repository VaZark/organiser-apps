# organiser-sticky: linking apps with Sticky Notes

Sticky Notes shows items of the organiser apps (Notes, Todo, Faves) as desktop stickies. Each app
**publishes** what it is to a shared registry at start, and the person **links** it (the switch, in
the app's Preferences or in Sticky Notes'). The registry is `$XDG_CONFIG_HOME/organiser/sticky-links.toml`
(`~/.config/organiser/`). Only the apps in this repository are supported: there is no registration
for other programs.

Apps use `organiser_sticky::store::publish(&SPEC)` and the Preferences row
`organiser_ui::widgets::sticky_link`. `sticky link|unlink APP_ID`, `apps`, `items` and the other
commands of the `sticky` command line work over the same engine as the GUI.

## The data contract

The database is SQLite in the layout `organiser-engine` makes (an `items` table with `kind`, `title`,
`fields` JSON, `archived`, `updated`, and the `items_fts` search table).

**The app decides what is a sticky.** An item of a published kind is a sticky while its JSON field
`sticky` is `true`; it may also give a starting colour in `sticky_color` (`#rrggbb`). Sticky Notes
only *reads* those. It never sets or clears `sticky`, and never deletes or archives an item. It does
write back the item's title and its text field, so editing a sticky edits the item.

**Sticky Notes decides how it shows.** Whether a sticky is a window on the desktop (`show_on_desktop`,
true by default) or only listed in Sticky Notes' own window, and the colour picked in Sticky, are
kept in `sticky-local.toml` beside the registry, never in the app's database. That state is dropped
when the app stops flagging the item. A database that is not in
the engine's layout is refused without being touched.

## Finding and starting Sticky Notes (and Flatpak)

Sticky's desktop file declares the URI scheme `organiser-sticky:` (`MimeType=x-scheme-handler/…`,
`Exec=… %u`). An app in a Flatpak sandbox cannot see the host, so it uses the desktop portals:
`OpenURI.SchemeSupported("organiser-sticky")` says whether Sticky is installed, and
`xdg-open organiser-sticky://start` (the OpenURI portal inside a sandbox) starts it as `--background`
does. Autostart uses the Background portal (`RequestBackground`). Outside a sandbox the same
functions look at `PATH` and write an XDG autostart entry. No Flatpak permission beyond
`xdg-config/organiser` (the shared registry) is needed.
