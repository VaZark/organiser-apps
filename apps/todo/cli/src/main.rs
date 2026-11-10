//! `todo`: the Todo app on the command line. Generic commands come from `organiser-cli-kit`;
//! the rest are thin calls into `todo-schema`.

use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command};
use organiser_cli_kit::{Extension, Output};
use organiser_engine::{Db, Node, Result, TagFilter};
use serde_json::json;
use todo_schema::due::label;
use todo_schema::lifecycle::{create_epic, delete_epic, set_status};
use todo_schema::status::Status;
use todo_schema::task::{EpicSummary, Task};
use todo_schema::views::{Filter, Scope, epics, forest, tasks, today};

struct Todo;

fn id() -> Arg {
    Arg::new("id")
        .required(true)
        .value_parser(clap::value_parser!(i64))
}

fn mark(s: Status) -> &'static str {
    match s {
        Status::Open => "[ ]",
        Status::InProgress => "[~]",
        Status::Done => "[x]",
        Status::Dismissed => "[-]",
    }
}

fn line(t: &Task, depth: usize) -> String {
    let mut s = format!(
        "{}{} {:>4}  {}",
        "    ".repeat(depth),
        mark(t.status),
        t.id,
        t.title
    );
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

fn walk(nodes: &[Node<Task>], depth: usize, out: &mut Vec<String>) {
    for n in nodes {
        out.push(line(&n.value, depth));
        walk(&n.children, depth + 1, out);
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
        include_archived: false,
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

fn tasks_out(v: &[Task]) -> Output {
    Output::Custom {
        human: if v.is_empty() {
            "(none)".into()
        } else {
            v.iter().map(|t| line(t, 0)).collect::<Vec<_>>().join("\n")
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
                "{} {:>4}  {}  {}/{} done",
                mark(e.status),
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
            list_args(
                Command::new("current").about("Tasks whose start-to-due span includes today"),
            ),
            Command::new("epics")
                .about("List epics with progress")
                .arg(Arg::new("all").long("all").action(ArgAction::SetTrue)),
            Command::new("epic-new")
                .about("Create an epic")
                .arg(Arg::new("title").required(true)),
            Command::new("epic-rm")
                .about("Delete an epic; its tasks stay")
                .arg(id()),
        ]
    }

    fn run(&self, db: &mut Db, name: &str, m: &ArgMatches) -> Option<Result<Output>> {
        let mut status = |s| {
            let id = *m.get_one::<i64>("id").unwrap_or(&0);
            Some(set_status(db, id, s).map(|changed| tasks_out(&changed)))
        };
        match name {
            "start" => return status(Status::InProgress),
            "done" => return status(Status::Done),
            "dismiss" => return status(Status::Dismissed),
            "reopen" => return status(Status::Open),
            _ => {}
        }
        match name {
            "current" => {
                Some(tasks(db, &filter(m, Scope::Current), today()).map(|t| tasks_out(&t)))
            }
            "tree" => Some(forest(db, &filter(m, Scope::All), today()).and_then(|f| {
                let mut lines = Vec::new();
                walk(&f, 0, &mut lines);
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
            "epics" => Some(epics(db, m.get_flag("all")).map(|v| epics_out(&v))),
            "epic-new" => Some(
                create_epic(db, m.get_one::<String>("title").map_or("", String::as_str)).map(|e| {
                    Output::Custom {
                        human: format!("created epic {} ({})", e.id, e.title),
                        json: json!(e),
                    }
                }),
            ),
            "epic-rm" => Some(
                delete_epic(db, *m.get_one::<i64>("id").unwrap_or(&0))
                    .map(|()| Output::Message("deleted epic".into())),
            ),
            _ => None,
        }
    }
}

fn main() -> ExitCode {
    organiser_cli_kit::run(&todo_schema::META, &todo_schema::SCHEMA, &Todo)
}
