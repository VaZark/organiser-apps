//! `notes`: the Notes app on the command line. Generic commands come from `organiser-cli-kit`;
//! the rest are thin calls into `notes-schema`.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use std::collections::BTreeMap;

use clap::{Arg, ArgAction, ArgMatches, Command};
use notes_schema::color::Color;
use notes_schema::folder::{Folder, FolderEdit, FolderSummary};
use notes_schema::lifecycle::{
    NewNote, create, create_folder_in, delete_folder, delete_folder_and_notes, edit, edit_folder,
    empty_trash, ensure_default_folder, move_folder, move_folder_into, move_note, purge, restore,
    set_archived, set_favorite, set_pinned, trash,
};
use notes_schema::md_note::{from_markdown, to_markdown};
use notes_schema::vault::Vault;
use notes_schema::note::{Edit, Note};
use notes_schema::views::{Filter, Scope, Sort, folders, notes};
use organiser_cli_kit::{Extension, Output};
use organiser_engine::{Db, Error, Result};
use serde_json::json;

mod import;

/// What the command line keeps while it runs: the vault, when there is one, and the files the
/// notes had when it started (to know which were deleted).
#[derive(Default)]
struct Notes {
    state: std::cell::RefCell<State>,
}

#[derive(Default)]
struct State {
    db_path: Option<std::path::PathBuf>,
    vault: Option<Vault>,
    known: BTreeMap<i64, String>,
}

