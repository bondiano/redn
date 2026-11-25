//! Encoding ([`ToEdn`]), decoding ([`FromEdn`]), and AST-decoding combinators.
//!
//! # Examples
//!
//! ```
//! use std::collections::BTreeMap;
//! use redn::{
//!     DecodeResult, FromEdn, TaggedValue, ToEdn, Value, decode_text, encode_text,
//!     map_get_keyword, to_edn_tagged, with_map, with_tagged,
//! };
//!
//! #[derive(Debug, PartialEq, Eq)]
//! struct User {
//!     id: i64,
//!     name: String,
//! }
//!
//! impl ToEdn for User {
//!     fn to_edn(&self) -> TaggedValue {
//!         let map = BTreeMap::from([
//!             (Value::keyword("id"), self.id.to_edn()),
//!             (Value::keyword("name"), self.name.to_edn()),
//!         ]);
//!         to_edn_tagged("app", "User", &map)
//!     }
//! }
//!
//! impl FromEdn for User {
//!     fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
//!         with_tagged("app", "User", value, |v| {
//!             with_map(v, |m| {
//!                 Ok(Self {
//!                     id: map_get_keyword("id", m)?,
//!                     name: map_get_keyword("name", m)?,
//!                 })
//!             })
//!         })
//!     }
//! }
//!
//! let user = User { id: 7, name: "Ada".into() };
//! let text = encode_text(&user);
//! assert_eq!(text, r#"#app/User {:id 7 :name "Ada"}"#);
//! assert_eq!(decode_text::<User>("user.edn", &text)?, user);
//! # Ok::<(), redn::Error>(())
//! ```

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList};
use std::convert::Infallible;
use std::hash::{BuildHasher, Hash};

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use redn_ast::{EdnMap, EdnSet, Float, Tagged, TaggedValue, Value, parse_text, render_text};
#[cfg(feature = "uuid")]
use uuid::Uuid;

#[cfg(any(feature = "chrono", feature = "uuid"))]
use crate::error::alt;
use crate::error::{DecodeError, DecodeResult, Error};
use crate::list::List;

/// Converts a value to EDN AST using [`ToEdn`] and renders it onto a single line.
pub fn encode_text<T: ToEdn + ?Sized>(value: &T) -> String {
    render_text(&value.to_edn())
}

/// Parses an EDN document into an AST and decodes it using [`FromEdn`].
///
/// # Errors
///
/// Returns [`Error::Parse`] if `source` is not a valid single-element EDN
/// document, or [`Error::Decode`] if `T::parse_edn` rejects the parsed AST.
pub fn decode_text<T: FromEdn>(source_name: &str, source: &str) -> Result<T, Error> {
    let ast = parse_text(source_name, source)?;
    let decoded = from_edn(&ast)?;
    Ok(decoded)
}

/// Decodes a [`TaggedValue`] into `T` using its [`FromEdn`] implementation.
///
/// # Errors
///
/// Returns a [`DecodeError`] if `value` does not match the structure or tag
/// expected by `T`.
pub fn from_edn<T: FromEdn>(value: &TaggedValue) -> DecodeResult<T> {
    T::parse_edn(value)
}

/// A type that can be converted to an EDN AST ([`TaggedValue`]).
///
/// Implement [`ToEdn::to_edn`] and get [`ToEdn::to_ednv`] automatically.
pub trait ToEdn {
    /// Converts `self` to a [`TaggedValue`].
    fn to_edn(&self) -> TaggedValue;

    /// Converts `self` to an untagged [`Value`], stripping any outer tag.
    fn to_ednv(&self) -> Value {
        self.to_edn().into_value()
    }
}

/// Encodes `value` to an untagged [`Value`] and attaches `#namespace/tag` (or `#tag` if `namespace` is empty).
pub fn to_edn_tagged<T: ToEdn + ?Sized>(
    namespace: impl Into<String>,
    tag: impl Into<String>,
    value: &T,
) -> TaggedValue {
    Tagged::new(namespace, tag, value.to_ednv())
}

/// A type that can be decoded from an EDN AST ([`TaggedValue`]).
pub trait FromEdn: Sized {
    /// Decodes a [`TaggedValue`] into `Self`.
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self>;

