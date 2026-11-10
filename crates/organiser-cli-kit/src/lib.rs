//! # organiser-cli-kit
//!
//! One command-line frontend for every app: `add list show edit rm mv tag untag tags …`, built
//! from the app's [`organiser_engine::Schema`]. An app can add its own subcommands through an
//! [`Extension`]. It holds no business rules: every command is one engine call.
//!
//! Layers (`tests/layers.rs` enforces them): [`primitives`] (pure argument and output rules),
//! [`components`] (the clap commands and their handlers).

pub mod components;
pub mod primitives;

use std::process::ExitCode;

use clap::{ArgMatches, Command};
use organiser_core::{AppMeta, paths};
use organiser_engine::{Db, Result, Schema};

pub use primitives::output::Output;

/// App-specific subcommands on top of the generic ones.
pub trait Extension {
    fn commands(&self) -> Vec<Command>;
    /// The kind `add` and `list` use without `--kind`, for a schema with more than one.
    fn default_kind(&self) -> Option<&'static str> {
        None
    }
    /// Handle a subcommand, or return `None` if it is not one of yours.
    fn run(&self, db: &mut Db, name: &str, args: &ArgMatches) -> Option<Result<Output>>;
}

/// No extra commands.
pub struct Plain;

impl Extension for Plain {
    fn commands(&self) -> Vec<Command> {
        Vec::new()
    }
    fn run(&self, _: &mut Db, _: &str, _: &ArgMatches) -> Option<Result<Output>> {
        None
    }
}

/// Parse `std::env::args`, open the database and run the command. Returns the exit code.
pub fn run(meta: &AppMeta, schema: &'static Schema, ext: &dyn Extension) -> ExitCode {
    let matches = components::commands::build(meta, schema, ext.commands()).get_matches();
    let json = matches.get_flag("json");
    let path = matches
        .get_one::<String>("db")
        .map_or_else(|| paths::db_path(meta), std::path::PathBuf::from);
    let result = Db::open(&path, schema).and_then(|mut db| {
        let (name, sub) = matches.subcommand().expect("subcommand is required");
        match ext.run(&mut db, name, sub) {
            Some(r) => r,
            None => components::handlers::run(&mut db, name, sub, ext.default_kind()),
        }
    });
    match result {
        Ok(out) => {
            println!("{}", out.render(json));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
