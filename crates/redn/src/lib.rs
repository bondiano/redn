//! EDN (Extensible Data Notation) parsing, formatting, typed encoding/decoding,
//! compile-time literals, and recursion schemes.
//!
//! # Overview
//!
//! - **AST, parser, and formatter**: [`Tagged`], [`TaggedValue`], [`Value`], [`Float`],
//!   [`parse_text`], [`Parser`], [`render_text`], [`pretty_text`], and [`Doc`].
//! - **Typed codecs** ([`codec`]): [`ToEdn`], [`FromEdn`], [`encode_text`], [`decode_text`],
//!   [`from_edn`], and `Result`-based combinators (`with_*`, [`vec_get`], `map_get_*`, [`alt`]).
//! - **Lists vs vectors** ([`list`]): [`Vec<T>`] maps to EDN vectors `[...]`; wrap in [`List<T>`]
//!   (or use [`std::collections::LinkedList`]) for parenthesized lists `(...)`.
//! - **Compile-time literals** (`qq`, `macros` feature): `edn!`, `edn_list!`, `edn_vec!`,
//!   `edn_set!`, `edn_map!`, and `from_edn!`.
//! - **Recursion schemes** ([`functor`]): [`functor::ValueF`], [`functor::project`],
//!   [`functor::embed`], [`functor::cata`], [`functor::para`], [`functor::ana`], and
//!   [`functor::hylo`].
//!
//! # Cargo features
//!
//! All features are additive and enabled by default:
//!
//! - `macros`: re-exports compile-time EDN literal macros under `redn::qq`.
//! - `uuid`: implements [`ToEdn`] and [`FromEdn`] for `uuid::Uuid` (`#uuid "..."`).
//! - `chrono`: implements [`ToEdn`] and [`FromEdn`] for `chrono::DateTime<chrono::Utc>`
//!   (`#inst "..."`).
//!
//! # Examples
//!
//! ```
//! use redn::{Tagged, Value, decode_text, encode_text, parse_text, render_text};
//!
//! let ast = parse_text("example", "#myapp/Point [10 20]")?;
//! assert_eq!(render_text(&ast), "#myapp/Point [10 20]");
//! assert_eq!(
//!     ast,
//!     Tagged::new(
//!         "myapp",
//!         "Point",
//!         Value::Vec(vec![
//!             Tagged::untagged(Value::Integer(10)),
//!             Tagged::untagged(Value::Integer(20)),
//!         ]),
//!     ),
//! );
//!
//! let encoded = encode_text(&(true, 42_i64));
//! assert_eq!(encoded, "[true 42]");
//! let decoded: (bool, i64) = decode_text("example", &encoded)?;
//! assert_eq!(decoded, (true, 42));
//! # Ok::<(), redn::Error>(())
//! ```

pub mod codec;
pub mod error;
pub mod functor;
pub mod list;

/// Compile-time EDN literal macros.
///
/// Each macro parses its string literal at compile time and expands directly
/// into AST constructors without runtime parsing.
///
/// # Examples
///
/// ```
/// use redn::qq::{edn, edn_list, edn_map, edn_set, edn_vec, from_edn};
/// use redn::{Tagged, Value, render_text};
///
/// let tagged = edn!("#myapp/flag true");
/// assert_eq!(tagged, Tagged::new("myapp", "flag", Value::Boolean(true)));
///
/// assert_eq!(render_text(&Tagged::NoTag(edn_list!("1 2 3"))), "(1 2 3)");
/// assert_eq!(render_text(&Tagged::NoTag(edn_vec!("1 2 3"))), "[1 2 3]");
/// assert_eq!(render_text(&Tagged::NoTag(edn_set!("a b a"))), "#{a b}");
/// assert_eq!(render_text(&Tagged::NoTag(edn_map!(":k 1"))), "{:k 1}");
///
/// let pair = from_edn!((bool, i64), "[true 42]")?;
/// assert_eq!(pair, (true, 42));
/// # Ok::<(), redn::DecodeError>(())
/// ```
///
/// Syntax errors in EDN literals are rejected at compile time:
///
/// ```compile_fail
/// let _ = redn::qq::edn!("(unclosed");
/// ```
#[cfg(feature = "macros")]
pub mod qq {
    pub use redn_macros::{edn, edn_list, edn_map, edn_set, edn_vec, from_edn};
}

pub use codec::{
    FromEdn, FromF64, ToEdn, decode_text, encode_text, from_edn, map_get_keyword, map_get_string,
    map_get_symbol, map_get_symbol_ns, map_get_with, to_edn_tagged, unexpected, vec_get,
    with_boolean, with_character, with_floating, with_fractional, with_integer, with_integral,
    with_keyword, with_list, with_map, with_nil, with_no_tag, with_set, with_string, with_symbol,
    with_tagged, with_textual, with_vec,
};
pub use error::{DecodeError, DecodeResult, Error, alt};
pub use list::List;
pub use redn_ast::{
    Doc, EdnList, EdnMap, EdnSet, EdnVec, Float, LayoutOptions, PageWidth, ParseError, Parser,
    Tagged, TaggedValue, Value, chars, mk_list, mk_map, mk_set, mk_vec, parse_text,
    pretty_tagged_value, pretty_text, pretty_text_with_options, pretty_value, render_text,
    strip_tag,
};

const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<DecodeError>();
    assert_send_sync::<Error>();
    assert_send_sync::<List<i64>>();
    assert_send_sync::<functor::Tag>();
    assert_send_sync::<functor::ValueF<()>>();
};
