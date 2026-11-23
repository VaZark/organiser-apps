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
    /// Called once the database is open, before any command: set up what the app needs to exist
    /// (its default rows, say).
    fn opened(&self, _db: &mut Db) -> Result<()> {
        Ok(())
    }
    /// Called with the database file about to be opened, before [`opened`](Self::opened).
    fn database(&self, _path: &std::path::Path) {}
    /// Called after the command has run, when it did not fail: save what the app keeps outside
    /// the database (its files, say).
    fn closed(&self, _db: &mut Db) -> Result<()> {
        Ok(())
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
    ext.database(&path);
    let result = Db::open(&path, schema).and_then(|mut db| {
        ext.opened(&mut db)?;
        let (name, sub) = matches.subcommand().expect("subcommand is required");
        let out = match ext.run(&mut db, name, sub) {
            Some(r) => r,
            None => components::handlers::run(&mut db, name, sub, ext.default_kind()),
        }?;
        ext.closed(&mut db)?;
        Ok(out)
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