fn id() -> Arg {
    Arg::new("id")
        .required(true)
        .value_parser(clap::value_parser!(i64))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn line(n: &Note) -> String {
    let mut s = format!(
        "{:>4}  {}{}{}",
        n.id,
        if n.pinned { "* " } else { "" },
        if n.favorite { "<3 " } else { "" },
        n.title
    );
    if let Some(r) = n.rating {
        s.push_str(&format!("  {}{}", "★".repeat(r as usize), "☆".repeat(5usize.saturating_sub(r as usize))));
    }
    if let Some(f) = n.folder_id {
        s.push_str(&format!("  in:{f}"));
    }
    if let Some(c) = n.color {
        s.push_str(&format!("  {c}"));
    }
    if !n.tags.is_empty() {
        s.push_str(&format!("  [{}]", n.tags.join(", ")));
    }
    s
}

fn out(v: &[Note]) -> Output {
    Output::Custom {
        human: if v.is_empty() {
            "(none)".into()
        } else {
            v.iter().map(line).collect::<Vec<_>>().join("\n")
        },
        json: json!(v),
    }
}

/// `--color`: a colour, `none` to clear it, or (absent) leave it alone.
fn color_arg(m: &ArgMatches) -> Result<Option<Option<Color>>> {
    match m.try_get_one::<String>("color").ok().flatten() {
        None => Ok(None),
        Some(v) if v.eq_ignore_ascii_case("none") => Ok(Some(None)),
        Some(v) => Color::parse(v)
            .map(|c| Some(Some(c)))
            .ok_or_else(|| Error::Invalid(format!("`{v}` is not a colour; expected #rrggbb or `none`"))),
    }
}

fn folder_out(f: &Folder, what: &str) -> Output {
    Output::Custom {
        human: format!("{what} section {} ({})", f.id, f.title),
        json: json!(f),
    }
}

/// The folders as a tree: each folder under the one it is in, indented, in order.
fn folders_out(v: &[FolderSummary]) -> Output {
    fn walk(v: &[FolderSummary], parent: Option<i64>, depth: usize, lines: &mut Vec<String>) {
        for s in v.iter().filter(|s| s.folder.parent == parent) {
            let f = &s.folder;
            let mut l = format!(
                "{:>4}  {}{}  {} notes ({} with those below), {} favourite",
                f.id,
                "  ".repeat(depth),
                f.title,
                s.notes,
                s.total,
                s.favorites
            );
            if f.is_default {
                l.push_str("  (default)");
            }
            if f.rated {
                l.push_str("  (rated)");
            }
            if let Some(c) = f.color {
                l.push_str(&format!("  {c}"));
            }
            lines.push(l);
            if depth < 64 {
                walk(v, Some(f.id), depth + 1, lines);
            }
        }
    }
    let mut lines = Vec::new();
    walk(v, None, 0, &mut lines);
    Output::Custom {
        human: if lines.is_empty() { "(none)".into() } else { lines.join("\n") },
        json: json!({ "sections": v }),
    }
}

fn one(n: Note) -> Output {
    out(&[n])
}

fn filter(m: &ArgMatches) -> Filter {
    let scope = if m.get_flag("trash") {
        Scope::Trash
    } else if m.get_flag("archive") {
        Scope::Archive
    } else if m.get_flag("favorites") {
        Scope::Favorites
    } else if let Some(f) = m.try_get_one::<i64>("folder").ok().flatten() {
        Scope::Folder(*f)
    } else {
        Scope::Notes
    };
    let recursive = m.try_get_one::<bool>("recursive").ok().flatten().copied().unwrap_or(false);
    let sort = match m.try_get_one::<String>("sort").ok().flatten().map(String::as_str) {
        Some("created") => Sort::Created,
        Some("title") => Sort::Title,
        Some("rating") => Sort::Rating,
        _ => Sort::Recent,
    };
    Filter {
        scope,
        sort,
        text: m.get_one::<String>("query").cloned(),
        // Every tag given must be on the note.
        tags: organiser_engine::TagFilter {
            all: m
                .get_many::<String>("tag")
                .map(|v| v.cloned().collect())
                .unwrap_or_default(),
            ..Default::default()
        },
        include_archived: m.get_flag("all"),
        recursive,
        ..Filter::default()
    }
}

impl Extension for Notes {
    fn default_kind(&self) -> Option<&'static str> {
        Some(notes_schema::NOTE)
    }

    fn database(&self, path: &std::path::Path) {
        self.state.borrow_mut().db_path = Some(path.to_path_buf());
    }

    /// The default section, "Unassigned", exists, and holds the notes that are in no other. With a
    /// vault, its files are read first, so what was changed outside is in.
    fn opened(&self, db: &mut Db) -> Result<()> {
        ensure_default_folder(db)?;
        let mut st = self.state.borrow_mut();
        if let Some(dir) = configured_vault() {
            let vault = Vault::open(dir, st.db_path.as_deref())?;
            vault.sync(db)?;
            st.known = vault.known(db)?;
            st.vault = Some(vault);
        }
        Ok(())
    }

    /// With a vault, the changes this command made reach the files.
    fn closed(&self, db: &mut Db) -> Result<()> {
        let st = self.state.borrow();
        if let Some(v) = &st.vault {
            v.flush(db, &st.known)?;
        }
        Ok(())
    }

    fn commands(&self) -> Vec<Command> {
        vec![
            Command::new("ls")
                .about("List notes, pinned first")
                .arg(
                    Arg::new("tag")
                        .long("tag")
                        .short('t')
                        .action(ArgAction::Append),
                )
                .arg(
                    Arg::new("query")
                        .long("query")
                        .short('q')
                        .help("full-text search"),
                )
                .arg(
                    Arg::new("archive")
                        .long("archive")
                        .action(ArgAction::SetTrue)
                        .help("list the archive"),
                )
                .arg(
                    Arg::new("trash")
                        .long("trash")
                        .action(ArgAction::SetTrue)
                        .help("list the trash"),
                )
                .arg(
                    Arg::new("all")
                        .long("all")
                        .action(ArgAction::SetTrue)
                        .help("include archived notes"),
                )
                .arg(
                    Arg::new("favorites")
                        .long("favorites")
                        .action(ArgAction::SetTrue)
                        .help("only the favourites"),
                )
                .arg(
                    Arg::new("folder")
                        .long("section")
                        .alias("folder")
                        .value_parser(clap::value_parser!(i64))
                        .help("only the notes in this section"),
                )
                .arg(
                    Arg::new("recursive")
                        .long("recursive")
                        .short('r')
                        .action(ArgAction::SetTrue)
                        .help("with --section: the notes of the sections inside it too"),
                )
                .arg(
                    Arg::new("sort")
                        .long("sort")
                        .value_parser(["recent", "created", "title", "rating"])
                        .default_value("recent"),
                ),
            Command::new("sections")
                .alias("folders")
                .about("List the sections as a tree, with how many notes each has"),
            Command::new("section-new")
                .alias("folder-new")
                .about("Create a section, at the top or inside another with --parent")
                .arg(Arg::new("title").required(true))
                .arg(
                    Arg::new("parent")
                        .long("parent")
                        .value_parser(clap::value_parser!(i64))
                        .help("the section to put it in"),
                )
                .arg(
                    Arg::new("rated")
                        .long("rated")
                        .action(ArgAction::SetTrue)
                        .help("its notes can be rated"),
                )
                .arg(Arg::new("color").long("color").help("#rrggbb"))
                .arg(Arg::new("icon").long("icon").help("an icon name (heart, star, home…)")),
            Command::new("section-set")
                .alias("folder-set")
                .about("Change a section. Switching rated off hides the ratings, it does not lose them")
                .arg(id())
                .arg(Arg::new("title").long("title"))
                .arg(Arg::new("color").long("color").help("#rrggbb, or none"))
                .arg(Arg::new("icon").long("icon").help("an icon name (heart, star, home…), or none"))
                .arg(Arg::new("notes").long("notes").help("what the section is for"))
                .arg(
                    Arg::new("rated")
                        .long("rated")
                        .value_name("on|off")
                        .help("let notes be rated"),
                ),
            Command::new("section-rm")
                .alias("folder-rm")
                .about("Delete a section; its notes and sections stay in the section it was in (Unassigned at the top), unless --with-notes")
                .arg(id())
                .arg(
                    Arg::new("with-notes")
                        .long("with-notes")
                        .action(ArgAction::SetTrue)
                        .help("delete its notes too"),
                ),
            Command::new("section-mv")
                .alias("folder-mv")
                .about("Put a section inside another (--parent), or at the top (--top); --before puts it before a section it is with")
                .arg(id())
                .arg(
                    Arg::new("parent")
                        .long("parent")
                        .value_parser(clap::value_parser!(i64))
                        .conflicts_with("top")
                        .help("the section to put it in"),
                )
                .arg(
                    Arg::new("top")
                        .long("top")
                        .action(ArgAction::SetTrue)
                        .help("at the top, in no section"),
                )
                .arg(
                    Arg::new("before")
                        .long("before")
                        .value_parser(clap::value_parser!(i64))
                        .help("before this section (in the same section)"),
                ),
            Command::new("move")
                .about("File a note in a section, or (without one) in the default section, Unassigned")
                .arg(id())
                .arg(
                    Arg::new("to")
                        .value_parser(clap::value_parser!(i64))
                        .help("the section; without one the note leaves its section"),
                ),
            Command::new("fave").about("Mark a note as a favourite").arg(id()),
            Command::new("unfave").about("Remove the favourite mark").arg(id()),
            Command::new("rate")
                .about("Rate a note 1 to 5, or `none`; only where its section offers ratings")
                .arg(id())
                .arg(Arg::new("stars").required(true)),
            Command::new("import-faves")
                .about("Copy the lists and entries of a Faves database into sections and notes")
                .arg(
                    Arg::new("from")
                        .long("from")
                        .help("the Faves database (default: where Faves keeps it)"),
                ),
            Command::new("pin").about("Pin a note").arg(id()),
            Command::new("unpin").about("Unpin a note").arg(id()),
            Command::new("archive").about("Archive a note").arg(id()),
            Command::new("unarchive")
                .about("Move a note out of the archive")
                .arg(id()),
            Command::new("trash")
                .about("Move a note to the trash")
                .arg(id()),
            Command::new("restore")
                .about("Restore a note from the trash")
                .arg(id()),
            Command::new("purge")
                .about("Delete a trashed note for good")
                .arg(id()),
            Command::new("empty-trash").about("Delete every trashed note for good"),
            Command::new("vault")
                .about("Keep the notes as Markdown files in a directory (each section is a directory, to any depth)")
                .subcommand_required(true)
                .subcommand(
                    Command::new("status").about("Show the directory and how many notes and files there are"),
                )
                .subcommand(Command::new("sync").about("Read the files, then write the notes out"))
                .subcommand(
                    Command::new("set")
                        .about("Choose the directory (read at once), or stop with --off")
                        .arg(Arg::new("dir"))
                        .arg(
                            Arg::new("default")
                                .long("default")
                                .action(ArgAction::SetTrue)
                                .help("use Notes in the documents folder"),
                        )
                        .arg(
                            Arg::new("off")
                                .long("off")
                                .action(ArgAction::SetTrue)
                                .help("keep the notes in the database alone again (the files stay)"),
                        ),
                )
                .subcommand(
                    Command::new("add")
                        .about("Move a Markdown file into the vault (a link to it if it cannot be moved)")
                        .arg(Arg::new("file").required(true))
                        .arg(
                            Arg::new("folder")
                                .long("section")
                                .alias("folder")
                                .short('s')
                                .help("the section, by name or as a path like Work/Design"),
                        ),
                ),
            Command::new("export")
                .about("Write a note as Markdown (to a file with -o, else to the screen)")
                .arg(id())
                .arg(Arg::new("out").long("out").short('o').help("file to write")),
            Command::new("import")
                .about("Create a note from a Markdown file; a first `# heading` is its title")
                .arg(Arg::new("file").required(true))
                .arg(
                    Arg::new("tag")
                        .long("tag")
                        .short('t')
                        .action(ArgAction::Append),
                ),
            Command::new("color")
                .about("Set a note's colour to any #rrggbb, or `none` to clear it")
                .arg(id())
                .arg(Arg::new("value").required(true)),
        ]
    }

    fn run(&self, db: &mut Db, name: &str, m: &ArgMatches) -> Option<Result<Output>> {
        let id = m
            .try_get_one::<i64>("id")
            .ok()
            .flatten()
            .copied()
            .unwrap_or(0);
        Some(match name {
            "ls" => notes(db, &filter(m)).map(|v| out(&v)),
            "vault" => vault_command(self, db, m),
            "sections" => folders(db).map(|v| folders_out(&v)),
            "section-new" => {
                let title = m.get_one::<String>("title").map_or("", String::as_str);
                color_arg(m).and_then(|c| {
                    let f = create_folder_in(db, m.get_one::<i64>("parent").copied(), title, m.get_flag("rated"))?;
                    let icon = m.get_one::<String>("icon").cloned().map(Some);
                    let f = if c.is_some() || icon.is_some() {
                        edit_folder(
                            db,
                            f.id,
                            &FolderEdit {
                                color: c,
                                icon,
                                ..FolderEdit::default()
                            },
                        )?
                    } else {
                        f
                    };
                    Ok(folder_out(&f, "created"))
                })
            }
            "section-set" => color_arg(m).and_then(|c| {
                let rated = match m.get_one::<String>("rated").map(String::as_str) {
                    None => None,
                    Some("on") => Some(true),
                    Some("off") => Some(false),
                    Some(other) => {
                        return Err(Error::Invalid(format!("`{other}` is not on or off")));
                    }
                };
                let f = edit_folder(
                    db,
                    id,
                    &FolderEdit {
                        title: m.get_one::<String>("title").cloned(),
                        notes: m.get_one::<String>("notes").cloned(),
                        rated,
                        color: c,
                        icon: m
                            .get_one::<String>("icon")
                            .map(|i| (!i.eq_ignore_ascii_case("none")).then(|| i.clone())),
                        ..FolderEdit::default()
                    },
                )?;
                Ok(folder_out(&f, "updated"))
            }),
            "section-rm" => if m.get_flag("with-notes") {
                delete_folder_and_notes(db, id)
                    .map(|n| Output::Message(format!("deleted the section and {n} notes")))
            } else {
                delete_folder(db, id).map(|()| Output::Message("deleted the section".into()))
            },
            "section-mv" => {
                // Into a folder or to the top first, then to its place among the others.
                let into = if m.get_flag("top") {
                    Some(None)
                } else {
                    m.get_one::<i64>("parent").map(|p| Some(*p))
                };
                let before = m.get_one::<i64>("before").copied();
                (|| {
                    if let Some(p) = into {
                        move_folder_into(db, id, p)?;
                    }
                    if before.is_some() {
                        move_folder(db, id, before)?;
                    }
                    if into.is_none() && before.is_none() {
                        return Err(Error::Invalid("say where: --parent, --top or --before".into()));
                    }
                    Ok(Output::Message(format!("moved section {id}")))
                })()
            }
            "move" => move_note(db, id, m.get_one::<i64>("to").copied()).map(one),
            "fave" => set_favorite(db, id, true).map(one),
            "unfave" => set_favorite(db, id, false).map(one),
            "rate" => {
                let v = m.get_one::<String>("stars").map_or("", String::as_str);
                let rating = if v.eq_ignore_ascii_case("none") {
                    Ok(None)
                } else {
                    v.parse::<u8>()
                        .ok()
                        .filter(|r| (1..=5).contains(r))
                        .map(Some)
                        .ok_or_else(|| Error::Invalid(format!("`{v}` is not 1 to 5 or `none`")))
                };
                rating.and_then(|r| {
                    edit(
                        db,
                        id,
                        &Edit {
                            rating: Some(r),
                            ..Edit::default()
                        },
                    )
                    .map(one)
                })
            }
            "import-faves" => import::run(db, m.get_one::<String>("from").map(String::as_str)),
            "pin" => set_pinned(db, id, true).map(one),
            "unpin" => set_pinned(db, id, false).map(one),
            "archive" => set_archived(db, id, true).map(one),
            "unarchive" => set_archived(db, id, false).map(one),
            "trash" => trash(db, id, now()).map(one),
            "restore" => restore(db, id).map(one),
            "purge" => purge(db, id).map(|()| Output::Message(format!("deleted note {id}"))),
            "empty-trash" => empty_trash(db).map(|n| Output::Message(format!("deleted {n} notes"))),
            "export" => db.get(id).and_then(|item| {
                let md = to_markdown(&Note::from(&item));
                match m.get_one::<String>("out") {
                    Some(path) => std::fs::write(path, &md)
                        .map(|()| Output::Message(format!("wrote {path}")))
                        .map_err(|e| Error::Invalid(format!("cannot write {path}: {e}"))),
                    None => Ok(Output::Custom {
                        human: md.trim_end().to_string(),
                        json: json!({ "markdown": md }),
                    }),
                }
            }),
            "import" => {
                let path = m.get_one::<String>("file").map_or("", String::as_str);
                std::fs::read_to_string(path)
                    .map_err(|e| Error::Invalid(format!("cannot read {path}: {e}")))
                    .and_then(|text| {
                        let stem = std::path::Path::new(path)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Imported note");
                        let (title, body) = from_markdown(&text, stem);
                        let tags = m
                            .get_many::<String>("tag")
                            .map(|v| v.cloned().collect())
                            .unwrap_or_default();
                        create(
                            db,
                            &NewNote {
                                title,
                                body,
                                tags,
                                ..NewNote::default()
                            },
                        )
                    })
                    .map(one)
            }
            "color" => {
                let v = m.get_one::<String>("value").map_or("", String::as_str);
                let color = if v.eq_ignore_ascii_case("none") {
                    Ok(None)
                } else {
                    Color::parse(v).map(Some).ok_or_else(|| {
                        Error::Invalid(format!("`{v}` is not a colour; expected #rrggbb or `none`"))
                    })
                };
                color.and_then(|c| {
                    edit(
                        db,
                        id,
                        &Edit {
                            color: Some(c),
                            ..Edit::default()
                        },
                    )
                    .map(one)
                })
            }
            _ => return None,
        })
    }
}

