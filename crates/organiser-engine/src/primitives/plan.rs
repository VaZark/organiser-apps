//! Compiling a [`Query`] to SQL text plus bound parameters. Pure: it never touches a database.
//! Every field name is resolved against the schema first, so only known identifiers reach SQL;
//! every value is bound, never interpolated.

use serde_json::Value;

use super::fts::match_expr;
use super::query::{Op, ParentFilter, Query};
use super::schema::{BUILTIN, Schema};
use crate::error::{Result, invalid};

#[derive(Debug, Clone, PartialEq)]
pub enum Param {
    Int(i64),
    Real(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// ` AND …` terms to splice after `WHERE 1=1`; positional `?` placeholders for `params`.
    pub where_sql: String,
    pub params: Vec<Param>,
    /// Everything after the `WHERE`: `ORDER BY …` and `LIMIT/OFFSET`.
    pub tail_sql: String,
}

/// The SQL expression for a field name: a built-in column or a JSON path into `fields`.
pub fn field_expr(schema: &Schema, kind: Option<&str>, name: &str) -> Result<String> {
    if BUILTIN.contains(&name) {
        return Ok(format!("items.{name}"));
    }
    let known = match kind {
        Some(k) => schema.kind(k).is_some_and(|k| k.field(name).is_some()),
        None => schema.kinds.iter().any(|k| k.field(name).is_some()),
    };
    if known {
        Ok(format!("json_extract(items.fields,'$.{name}')"))
    } else {
        invalid(format!("unknown field {name:?}"))
    }
}

fn param(v: &Value) -> Result<Param> {
    match v {
        Value::String(s) => Ok(Param::Text(s.clone())),
        Value::Bool(b) => Ok(Param::Int(i64::from(*b))),
        Value::Number(n) => Ok(n
            .as_i64()
            .map_or_else(|| Param::Real(n.as_f64().unwrap_or(0.0)), Param::Int)),
        other => invalid(format!("cannot filter on {other}")),
    }
}

fn like_escape(s: &str) -> String {
    let mut out = String::from("%");
    for c in s.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

pub fn plan(schema: &Schema, q: &Query) -> Result<Plan> {
    let mut sql = String::new();
    let mut params = Vec::new();
    let kind = q.kind.as_deref();

    if let Some(k) = kind {
        if schema.kind(k).is_none() {
            return invalid(format!("unknown kind {k:?}"));
        }
        sql.push_str(" AND items.kind = ?");
        params.push(Param::Text(k.into()));
    }
    match q.parent {
        ParentFilter::Any => {}
        ParentFilter::Root => sql.push_str(" AND items.parent_id IS NULL"),
        ParentFilter::Of(id) => {
            sql.push_str(" AND items.parent_id = ?");
            params.push(Param::Int(id));
        }
    }
    if let Some(a) = q.archived {
        sql.push_str(" AND items.archived = ?");
        params.push(Param::Int(i64::from(a)));
    }
    let expr = q.text.as_deref().and_then(match_expr);
    if let Some(expr) = &expr {
        sql.push_str(" AND items.id IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?)");
        params.push(Param::Text(expr.clone()));
    }
    for f in &q.filters {
        let e = field_expr(schema, kind, &f.field)?;
        match f.op {
            Op::IsNull => sql.push_str(&format!(" AND {e} IS NULL")),
            Op::NotNull => sql.push_str(&format!(" AND {e} IS NOT NULL")),
            Op::Contains => {
                let Value::String(s) = &f.value else {
                    return invalid("contains needs a string");
                };
                sql.push_str(&format!(" AND {e} LIKE ? ESCAPE '\\'"));
                params.push(Param::Text(like_escape(s)));
            }
            op => {
                let sym = match op {
                    Op::Eq => "=",
                    Op::Ne => "<>",
                    Op::Lt => "<",
                    Op::Le => "<=",
                    Op::Gt => ">",
                    _ => ">=",
                };
                sql.push_str(&format!(" AND {e} {sym} ?"));
                params.push(param(&f.value)?);
            }
        }
    }

    let mut tail = String::from(" ORDER BY ");
    if q.sort.is_empty() {
        tail.push_str(if expr.is_some() {
            "items.updated DESC"
        } else {
            "items.position"
        });
    } else {
        let parts: Result<Vec<String>> = q
            .sort
            .iter()
            .map(|s| {
                Ok(format!(
                    "{} {}",
                    field_expr(schema, kind, &s.field)?,
                    if s.desc { "DESC" } else { "ASC" }
                ))
            })
            .collect();
        tail.push_str(&parts?.join(", "));
    }
    tail.push_str(", items.id");
    match (q.limit, q.offset) {
        (Some(l), o) => tail.push_str(&format!(" LIMIT {l} OFFSET {o}")),
        (None, 0) => {}
        (None, o) => tail.push_str(&format!(" LIMIT -1 OFFSET {o}")),
    }
    Ok(Plan {
        where_sql: sql,
        params,
        tail_sql: tail,
    })
}