    /// Decodes an untagged [`Value`] into `Self`.
    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        Self::parse_edn(&Tagged::NoTag(value.clone()))
    }
}

/// Conversion from EDN `f64` floats used by [`with_fractional`].
pub trait FromF64: Sized {
    /// Converts an `f64` into `Self`.
    fn from_f64(value: f64) -> Self;
}

impl FromF64 for f64 {
    fn from_f64(value: f64) -> Self {
        value
    }
}

impl FromF64 for f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f64 -> f32 narrowing is the explicit contract of FromF64 for f32"
    )]
    fn from_f64(value: f64) -> Self {
        value as Self
    }
}

// ---------------------------------------------------------------------------
// AST-decoding combinators
// ---------------------------------------------------------------------------

/// Requires `tv` to carry tag `#namespace/name` and passes its inner [`Value`] to `f`.
pub fn with_tagged<'a, T>(
    namespace: &str,
    name: &str,
    tv: &'a TaggedValue,
    f: impl FnOnce(&'a Value) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match tv {
        Tagged::Tagged {
            namespace: got_ns,
            name: got_name,
            value,
        } if got_ns == namespace && got_name == name => f(value),
        Tagged::Tagged {
            namespace: got_ns,
            name: got_name,
            ..
        } => Err(DecodeError::new(format!(
            "unexpected tag. expecting: #{}; got: #{}",
            ns_to_text(namespace, name),
            ns_to_text(got_ns, got_name),
        ))),
        Tagged::NoTag(_) => Err(DecodeError::new("expected tagged value")),
    }
}

/// Requires `tv` to have no tag and passes its inner [`Value`] to `f`.
pub fn with_no_tag<'a, T>(
    tv: &'a TaggedValue,
    f: impl FnOnce(&'a Value) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match tv {
        Tagged::NoTag(value) => f(value),
        Tagged::Tagged {
            namespace, name, ..
        } => Err(DecodeError::new(format!(
            "no tag expected, got #{}",
            ns_to_text(namespace, name),
        ))),
    }
}

/// Matches [`Value::Nil`] and invokes `f`.
pub fn with_nil<T>(value: &Value, f: impl FnOnce() -> DecodeResult<T>) -> DecodeResult<T> {
    match value {
        Value::Nil => f(),
        other => unexpected(other, "nil"),
    }
}

/// Matches [`Value::Boolean`] and passes the `bool` to `f`.
pub fn with_boolean<T>(value: &Value, f: impl FnOnce(bool) -> DecodeResult<T>) -> DecodeResult<T> {
    match value {
        Value::Boolean(b) => f(*b),
        other => unexpected(other, "boolean"),
    }
}

/// Matches [`Value::String`] and passes the string slice to `f`.
pub fn with_string<'a, T>(
    value: &'a Value,
    f: impl FnOnce(&'a str) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match value {
        Value::String(s) => f(s),
        other => unexpected(other, "string"),
    }
}

/// Matches [`Value::Character`] and passes the `char` to `f`.
pub fn with_character<T>(
    value: &Value,
    f: impl FnOnce(char) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match value {
        Value::Character(c) => f(*c),
        other => unexpected(other, "char"),
    }
}

/// Matches [`Value::Symbol`] and passes `(namespace, name)` to `f`.
pub fn with_symbol<'a, T>(
    value: &'a Value,
    f: impl FnOnce(&'a str, &'a str) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match value {
        Value::Symbol { namespace, name } => f(namespace, name),
        other => unexpected(other, "symbol"),
    }
}

/// Matches [`Value::Keyword`] and passes the keyword name (without `:`) to `f`.
pub fn with_keyword<'a, T>(
    value: &'a Value,
    f: impl FnOnce(&'a str) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match value {
        Value::Keyword(k) => f(k),
        other => unexpected(other, "keyword"),
    }
}

/// Matches any textual EDN value (`String`, `Character`, `Keyword`, or `Symbol`).
pub fn with_textual<T>(value: &Value, f: impl FnOnce(&str) -> DecodeResult<T>) -> DecodeResult<T> {
    match value {
        Value::String(s) | Value::Keyword(s) => f(s),
        Value::Character(c) => {
            let mut buf = [0_u8; 4];
            f(c.encode_utf8(&mut buf))
        }
        Value::Symbol { namespace, name } => {
            if namespace.is_empty() {
                f(name)
            } else {
                let joined = format!("{namespace}/{name}");
                f(&joined)
            }
        }
        other => Err(DecodeError::unexpected_any(
            other,
            ["string", "char", "keyword", "symbol"],
        )),
    }
}

