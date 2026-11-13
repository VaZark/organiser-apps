//! `todo`: the Todo app on the command line. Generic commands come from `organiser-cli-kit`;
//! the rest are thin calls into `todo-schema`.

use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command};
use organiser_cli_kit::{Extension, Output};
use organiser_engine::{Db, Error, Node, Result, TagFilter};
use serde_json::json;
use todo_schema::due::label;
use todo_schema::lifecycle::{
    create_epic, create_section, delete_epic, delete_section, edit_epic, reparent, set_archived,
    set_status,
};
use todo_schema::status::Statuses;
use todo_schema::statuses::{self, StatusEdit, ensure_defaults};
use todo_schema::task::{Edit, EpicSummary, Task};
use todo_schema::vault::Vault;
use todo_schema::views::{Filter, Scope, epics, forest, tasks, today};

/// What the command line keeps while it runs: the vault, when there is one, and the files the
/// tasks had when it started (to know which were deleted).
#[derive(Default)]
struct Todo {
    state: std::cell::RefCell<State>,
}

#[derive(Default)]
struct State {
    db_path: Option<std::path::PathBuf>,
    vault: Option<Vault>,
    known: std::collections::BTreeMap<i64, String>,
}

fn id() -> Arg {
    Arg::new("id")
        .required(true)
        .value_parser(clap::value_parser!(i64))
}

/// `[ ]` to do, `[~]` open but past the start, `[x]` finished.
fn mark(st: &Statuses, id: Option<i64>) -> &'static str {
    match st.resolve(id) {
        Some(s) if s.closed => "[x]",
        Some(s) if st.first_open().map(|f| f.id) != Some(s.id) => "[~]",
        _ => "[ ]",
    }
}

/// ` (Name)` unless it is the status a new task starts in.
fn status_tag(st: &Statuses, id: Option<i64>) -> String {
    match st.resolve(id) {
        Some(s) if st.first_open().map(|f| f.id) != Some(s.id) => format!("  ({})", s.name),
        _ => String::new(),
    }
}

fn line(t: &Task, depth: usize, st: &Statuses) -> String {
    let mut s = format!(
        "{}{} {:>4}  {}",
        "    ".repeat(depth),
        mark(st, t.status),
        t.id,
        t.title
    );
    s.push_str(&status_tag(st, t.status));
    if t.priority.is_urgent() {
        s.push_str(&format!("  !{}", t.priority.label().to_lowercase()));
    }
    if let Some(d) = t.due {
        s.push_str(&format!("  ({})", label(d, today())));
    }
    if let Some(e) = t.epic {
        s.push_str(&format!("  epic:{e}"));
    }
    if !t.tags.is_empty() {
        s.push_str(&format!("  [{}]", t.tags.join(", ")));
    }
    s
}

fn walk(nodes: &[Node<Task>], depth: usize, out: &mut Vec<String>, st: &Statuses) {
    for n in nodes {
        out.push(line(&n.value, depth, st));
        walk(&n.children, depth + 1, out, st);
    }
}

fn filter(m: &ArgMatches, scope: Scope) -> Filter {
    let many = |name: &str| -> Vec<String> {
        m.try_get_many::<String>(name)
            .ok()
            .flatten()
            .map(|v| v.cloned().collect())
            .unwrap_or_default()
    };
    Filter {
        scope,
        show_closed: m
            .try_get_one::<bool>("all")
            .ok()
            .flatten()
            .copied()
            .unwrap_or(false),
        text: m.try_get_one::<String>("query").ok().flatten().cloned(),
        tags: TagFilter {
            all: many("tag"),
            ..TagFilter::default()
        },
        epic: m.try_get_one::<i64>("epic").ok().flatten().copied(),
        no_epic: false,
        include_archived: false,
        hide_subtasks: false,
        sort: Default::default(),
        fields: Default::default(),
    }
}

