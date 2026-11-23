//! One engine call per command.

use clap::ArgMatches;
use organiser_engine::{
    ChildPolicy, Db, Error, FieldFilter, ItemPatch, NewItem, Query, Result, Sort, TagFilter,
};
use serde_json::Value;

use crate::primitives::output::Output;
use crate::primitives::spec;

fn bad<T>(msg: String) -> Result<T> {
    Err(Error::Invalid(msg))
}

fn strings(m: &ArgMatches, name: &str) -> Vec<String> {
    m.get_many::<String>(name)
        .map(|v| v.cloned().collect())
        .unwrap_or_default()
}

fn parent(m: &ArgMatches) -> Result<Option<Option<i64>>> {
    m.get_one::<String>("parent")
        .map(|p| spec::parent(p).or_else(bad))
        .transpose()
}

/// The kind named by `--kind`, else the app's default, else the schema's only kind.
fn kind_of(db: &Db, m: &ArgMatches, default: Option<&'static str>) -> Result<&'static str> {
    let kinds = db.schema().kinds;
    match (m.get_one::<String>("kind"), default, kinds) {
        (Some(k), _, _) => Ok(kinds.iter().find(|x| x.name == k).map_or("", |x| x.name)),
        (None, Some(d), _) => Ok(d),
        (None, None, [only]) => Ok(only.name),
        _ => bad("--kind is required".into()),
    }
}

/// `key=value` pairs read as their field's type.
fn field_values(db: &Db, kind: &str, pairs: &[String]) -> Result<Vec<(String, Value)>> {
    let k = db
        .schema()
        .kind(kind)
        .ok_or_else(|| Error::Invalid(format!("unknown kind {kind:?}")))?;
    pairs
        .iter()
        .map(|p| {
            let (key, raw) = spec::key_value(p).or_else(bad)?;
            let f = k
                .field(key)
                .ok_or_else(|| Error::Invalid(format!("{kind} has no field {key:?}")))?;
            Ok((key.to_string(), f.parse_input(raw)?))
        })
        .collect()
}

pub fn run(
    db: &mut Db,
    name: &str,
    m: &ArgMatches,
    default_kind: Option<&'static str>,
) -> Result<Output> {
    match name {
        "add" => {
            let kind = kind_of(db, m, default_kind)?;
            let mut new = NewItem::new(
                kind,
                m.get_one::<String>("title").map_or("", String::as_str),
            );
            new.parent_id = parent(m)?.flatten();
            new.tags = strings(m, "tag");
            new.fields
                .extend(field_values(db, kind, &strings(m, "set"))?);
            Ok(Output::Item(db.create(new)?))
        }
        "list" => {
            let mut q = Query {
                kind: m
                    .try_get_one::<String>("kind")
                    .ok()
                    .flatten()
                    .cloned()
                    .or_else(|| default_kind.map(String::from)),
                ..Query::default()
            };
            if let Some(p) = parent(m)? {
                q.parent = p.map_or(
                    organiser_engine::ParentFilter::Root,
                    organiser_engine::ParentFilter::Of,
                );
            }
            q.text = m.get_one::<String>("query").cloned();
            q.tags = TagFilter {
                all: strings(m, "tag"),
                any: strings(m, "any_tag"),
                exclude: strings(m, "no_tag"),
                ..TagFilter::default()
            };
            q.archived = if m.get_flag("all") {
                None
            } else {
                Some(m.get_flag("archived"))
            };
            for w in strings(m, "where") {
                let (field, op, raw) = spec::filter(&w).or_else(bad)?;
                let typed = q
                    .kind
                    .as_deref()
                    .or(match db.schema().kinds {
                        [only] => Some(only.name),
                        _ => None,
                    })
                    .and_then(|k| db.schema().kind(k))
                    .and_then(|k| k.field(field))
                    .map_or_else(
                        || {
                            Ok(raw
                                .parse::<i64>()
                                .map_or_else(|_| Value::from(raw), Value::from))
                        },
                        |f| f.parse_input(raw),
                    )?;
                q.filters.push(FieldFilter {
                    field: field.into(),
                    op,
                    value: typed,
                });
            }
            q.sort = strings(m, "sort")
                .iter()
                .map(|s| spec::sort(s))
                .map(|(f, d)| Sort {
                    field: f.into(),
                    desc: d,
                })
                .collect();
            q.limit = m.get_one::<usize>("limit").copied();
            Ok(Output::Items(db.list(&q)?))
        }
        "show" => Ok(Output::Item(db.get(*m.get_one::<i64>("id").unwrap_or(&0))?)),
        "edit" => {
            let id = *m.get_one::<i64>("id").unwrap_or(&0);
            let kind = db.get(id)?.kind;
            let mut patch = ItemPatch {
                title: m.get_one::<String>("title").cloned(),
                ..ItemPatch::default()
            };
            patch
                .fields
                .extend(field_values(db, &kind, &strings(m, "set"))?);
            for u in strings(m, "unset") {
                patch.fields.insert(u, Value::Null);
            }
            if m.get_flag("archive") {
                patch.archived = Some(true);
            } else if m.get_flag("unarchive") {
                patch.archived = Some(false);
            }
            Ok(Output::Item(db.update(id, patch)?))
        }
        "rm" => {
            let n = db.delete(*m.get_one::<i64>("id").unwrap_or(&0))?;
            Ok(Output::Message(format!(
                "deleted {n} item{}",
                if n == 1 { "" } else { "s" }
            )))
        }
        "mv" => {
            let to = parent(m)?.flatten();
            let pos = m.get_one::<i64>("position").copied();
            Ok(Output::Item(db.move_item(
                *m.get_one::<i64>("id").unwrap_or(&0),
                to,
                pos,
            )?))
        }
        "tag" => Ok(Output::Item(db.add_tags(
            *m.get_one::<i64>("id").unwrap_or(&0),
            &strings(m, "names"),
        )?)),
        "untag" => Ok(Output::Item(db.remove_tags(
            *m.get_one::<i64>("id").unwrap_or(&0),
            &strings(m, "names"),
        )?)),
        "tags" => Ok(Output::Tags(db.tags()?)),
        "tag-new" => {
            let t = db.create_tag(
                &m.get_one::<String>("name").cloned().unwrap_or_default(),
                m.get_one::<String>("parent").map(String::as_str),
            )?;
            Ok(Output::Message(format!("created {}", t.name)))
        }
        "tag-parent" => {
            let t = db.set_tag_parent(
                &m.get_one::<String>("name").cloned().unwrap_or_default(),
                m.get_one::<String>("parent").map(String::as_str),
            )?;
            Ok(Output::Message(format!(
                "{} now under {}",
                t.name,
                t.parent.as_deref().unwrap_or("(root)")
            )))
        }
        "tag-rm" => {
            let policy = if m.get_flag("with_children") {
                ChildPolicy::Delete
            } else {
                ChildPolicy::Detach
            };
            let n = db.delete_tag(
                &m.get_one::<String>("name").cloned().unwrap_or_default(),
                policy,
            )?;
            Ok(Output::Message(format!(
                "deleted {n} tag{}",
                if n == 1 { "" } else { "s" }
            )))
        }
        other => bad(format!("unknown command {other:?}")),
    }
}