fn main() -> ExitCode {
    organiser_cli_kit::run(&notes_schema::META, &notes_schema::SCHEMA, &Notes::default())
}

/// The vault directory the settings name; `NOTES_VAULT` overrides it (for trying one out), and
/// set to nothing it means no vault at all, whatever the settings say (for scripts and tests that
/// must not touch the person's own).
fn configured_vault() -> Option<std::path::PathBuf> {
    match std::env::var_os("NOTES_VAULT") {
        Some(v) if v.is_empty() => None,
        Some(v) => Some(std::path::PathBuf::from(v)),
        None => organiser_core::settings::Settings::load().settings.notes.vault_dir,
    }
}

/// `vault` and its subcommands.
fn vault_command(app: &Notes, db: &mut Db, sub: &ArgMatches) -> Result<Output> {
    let (name, m) = sub.subcommand().expect("a vault subcommand is required");
    match name {
        "set" => {
            let mut loaded = organiser_core::settings::Settings::load();
            if m.get_flag("off") {
                loaded.settings.notes.vault_dir = None;
                loaded.settings.save().map_err(Error::Invalid)?;
                return Ok(Output::Message("the notes are kept in the database alone".into()));
            }
            let dir = if m.get_flag("default") {
                organiser_core::paths::documents_folder(&notes_schema::META)
            } else {
                m.get_one::<String>("dir")
                    .map(std::path::PathBuf::from)
                    .ok_or_else(|| Error::Invalid("give a directory, --default or --off".into()))?
            };
            let dir = std::path::absolute(&dir).unwrap_or(dir);
            let db_path = app.state.borrow().db_path.clone();
            let vault = Vault::open(&dir, db_path.as_deref())?;
            let known = vault.known(db)?;
            let mut rep = vault.sync(db)?;
            rep.add(vault.flush(db, &known)?);
            loaded.settings.notes.vault_dir = Some(vault.dir().to_path_buf());
            loaded.settings.save().map_err(Error::Invalid)?;
            Ok(report_out(&format!("notes are kept in {}", vault.dir().display()), &rep))
        }
        _ => {
            let st = app.state.borrow();
            let Some(vault) = &st.vault else {
                return Err(Error::Invalid(
                    "there is no vault: choose a directory with `notes vault set <dir>`".into(),
                ));
            };
            match name {
                "status" => {
                    let files = vault.known(db)?.len();
                    let all = db
                        .list(&organiser_engine::Query {
                            kind: Some(notes_schema::NOTE.into()),
                            ..Default::default()
                        })?
                        .len();
                    Ok(Output::Message(format!(
                        "{}: {files} of {all} notes have a file",
                        vault.dir().display()
                    )))
                }
                "sync" => {
                    // `opened` already read the files and `closed` writes the notes: say what changed.
                    let mut rep = vault.sync(db)?;
                    rep.add(vault.flush(db, &st.known)?);
                    Ok(report_out("done", &rep))
                }
                _ => {
                    let folder = match m.get_one::<String>("folder") {
                        Some(path) => Some(folder_by_path(db, path)?),
                        None => None,
                    };
                    let file = m.get_one::<String>("file").map_or("", String::as_str);
                    let (at, linked) = vault.add_file(db, std::path::Path::new(file), folder)?;
                    vault.sync(db)?;
                    Ok(Output::Message(format!(
                        "{} {}",
                        if linked { "linked as" } else { "moved to" },
                        at.display()
                    )))
                }
            }
        }
    }
}