fn list_args(c: Command) -> Command {
    c.arg(
        Arg::new("tag")
            .long("tag")
            .short('t')
            .action(ArgAction::Append)
            .help("only tasks with this tag"),
    )
    .arg(
        Arg::new("query")
            .long("query")
            .short('q')
            .help("full-text search"),
    )
    .arg(
        Arg::new("epic")
            .long("epic")
            .value_parser(clap::value_parser!(i64))
            .help("only tasks in this epic"),
    )
    .arg(
        Arg::new("all")
            .long("all")
            .action(ArgAction::SetTrue)
            .help("include done and won't-do tasks"),
    )
}

fn tasks_out(v: &[Task], st: &Statuses) -> Output {
    Output::Custom {
        human: if v.is_empty() {
            "(none)".into()
        } else {
            v.iter()
                .map(|t| line(t, 0, st))
                .collect::<Vec<_>>()
                .join("\n")
        },
        json: json!(v),
    }
}

fn epics_out(v: &[EpicSummary]) -> Output {
    let human = v
        .iter()
        .map(|s| {
            let e = &s.epic;
            let mut l = format!(
                "{:>4}  {}  {}/{} done",
                e.id,
                e.title,
                s.total - s.open,
                s.total
            );
            if let Some(d) = e.due {
                l.push_str(&format!("  ({})", label(d, today())));
            }
            l
        })
        .collect::<Vec<_>>()
        .join("\n");
    Output::Custom {
        human: if v.is_empty() { "(none)".into() } else { human },
        json: json!(v),
    }
}

