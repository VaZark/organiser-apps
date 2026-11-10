//! What an app stores: item kinds and their typed fields. Fields live as JSON on the item and
//! are validated here, so every frontend gets the same rules.

use serde_json::{Map, Value};

use crate::error::{Error, Result, invalid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// Plain string.
    Text,
    /// String holding Markdown.
    Markdown,
    /// String with a scheme, e.g. `https://…`, `mailto:…`.
    Url,
    /// `YYYY-MM-DD`.
    Date,
    /// `YYYY-MM-DDTHH:MM[:SS…]` (RFC 3339 shaped).
    DateTime,
    Number,
    Bool,
    /// Integer `0..=5`.
    Rating,
    /// One of a fixed set of strings.
    Enum(&'static [&'static str]),
    /// Array of strings.
    List,
    /// Array of `{ "text": string, "done": bool }`.
    Checklist,
    /// Integer id of an item of the named kind. The engine checks the target exists and has that
    /// kind, and removes the field when the target is deleted.
    Reference(&'static str),
}

/// Where a gate finds the item that holds its flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVia {
    /// The [`FieldType::Reference`] field of this kind that names the item.
    Field(&'static str),
    /// The item's parent, which must be of this kind.
    Parent(&'static str),
}

/// A field that is only allowed while a flag is on, in the item found through [`GateVia`].
/// Example: an entry's `rating` is allowed only when its list has `rated` switched on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gate {
    pub via: GateVia,
    /// A [`FieldType::Bool`] field of the kind that holds the flag.
    pub flag: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub name: &'static str,
    pub ty: FieldType,
    pub required: bool,
    /// Included in full-text search (string, list and checklist fields).
    pub searchable: bool,
    /// Gets a SQLite expression index, for fields used in filters and sorting.
    pub indexed: bool,
    /// Filled in on create when the field is missing (string-valued fields only).
    pub default: Option<&'static str>,
    /// Allowed only while the gate's flag is on; see [`Gate`].
    pub gate: Option<Gate>,
}