/// A folder by its name or a path of names (`Work/Design`), case ignored: each name is one of the
/// folders in the one before.
fn folder_by_path(db: &Db, path: &str) -> Result<i64> {
    let all = folders(db)?;
    let mut parent: Option<i64> = None;
    for name in path.split('/').map(str::trim).filter(|n| !n.is_empty()) {
        let hit = all
            .iter()
            .find(|s| s.folder.parent == parent && s.folder.title.eq_ignore_ascii_case(name))
            // A lone name: any folder of that name, wherever it is.
            .or_else(|| {
                (parent.is_none() && !path.contains('/'))
                    .then(|| all.iter().find(|s| s.folder.title.eq_ignore_ascii_case(name)))
                    .flatten()
            })
            .ok_or_else(|| Error::Invalid(format!("there is no section `{name}` in `{path}`")))?;
        parent = Some(hit.folder.id);
    }
    parent.ok_or_else(|| Error::Invalid("give a section".into()))
}

fn report_out(head: &str, rep: &notes_schema::vault::Report) -> Output {
    let mut human = format!("{head}: {}", rep.summary());
    for w in &rep.warnings {
        human.push_str(&format!("\nwarning: {w}"));
    }
    Output::Custom {
        human,
        json: json!({
            "created": rep.created, "updated": rep.updated, "moved": rep.moved,
            "trashed": rep.trashed, "written": rep.written, "removed": rep.removed,
            "conflicts": rep.conflicts, "warnings": rep.warnings,
        }),
    }
}