/// Matches [`Value::Integer`] and passes the `i64` to `f`.
pub fn with_integer<T>(value: &Value, f: impl FnOnce(i64) -> DecodeResult<T>) -> DecodeResult<T> {
    match value {
        Value::Integer(i) => f(*i),
        other => unexpected(other, "integer"),
    }
}

/// Matches [`Value::Integer`], converts it via [`TryFrom<i64>`], and passes the result to `f`.
pub fn with_integral<I, T>(value: &Value, f: impl FnOnce(I) -> DecodeResult<T>) -> DecodeResult<T>
where
    I: TryFrom<i64>,
{
    match value {
        Value::Integer(i) => match I::try_from(*i) {
            Ok(converted) => f(converted),
            Err(_) => Err(DecodeError::new(format!(
                "integer {i} out of range for target type"
            ))),
        },
        other => unexpected(other, "integer"),
    }
}

/// Matches [`Value::Floating`] and passes the `f64` to `f`.
pub fn with_floating<T>(value: &Value, f: impl FnOnce(f64) -> DecodeResult<T>) -> DecodeResult<T> {
    match value {
        Value::Floating(d) => f(d.into_inner()),
        other => unexpected(other, "double"),
    }
}

/// Matches [`Value::Floating`], converts it via [`FromF64`], and passes the result to `f`.
pub fn with_fractional<F, T>(value: &Value, f: impl FnOnce(F) -> DecodeResult<T>) -> DecodeResult<T>
where
    F: FromF64,
{
    match value {
        Value::Floating(d) => f(F::from_f64(d.into_inner())),
        other => unexpected(other, "double"),
    }
}

/// Matches [`Value::List`] and passes its elements slice to `f`.
pub fn with_list<'a, T>(
    value: &'a Value,
    f: impl FnOnce(&'a [TaggedValue]) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match value {
        Value::List(items) => f(items),
        other => unexpected(other, "list"),
    }
}

/// Matches [`Value::Vec`] and passes its elements slice to `f`.
pub fn with_vec<'a, T>(
    value: &'a Value,
    f: impl FnOnce(&'a [TaggedValue]) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match value {
        Value::Vec(items) => f(items),
        other => unexpected(other, "vector"),
    }
}

/// Matches [`Value::Map`] and passes the [`EdnMap`] reference to `f`.
pub fn with_map<'a, T>(
    value: &'a Value,
    f: impl FnOnce(&'a EdnMap) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match value {
        Value::Map(map) => f(map),
        other => unexpected(other, "map"),
    }
}

/// Matches [`Value::Set`] and passes the [`EdnSet`] reference to `f`.
pub fn with_set<'a, T>(
    value: &'a Value,
    f: impl FnOnce(&'a EdnSet) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match value {
        Value::Set(set) => f(set),
        other => unexpected(other, "set"),
    }
}

/// Returns a [`DecodeError`] indicating that `value` was not of the `expected` kind.
pub fn unexpected<T>(value: &Value, expected: impl Into<String>) -> DecodeResult<T> {
    Err(DecodeError::unexpected(value, expected))
}

/// Decodes the element at `index` from a vector slice, failing if out of bounds.
pub fn vec_get<T: FromEdn>(index: usize, vec: &[TaggedValue]) -> DecodeResult<T> {
    match vec.get(index) {
        Some(item) => T::parse_edn(item),
        None => Err(DecodeError::new(format!(
            "expected vector with at least {} elements",
            index + 1,
        ))),
    }
}

/// Looks up `key` in `map` and applies `decoder` to the stored [`TaggedValue`].
pub fn map_get_with<'a, T>(
    key: &TaggedValue,
    map: &'a EdnMap,
    decoder: impl FnOnce(&'a TaggedValue) -> DecodeResult<T>,
) -> DecodeResult<T> {
    match map.get(key) {
        Some(tv) => decoder(tv),
        None => Err(DecodeError::new(format!(
            "key not found: {}",
            render_text(key),
        ))),
    }
}

