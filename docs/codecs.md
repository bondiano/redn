# Encoding and Decoding (`ToEdn` / `FromEdn`)

Guide and reference for converting between Rust types and EDN values via `redn::codec` and
`redn::error`.

## Top-Level Entry Points

- `encode_text(&value)` converts any `T: ToEdn` to AST and renders it on a single line.
- `decode_text::<T>(source_name, source)` parses EDN text and decodes it via `FromEdn`, returning
  `Result<T, redn::Error>` (`Error::Parse(ParseError)` or `Error::Decode(DecodeError)`).
- `from_edn::<T>(&ast)` decodes an already-parsed `TaggedValue`, returning `DecodeResult<T>`.
- `to_edn_tagged(namespace, tag, &value)` encodes `value` to an untagged `Value` and wraps it
  with `#namespace/tag` (or `#tag` when `namespace` is empty).

## Built-In Type Mappings

| Rust type | EDN representation | Notes |
| --- | --- | --- |
| `TaggedValue` | unchanged | Identity codec |
| `Value` | untagged `Value` | Strips tag on decode |
| `std::convert::Infallible` | uninhabited | Always fails on decode |
| `()` | `nil` | |
| `bool` | `true` / `false` | |
| `String`, `Box<str>`, `&str`*, `Cow<str>`* | `"..."` | `FromEdn` accepts `String`, `Character`, `Keyword`, or `Symbol` via `with_textual` |
| `char` | `\c`, `\space`, `\uXXXX` | |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32` | `42` | `FromEdn` checks target bounds via `TryFrom<i64>` |
| `f32`, `f64` | `42.0` | |
| `Option<T>` | `nil` or `T` | Any `nil` (tagged or untagged) decodes to `None`; `Some(v)` preserves `v`'s tag |
| `List<T>`, `LinkedList<T>` | `(...)` | Use `List<T>` wrapper when a `Vec<T>` should serialize as a list |
| `Vec<T>`, `[T; N]`*, `&[T]`* | `[...]` | |
| `BTreeSet<T>`, `HashSet<T>` | `#{...}` | |
| `BTreeMap<K, V>`, `HashMap<K, V>` | `{k v ...}` | |
| `(A, B)`, `(A, B, C)`, `(A, B, C, D)` | `[a b ...]`, exact length | Fails if vector length mismatches |
| `uuid::Uuid` (`uuid` feature) | `#uuid "..."` | Decodes `#uuid "..."` or untagged `"..."` |
| `chrono::DateTime<Utc>` (`chrono` feature) | `#inst "1985-04-12T23:20:50.52Z"` | Formats RFC 3339 `Z` with trimmed fractional zeros; decodes RFC 3339 and `UTC`-suffixed timestamps, tagged or untagged |

Types marked `*` implement `ToEdn` only. `&T`, `Box<T>`, and `Cow<'_, T>` delegate to `T::to_edn`.

## Implementing `ToEdn` and `FromEdn`

`ToEdn` requires `to_edn(&self) -> TaggedValue` and provides `to_ednv(&self) -> Value`.
`FromEdn` requires `parse_edn(&TaggedValue) -> DecodeResult<Self>` and provides `parse_ednv(&Value)`.

```rust
use std::collections::BTreeMap;
use redn::{
    DecodeResult, FromEdn, TaggedValue, ToEdn, Value, map_get_keyword, to_edn_tagged,
    with_map, with_tagged,
};

#[derive(Debug, PartialEq, Eq)]
struct Endpoint {
    host: String,
    port: u16,
}

impl ToEdn for Endpoint {
    fn to_edn(&self) -> TaggedValue {
        let map = BTreeMap::from([
            (Value::keyword("host"), self.host.to_edn()),
            (Value::keyword("port"), self.port.to_edn()),
        ]);
        to_edn_tagged("net", "Endpoint", &map)
    }
}

impl FromEdn for Endpoint {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        with_tagged("net", "Endpoint", value, |v| {
            with_map(v, |m| {
                Ok(Self {
                    host: map_get_keyword("host", m)?,
                    port: map_get_keyword("port", m)?,
                })
            })
        })
    }
}
```

## Decoder Combinators

Because every combinator returns `DecodeResult<T>` (`Result<T, DecodeError>`), decoders compose
directly with `?`.

- **Tag guards**: `with_tagged(ns, name, tv, f)`, `with_no_tag(tv, f)`.
- **Primitive guards**: `with_nil`, `with_boolean`, `with_string`, `with_character`, `with_symbol`,
  `with_keyword`, `with_textual`, `with_integer`, `with_integral`, `with_floating`,
  `with_fractional`.
- **Collection guards**: `with_list`, `with_vec`, `with_map`, `with_set`.
- **Lookup helpers**:
  - `vec_get::<T>(index, slice)` decodes element `index` or fails with an expected-length error.
  - `map_get_with(&key, map, decoder)` looks up an arbitrary `TaggedValue` key.
  - `map_get_keyword::<T>(key, map)`, `map_get_string::<T>(key, map)`,
    `map_get_symbol::<T>(name, map)`, `map_get_symbol_ns::<T>(ns, name, map)`.
- **Alternatives**: `alt(first, || second)` runs `second` lazily when `first` fails and merges
  both branches' expected labels into the resulting `DecodeError`.

## Error Formatting

`DecodeError` holds a primary message and a list of expected labels. When expected labels are
present (such as from `with_textual` or chained `alt` calls), `Display` renders:

```text
Unexpected integer
expected
  string, char, keyword, symbol
```

When no expected labels are attached (such as missing map keys or mismatched tags), `Display`
renders the message alone (for example `key not found: :host`).
