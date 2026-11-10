//! Running a [`Query`]: the compiled plan plus weaver's tag conditions.

use rusqlite::{Connection, params_from_iter, types::Value as Sql};

use super::{items, tags};
use crate::error::Result;
use crate::primitives::item::Item;
use crate::primitives::plan::{Param, plan};
use crate::primitives::query::Query;
use crate::primitives::schema::Schema;

fn bind(p: Param) -> Sql {
    match p {
        Param::Int(i) => Sql::Integer(i),
        Param::Real(r) => Sql::Real(r),
        Param::Text(t) => Sql::Text(t),
    }
}

pub fn find(conn: &Connection, schema: &Schema, q: &Query) -> Result<Vec<Item>> {
    let p = plan(schema, q)?;
    let mut sql = format!(
        "SELECT {} FROM items WHERE 1=1{}",
        items::COLUMNS,
        p.where_sql
    );
    let mut args: Vec<Sql> = p.params.into_iter().map(bind).collect();

    let tag_filter = q.tags.clone().normalized();
    tag_filter.validate()?;
    let (tag_sql, tag_args) = weaver::conditions(&tag_filter, &tags::OWNER, "items.id");
    sql.push_str(&tag_sql);
    args.extend(tag_args);
    sql.push_str(&p.tail_sql);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args), items::from_row)?;
    let found = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    items::with_tags(conn, found)
}
