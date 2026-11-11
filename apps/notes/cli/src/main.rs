//! `notes`: the Notes app on the command line. Generic commands come from `organiser-cli-kit`;
//! the rest are thin calls into `notes-schema`.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Arg, ArgAction, ArgMatches, Command};
use notes_schema::color::Color;
use notes_schema::lifecycle::{
    NewNote, create, edit, empty_trash, purge, restore, set_archived, set_pinned, trash,
};
use notes_schema::md_note::{from_markdown, to_markdown};
use notes_schema::note::{Edit, Note};
use notes_schema::views::{Filter, Scope, notes};
use organiser_cli_kit::{Extension, Output};
use organiser_engine::{Db, Error, Result};
use serde_json::json;

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
        "{:>4}  {}{}",
        n.id,
        if n.pinned { "* " } else { "" },
        n.title
    );
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

fn one(n: Note) -> Output {
    out(&[n])
}

fn filter(m: &ArgMatches) -> Filter {
    let scope = if m.get_flag("trash") {
        Scope::Trash
    } else if m.get_flag("archive") {
        Scope::Archive
    } else {
        Scope::Notes
    };
    Filter {
        scope,
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