/// Looks up an untagged `:key` in `map` and decodes its value via [`FromEdn`].
pub fn map_get_keyword<T: FromEdn>(key: &str, map: &EdnMap) -> DecodeResult<T> {
    let lookup = Tagged::NoTag(Value::Keyword(key.to_owned()));
    map_get_with(&lookup, map, T::parse_edn)
}

/// Looks up an untagged `"key"` string in `map` and decodes its value via [`FromEdn`].
pub fn map_get_string<T: FromEdn>(key: &str, map: &EdnMap) -> DecodeResult<T> {
    let lookup = Tagged::NoTag(Value::String(key.to_owned()));
    map_get_with(&lookup, map, T::parse_edn)
}

/// Looks up an unqualified untagged symbol `name` in `map` and decodes its value via [`FromEdn`].
pub fn map_get_symbol<T: FromEdn>(name: &str, map: &EdnMap) -> DecodeResult<T> {
    map_get_symbol_ns("", name, map)
}

/// Looks up a namespaced untagged symbol `namespace/name` in `map` and decodes its value via [`FromEdn`].
pub fn map_get_symbol_ns<T: FromEdn>(namespace: &str, name: &str, map: &EdnMap) -> DecodeResult<T> {
    let lookup = Tagged::NoTag(Value::symbol(namespace, name));
    map_get_with(&lookup, map, T::parse_edn)
}

fn ns_to_text<'a>(namespace: &str, name: &'a str) -> Cow<'a, str> {
    if namespace.is_empty() {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("{namespace}/{name}"))
    }
}

// ---------------------------------------------------------------------------
// Standard ToEdn implementations
// ---------------------------------------------------------------------------

impl<T: ToEdn + ?Sized> ToEdn for &T {
    fn to_edn(&self) -> TaggedValue {
        (**self).to_edn()
    }

    fn to_ednv(&self) -> Value {
        (**self).to_ednv()
    }
}

impl<T: ToEdn + ?Sized> ToEdn for Box<T> {
    fn to_edn(&self) -> TaggedValue {
        (**self).to_edn()
    }

    fn to_ednv(&self) -> Value {
        (**self).to_ednv()
    }
}

impl<T: ToEdn + ToOwned + ?Sized> ToEdn for Cow<'_, T> {
    fn to_edn(&self) -> TaggedValue {
        self.as_ref().to_edn()
    }

    fn to_ednv(&self) -> Value {
        self.as_ref().to_ednv()
    }
}

impl ToEdn for TaggedValue {
    fn to_edn(&self) -> TaggedValue {
        self.clone()
    }
}

impl ToEdn for Value {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(self.clone())
    }

    fn to_ednv(&self) -> Value {
        self.clone()
    }
}

impl ToEdn for Infallible {
    fn to_edn(&self) -> TaggedValue {
        match *self {}
    }
}

impl ToEdn for () {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Nil)
    }
}

impl ToEdn for bool {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Boolean(*self))
    }
}

impl ToEdn for str {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::String(self.to_owned()))
    }
}

impl ToEdn for String {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::String(self.clone()))
    }
}

impl ToEdn for char {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Character(*self))
    }
}

macro_rules! impl_to_edn_int {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToEdn for $ty {
                fn to_edn(&self) -> TaggedValue {
                    Tagged::NoTag(Value::Integer(i64::from(*self)))
                }
            }
        )+
    };
}

impl_to_edn_int!(i8, i16, i32, i64, u8, u16, u32);

impl ToEdn for f32 {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Floating(Float::new(f64::from(*self))))
    }
}

impl ToEdn for f64 {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Floating(Float::new(*self)))
    }
}

impl<T: ToEdn> ToEdn for Option<T> {
    fn to_edn(&self) -> TaggedValue {
        match self {
            None => Tagged::NoTag(Value::Nil),
            Some(inner) => inner.to_edn(),
        }
    }
}

impl<T: ToEdn> ToEdn for List<T> {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::List(self.0.iter().map(ToEdn::to_edn).collect()))
    }
}

impl<T: ToEdn> ToEdn for LinkedList<T> {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::List(self.iter().map(ToEdn::to_edn).collect()))
    }
}

impl<T: ToEdn> ToEdn for [T] {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Vec(self.iter().map(ToEdn::to_edn).collect()))
    }
}

