//! `sticky`: the command-line frontend of Sticky Notes. All behaviour is in `sticky-engine` and
//! `organiser-sticky`; this parses arguments and prints.

use sticky_engine::{Engine, Key};

const USAGE: &str = "usage:
  sticky apps                         the apps that offer stickies, and whether each is linked
  sticky items                        the stickies of the linked apps
  sticky link APP_ID | unlink APP_ID
  sticky desktop APP_ID ITEM_ID on|off   show a sticky on the desktop, or only in Sticky Notes' window
  sticky color APP_ID ITEM_ID #rrggbb|none
  sticky title APP_ID ITEM_ID TEXT | text APP_ID ITEM_ID TEXT";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}

fn key(app: &str, id: &str) -> Result<Key, String> {
    Ok(Key::new(
        app,
        id.parse()
            .map_err(|_| format!("{id:?} is not an item id"))?,
    ))
}

fn run(args: &[String]) -> Result<String, String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut e = Engine::new();
    match a.as_slice() {
        ["apps"] => {
            e.refresh()?;
            if e.links().links.is_empty() {
                return Ok("No apps yet.".into());
            }
            Ok(e.links()
                .links
                .iter()
                .map(|l| {
                    format!(
                        "{}  {}  {}  {}",
                        if l.enabled { "on " } else { "off" },
                        l.app_id,
                        l.title(),
                        l.db_path
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        ["items"] => {
            e.refresh()?;
            Ok(e.items()
                .iter()
                .map(|i| {
                    format!(
                        "{}  {}  {}  {}  {}",
                        i.key.app_id,
                        i.key.id,
                        if i.show_on_desktop {
                            "desktop"
                        } else {
                            "window "
                        },
                        i.color.as_deref().unwrap_or("-"),
                        i.title
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        ["link", id] => switch(&mut e, id, true),
        ["unlink", id] => switch(&mut e, id, false),
        ["desktop", app, id, state @ ("on" | "off")] => {
            e.refresh()?;
            e.set_show_on_desktop(&key(app, id)?, *state == "on")?;
            Ok(String::new())
        }
        ["color", app, id, c] => {
            e.refresh()?;
            e.set_color(&key(app, id)?, (*c != "none").then_some(*c))?;
            Ok(String::new())
        }
        ["title", app, id, t] => {
            e.refresh()?;
            e.set_title(&key(app, id)?, t)?;
            Ok(String::new())
        }
        ["text", app, id, t] => {
            e.refresh()?;
            e.set_body(&key(app, id)?, t)?;
            Ok(String::new())
        }
        [] | ["help" | "--help" | "-h"] => Ok(USAGE.into()),
        _ => Err(USAGE.into()),
    }
}

fn switch(e: &mut Engine, id: &str, on: bool) -> Result<String, String> {
    e.refresh()?;
    if e.links().get(id).is_none() {
        return Err(format!("{id} is not one of the apps (see `sticky apps`)"));
    }
    e.set_linked(id, on)?;
    Ok(format!("{} {id}", if on { "Linked" } else { "Unlinked" }))
}
