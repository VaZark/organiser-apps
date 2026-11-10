//! `faves`: the Faves app on the command line. Generic commands (`add list show edit rm mv tag …`)
//! come from `organiser-cli-kit`; the rest are thin calls into `faves-schema`.
//!
//! An entry belongs to a list: `faves add "Alien" -p 1 -s rating=5` adds it to list 1.

use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command};
use faves_schema::lifecycle::{create_list, delete_list, edit_list, move_entry, set_favorite};
use faves_schema::model::{Entry, ListEdit, ListSummary};
use faves_schema::views::{Filter, Sort, entries, lists};
use organiser_cli_kit::{Extension, Output};
use organiser_engine::{Db, Error, Result};
use serde_json::json;

struct Faves;

fn id() -> Arg {
    Arg::new("id")
        .required(true)
        .value_parser(clap::value_parser!(i64))
}

fn stars(r: Option<u8>) -> String {
    r.map(|r| {
        format!(
            "{}{}",
            "★".repeat(r as usize),
            "☆".repeat(5usize.saturating_sub(r as usize))
        )
    })
    .unwrap_or_default()
}

fn entry_line(e: &Entry) -> String {
    let mut s = format!(
        "{} {:>4}  {}",
        if e.favorite { "♥" } else { " " },
        e.id,
        e.title
    );
    if let Some(st) = e.status {
        s.push_str(&format!("  [{}]", st.label()));
    }
    if e.rating.is_some() {
        s.push_str(&format!("  {}", stars(e.rating)));
    }
    if !e.tags.is_empty() {
        s.push_str(&format!("  #{}", e.tags.join(" #")));
    }
    s
}

fn entries_out(v: &[Entry]) -> Output {
    Output::Custom {
        human: if v.is_empty() {
            "(none)".into()
        } else {
            v.iter().map(entry_line).collect::<Vec<_>>().join("\n")
        },
        json: json!(v),
    }
}

fn lists_out(v: &[ListSummary]) -> Output {
    let human = v
        .iter()
        .map(|s| {
            let mut extras = Vec::new();
            if s.list.rated {
                extras.push("rated");
            }
            if s.list.tracked {
                extras.push("tracked");
            }
            let extras = if extras.is_empty() {
                String::new()
            } else {
                format!("  ({})", extras.join(", "))
            };
            format!(
                "{:>4}  {}  {} entries, {} favourite{extras}",
                s.list.id, s.list.title, s.entries, s.favorites
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Output::Custom {
        human: if v.is_empty() { "(none)".into() } else { human },
        json: json!(v),
    }
}

fn on_off(m: &ArgMatches, name: &str) -> Result<Option<bool>> {
    m.get_one::<String>(name)
        .map(|v| match v.as_str() {
            "on" | "true" | "yes" => Ok(true),
            "off" | "false" | "no" => Ok(false),
            other => Err(Error::Invalid(format!(
                "{name}: expected on or off, got {other:?}"
            ))),
        })
        .transpose()
}

impl Extension for Faves {
    fn default_kind(&self) -> Option<&'static str> {
        Some(faves_schema::ENTRY)
    }

    fn commands(&self) -> Vec<Command> {
        let toggle = |name: &'static str, help: &'static str| {
            Arg::new(name).long(name).value_name("on|off").help(help)
        };
        vec![
            Command::new("lists").about("List your lists with how many entries each has"),
            Command::new("list-new")
                .about("Create a list")
                .arg(Arg::new("title").required(true))
                .arg(
                    Arg::new("rated")
                        .long("rated")
                        .action(ArgAction::SetTrue)
                        .help("its entries can be rated"),
                )
                .arg(
                    Arg::new("tracked")
                        .long("tracked")
                        .action(ArgAction::SetTrue)
                        .help("its entries have a status"),
                ),
            Command::new("list-set")
                .about(
                    "Change a list. Switching rated or tracked off removes the ratings or statuses",
                )
                .arg(id())
                .arg(Arg::new("title").long("title"))
                .arg(Arg::new("notes").long("notes"))
                .arg(toggle("rated", "let entries be rated"))
                .arg(toggle("tracked", "give entries a status")),
            Command::new("list-rm")
                .about("Delete a list and all its entries")
                .arg(id()),
            Command::new("fave")
                .about("Mark an entry as a favourite")
                .arg(id()),
            Command::new("unfave")
                .about("Remove the favourite mark")
                .arg(id()),
            Command::new("move")
                .about("Move an entry to another list")
                .arg(id())
                .arg(
                    Arg::new("to")
                        .required(true)
                        .value_parser(clap::value_parser!(i64)),
                ),
            Command::new("favorites")
                .about("Favourites from every list")
                .arg(
                    Arg::new("sort")
                        .long("sort")
                        .value_parser(["manual", "title", "rating", "recent"])
                        .default_value("rating"),
                ),
        ]
    }

    fn run(&self, db: &mut Db, name: &str, m: &ArgMatches) -> Option<Result<Output>> {
        let id = || *m.get_one::<i64>("id").unwrap_or(&0);
        Some(match name {
            "lists" => lists(db).map(|v| lists_out(&v)),
            "list-new" => create_list(
                db,
                m.get_one::<String>("title").map_or("", String::as_str),
                m.get_flag("rated"),
                m.get_flag("tracked"),
            )
            .map(|l| Output::Custom {
                human: format!("created list {} ({})", l.id, l.title),
                json: json!(l),
            }),
            "list-set" => on_off(m, "rated").and_then(|rated| {
                let tracked = on_off(m, "tracked")?;
                let edit = ListEdit {
                    title: m.get_one::<String>("title").cloned(),
                    notes: m.get_one::<String>("notes").cloned(),
                    rated,
                    tracked,
                };
                edit_list(db, id(), &edit).map(|l| Output::Custom {
                    human: format!("{}: rated {}, tracked {}", l.title, l.rated, l.tracked),
                    json: json!(l),
                })
            }),
            "list-rm" => delete_list(db, id()).map(|n| {
                Output::Message(format!("deleted {n} item{}", if n == 1 { "" } else { "s" }))
            }),
            "fave" => set_favorite(db, id(), true).map(|e| entries_out(&[e])),
            "unfave" => set_favorite(db, id(), false).map(|e| entries_out(&[e])),
            "move" => move_entry(db, id(), *m.get_one::<i64>("to").unwrap_or(&0))
                .map(|e| entries_out(&[e])),
            "favorites" => {
                let sort = match m.get_one::<String>("sort").map(String::as_str) {
                    Some("title") => Sort::Title,
                    Some("recent") => Sort::Recent,
                    Some("manual") => Sort::Manual,
                    _ => Sort::Rating,
                };
                entries(
                    db,
                    &Filter {
                        favorites: true,
                        sort,
                        ..Filter::default()
                    },
                )
                .map(|v| entries_out(&v))
            }
            _ => return None,
        })
    }
}

fn main() -> ExitCode {
    organiser_cli_kit::run(&faves_schema::META, &faves_schema::SCHEMA, &Faves)
}
