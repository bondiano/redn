//! Base functor and recursion schemes for EDN ASTs.
//!
//! [`ValueF<R>`] factors recursive child positions out of [`TaggedValue`], enabling
//! single-layer [`project`]/[`embed`] conversions as well as catamorphisms
//! ([`cata`], [`cata_owned`]), paramorphisms ([`para`]), anamorphisms ([`ana`]),
//! and hylomorphisms ([`hylo`]).
//!
//! # Examples
//!
//! Sum every integer in a nested EDN document with [`cata`]:
//!
//! ```
//! use redn::functor::{ValueF, cata};
//! use redn::parse_text;
//!
//! let ast = parse_text("nums", "(1 [2 3] {:a 4})")?;
//! let sum: i64 = cata(&ast, &mut |layer| match layer {
//!     ValueF::Integer(_, i) => i,
//!     ValueF::List(_, xs) | ValueF::Vec(_, xs) | ValueF::Set(_, xs) => xs.into_iter().sum(),
//!     ValueF::Map(_, ps) => ps.into_iter().map(|(k, v)| k + v).sum(),
//!     _ => 0,
//! });
//! assert_eq!(sum, 10);
//! # Ok::<(), redn::ParseError>(())
//! ```

use std::fmt;

use redn_ast::{Float, Tagged, TaggedValue, Value};

/// Optional tag metadata attached to a single [`ValueF`] layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag {
    /// Tag namespace, or empty when unqualified.
    pub namespace: String,
    /// Tag name.
    pub name: String,
}

impl Tag {
    /// Creates a new tag from a namespace (possibly empty) and tag name.
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    /// Creates an unqualified `#name` tag with an empty namespace.
    pub fn unqualified(name: impl Into<String>) -> Self {
        Self {
            namespace: String::new(),
            name: name.into(),
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.namespace.is_empty() {
            write!(f, "#{}", self.name)
        } else {
            write!(f, "#{}/{}", self.namespace, self.name)
        }
    }
}

/// Non-recursive base functor for [`TaggedValue`], parameterized over child positions `R`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueF<R> {
    /// `nil`
    Nil(Option<Tag>),
    /// `true` or `false`
    Boolean(Option<Tag>, bool),
    /// `"string"`
    String(Option<Tag>, String),
    /// `\c`
    Character(Option<Tag>, char),
    /// `namespace/name` or `name`
    Symbol {
        /// Optional outer tag.
        tag: Option<Tag>,
        /// Symbol namespace, or empty when unqualified.
        namespace: String,
        /// Symbol name.
        name: String,
    },
    /// `:keyword`
    Keyword(Option<Tag>, String),
    /// `42`
    Integer(Option<Tag>, i64),
    /// `3.14`
    Floating(Option<Tag>, Float),
    /// `(a b c)`
    List(Option<Tag>, Vec<R>),
    /// `[a b c]`
    Vec(Option<Tag>, Vec<R>),
    /// `{k v}`
    Map(Option<Tag>, Vec<(R, R)>),
    /// `#{a b}`
    Set(Option<Tag>, Vec<R>),
}

impl<R> ValueF<R> {
    /// Borrows the optional outer tag on this layer.
    pub const fn tag(&self) -> Option<&Tag> {
        match self {
            Self::Nil(t)
            | Self::Boolean(t, _)
            | Self::String(t, _)
            | Self::Character(t, _)
            | Self::Symbol { tag: t, .. }
            | Self::Keyword(t, _)
            | Self::Integer(t, _)
            | Self::Floating(t, _)
            | Self::List(t, _)
            | Self::Vec(t, _)
            | Self::Map(t, _)
            | Self::Set(t, _) => t.as_ref(),
        }
    }

