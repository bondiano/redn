//! Core AST types, parser, and Wadler-Lindig pretty printer for EDN.
//!
//! Zero-dependency crate shared between `redn` and `redn-macros` so runtime and
//! compile-time EDN parsing use the exact same parser and AST representation.
//!
//! # Examples
//!
//! ```
//! use redn_ast::{parse_text, pretty_text, render_text};
//!
//! let ast = parse_text("example.edn", "{:b [1 2], :a true}")?;
//! assert_eq!(render_text(&ast), "{:a true :b [1 2]}");
//! assert_eq!(pretty_text(&ast), "{:a true :b [1 2]}");
//! # Ok::<(), redn_ast::ParseError>(())
//! ```

pub mod chars;
pub mod float;
pub mod parser;
pub mod printer;
pub mod tagged;
pub mod value;

pub use float::Float;
pub use parser::{ParseError, Parser, parse_text};
pub use printer::{
    Doc, LayoutOptions, PageWidth, pretty_tagged_value, pretty_text, pretty_text_with_options,
    pretty_value, render_text,
};
pub use tagged::{Tagged, strip_tag};
pub use value::{
    EdnList, EdnMap, EdnSet, EdnVec, TaggedValue, Value, mk_list, mk_map, mk_set, mk_vec,
};

const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<Float>();
    assert_send_sync::<TaggedValue>();
    assert_send_sync::<Value>();
    assert_send_sync::<ParseError>();
    assert_send_sync::<Doc>();
};
