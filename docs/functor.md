# Base Functor and Recursion Schemes (`redn::functor`)

`redn::functor` factors recursive child positions out of `TaggedValue` via the non-recursive base
functor `ValueF<R>`, letting you write generic folds, unfolds, and AST transformations without
manual recursion boilerplate.

## Types and Single-Layer Conversions

- `Tag { pub namespace: String, pub name: String }`: optional tag metadata attached to every
  `ValueF<R>` variant.
- `ValueF<R>`: mirrors all 12 `Value` variants, with child positions parameterized by `R`
  (`List(Option<Tag>, Vec<R>)`, `Vec(Option<Tag>, Vec<R>)`, `Map(Option<Tag>, Vec<(R, R)>)`,
  `Set(Option<Tag>, Vec<R>)`).
- `ValueF::map(self, f)` maps over every direct child position; `ValueF::try_map(self, f)` maps
  fallibly with short-circuiting `Result`.
- `project(TaggedValue) -> ValueF<TaggedValue>` unwraps one layer by value.
- `project_ref(&TaggedValue) -> ValueF<&TaggedValue>` unwraps one layer by reference without
  cloning nested children.
- `embed(ValueF<TaggedValue>) -> TaggedValue` re-wraps one layer into a `TaggedValue`.

## Schemes

| Function | Direction | Signature summary |
| --- | --- | --- |
| `cata` | bottom-up fold (borrowed) | `(&TaggedValue, &mut FnMut(ValueF<A>) -> A) -> A` |
| `cata_owned` | bottom-up fold (owning) | `(TaggedValue, &mut FnMut(ValueF<A>) -> A) -> A` |
| `para` | bottom-up fold with original subtrees | `(&'a TaggedValue, &mut FnMut(ValueF<(&'a TaggedValue, A)>) -> A) -> A` |
| `ana` | top-down unfold | `(S, &mut FnMut(S) -> ValueF<S>) -> TaggedValue` |
| `hylo` | fused unfold then fold (no intermediate AST) | `(S, coalg, alg) -> A` |

## Examples

### Bottom-up fold with `cata`

Sum every integer in an arbitrary nested EDN tree:

```rust
use redn::functor::{ValueF, cata};
use redn::parse_text;

let ast = parse_text("nums", "(1 [2 3] {:a 4 :b #{5}})")?;
let total: i64 = cata(&ast, &mut |layer| match layer {
    ValueF::Integer(_, n) => n,
    ValueF::List(_, xs) | ValueF::Vec(_, xs) | ValueF::Set(_, xs) => xs.into_iter().sum(),
    ValueF::Map(_, pairs) => pairs.into_iter().map(|(k, v)| k + v).sum(),
    _ => 0,
});
assert_eq!(total, 15);
```

### Inspecting original subtrees with `para`

`para` passes `(&TaggedValue, A)` at every child slot, which is useful when parent decisions
depend on raw syntax (such as operator heads in S-expressions) while still reusing already-folded
child results:

```rust
use redn::functor::{ValueF, para};
use redn::{Tagged, Value, parse_text};

let expr = parse_text("calc", "(+ (* 2 3) 4)")?;
let result: Result<i64, String> = para(&expr, &mut |layer| match layer {
    ValueF::Integer(_, i) => Ok(i),
    ValueF::Symbol { .. } => Err("bare operator".into()),
    ValueF::List(_, items) => {
        let Some(((Tagged::NoTag(Value::Symbol { name: op, .. }), _), args)) =
            items.split_first()
        else {
            return Err("expected operator head".into());
        };
        let nums: Vec<i64> = args.iter().map(|(_, r)| r.clone()).collect::<Result<_, _>>()?;
        match op.as_str() {
            "+" => Ok(nums.into_iter().sum()),
            "*" => Ok(nums.into_iter().product()),
            other => Err(format!("unknown op: {other}")),
        }
    }
    _ => Err("unsupported".into()),
});
assert_eq!(result, Ok(10));
```

### Building and fusing trees with `ana` and `hylo`

```rust
use redn::functor::{ValueF, ana, hylo};
use redn::render_text;

let tree = ana(2_i64, &mut |n| {
    if n <= 0 {
        ValueF::Integer(None, 1)
    } else {
        ValueF::Vec(None, vec![n - 1, n - 1])
    }
});
assert_eq!(render_text(&tree), "[[1 1] [1 1]]");

// Compute leaf sum directly without allocating any TaggedValue:
let sum: i64 = hylo(
    2_i64,
    &mut |n| {
        if n <= 0 {
            ValueF::Integer(None, 1)
        } else {
            ValueF::Vec(None, vec![n - 1, n - 1])
        }
    },
    &mut |layer| match layer {
        ValueF::Integer(_, i) => i,
        ValueF::Vec(_, xs) => xs.into_iter().sum(),
        _ => 0,
    },
);
assert_eq!(sum, 4);
```