impl Extension for Todo {
    fn default_kind(&self) -> Option<&'static str> {
        Some(todo_schema::TASK)
    }

    fn database(&self, path: &std::path::Path) {
        self.state.borrow_mut().db_path = Some(path.to_path_buf());
    }

    /// With a vault, its files are read first, so what was changed outside is in.
    fn opened(&self, db: &mut Db) -> Result<()> {
        ensure_defaults(db)?;
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
            Command::new("start")
                .about("Mark a task in progress")
                .arg(id()),
            Command::new("done")
                .about("Mark a task (and its open subtasks) done")
                .arg(id()),
            Command::new("dismiss")
                .about("Mark a task (and its open subtasks) won't-do")
                .arg(id()),
            Command::new("reopen")
                .about("Reopen a task (and the closed tasks above it)")
                .arg(id()),
            list_args(Command::new("tree").about("Tasks with their subtasks")),
            list_args(Command::new("current").about("The tasks that are In progress")),
            list_args(Command::new("archived").about("The archived tasks")),
            Command::new("archive")
                .about("Archive a task (and its subtasks)")
                .arg(id()),
            Command::new("unarchive")
                .about("Bring a task back from the archive (with the tasks above it)")
                .arg(id()),
            Command::new("vault")
                .about("Keep the tasks as Markdown files in a directory (an epic is a directory; a task with subtasks is a directory with an index.md)")
                .subcommand_required(true)
                .subcommand(Command::new("status").about("Show the directory and how many tasks have a file"))
                .subcommand(Command::new("sync").about("Read the files, then write the tasks out"))
                .subcommand(
                    Command::new("set")
                        .about("Choose the directory (read at once), or stop with --off")
                        .arg(Arg::new("dir"))
                        .arg(Arg::new("default").long("default").action(ArgAction::SetTrue).help("use Todo in the documents folder"))
                        .arg(Arg::new("off").long("off").action(ArgAction::SetTrue).help("keep the tasks in the database alone again (the files stay)")),
                )
                .subcommand(
                    Command::new("add")
                        .about("Move a Markdown file into the vault (a link to it if it cannot be moved)")
                        .arg(Arg::new("file").required(true))
                        .arg(Arg::new("epic").long("epic").short('e').help("the epic, by name")),
                ),
            Command::new("epics").about("List epics with progress"),
            Command::new("epic-new")
                .about("Create an epic")
                .arg(Arg::new("title").required(true))
                .arg(Arg::new("color").long("color").help("#rrggbb"))
                .arg(Arg::new("icon").long("icon").help("an icon name (heart, star, home…)")),
            Command::new("epic-rm")
                .about("Delete an epic; its tasks stay")
                .arg(id()),
            Command::new("section-new")
                .about("Add a section to an epic")
                .arg(id().help("the epic"))
                .arg(Arg::new("title").required(true)),
            Command::new("section-rm")
                .about("Delete a section; its tasks stay in the epic")
                .arg(id()),
            Command::new("move")
                .about("Make a task a subtask of another, or (--out) a task of its own")
                .arg(id())
                .arg(
                    Arg::new("under")
                        .long("under")
                        .value_parser(clap::value_parser!(i64))
                        .help("the task to nest it under"),
                )
                .arg(
                    Arg::new("out")
                        .long("out")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("under")
                        .help("take it out of its parent"),
                ),
            Command::new("status")
                .about("Put a task in a status, by name")
                .arg(id())
                .arg(Arg::new("name").required(true)),
            Command::new("statuses").about("List the statuses, in board order"),
            Command::new("status-new")
                .about("Create a status")
                .arg(Arg::new("name").required(true))
                .arg(Arg::new("color").long("color").help("#rrggbb"))
                .arg(
                    Arg::new("closed")
                        .long("closed")
                        .action(ArgAction::SetTrue)
                        .help("it means finished"),
                ),
            Command::new("status-edit")
                .about("Rename, recolour or change a status")
                .arg(id())
                .arg(Arg::new("name").long("name"))
                .arg(Arg::new("color").long("color").help("#rrggbb, or `none`"))
                .arg(
                    Arg::new("closed")
                        .long("closed")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("open"),
                )
                .arg(Arg::new("open").long("open").action(ArgAction::SetTrue)),
            Command::new("status-move")
                .about("Move a status earlier (-1) or later (1) on the board")
                .arg(id())
                .arg(
                    Arg::new("step")
                        .required(true)
                        .allow_negative_numbers(true)
                        .value_parser(clap::value_parser!(i32)),
                ),
            Command::new("status-rm")
                .about("Delete a status; its tasks move to another of the same kind")
                .arg(id()),
        ]
    }

    fn run(&self, db: &mut Db, name: &str, m: &ArgMatches) -> Option<Result<Output>> {
        // Every command starts from a database that has its statuses (and has had older data
        // moved onto them), including the generic ones such as `add` and `list`.
        if let Err(e) = ensure_defaults(db) {
            return Some(Err(e));
        }
        let st = match statuses::load(db) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        let id = m
            .try_get_one::<i64>("id")
            .ok()
            .flatten()
            .copied()
            .unwrap_or(0);
        let text = |k: &str| m.try_get_one::<String>(k).ok().flatten().cloned();

        // The commands that put a task in a status: which one is looked up, not assumed.
        let target = match name {
            "start" => Some(
                st.by_name("In progress")
                    .or_else(|| st.open().nth(1))
                    .map(|s| s.id),
            ),
            "done" => Some(st.first_closed().map(|s| s.id)),
            "dismiss" => Some(
                st.by_name("Won't do")
                    .or_else(|| st.closed().nth(1))
                    .map(|s| s.id),
            ),
            "reopen" => Some(st.first_open().map(|s| s.id)),
            "status" => {
                let want = text("name").unwrap_or_default();
                Some(match st.by_name(&want) {
                    Some(s) => Some(s.id),
                    None => {
                        let have: Vec<&str> = st.all().iter().map(|s| s.name.as_str()).collect();
                        return Some(Err(Error::Invalid(format!(
                            "there is no status `{want}`; the statuses are: {}",
                            have.join(", ")
                        ))));
                    }
                })
            }
            _ => None,
        };
        if let Some(t) = target {
            let Some(t) = t else {
                return Some(Err(Error::Invalid(format!(
                    "no status fits `{name}`; see `statuses`"
                ))));
            };
            let out = set_status(db, id, t);
            return Some(out.and_then(|changed| Ok(tasks_out(&changed, &statuses::load(db)?))));
        }

        match name {
            "current" => {
                Some(tasks(db, &filter(m, Scope::Current), today()).map(|t| tasks_out(&t, &st)))
            }
            "archived" => Some(tasks(db, &filter(m, Scope::Archived), today()).map(|t| tasks_out(&t, &st))),
            "archive" => Some(set_archived(db, id, true).map(|t| tasks_out(&[t], &st))),
            "unarchive" => Some(set_archived(db, id, false).map(|t| tasks_out(&[t], &st))),
            "vault" => Some(vault_command(self, db, m)),
            "tree" => Some(forest(db, &filter(m, Scope::All), today()).and_then(|f| {
                let mut lines = Vec::new();
                walk(&f, 0, &mut lines, &st);
                let flat = tasks(db, &filter(m, Scope::All), today())?;
                Ok(Output::Custom {
                    human: if lines.is_empty() {
                        "(none)".into()
                    } else {
                        lines.join("\n")
                    },
                    json: json!(flat),
                })
            })),
            "epics" => Some(epics(db).map(|v| epics_out(&v))),
            "epic-new" => Some(
                create_epic(db, m.get_one::<String>("title").map_or("", String::as_str))
                    .and_then(|e| {
                        // A colour and an icon given at the start.
                        let (color, icon) =
                            (m.get_one::<String>("color"), m.get_one::<String>("icon"));
                        if color.is_none() && icon.is_none() {
                            return Ok(e);
                        }
                        edit_epic(
                            db,
                            e.id,
                            &Edit {
                                color: color.map(|c| Some(c.clone())),
                                icon: icon.map(|i| Some(i.clone())),
                                ..Edit::default()
                            },
                        )
                    })
                    .map(|e| Output::Custom {
                        human: format!("created epic {} ({})", e.id, e.title),
                        json: json!(e),
                    }),
            ),
            "section-new" => Some(
                create_section(db, id, m.get_one::<String>("title").map_or("", String::as_str))
                    .map(|s| Output::Custom {
                        human: format!("created section {} ({})", s.id, s.title),
                        json: json!(s),
                    }),
            ),
            "section-rm" => {
                Some(delete_section(db, id).map(|()| Output::Message("deleted section".into())))
            }
            "move" => Some({
                let under = m.get_one::<i64>("under").copied();
                if under.is_none() && !m.get_flag("out") {
                    Err(Error::Invalid("give --under TASK or --out".into()))
                } else {
                    reparent(db, id, under).map(|t| tasks_out(&[t], &st))
                }
            }),
            "epic-rm" => Some(delete_epic(db, id).map(|()| Output::Message("deleted epic".into()))),
            "statuses" => Some(Ok(statuses_out(&st))),
            "status-new" => Some(
                statuses::create(
                    db,
                    &text("name").unwrap_or_default(),
                    text("color").as_deref(),
                    m.get_flag("closed"),
                )
                .and_then(|_| Ok(statuses_out(&statuses::load(db)?))),
            ),
            "status-edit" => {
                let color = text("color").map(|c| (!c.eq_ignore_ascii_case("none")).then_some(c));
                let closed = if m.get_flag("closed") {
                    Some(true)
                } else if m.get_flag("open") {
                    Some(false)
                } else {
                    None
                };
                let edit = StatusEdit {
                    name: text("name"),
                    color,
                    closed,
                };
                Some(
                    statuses::edit(db, id, &edit)
                        .and_then(|_| Ok(statuses_out(&statuses::load(db)?))),
                )
            }
            "status-move" => {
                let step = m
                    .try_get_one::<i32>("step")
                    .ok()
                    .flatten()
                    .copied()
                    .unwrap_or(0);
                Some(
                    statuses::move_by(db, id, step)
                        .and_then(|()| Ok(statuses_out(&statuses::load(db)?))),
                )
            }
            "status-rm" => {
                Some(statuses::delete(db, id).and_then(|()| Ok(statuses_out(&statuses::load(db)?))))
            }
            _ => None,
        }
    }
}