impl<T: ToEdn, const N: usize> ToEdn for [T; N] {
    fn to_edn(&self) -> TaggedValue {
        self.as_slice().to_edn()
    }
}

impl<T: ToEdn> ToEdn for Vec<T> {
    fn to_edn(&self) -> TaggedValue {
        self.as_slice().to_edn()
    }
}

impl<T: ToEdn> ToEdn for BTreeSet<T> {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Set(self.iter().map(ToEdn::to_edn).collect()))
    }
}

impl<T: ToEdn, S: BuildHasher> ToEdn for HashSet<T, S> {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Set(self.iter().map(ToEdn::to_edn).collect()))
    }
}

impl<K: ToEdn, V: ToEdn> ToEdn for BTreeMap<K, V> {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Map(
            self.iter().map(|(k, v)| (k.to_edn(), v.to_edn())).collect(),
        ))
    }
}

impl<K: ToEdn, V: ToEdn, S: BuildHasher> ToEdn for HashMap<K, V, S> {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Map(
            self.iter().map(|(k, v)| (k.to_edn(), v.to_edn())).collect(),
        ))
    }
}

impl<A: ToEdn, B: ToEdn> ToEdn for (A, B) {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Vec(vec![self.0.to_edn(), self.1.to_edn()]))
    }
}

impl<A: ToEdn, B: ToEdn, C: ToEdn> ToEdn for (A, B, C) {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Vec(vec![
            self.0.to_edn(),
            self.1.to_edn(),
            self.2.to_edn(),
        ]))
    }
}

impl<A: ToEdn, B: ToEdn, C: ToEdn, D: ToEdn> ToEdn for (A, B, C, D) {
    fn to_edn(&self) -> TaggedValue {
        Tagged::NoTag(Value::Vec(vec![
            self.0.to_edn(),
            self.1.to_edn(),
            self.2.to_edn(),
            self.3.to_edn(),
        ]))
    }
}

#[cfg(feature = "chrono")]
impl ToEdn for DateTime<Utc> {
    fn to_edn(&self) -> TaggedValue {
        to_edn_tagged("", "inst", &format_utc_inst(*self))
    }
}

#[cfg(feature = "uuid")]
impl ToEdn for Uuid {
    fn to_edn(&self) -> TaggedValue {
        to_edn_tagged("", "uuid", &self.to_string())
    }
}

// ---------------------------------------------------------------------------
// Standard FromEdn implementations
// ---------------------------------------------------------------------------

impl FromEdn for TaggedValue {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Ok(value.clone())
    }
}

impl FromEdn for Value {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        Ok(value.clone())
    }
}

impl FromEdn for Infallible {
    fn parse_edn(_value: &TaggedValue) -> DecodeResult<Self> {
        Err(DecodeError::new("unable to construct Infallible value"))
    }
}

impl FromEdn for () {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_nil(value, || Ok(()))
    }
}

impl FromEdn for bool {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_boolean(value, Ok)
    }
}

impl FromEdn for String {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_textual(value, |s| Ok(s.to_owned()))
    }
}

impl FromEdn for Box<str> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_textual(value, |s| Ok(s.into()))
    }
}

impl FromEdn for char {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_character(value, Ok)
    }
}

impl FromEdn for i64 {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_integer(value, Ok)
    }
}

macro_rules! impl_from_edn_integral {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl FromEdn for $ty {
                fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
                    Self::parse_ednv(value.value())
                }

                fn parse_ednv(value: &Value) -> DecodeResult<Self> {
                    with_integral(value, Ok)
                }
            }
        )+
    };
}

impl_from_edn_integral!(i8, i16, i32, u8, u16, u32);

impl FromEdn for f64 {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_floating(value, Ok)
    }
}

impl FromEdn for f32 {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_fractional(value, Ok)
    }
}

impl<T: FromEdn> FromEdn for Option<T> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        match value.value() {
            Value::Nil => Ok(None),
            _ => T::parse_edn(value).map(Some),
        }
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        match value {
            Value::Nil => Ok(None),
            other => T::parse_ednv(other).map(Some),
        }
    }
}

impl<T: FromEdn> FromEdn for List<T> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_list(value, |items| {
            items
                .iter()
                .map(T::parse_edn)
                .collect::<DecodeResult<Vec<_>>>()
                .map(List)
        })
    }
}

