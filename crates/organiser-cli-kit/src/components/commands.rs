//! The generic command tree.

use clap::{Arg, ArgAction, Command};
use organiser_core::AppMeta;
use organiser_engine::Schema;

fn many(name: &'static str, long: &'static str, short: Option<char>, help: &'static str) -> Arg {
    let a = Arg::new(name)
        .long(long)
        .action(ArgAction::Append)
        .value_name(name)
        .help(help);
    match short {
        Some(s) => a.short(s),
        None => a,
    }
}

fn id() -> Arg {
    Arg::new("id")
        .required(true)
        .value_parser(clap::value_parser!(i64))
        .help("item id")
}

fn kind(schema: &Schema) -> Arg {
    let names: Vec<&'static str> = schema.kinds.iter().map(|k| k.name).collect();
    Arg::new("kind")
        .long("kind")
        .value_parser(names)
        .help("item kind (optional with a single kind)")
}

pub fn build(meta: &AppMeta, schema: &Schema, extra: Vec<Command>) -> Command {
    Command::new(meta.slug)
        .about(meta.comment)
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("db")
                .long("db")
                .global(true)
                .value_name("PATH")
                .help("database file (default: the app's XDG data dir)"),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("print JSON"),
        )
        .subcommand(
            Command::new("add")
                .about("Add an item")
                .arg(Arg::new("title").required(true))
                .arg(kind(schema))
                .arg(
                    Arg::new("parent")
                        .long("parent")
                        .short('p')
                        .help("parent id"),
                )
                .arg(many("set", "set", Some('s'), "field value, key=value"))
                .arg(many("tag", "tag", Some('t'), "tag to add")),
        )
        .subcommand(
            Command::new("list")
                .about("List items")
                .arg(kind(schema))
                .arg(
                    Arg::new("parent")
                        .long("parent")
                        .short('p')
                        .help("only children of this id, or `root`"),
                )
                .arg(many(
                    "tag",
                    "tag",
                    Some('t'),
                    "must have this tag (or a descendant tag)",
                ))
                .arg(many(
                    "any_tag",
                    "any-tag",
                    None,
                    "must have at least one of these tags",
                ))
                .arg(many("no_tag", "no-tag", None, "must not have this tag"))
                .arg(
                    Arg::new("query")
                        .long("query")
                        .short('q')
                        .help("full-text search"),
                )
                .arg(many(
                    "where",
                    "where",
                    Some('w'),
                    "field filter: f=v f!=v f<v f<=v f>v f>=v f~text",
                ))
                .arg(many("sort", "sort", None, "field or field:desc"))
                .arg(
                    Arg::new("limit")
                        .long("limit")
                        .value_parser(clap::value_parser!(usize)),
                )
                .arg(
                    Arg::new("archived")
                        .long("archived")
                        .action(ArgAction::SetTrue)
                        .help("only archived items"),
                )
                .arg(
                    Arg::new("all")
                        .long("all")
                        .action(ArgAction::SetTrue)
                        .help("archived and not"),
                ),
        )
        .subcommand(Command::new("show").about("Show one item").arg(id()))
        .subcommand(
            Command::new("edit")
                .about("Change an item")
                .arg(id())
                .arg(Arg::new("title").long("title"))
                .arg(many("set", "set", Some('s'), "field value, key=value"))
                .arg(many("unset", "unset", Some('u'), "field to clear"))
                .arg(
                    Arg::new("archive")
                        .long("archive")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("unarchive"),
                )
                .arg(
                    Arg::new("unarchive")
                        .long("unarchive")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("rm")
                .about("Delete an item and everything below it")
                .arg(id()),
        )
        .subcommand(
            Command::new("mv")
                .about("Move an item under another (or `root`)")
                .arg(id())
                .arg(
                    Arg::new("parent")
                        .required(true)
                        .help("new parent id, or `root`"),
                )
                .arg(
                    Arg::new("position")
                        .long("position")
                        .value_parser(clap::value_parser!(i64)),
                ),
        )
        .subcommand(
            Command::new("tag")
                .about("Add tags to an item")
                .arg(id())
                .arg(Arg::new("names").required(true).num_args(1..)),
        )
        .subcommand(
            Command::new("untag")
                .about("Remove tags from an item")
                .arg(id())
                .arg(Arg::new("names").required(true).num_args(1..)),
        )
        .subcommand(Command::new("tags").about("List the tag vocabulary"))
        .subcommand(
            Command::new("tag-new")
                .about("Create a tag, optionally under a parent tag")
                .arg(Arg::new("name").required(true))
                .arg(Arg::new("parent").long("parent")),
        )
        .subcommand(
            Command::new("tag-parent")
                .about("Set a tag's parent (omit to make it a root)")
                .arg(Arg::new("name").required(true))
                .arg(Arg::new("parent")),
        )
        .subcommand(
            Command::new("tag-rm")
                .about("Delete a tag; its children become roots unless --with-children")
                .arg(Arg::new("name").required(true))
                .arg(
                    Arg::new("with_children")
                        .long("with-children")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommands(extra)
}