impl Field {
    pub const fn new(name: &'static str, ty: FieldType) -> Self {
        Self {
            name,
            ty,
            required: false,
            searchable: false,
            indexed: false,
            default: None,
            gate: None,
        }
    }
    /// Allow this field only while `flag` is on in the item that this kind's `via` field points to.
    pub const fn gated(mut self, via: &'static str, flag: &'static str) -> Self {
        self.gate = Some(Gate {
            via: GateVia::Field(via),
            flag,
        });
        self
    }
    /// Allow this field only while `flag` is on in the item's parent, which is of `parent_kind`.
    pub const fn gated_by_parent(mut self, parent_kind: &'static str, flag: &'static str) -> Self {
        self.gate = Some(Gate {
            via: GateVia::Parent(parent_kind),
            flag,
        });
        self
    }
    pub const fn default_to(mut self, v: &'static str) -> Self {
        self.default = Some(v);
        self
    }
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub const fn searchable(mut self) -> Self {
        self.searchable = true;
        self
    }
    pub const fn indexed(mut self) -> Self {
        self.indexed = true;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Kind {
    pub name: &'static str,
    pub fields: &'static [Field],
}

impl Kind {
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Schema {
    pub kinds: &'static [Kind],
}

fn ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Field names that mean the built-in columns in queries.
pub(crate) const BUILTIN: &[&str] = &[
    "id",
    "parent_id",
    "title",
    "position",
    "archived",
    "created",
    "updated",
];

impl Schema {
    pub fn kind(&self, name: &str) -> Option<&Kind> {
        self.kinds.iter().find(|k| k.name == name)
    }

    /// Names become SQL identifiers and JSON paths, so they must be plain.
    pub(crate) fn check(&self) -> Result<()> {
        for k in self.kinds {
            if !ident(k.name) {
                return Err(Error::Schema(format!("bad kind name {:?}", k.name)));
            }
            for f in k.fields {
                if let FieldType::Reference(t) = f.ty
                    && self.kind(t).is_none()
                {
                    return Err(Error::Schema(format!(
                        "{}.{} refers to unknown kind {t:?}",
                        k.name, f.name
                    )));
                }
                if let Some(g) = f.gate {
                    let Some(target) = self.gate_target(k, &g) else {
                        return Err(Error::Schema(format!(
                            "{}.{} is gated through something that is not a reference of {}",
                            k.name, f.name, k.name
                        )));
                    };
                    let flag_ok = self
                        .kind(target)
                        .and_then(|t| t.field(g.flag))
                        .is_some_and(|f| f.ty == FieldType::Bool);
                    if !flag_ok {
                        return Err(Error::Schema(format!(
                            "{}.{} is gated by {target}.{}, which is not a bool field",
                            k.name, f.name, g.flag
                        )));
                    }
                }
                if !ident(f.name) || BUILTIN.contains(&f.name) {
                    return Err(Error::Schema(format!(
                        "bad field name {:?} in {}",
                        f.name, k.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Fill missing fields that have a default.
    pub fn apply_defaults(&self, kind: &str, fields: &mut Map<String, Value>) {
        let Some(k) = self.kind(kind) else { return };
        for f in k.fields {
            if let Some(d) = f.default
                && !fields.contains_key(f.name)
            {
                fields.insert(f.name.into(), Value::String(d.into()));
            }
        }
    }

    /// Check a complete field map against a kind: no unknown fields, required ones present,
    /// every value of the right type.
    pub fn validate(&self, kind: &str, fields: &Map<String, Value>) -> Result<()> {
        let Some(k) = self.kind(kind) else {
            return invalid(format!("unknown kind {kind:?}"));
        };
        for (name, v) in fields {
            let Some(f) = k.field(name) else {
                return invalid(format!("{kind} has no field {name:?}"));
            };
            check_value(f, v)?;
        }
        for f in k.fields.iter().filter(|f| f.required) {
            if !fields.contains_key(f.name) {
                return invalid(format!("{kind}.{} is required", f.name));
            }
        }
        Ok(())
    }

    /// The gated fields of `kind`, with their gates.
    pub fn gates(&self, kind: &str) -> Vec<(&'static str, Gate)> {
        let Some(k) = self.kind(kind) else {
            return Vec::new();
        };
        k.fields
            .iter()
            .filter_map(|f| Some((f.name, f.gate?)))
            .collect()
    }

    /// The kind of item that holds a gate's flag.
    fn gate_target(&self, kind: &Kind, gate: &Gate) -> Option<&'static str> {
        match gate.via {
            GateVia::Parent(k) => Some(k),
            GateVia::Field(f) => match kind.field(f)?.ty {
                FieldType::Reference(t) => Some(t),
                _ => None,
            },
        }
    }

    /// Every `(kind, field, gate)` whose gate is a flag on items of `target`: the fields that
    /// depend on an item of that kind.
    pub fn gated_by(&self, target: &str) -> Vec<(&'static str, &'static str, Gate)> {
        let mut out = Vec::new();
        for k in self.kinds {
            for f in k.fields {
                if let Some(g) = f.gate
                    && self.gate_target(k, &g) == Some(target)
                {
                    out.push((k.name, f.name, g));
                }
            }
        }
        out
    }

    /// The `(target id, target kind)` of every reference in these fields. Call after
    /// [`Schema::validate`], which has already checked they are integers.
    pub fn references(&self, kind: &str, fields: &Map<String, Value>) -> Vec<(i64, &'static str)> {
        let Some(k) = self.kind(kind) else {
            return Vec::new();
        };
        k.fields
            .iter()
            .filter_map(|f| match (f.ty, fields.get(f.name)?.as_i64()) {
                (FieldType::Reference(target), Some(id)) => Some((id, target)),
                _ => None,
            })
            .collect()
    }

    /// Every `(kind, field)` that points at items of `target`.
    pub fn referrers(&self, target: &str) -> Vec<(&'static str, &'static str)> {
        self.kinds
            .iter()
            .flat_map(|k| k.fields.iter().map(move |f| (k.name, f)))
            .filter(|(_, f)| matches!(f.ty, FieldType::Reference(t) if t == target))
            .map(|(k, f)| (k, f.name))
            .collect()
    }

    /// Text that full-text search should index for these fields.
    pub(crate) fn search_text(&self, kind: &str, fields: &Map<String, Value>) -> String {
        let Some(k) = self.kind(kind) else {
            return String::new();
        };
        let mut out = Vec::new();
        for f in k.fields.iter().filter(|f| f.searchable) {
            match fields.get(f.name) {
                Some(Value::String(s)) => out.push(s.clone()),
                Some(Value::Array(a)) => {
                    for v in a {
                        match v {
                            Value::String(s) => out.push(s.clone()),
                            Value::Object(o) => {
                                if let Some(Value::String(s)) = o.get("text") {
                                    out.push(s.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        out.join("\n")
    }
}

impl Field {
    /// Read a value a person typed (CLI argument, form entry) as this field's type. Lists and
    /// checklists accept JSON, and lists also a comma-separated string.
    pub fn parse_input(&self, raw: &str) -> Result<Value> {
        let t = raw.trim();
        let v = match self.ty {
            FieldType::Number => match t.parse::<i64>() {
                Ok(i) => Value::from(i),
                Err(_) => Value::from(t.parse::<f64>().map_err(|_| {
                    Error::Invalid(format!("{}: {t:?} is not a number", self.name))
                })?),
            },
            FieldType::Bool => match t.to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" | "on" => Value::Bool(true),
                "false" | "no" | "0" | "off" => Value::Bool(false),
                _ => return invalid(format!("{}: {t:?} is not true/false", self.name)),
            },
            FieldType::Rating | FieldType::Reference(_) => {
                Value::from(t.parse::<i64>().map_err(|_| {
                    Error::Invalid(format!("{}: {t:?} is not an integer", self.name))
                })?)
            }
            FieldType::List => match serde_json::from_str::<Value>(t) {
                Ok(v @ Value::Array(_)) => v,
                _ => Value::Array(
                    t.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(Value::from)
                        .collect(),
                ),
            },
            FieldType::Checklist => serde_json::from_str(t)?,
            _ => Value::String(t.to_string()),
        };
        check_value(self, &v)?;
        Ok(v)
    }
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        && matches!(s[5..7].parse::<u8>(), Ok(1..=12))
        && matches!(s[8..10].parse::<u8>(), Ok(1..=31))
}

fn check_value(f: &Field, v: &Value) -> Result<()> {
    let bad = |what: &str| invalid(format!("{} must be {what}", f.name));
    match (f.ty, v) {
        (FieldType::Text | FieldType::Markdown, Value::String(_)) => Ok(()),
        (FieldType::Url, Value::String(s)) => {
            let ok = !s.chars().any(char::is_whitespace)
                && s.split_once(':').is_some_and(|(scheme, rest)| {
                    !scheme.is_empty()
                        && scheme
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
                        && !rest.is_empty()
                });
            if ok {
                Ok(())
            } else {
                bad("a URL with a scheme")
            }
        }
        (FieldType::Date, Value::String(s)) if is_date(s) => Ok(()),
        (FieldType::DateTime, Value::String(s))
            if s.len() >= 16 && is_date(&s[..10]) && s.as_bytes()[10] == b'T' =>
        {
            Ok(())
        }
        (FieldType::Number, Value::Number(_)) | (FieldType::Bool, Value::Bool(_)) => Ok(()),
        (FieldType::Rating, Value::Number(n))
            if n.as_i64().is_some_and(|n| (0..=5).contains(&n)) =>
        {
            Ok(())
        }
        (FieldType::Reference(_), Value::Number(n)) if n.is_i64() => Ok(()),
        (FieldType::Enum(opts), Value::String(s)) => {
            if opts.contains(&s.as_str()) {
                Ok(())
            } else {
                bad(&format!("one of {opts:?}"))
            }
        }
        (FieldType::List, Value::Array(a)) if a.iter().all(Value::is_string) => Ok(()),
        (FieldType::Checklist, Value::Array(a))
            if a.iter().all(|e| {
                e.as_object().is_some_and(|o| {
                    o.get("text").is_some_and(Value::is_string)
                        && o.get("done").is_some_and(Value::is_boolean)
                        && o.len() == 2
                })
            }) =>
        {
            Ok(())
        }
        (ty, _) => bad(&format!("{ty:?}")),
    }
}