impl<T: FromEdn> FromEdn for LinkedList<T> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_list(value, |items| items.iter().map(T::parse_edn).collect())
    }
}

impl<T: FromEdn> FromEdn for Vec<T> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_vec(value, |items| items.iter().map(T::parse_edn).collect())
    }
}

impl<T: FromEdn + Ord> FromEdn for BTreeSet<T> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_set(value, |items| items.iter().map(T::parse_edn).collect())
    }
}

impl<T: FromEdn + Eq + Hash, S: BuildHasher + Default> FromEdn for HashSet<T, S> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_set(value, |items| items.iter().map(T::parse_edn).collect())
    }
}

impl<K: FromEdn + Ord, V: FromEdn> FromEdn for BTreeMap<K, V> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_map(value, |map| {
            map.iter()
                .map(|(k, v)| Ok((K::parse_edn(k)?, V::parse_edn(v)?)))
                .collect()
        })
    }
}

impl<K: FromEdn + Eq + Hash, V: FromEdn, S: BuildHasher + Default> FromEdn for HashMap<K, V, S> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_map(value, |map| {
            map.iter()
                .map(|(k, v)| Ok((K::parse_edn(k)?, V::parse_edn(v)?)))
                .collect()
        })
    }
}

impl<A: FromEdn, B: FromEdn> FromEdn for (A, B) {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_vec(value, |items| match items {
            [a, b] => Ok((A::parse_edn(a)?, B::parse_edn(b)?)),
            _ => Err(DecodeError::new("vector of size 2 expected")),
        })
    }
}

impl<A: FromEdn, B: FromEdn, C: FromEdn> FromEdn for (A, B, C) {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_vec(value, |items| match items {
            [a, b, c] => Ok((A::parse_edn(a)?, B::parse_edn(b)?, C::parse_edn(c)?)),
            _ => Err(DecodeError::new("vector of size 3 expected")),
        })
    }
}

impl<A: FromEdn, B: FromEdn, C: FromEdn, D: FromEdn> FromEdn for (A, B, C, D) {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        Self::parse_ednv(value.value())
    }

    fn parse_ednv(value: &Value) -> DecodeResult<Self> {
        with_vec(value, |items| match items {
            [a, b, c, d] => Ok((
                A::parse_edn(a)?,
                B::parse_edn(b)?,
                C::parse_edn(c)?,
                D::parse_edn(d)?,
            )),
            _ => Err(DecodeError::new("vector of size 4 expected")),
        })
    }
}

#[cfg(feature = "chrono")]
impl FromEdn for DateTime<Utc> {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        alt(
            with_tagged("", "inst", value, |v| with_string(v, parse_utc_inst)),
            || with_no_tag(value, |v| with_string(v, parse_utc_inst)),
        )
    }
}

#[cfg(feature = "uuid")]
impl FromEdn for Uuid {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        alt(
            with_tagged("", "uuid", value, |v| with_string(v, parse_uuid)),
            || with_no_tag(value, |v| with_string(v, parse_uuid)),
        )
    }
}

#[cfg(feature = "chrono")]
fn format_utc_inst(dt: DateTime<Utc>) -> String {
    let base = dt.format("%Y-%m-%dT%H:%M:%S");
    let nanos = dt.timestamp_subsec_nanos();
    if nanos == 0 {
        format!("{base}Z")
    } else {
        let padded = format!("{nanos:09}");
        let trimmed = padded.trim_end_matches('0');
        format!("{base}.{trimmed}Z")
    }
}

#[cfg(feature = "chrono")]
fn parse_utc_inst(text: &str) -> DecodeResult<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Some(prefix) = text.strip_suffix("UTC") {
        let normalized = format!("{prefix}Z");
        if let Ok(dt) = DateTime::parse_from_rfc3339(&normalized) {
            return Ok(dt.with_timezone(&Utc));
        }
    }
    Err(DecodeError::new(format!(
        "invalid RFC 3339 timestamp: {text:?}"
    )))
}

#[cfg(feature = "uuid")]
fn parse_uuid(text: &str) -> DecodeResult<Uuid> {
    Uuid::parse_str(text).map_err(|_| DecodeError::new("invalid UUID string"))
}
