# redn

EDN ([Extensible Data Notation](https://github.com/edn-format/edn)) parsing, formatting,
typed encoding/decoding, compile-time literals, and recursion schemes for Rust.

## Features

- **Zero-dependency core AST, parser, and pretty printer** (`redn-ast`): full EDN syntax
  including tags (`#ns/tag`, `#tag`), discards (`#_`), commas-as-whitespace, `;` line comments,
  named and Unicode character literals, single-line rendering, and Wadler-Lindig multi-line
  pretty-printing.
- **Typed codecs (`ToEdn` / `FromEdn`)**: ergonomic `Result`-based combinators (`with_tagged`,
  `with_map`, `map_get_keyword`, `alt`, etc.) with structured error reporting and accumulated
  expected-type labels.
- **Compile-time EDN literals (`redn::qq`)**: proc-macros (`edn!`, `edn_list!`, `edn_vec!`,
  `edn_set!`, `edn_map!`, `from_edn!`) that validate syntax at compile time and emit direct AST
  constructors without runtime parsing.
- **Recursion schemes (`redn::functor`)**: non-recursive base functor `ValueF<R>` with `project`,
  `project_ref`, `embed`, `cata`, `cata_owned`, `para`, `ana`, and `hylo`.
- **Safe by construction**: `#![forbid(unsafe_code)]` across the entire workspace; all public
  types are `Send + Sync + 'static`.

## Installation

Add `redn` to your `Cargo.toml`:

```toml
[dependencies]
redn = "0.1"
```

### Cargo Features

All features are additive and enabled by default:

| Feature | Default | Capability |
| --- | --- | --- |
| `macros` | yes | Compile-time EDN literal macros in `redn::qq` |
| `uuid` | yes | `ToEdn` / `FromEdn` for `uuid::Uuid` (`#uuid "..."`) |
| `chrono` | yes | `ToEdn` / `FromEdn` for `chrono::DateTime<chrono::Utc>` (`#inst "..."`) |

For the core AST, parser, printer, codecs, and recursion schemes with no optional dependencies:

```toml
[dependencies]
redn = { version = "0.1", default-features = false }
```

## Quick Start

### Parse, render, and pretty-print AST

```rust
use redn::{parse_text, pretty_text, render_text};

fn main() -> Result<(), redn::Error> {
    let ast = parse_text("example.edn", r#"#app/config {:port 8080, :hosts ["a" "b"]}"#)?;
    assert_eq!(
        render_text(&ast),
        r#"#app/config {:hosts ["a" "b"] :port 8080}"#,
    );
    let _formatted = pretty_text(&ast);
    Ok(())
}
```

### Custom typed encoding and decoding

```rust
use std::collections::BTreeMap;
use redn::{
    DecodeResult, FromEdn, TaggedValue, ToEdn, Value, decode_text, encode_text,
    map_get_keyword, to_edn_tagged, with_map, with_tagged,
};

#[derive(Debug, PartialEq, Eq)]
struct Person {
    first: String,
    last: String,
}

impl ToEdn for Person {
    fn to_edn(&self) -> TaggedValue {
        let fields = BTreeMap::from([
            (Value::keyword("first"), self.first.to_edn()),
            (Value::keyword("last"), self.last.to_edn()),
        ]);
        to_edn_tagged("myapp", "Person", &fields)
    }
}

impl FromEdn for Person {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        with_tagged("myapp", "Person", value, |inner| {
            with_map(inner, |map| {
                Ok(Self {
                    first: map_get_keyword("first", map)?,
                    last: map_get_keyword("last", map)?,
                })
            })
        })
    }
}

fn main() -> Result<(), redn::Error> {
    let fred = Person {
        first: "Fred".into(),
        last: "Mertz".into(),
    };
    let text = encode_text(&fred);
    assert_eq!(text, r#"#myapp/Person {:first "Fred" :last "Mertz"}"#);

    let decoded: Person = decode_text("person.edn", &text)?;
    assert_eq!(decoded, fred);
    Ok(())
}
```

### Compile-time EDN literals (`macros` feature)

```rust
use redn::qq::{edn, edn_map, from_edn};
use redn::{Tagged, Value};

let tagged = edn!("#myapp/flag true");
assert_eq!(tagged, Tagged::new("myapp", "flag", Value::Boolean(true)));

let map = edn_map!(":a 1, :b 2");
let pair: (bool, i64) = from_edn!("[true 42]")?;
assert_eq!(pair, (true, 42));
```

Invalid EDN syntax inside `edn!`, `edn_list!`, `edn_vec!`, `edn_set!`, `edn_map!`, or
`from_edn!` fails compilation with the parser's error message.

## Documentation

Guides:

- [AST, Parsing, and Pretty-Printing](docs/ast.md) — data model, EDN syntax rules, ordering
  guarantees, and Wadler-Lindig layout options.
- [Encoding and Decoding (`ToEdn` / `FromEdn`)](docs/codecs.md) — built-in type mappings,
  combinators, and error reporting.
- [Compile-Time Macros (`redn::qq`)](docs/macros.md) — literal macros, compile-time validation,
  and `from_edn!`.
- [Base Functor and Recursion Schemes (`redn::functor`)](docs/functor.md) — `ValueF<R>`,
  `project`/`embed`, `cata`, `para`, `ana`, and `hylo`.

Build local API reference docs with:

```sh
just doc
```

## Workspace Layout

| Crate | Purpose |
| --- | --- |
| [`crates/redn`](crates/redn) | Main user-facing crate: re-exports AST and macros, adds codecs and recursion schemes |
| [`crates/redn-ast`](crates/redn-ast) | Zero-dependency AST types (`Tagged`, `Value`, `Float`), parser, and Wadler-Lindig printer |
| [`crates/redn-macros`](crates/redn-macros) | Proc-macro crate providing compile-time EDN literal macros |

## Development

Requires Rust `1.95.0` (pinned in [rust-toolchain.toml](rust-toolchain.toml)), [`just`](https://just.systems/),
[`cargo-machete`](https://github.com/bnjbvr/cargo-machete), and Node.js (for [tools/style/no-iife.mjs](tools/style/no-iife.mjs)).

```sh
just fmt           # format workspace with rustfmt.toml
just lint          # fmt --check, clippy -D warnings (all targets + isolated features), cargo machete, no-iife
just test          # workspace unit/property/doc tests + isolated feature matrix
just doc           # build rustdoc with -D warnings
just check         # full CI gate: lint + test + doc
```

## License

MIT.
