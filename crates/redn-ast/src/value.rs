//! Core EDN AST value types and collection constructors.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::float::Float;
use crate::printer::render_text;
use crate::tagged::Tagged;

/// A top-level or nested EDN element with an optional `#ns/tag` tag.
pub type TaggedValue = Tagged<String, Value>;

/// Sequence of elements inside an EDN list `(...)`.
pub type EdnList = Vec<TaggedValue>;

/// Sequence of elements inside an EDN vector `[...]`.
pub type EdnVec = Vec<TaggedValue>;

/// Ordered map of EDN key-value pairs `{...}`.
pub type EdnMap = BTreeMap<TaggedValue, TaggedValue>;

/// Ordered set of unique EDN elements `#{...}`.
pub type EdnSet = BTreeSet<TaggedValue>;

/// An untagged EDN value.
///
/// Variant declaration order defines the canonical [`Ord`] ordering inside
/// [`EdnMap`] and [`EdnSet`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    /// `nil`
    Nil,
    /// `true` or `false`
    Boolean(bool),
    /// `"a string"`
    String(String),
    /// `\c`
    Character(char),
    /// `namespace/name` (or `name` when `namespace` is empty)
    Symbol {
        /// Symbol namespace, or empty when unqualified.
        namespace: String,
        /// Symbol name.
        name: String,
    },
    /// `:a-keyword` (stored without the leading `:`)
    Keyword(String),
    /// `42`
    Integer(i64),
    /// `3.14`
    Floating(Float),
    /// `(a list)`
    List(EdnList),
    /// `[a vector]`
    Vec(EdnVec),
    /// `{:a map}`
    Map(EdnMap),
    /// `#{a set}`
    Set(EdnSet),
}

impl Value {
    /// Constructs a [`Value::Symbol`] from a namespace (possibly empty) and name.
    pub fn symbol(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Symbol {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    /// Constructs a [`Value::Keyword`] without a leading `:`.
    pub fn keyword(name: impl Into<String>) -> Self {
        Self::Keyword(name.into())
    }

    /// Constructs a [`Value::String`].
    pub fn string(text: impl Into<String>) -> Self {
        Self::String(text.into())
    }

    /// Constructs a [`Value::Floating`].
    pub const fn floating(value: f64) -> Self {
        Self::Floating(Float::new(value))
    }

    /// Returns the human-readable kind name used in decoding error messages.
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Nil => "nil",
            Self::Boolean(_) => "boolean",
            Self::String(_) => "string",
            Self::Character(_) => "character",
            Self::Symbol { .. } => "symbol",
            Self::Keyword(_) => "keyword",
            Self::Integer(_) => "integer",
            Self::Floating(_) => "floating",
            Self::List(_) => "list",
            Self::Vec(_) => "vector",
            Self::Map(_) => "map",
            Self::Set(_) => "set",
        }
    }
}

impl fmt::Display for TaggedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&render_text(self))
    }
}

/// Builds a [`Value::List`] from any iterable of [`TaggedValue`] items.
pub fn mk_list(items: impl IntoIterator<Item = TaggedValue>) -> Value {
    Value::List(items.into_iter().collect())
}

/// Builds a [`Value::Vec`] from any iterable of [`TaggedValue`] items.
pub fn mk_vec(items: impl IntoIterator<Item = TaggedValue>) -> Value {
    Value::Vec(items.into_iter().collect())
}

/// Builds a [`Value::Map`] from any iterable of `(TaggedValue, TaggedValue)` pairs.
pub fn mk_map(pairs: impl IntoIterator<Item = (TaggedValue, TaggedValue)>) -> Value {
    Value::Map(pairs.into_iter().collect())
}

/// Builds a [`Value::Set`] from any iterable of [`TaggedValue`] items.
pub fn mk_set(items: impl IntoIterator<Item = TaggedValue>) -> Value {
    Value::Set(items.into_iter().collect())
}