    /// Maps a function over every direct child position (`Functor::fmap`).
    pub fn map<B>(self, mut f: impl FnMut(R) -> B) -> ValueF<B> {
        match self {
            Self::Nil(t) => ValueF::Nil(t),
            Self::Boolean(t, b) => ValueF::Boolean(t, b),
            Self::String(t, s) => ValueF::String(t, s),
            Self::Character(t, c) => ValueF::Character(t, c),
            Self::Symbol {
                tag,
                namespace,
                name,
            } => ValueF::Symbol {
                tag,
                namespace,
                name,
            },
            Self::Keyword(t, k) => ValueF::Keyword(t, k),
            Self::Integer(t, i) => ValueF::Integer(t, i),
            Self::Floating(t, d) => ValueF::Floating(t, d),
            Self::List(t, xs) => ValueF::List(t, xs.into_iter().map(f).collect()),
            Self::Vec(t, xs) => ValueF::Vec(t, xs.into_iter().map(f).collect()),
            Self::Map(t, pairs) => {
                ValueF::Map(t, pairs.into_iter().map(|(k, v)| (f(k), f(v))).collect())
            }
            Self::Set(t, xs) => ValueF::Set(t, xs.into_iter().map(f).collect()),
        }
    }

    /// Maps a fallible function over every direct child position (`Traversable::traverse`).
    pub fn try_map<B, E>(self, mut f: impl FnMut(R) -> Result<B, E>) -> Result<ValueF<B>, E> {
        match self {
            Self::Nil(t) => Ok(ValueF::Nil(t)),
            Self::Boolean(t, b) => Ok(ValueF::Boolean(t, b)),
            Self::String(t, s) => Ok(ValueF::String(t, s)),
            Self::Character(t, c) => Ok(ValueF::Character(t, c)),
            Self::Symbol {
                tag,
                namespace,
                name,
            } => Ok(ValueF::Symbol {
                tag,
                namespace,
                name,
            }),
            Self::Keyword(t, k) => Ok(ValueF::Keyword(t, k)),
            Self::Integer(t, i) => Ok(ValueF::Integer(t, i)),
            Self::Floating(t, d) => Ok(ValueF::Floating(t, d)),
            Self::List(t, xs) => Ok(ValueF::List(
                t,
                xs.into_iter().map(f).collect::<Result<Vec<_>, _>>()?,
            )),
            Self::Vec(t, xs) => Ok(ValueF::Vec(
                t,
                xs.into_iter().map(f).collect::<Result<Vec<_>, _>>()?,
            )),
            Self::Map(t, pairs) => Ok(ValueF::Map(
                t,
                pairs
                    .into_iter()
                    .map(|(k, v)| Ok((f(k)?, f(v)?)))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Self::Set(t, xs) => Ok(ValueF::Set(
                t,
                xs.into_iter().map(f).collect::<Result<Vec<_>, _>>()?,
            )),
        }
    }
}

/// Projects one layer of an owned [`TaggedValue`] into [`ValueF`].
pub fn project(tv: TaggedValue) -> ValueF<TaggedValue> {
    let (tag, value) = match tv {
        Tagged::NoTag(v) => (None, v),
        Tagged::Tagged {
            namespace,
            name,
            value,
        } => (Some(Tag { namespace, name }), value),
    };
    match value {
        Value::Nil => ValueF::Nil(tag),
        Value::Boolean(b) => ValueF::Boolean(tag, b),
        Value::String(s) => ValueF::String(tag, s),
        Value::Character(c) => ValueF::Character(tag, c),
        Value::Symbol { namespace, name } => ValueF::Symbol {
            tag,
            namespace,
            name,
        },
        Value::Keyword(k) => ValueF::Keyword(tag, k),
        Value::Integer(i) => ValueF::Integer(tag, i),
        Value::Floating(d) => ValueF::Floating(tag, d),
        Value::List(xs) => ValueF::List(tag, xs),
        Value::Vec(xs) => ValueF::Vec(tag, xs),
        Value::Map(m) => ValueF::Map(tag, m.into_iter().collect()),
        Value::Set(s) => ValueF::Set(tag, s.into_iter().collect()),
    }
}

/// Projects one layer of a borrowed [`TaggedValue`] without cloning nested children.
pub fn project_ref(tv: &TaggedValue) -> ValueF<&TaggedValue> {
    let (tag, value) = match tv {
        Tagged::NoTag(v) => (None, v),
        Tagged::Tagged {
            namespace,
            name,
            value,
        } => (
            Some(Tag {
                namespace: namespace.clone(),
                name: name.clone(),
            }),
            value,
        ),
    };
    match value {
        Value::Nil => ValueF::Nil(tag),
        Value::Boolean(b) => ValueF::Boolean(tag, *b),
        Value::String(s) => ValueF::String(tag, s.clone()),
        Value::Character(c) => ValueF::Character(tag, *c),
        Value::Symbol { namespace, name } => ValueF::Symbol {
            tag,
            namespace: namespace.clone(),
            name: name.clone(),
        },
        Value::Keyword(k) => ValueF::Keyword(tag, k.clone()),
        Value::Integer(i) => ValueF::Integer(tag, *i),
        Value::Floating(d) => ValueF::Floating(tag, *d),
        Value::List(xs) => ValueF::List(tag, xs.iter().collect()),
        Value::Vec(xs) => ValueF::Vec(tag, xs.iter().collect()),
        Value::Map(m) => ValueF::Map(tag, m.iter().collect()),
        Value::Set(s) => ValueF::Set(tag, s.iter().collect()),
    }
}

/// Embeds one [`ValueF`] layer back into a [`TaggedValue`].
pub fn embed(layer: ValueF<TaggedValue>) -> TaggedValue {
    let (tag, value) = match layer {
        ValueF::Nil(t) => (t, Value::Nil),
        ValueF::Boolean(t, b) => (t, Value::Boolean(b)),
        ValueF::String(t, s) => (t, Value::String(s)),
        ValueF::Character(t, c) => (t, Value::Character(c)),
        ValueF::Symbol {
            tag,
            namespace,
            name,
        } => (tag, Value::Symbol { namespace, name }),
        ValueF::Keyword(t, k) => (t, Value::Keyword(k)),
        ValueF::Integer(t, i) => (t, Value::Integer(i)),
        ValueF::Floating(t, d) => (t, Value::Floating(d)),
        ValueF::List(t, xs) => (t, Value::List(xs)),
        ValueF::Vec(t, xs) => (t, Value::Vec(xs)),
        ValueF::Map(t, pairs) => (t, Value::Map(pairs.into_iter().collect())),
        ValueF::Set(t, xs) => (t, Value::Set(xs.into_iter().collect())),
    };
    match tag {
        None => Tagged::NoTag(value),
        Some(Tag { namespace, name }) => Tagged::Tagged {
            namespace,
            name,
            value,
        },
    }
}

/// Bottom-up fold (catamorphism) over a borrowed [`TaggedValue`].
pub fn cata<A>(value: &TaggedValue, alg: &mut impl FnMut(ValueF<A>) -> A) -> A {
    let layer = project_ref(value).map(|child| cata(child, alg));
    alg(layer)
}

/// Consuming bottom-up fold (catamorphism) over an owned [`TaggedValue`].
pub fn cata_owned<A>(value: TaggedValue, alg: &mut impl FnMut(ValueF<A>) -> A) -> A {
    let layer = project(value).map(|child| cata_owned(child, alg));
    alg(layer)
}

/// Bottom-up fold (paramorphism) giving the algebra both each original child and its folded result.
pub fn para<'a, A>(
    value: &'a TaggedValue,
    alg: &mut impl FnMut(ValueF<(&'a TaggedValue, A)>) -> A,
) -> A {
    let layer = project_ref(value).map(|child| (child, para(child, alg)));
    alg(layer)
}

/// Top-down unfold (anamorphism) building a [`TaggedValue`] from a seed and coalgebra.
pub fn ana<S>(seed: S, coalg: &mut impl FnMut(S) -> ValueF<S>) -> TaggedValue {
    let layer = coalg(seed).map(|next_seed| ana(next_seed, coalg));
    embed(layer)
}

/// Fused unfold-then-fold (hylomorphism) avoiding allocation of an intermediate [`TaggedValue`].
pub fn hylo<S, A>(
    seed: S,
    coalg: &mut impl FnMut(S) -> ValueF<S>,
    alg: &mut impl FnMut(ValueF<A>) -> A,
) -> A {
    let layer = coalg(seed).map(|next_seed| hylo(next_seed, coalg, alg));
    alg(layer)
}
