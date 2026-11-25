use redn::functor::{Tag, ValueF, ana, cata, cata_owned, embed, hylo, para, project};
use redn::{Tagged, TaggedValue, Value, parse_text};

#[test]
fn project_and_embed_roundtrip() {
    let ast =
        parse_text("t", r"#ns/doc {:a [1 2] :b #{true nil} :c (x/y \space)}").expect("valid EDN");
    let rebuilt = embed(project(ast.clone()));
    assert_eq!(rebuilt, ast);
}

#[test]
fn cata_display() {
    let nilly_willy = parse_text("t", "(nil #foo/bar nil)").expect("valid EDN");
    let display = cata(&nilly_willy, &mut |layer| match layer {
        ValueF::Nil(None) => "nil".to_owned(),
        ValueF::Nil(Some(tag)) => format!("{tag} nil"),
        ValueF::List(_, xs) => format!("({})", xs.join(" ")),
        other => panic!("unexpected layer: {other:?}"),
    });
    assert_eq!(display, "(nil #foo/bar nil)");
}

#[test]
fn cata_arith() {
    let expr = parse_text("t", "(1 (2 3) (4 5))").expect("valid EDN");
    let sum: Result<f64, &'static str> = cata_owned(expr, &mut |layer| match layer {
        #[expect(
            clippy::cast_precision_loss,
            reason = "small test integers convert exactly to f64"
        )]
        ValueF::Integer(_, i) => Ok(i as f64),
        ValueF::Floating(_, d) => Ok(d.into_inner()),
        ValueF::List(_, xs) => {
            let nums: Vec<f64> = xs.into_iter().collect::<Result<_, _>>()?;
            Ok(nums.into_iter().sum())
        }
        ValueF::Nil(_) => Err("nil is not a number"),
        _ => Err("unsupported node"),
    });
    assert_eq!(sum, Ok(15.0));
}

#[test]
fn para_calc() {
    let expr = parse_text("t", "(+ (/ 6 3 2) (* 2 3) (- 4 5))").expect("valid EDN");

    let value: Result<f64, String> = para(&expr, &mut |layer| match layer {
        ValueF::Symbol { .. } => Err("Operator in a value position".to_owned()),
        #[expect(
            clippy::cast_precision_loss,
            reason = "small test integers convert exactly to f64"
        )]
        ValueF::Integer(_, i) => Ok(i as f64),
        ValueF::Floating(_, d) => Ok(d.into_inner()),
        ValueF::List(_, items) => {
            let Some((head, tail)) = items.split_first() else {
                return Err("empty form".to_owned());
            };
            let (Tagged::NoTag(Value::Symbol { name: op, .. }), Err(_)) = head else {
                return Err(format!("Form head is not a symbol: {}", head.0));
            };
            let args: Vec<f64> = tail
                .iter()
                .map(|(_, res)| res.clone())
                .collect::<Result<_, _>>()?;
            match op.as_str() {
                "+" => Ok(args.into_iter().sum()),
                "-" => Ok(args.into_iter().rev().fold(0.0, |acc, x| x - acc)),
                "*" => Ok(args.into_iter().product()),
                "/" => match args.as_slice() {
                    [] => Err("No arguments given to `/`".to_owned()),
                    [n] => Ok(1.0 / n),
                    [first, rest @ ..] => Ok(rest.iter().fold(*first, |acc, d| acc / d)),
                },
                other => Err(format!("Unknown operation: {other}")),
            }
        }
        ValueF::Nil(_) => Err("nil is not a number".to_owned()),
        _ => Err("unsupported".to_owned()),
    });

    // (+ (/ 6 3 2) (* 2 3) (- 4 5)) => 1.0 + 6.0 + (-1.0) = 6.0
    assert_eq!(value, Ok(6.0));
}

#[test]
fn ana_and_hylo_build_and_fold_countdown() {
    let built: TaggedValue = ana(3_i64, &mut |n| {
        if n <= 0 {
            ValueF::Integer(Some(Tag::unqualified("base")), 0)
        } else {
            ValueF::List(None, vec![n - 1, n - 1])
        }
    });
    let leaf_count = cata(&built, &mut |layer| match layer {
        ValueF::Integer(_, _) => 1_usize,
        ValueF::List(_, xs) => xs.into_iter().sum(),
        _ => 0,
    });
    assert_eq!(leaf_count, 8);

    let hylo_count = hylo(
        3_i64,
        &mut |n| {
            if n <= 0 {
                ValueF::Integer(None, 0)
            } else {
                ValueF::List(None, vec![n - 1, n - 1])
            }
        },
        &mut |layer| match layer {
            ValueF::Integer(_, _) => 1_usize,
            ValueF::List(_, xs) => xs.into_iter().sum(),
            _ => 0,
        },
    );
    assert_eq!(hylo_count, leaf_count);
}
