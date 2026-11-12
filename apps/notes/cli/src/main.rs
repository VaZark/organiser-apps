//! `notes`: the Notes app on the command line. Generic commands come from `organiser-cli-kit`;
//! the rest are thin calls into `notes-schema`.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Arg, ArgAction, ArgMatches, Command};
use notes_schema::color::Color;
use notes_schema::folder::{Folder, FolderEdit, FolderSummary};
use notes_schema::section::SectionEdit;
use notes_schema::lifecycle::{
    NewNote, create, create_folder, create_section, delete_folder, delete_folder_and_notes,
    delete_section, edit, edit_folder, edit_section, empty_trash, ensure_default_folder, move_note, purge, restore,
    set_archived, set_favorite, set_pinned, set_section, trash,
};
use notes_schema::md_note::{from_markdown, to_markdown};
use notes_schema::note::{Edit, Note};
use notes_schema::views::{Filter, Scope, Sort, folders, notes};
use organiser_cli_kit::{Extension, Output};
use organiser_engine::{Db, Error, Result};
use serde_json::json;

mod import;

struct Notes;

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
        if let Some(sec) = n.section {
            s.push_str(&format!("/{sec}"));
        }
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
        human: format!("{what} folder {} ({})", f.id, f.title),
        json: json!(f),
    }
}

fn folders_out(v: &[FolderSummary]) -> Output {
    let lines: Vec<String> = v
        .iter()
        .map(|s| {
            let f = &s.folder;
            let mut l = format!("{:>4}  {}  {} notes, {} favourite", f.id, f.title, s.notes, s.favorites);
            if f.is_default {
                l.push_str("  (default)");
            }
            if f.rated {
                l.push_str("  (rated)");
            }
            if let Some(c) = f.color {
                l.push_str(&format!("  {c}"));
            }
            l
        })
        .collect();
    Output::Custom {
        human: if lines.is_empty() { "(none)".into() } else { lines.join("\n") },
        json: json!({ "folders": v }),
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
        ..Filter::default()
    }
}

impl Extension for Notes {
    fn default_kind(&self) -> Option<&'static str> {
        Some(notes_schema::NOTE)
    }

    /// The default folder, "Unassigned", exists, and holds the notes that are in no other.
    fn opened(&self, db: &mut Db) -> Result<()> {
        ensure_default_folder(db).map(|_| ())
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
                        .long("folder")
                        .value_parser(clap::value_parser!(i64))
                        .help("only the notes in this folder"),
                )
                .arg(
                    Arg::new("sort")
                        .long("sort")
                        .value_parser(["recent", "created", "title", "rating"])
                        .default_value("recent"),
                ),
            Command::new("folders").about("List the folders with how many notes each has"),
            Command::new("folder-new")
                .about("Create a folder")
                .arg(Arg::new("title").required(true))
                .arg(
                    Arg::new("rated")
                        .long("rated")
                        .action(ArgAction::SetTrue)
                        .help("its notes can be rated"),
                )
                .arg(Arg::new("color").long("color").help("#rrggbb"))
                .arg(Arg::new("icon").long("icon").help("an icon name (heart, star, home…)")),
            Command::new("folder-set")
                .about("Change a folder. Switching rated off hides the ratings, it does not lose them")
                .arg(id())
                .arg(Arg::new("title").long("title"))
                .arg(Arg::new("color").long("color").help("#rrggbb, or none"))
                .arg(Arg::new("icon").long("icon").help("an icon name (heart, star, home…), or none"))
                .arg(Arg::new("notes").long("notes").help("what the folder is for"))
                .arg(
                    Arg::new("rated")
                        .long("rated")
                        .value_name("on|off")
                        .help("let notes be rated"),
                ),
            Command::new("folder-rm")
                .about("Delete a folder; its notes stay, in Unassigned, unless --with-notes")
                .arg(id())
                .arg(
                    Arg::new("with-notes")
                        .long("with-notes")
                        .action(ArgAction::SetTrue)
                        .help("delete its notes too"),
                ),
            Command::new("section-new")
                .about("Add a section to a folder")
                .arg(id().help("the folder"))
                .arg(Arg::new("title").required(true))
                .arg(Arg::new("color").long("color").help("#rrggbb"))
                .arg(Arg::new("notes").long("notes").help("what the section is for")),
            Command::new("section-rm")
                .about("Delete a section; its notes stay in the folder")
                .arg(id()),
            Command::new("section")
                .about("Put a note in a section of its folder, or (without one) in none")
                .arg(id())
                .arg(
                    Arg::new("section")
                        .value_parser(clap::value_parser!(i64))
                        .help("the section"),
                ),
            Command::new("move")
                .about("File a note in a folder, or (without one) in the default folder, Unassigned")
                .arg(id())
                .arg(
                    Arg::new("to")
                        .value_parser(clap::value_parser!(i64))
                        .help("the folder; without one the note leaves its folder"),
                ),
            Command::new("fave").about("Mark a note as a favourite").arg(id()),
            Command::new("unfave").about("Remove the favourite mark").arg(id()),
            Command::new("rate")
                .about("Rate a note 1 to 5, or `none`; only where its folder offers ratings")
                .arg(id())
                .arg(Arg::new("stars").required(true)),
            Command::new("import-faves")
                .about("Copy the lists and entries of a Faves database into folders and notes")
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
            "folders" => folders(db).map(|v| folders_out(&v)),
            "folder-new" => {
                let title = m.get_one::<String>("title").map_or("", String::as_str);
                color_arg(m).and_then(|c| {
                    let f = create_folder(db, title, m.get_flag("rated"))?;
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
            "folder-set" => color_arg(m).and_then(|c| {
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
            "folder-rm" => if m.get_flag("with-notes") {
                delete_folder_and_notes(db, id)
                    .map(|n| Output::Message(format!("deleted the folder and {n} notes")))
            } else {
                delete_folder(db, id).map(|()| Output::Message("deleted the folder".into()))
            },
            "section-new" => color_arg(m).and_then(|c| {
                let title = m.get_one::<String>("title").map_or("", String::as_str);
                let s = create_section(db, id, title)?;
                let notes = m.get_one::<String>("notes").cloned();
                let s = if c.is_some() || notes.is_some() {
                    edit_section(
                        db,
                        s.id,
                        &SectionEdit {
                            color: c,
                            notes,
                            ..SectionEdit::default()
                        },
                    )?
                } else {
                    s
                };
                Ok(Output::Custom {
                    human: format!("created section {} ({})", s.id, s.title),
                    json: json!(s),
                })
            }),
            "section-rm" => delete_section(db, id).map(|()| Output::Message("deleted the section".into())),
            "section" => set_section(db, id, m.get_one::<i64>("section").copied()).map(one),
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
    organiser_cli_kit::run(&notes_schema::META, &notes_schema::SCHEMA, &Notes)
}