fn statuses_out(st: &Statuses) -> Output {
    let human = st
        .all()
        .iter()
        .map(|s| {
            format!(
                "{:>4}  {:<14} {}  {}",
                s.id,
                s.name,
                if s.closed { "closed" } else { "open  " },
                s.color.as_deref().unwrap_or("-"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Output::Custom {
        human,
        json: json!(st.all()),
    }
}

fn main() -> ExitCode {
    organiser_cli_kit::run(&todo_schema::META, &todo_schema::SCHEMA, &Todo::default())
}

/// The vault directory the settings name; `TODO_VAULT` overrides it (for trying one out), and set
/// to nothing it means no vault at all, whatever the settings say (for scripts and tests that must
/// not touch the person's own).
fn configured_vault() -> Option<std::path::PathBuf> {
    match std::env::var_os("TODO_VAULT") {
        Some(v) if v.is_empty() => None,
        Some(v) => Some(std::path::PathBuf::from(v)),
        None => organiser_core::settings::Settings::load().settings.todo.vault_dir,
    }
}

fn report_out(head: &str, rep: &todo_schema::vault::Report) -> Output {
    let mut human = format!("{head}: {}", rep.summary_with("archived"));
    for w in &rep.warnings {
        human.push_str(&format!("\nwarning: {w}"));
    }
    Output::Custom {
        human,
        json: json!({
            "created": rep.created, "updated": rep.updated, "moved": rep.moved,
            "archived": rep.trashed, "written": rep.written, "removed": rep.removed,
            "conflicts": rep.conflicts, "warnings": rep.warnings,
        }),
    }
}

/// `vault` and its subcommands.
fn vault_command(app: &Todo, db: &mut Db, sub: &ArgMatches) -> Result<Output> {
    let (name, m) = sub.subcommand().expect("a vault subcommand is required");
    if name == "set" {
        let mut loaded = organiser_core::settings::Settings::load();
        if m.get_flag("off") {
            loaded.settings.todo.vault_dir = None;
            loaded.settings.save().map_err(Error::Invalid)?;
            return Ok(Output::Message("the tasks are kept in the database alone".into()));
        }
        let dir = if m.get_flag("default") {
            organiser_core::paths::documents_folder(&todo_schema::META)
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
        loaded.settings.todo.vault_dir = Some(vault.dir().to_path_buf());
        loaded.settings.save().map_err(Error::Invalid)?;
        return Ok(report_out(&format!("tasks are kept in {}", vault.dir().display()), &rep));
    }
    let st = app.state.borrow();
    let Some(vault) = &st.vault else {
        return Err(Error::Invalid("there is no vault: choose a directory with `todo vault set <dir>`".into()));
    };
    match name {
        "status" => {
            let files = vault.known(db)?.len();
            let all = db
                .list(&organiser_engine::Query { kind: Some(todo_schema::TASK.into()), ..Default::default() })?
                .len();
            Ok(Output::Message(format!("{}: {files} of {all} tasks have a file", vault.dir().display())))
        }
        "sync" => {
            let mut rep = vault.sync(db)?;
            rep.add(vault.flush(db, &st.known)?);
            Ok(report_out("done", &rep))
        }
        _ => {
            let epic = match m.get_one::<String>("epic") {
                Some(n) => Some(
                    todo_schema::lifecycle::epic_by_title(db, n)
                        .ok_or_else(|| Error::Invalid(format!("there is no epic `{n}`")))?,
                ),
                None => None,
            };
            let file = m.get_one::<String>("file").map_or("", String::as_str);
            let (at, linked) = vault.add_file(db, std::path::Path::new(file), epic)?;
            vault.sync(db)?;
            Ok(Output::Message(format!("{} {}", if linked { "linked as" } else { "moved to" }, at.display())))
        }
    }
}
