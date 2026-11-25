# Compile-Time EDN Macros (`redn::qq`)

When the `macros` Cargo feature is enabled (on by default), `redn::qq` exports six proc-macros
that parse EDN literals at compile time using `redn-ast`.

## Available Macros

| Macro | Input | Output type |
| --- | --- | --- |
| `edn!("...")` | Any single EDN form | `redn::TaggedValue` |
| `edn_list!("...")` | List elements (without outer `(` `)`) | `redn::Value` (`Value::List`) |
| `edn_vec!("...")` | Vector elements (without outer `[` `]`) | `redn::Value` (`Value::Vec`) |
| `edn_set!("...")` | Set elements (without outer `#{` `}`) | `redn::Value` (`Value::Set`) |
| `edn_map!("...")` | Key-value pairs (without outer `{` `}`) | `redn::Value` (`Value::Map`) |
| `from_edn!("...")` / `from_edn!(T, "...")` | Any single EDN form (+ optional target type) | `redn::DecodeResult<T>` |

## Examples

```rust
use redn::qq::{edn, edn_list, edn_map, edn_set, edn_vec, from_edn};
use redn::{Tagged, Value, render_text};

let tagged = edn!("#myapp/user {:id 1 :active true}");

let list = edn_list!("1, 2, 3");
let vec = edn_vec!("false 42 #my/tag sym");
let set = edn_set!("a is a"); // deduplicated at compile time -> #{a is}
let map = edn_map!(r#"
    :host "localhost"
    :port 8080
"#);

assert_eq!(render_text(&Tagged::NoTag(set)), "#{a is}");

// Explicit type or context-inferred type:
let tuple = from_edn!((bool, i64), "[true 42]")?;
let inferred: (bool, i64) = from_edn!("[true 42]")?;
assert_eq!(tuple, inferred);
```

## Compile-Time Guarantees and Raw Strings

- All six macros parse their string literal during macro expansion and emit direct constructor
  calls (`TaggedValue::new`, `Value::...`, `BTreeMap::from`, `BTreeSet::from`) with zero runtime
  parsing.
- Syntax errors (unclosed delimiters, odd map forms, invalid character escapes, integer overflow)
  are reported as compiler errors pointing to the literal span.
- Use Rust raw string literals (`r"..."` or `r#"..."#`) when EDN inputs contain backslashes
  (`\space`, `\uA650`) or embedded double quotes (`"text"`).
- `from_edn!` validates EDN syntax and lifts the AST at compile time, then invokes
  `redn::from_edn` at runtime so any downstream `FromEdn` implementation works without extra macro
  registration.
