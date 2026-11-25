# AST, Parsing, and Pretty-Printing

Reference and usage guide for the core AST types, parser, and formatter exported from `redn`
(implemented in `redn-ast`).

## AST Data Model

Every EDN element is a [`TaggedValue`] (`Tagged<String, Value>`), separating optional reader tags
from the underlying untagged [`Value`]:

```rust
pub enum Tagged<T, A> {
    Tagged { namespace: T, name: T, value: A },
    NoTag(A),
}
```

An unqualified tag like `#uuid "..."` stores `namespace: ""` and `name: "uuid"`. A qualified tag
like `#myapp/Person {...}` stores `namespace: "myapp"` and `name: "Person"`.

[`Value`] covers all 12 built-in EDN forms:

| EDN syntax | Rust variant | Backing type |
| --- | --- | --- |
| `nil` | `Value::Nil` | — |
| `true`, `false` | `Value::Boolean(bool)` | `bool` |
| `"hello\n"` | `Value::String(String)` | `String` |
| `\a`, `\space`, `\uA650` | `Value::Character(char)` | `char` |
| `foo`, `ns/foo`, `/` | `Value::Symbol { namespace, name }` | `(String, String)` |
| `:foo`, `:ns/foo` | `Value::Keyword(String)` | `String` (without leading `:`) |
| `42`, `-7` | `Value::Integer(i64)` | `i64` |
| `3.14`, `1e-5` | `Value::Floating(Float)` | `Float` (`f64` wrapper) |
| `(a b)` | `Value::List(EdnList)` | `Vec<TaggedValue>` |
| `[a b]` | `Value::Vec(EdnVec)` | `Vec<TaggedValue>` |
| `{:a 1 :b 2}` | `Value::Map(EdnMap)` | `BTreeMap<TaggedValue, TaggedValue>` |
| `#{1 2}` | `Value::Set(EdnSet)` | `BTreeSet<TaggedValue>` |

Collection helpers `mk_list`, `mk_vec`, `mk_map`, and `mk_set` build the corresponding variants
from any `IntoIterator`.

## Ordering and `Float` Semantics

Because EDN maps and sets permit arbitrary keys (including tagged values, floats, and nested
collections), `TaggedValue`, `Value`, and `Float` all implement total `Eq`, `Ord`, and `Hash`:

- **`Tagged` order**: `Tagged::Tagged` sorts before `Tagged::NoTag`, then by `namespace`, `name`,
  and `value`.
- **`Value` order**: variants sort in declaration order (`Nil < Boolean < String < Character <
  Symbol < Keyword < Integer < Floating < List < Vec < Map < Set`).
- **`Float` order**: `-0.0` and `+0.0` compare and hash identically; all `NaN` bit patterns
  compare equal to one another and greater than `+inf`. The original `f64` bit pattern is
  preserved for retrieval (`Float::into_inner`) and formatting.

## Parsing

`parse_text(source_name, source)` parses a single top-level EDN element (ignoring leading and
trailing whitespace, commas, `;` line comments, and `#_` discards) and rejects trailing tokens:

```rust
use redn::parse_text;

let ast = parse_text("config.edn", "; settings\n{:port 8080, #_ :unused 0}")?;
```

For streaming or custom partial parsing, construct `Parser::new(source_name, source)` directly and
call element parsers (`parse_tagged`, `parse_value`, `parse_collections`, `parse_symbol`,
`parse_keyword`, `parse_number`, `parse_string`, `parse_character`, `parse_discard`, `drop_ws`).

### Syntax Rules

- **Whitespace**: Unicode whitespace, `,`, and `; ... \n` comments are skipped everywhere between
  tokens.
- **Discards (`#_`)**: `#_` skips the immediately following element (tagged or untagged). Chained
  discards (`#_a #_b c`) and discards before tags (`#_a #tag b`) are supported.
- **Reserved words**: `nil`, `true`, and `false` only match when not followed by a symbol
  character or `/`, so `nil-value`, `true?`, and `false/flag` parse as symbols.
- **Integers and floats**: signed 64-bit integers (`i64::MIN..=i64::MAX`) and IEEE-754 `f64`
  literals with decimal fraction and/or exponent (`42.0`, `1e5`, `-1.5e-10`). Out-of-range
  integers return a `ParseError`.
- **Characters**: `\newline`, `\return`, `\space`, `\tab`, hex Unicode escapes `\uXXXX` (1+ hex
  digits, validated as a Unicode scalar value), or `\c` for any printable character.
- **Strings**: double-quoted strings with `\n`, `\r`, `\t`, `\\`, `\"` escapes.
- **Maps**: require an even number of forms; odd counts fail with a `ParseError`.

Parse failures return `ParseError`, exposing `source_name()`, `offset()` (0-based byte offset),
`line()` (1-based), `column()` (1-based Unicode scalar column), `message()`, and `expected()`.

## Rendering and Pretty-Printing

Two entry points format a `TaggedValue` back to EDN text:

- `render_text(&tv)`: single-line unbounded output.
- `pretty_text(&tv)`: Wadler-Lindig layout targeting 80 columns (`DEFAULT_LINE_WIDTH`) with
  column-aligned collections.
- `pretty_text_with_options(&tv, options)`: custom `LayoutOptions` (`PageWidth::Unbounded` or
  `PageWidth::AvailablePerLine { line_width, ribbon_fraction }`).

Formatting guarantees:

- Every finite `Float` always renders with `.` or `e` (for example `42.0`, never `42`), so
  rendered floats always parse back as `Value::Floating`.
- Characters render as `\newline`, `\return`, `\tab`, `\space`, `\c` for printable ASCII other
  than `\` and `'`, and `\uXXXX` (uppercase hex, zero-padded to at least 4 digits) for all other
  scalar values.
- `render_text` and `pretty_text` both round-trip with `parse_text` for all valid EDN ASTs.
