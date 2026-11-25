use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use redn::chars::{
    KEYWORD_CHARS, KEYWORD_INITIAL_CHARS, SYMBOL_CHARS, SYMBOL_INITIAL_CHARS, TAG_CHARS,
};
use redn::{
    Float, Tagged, TaggedValue, Value, mk_list, mk_map, mk_set, mk_vec, parse_text, pretty_text,
    render_text,
};

fn char_from(alphabet: &'static str) -> impl Strategy<Value = char> {
    let chars: Vec<char> = alphabet.chars().collect();
    prop::sample::select(chars)
}

fn gen_text(alphabet: &'static str) -> impl Strategy<Value = String> {
    prop::collection::vec(char_from(alphabet), 1..=16)
        .prop_map(|cs| cs.into_iter().collect::<String>())
}

fn gen_text_initial(initial: &'static str, rest: &'static str) -> impl Strategy<Value = String> {
    (
        char_from(initial),
        prop::collection::vec(char_from(rest), 0..=15),
    )
        .prop_map(|(first, tail)| {
            let mut s = String::with_capacity(1 + tail.len());
            s.push(first);
            s.extend(tail);
            s
        })
}

fn gen_tag_ident() -> impl Strategy<Value = String> {
    gen_text(TAG_CHARS)
}

fn is_valid_edn_symbol(s: &str) -> bool {
    if matches!(s, "nil" | "true" | "false") {
        return false;
    }
    if s.starts_with(['+', '-', '.']) && s.as_bytes().get(1).is_some_and(u8::is_ascii_digit) {
        return false;
    }
    true
}

fn gen_symbol_part() -> impl Strategy<Value = String> {
    gen_text_initial(SYMBOL_INITIAL_CHARS, SYMBOL_CHARS).prop_filter(
        "avoid reserved literals and numeric +/-/. prefixes per EDN spec",
        |s| is_valid_edn_symbol(s),
    )
}

fn gen_string_with_escapes() -> impl Strategy<Value = String> {
    let char_strat = prop_oneof![
        16 => any::<char>(),
        1 => prop::sample::select(vec!['\\', '\n', '\r', '\t', '"']),
    ];
    prop::collection::vec(char_strat, 0..=16).prop_map(|cs| cs.into_iter().collect::<String>())
}

fn gen_finite_float() -> impl Strategy<Value = Value> {
    any::<f64>()
        .prop_filter("EDN only represents finite floats", |f| f.is_finite())
        .prop_map(|f| Value::Floating(Float::new(f)))
}

fn gen_atomic_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Nil),
        any::<bool>().prop_map(Value::Boolean),
        prop::collection::vec(any::<char>(), 0..=16)
            .prop_map(|cs| Value::String(cs.into_iter().collect())),
        gen_string_with_escapes().prop_map(Value::String),
        any::<char>().prop_map(Value::Character),
        any::<i64>().prop_map(Value::Integer),
        gen_finite_float(),
        gen_text_initial(KEYWORD_INITIAL_CHARS, KEYWORD_CHARS).prop_map(Value::Keyword),
        (gen_symbol_part(), gen_symbol_part()).prop_map(|(ns, name)| Value::symbol(ns, name)),
        gen_symbol_part().prop_map(|name| Value::symbol("", name)),
    ]
}

fn wrap_tagged(value_strat: BoxedStrategy<Value>) -> BoxedStrategy<TaggedValue> {
    prop_oneof![
        value_strat.clone().prop_map(Tagged::NoTag),
        (gen_tag_ident(), gen_tag_ident(), value_strat.clone())
            .prop_map(|(ns, name, v)| Tagged::new(ns, name, v)),
        (gen_tag_ident(), value_strat).prop_map(|(name, v)| Tagged::new("", name, v)),
    ]
    .boxed()
}

fn gen_tagged_value() -> BoxedStrategy<TaggedValue> {
    let leaf = wrap_tagged(gen_atomic_value().boxed());
    leaf.prop_recursive(3, 64, 8, |inner| {
        let collections = prop_oneof![
            prop::collection::vec(inner.clone(), 0..=8).prop_map(mk_set),
            prop::collection::vec(
                (wrap_tagged(gen_atomic_value().boxed()), inner.clone()),
                0..=8,
            )
            .prop_map(mk_map),
            prop::collection::vec(inner.clone(), 0..=8).prop_map(mk_vec),
            prop::collection::vec(inner, 0..=8).prop_map(mk_list),
        ];
        wrap_tagged(collections.boxed())
    })
    .boxed()
}

proptest! {
    #[test]
    fn prop_rendered_can_parse_back(tv in gen_tagged_value()) {
        let rendered = render_text(&tv);
        let reparsed = parse_text("<generated>", &rendered);
        prop_assert_eq!(reparsed, Ok(tv), "rendered: {}", rendered);
    }

    #[test]
    fn prop_pretty_can_parse_back(tv in gen_tagged_value()) {
        let pretty = pretty_text(&tv);
        let reparsed = parse_text("<generated>", &pretty);
        prop_assert_eq!(reparsed, Ok(tv), "pretty:\n{}", pretty);
    }
}

#[test]
fn prop_regr_empty_containers() {
    let empty_list = parse_text("<regr_empty_containers>", "( )").expect("valid empty list");
    assert_eq!(empty_list, Tagged::NoTag(Value::List(Vec::new())));

    let empty_vec = parse_text("<regr_empty_containers>", "[ ]").expect("valid empty vec");
    assert_eq!(empty_vec, Tagged::NoTag(Value::Vec(Vec::new())));

    let empty_set = parse_text("<regr_empty_containers>", "#{ }").expect("valid empty set");
    assert_eq!(empty_set, Tagged::NoTag(Value::Set(BTreeSet::new())));

    let empty_map = parse_text("<regr_empty_containers>", "{ }").expect("valid empty map");
    assert_eq!(empty_map, Tagged::NoTag(Value::Map(BTreeMap::new())));
}

fn check_pretty(plain: &str, expected_lines: &[&str]) {
    let value = parse_text("<propPretty>", plain).expect("valid EDN input");
    assert_eq!(render_text(&value), plain);
    let rendered_pretty = pretty_text(&value);
    let actual_lines: Vec<&str> = rendered_pretty.lines().collect();
    assert_eq!(actual_lines, expected_lines);
}

#[test]
fn prop_pretty_examples() {
    check_pretty(
        "{list (volume own would using shown small pound lady fill luck we flat education) \
         map {company grabbed dress fully excited enter goose forth require see sport studied} \
         set #{below best clearly explanation laid quiet race ring stopped street various} \
         vec [right square or seed thumb catch lose sold you talk combine troops steel vessels]}",
        &[
            "{list (volume",
            "       own",
            "       would",
            "       using",
            "       shown",
            "       small",
            "       pound",
            "       lady",
            "       fill",
            "       luck",
            "       we",
            "       flat",
            "       education)",
            " map {company grabbed",
            "      dress fully",
            "      excited enter",
            "      goose forth",
            "      require see",
            "      sport studied}",
            " set #{below",
            "       best",
            "       clearly",
            "       explanation",
            "       laid",
            "       quiet",
            "       race",
            "       ring",
            "       stopped",
            "       street",
            "       various}",
            " vec [right",
            "      square",
            "      or",
            "      seed",
            "      thumb",
            "      catch",
            "      lose",
            "      sold",
            "      you",
            "      talk",
            "      combine",
            "      troops",
            "      steel",
            "      vessels]}",
        ],
    );
}

#[test]
fn example_edn_kitchensink() {
    let source = include_str!("../fixtures/example.edn");
    let ast = parse_text("example.edn", source).expect("example.edn parses cleanly");
    let expected = Tagged::NoTag(Value::List(vec![
        Tagged::new("rust", "edn", Value::symbol("example", "kitchensink")),
        Tagged::NoTag(Value::Nil),
        Tagged::NoTag(Value::Character(' ')),
        Tagged::NoTag(Value::string("dynamic \"typing\"")),
        Tagged::NoTag(mk_map([
            (
                Tagged::NoTag(Value::keyword("bools")),
                Tagged::NoTag(mk_set([
                    Tagged::NoTag(Value::Boolean(false)),
                    Tagged::NoTag(Value::Boolean(true)),
                ])),
            ),
            (
                Tagged::NoTag(Value::keyword("numbers")),
                Tagged::NoTag(mk_vec([
                    Tagged::NoTag(Value::Integer(42)),
                    Tagged::NoTag(Value::floating(42.0)),
                ])),
            ),
        ])),
    ]));
    assert_eq!(ast, expected);
}

#[test]
fn parser_discards_comments_commas_and_edge_cases() {
    assert_eq!(
        parse_text("discard", "#_ignored 42").expect("discard before value"),
        Tagged::NoTag(Value::Integer(42)),
    );
    assert_eq!(
        parse_text("discard", "#_first #_second #ns/tag 7").expect("chained discards before tag"),
        Tagged::new("ns", "tag", Value::Integer(7)),
    );
    assert_eq!(
        parse_text("discard", "#_#ns/tag [1 2] :ok").expect("discard of tagged value"),
        Tagged::NoTag(Value::keyword("ok")),
    );
    assert_eq!(
        parse_text("discard", "[1, #_2, ; comment\n 3]").expect("discard inside vector"),
        Tagged::NoTag(mk_vec([
            Tagged::NoTag(Value::Integer(1)),
            Tagged::NoTag(Value::Integer(3)),
        ])),
    );
    assert_eq!(
        parse_text("symbols", "(/ - + -foo +bar nil-val true? false/flag)").expect("edge symbols"),
        Tagged::NoTag(mk_list([
            Tagged::NoTag(Value::symbol("", "/")),
            Tagged::NoTag(Value::symbol("", "-")),
            Tagged::NoTag(Value::symbol("", "+")),
            Tagged::NoTag(Value::symbol("", "-foo")),
            Tagged::NoTag(Value::symbol("", "+bar")),
            Tagged::NoTag(Value::symbol("", "nil-val")),
            Tagged::NoTag(Value::symbol("", "true?")),
            Tagged::NoTag(Value::symbol("false", "flag")),
        ])),
    );
    assert_eq!(
        parse_text(
            "chars",
            r"[\newline \return \space \tab \n \u \u0041 \uA650 \u1F600 \?]"
        )
        .expect("character literals"),
        Tagged::NoTag(mk_vec([
            Tagged::NoTag(Value::Character('\n')),
            Tagged::NoTag(Value::Character('\r')),
            Tagged::NoTag(Value::Character(' ')),
            Tagged::NoTag(Value::Character('\t')),
            Tagged::NoTag(Value::Character('n')),
            Tagged::NoTag(Value::Character('u')),
            Tagged::NoTag(Value::Character('A')),
            Tagged::NoTag(Value::Character('Ꙑ')),
            Tagged::NoTag(Value::Character('😀')),
            Tagged::NoTag(Value::Character('?')),
        ])),
    );
    assert_eq!(
        parse_text(
            "nums",
            "[-9223372036854775808 9223372036854775807 +42 1e5 -1.5e-10]"
        )
        .expect("extreme numbers"),
        Tagged::NoTag(mk_vec([
            Tagged::NoTag(Value::Integer(i64::MIN)),
            Tagged::NoTag(Value::Integer(i64::MAX)),
            Tagged::NoTag(Value::Integer(42)),
            Tagged::NoTag(Value::floating(1e5)),
            Tagged::NoTag(Value::floating(-1.5e-10)),
        ])),
    );
}

#[test]
fn float_ordering_and_set_behavior() {
    let neg_zero = Tagged::NoTag(Value::floating(-0.0));
    let pos_zero = Tagged::NoTag(Value::floating(0.0));
    assert_eq!(neg_zero, pos_zero);

    let set = mk_set([
        Tagged::NoTag(Value::floating(f64::NAN)),
        Tagged::NoTag(Value::floating(f64::NAN)),
        neg_zero,
        pos_zero,
        Tagged::NoTag(Value::floating(1.0)),
    ]);
    let Value::Set(items) = set else {
        panic!("expected set");
    };
    assert_eq!(items.len(), 3);
}

#[test]
fn parse_errors_report_location_and_reason() {
    let err = parse_text("bad.edn", "{:a 1 :b}").expect_err("odd map must fail");
    assert_eq!(err.source_name(), "bad.edn");
    assert_eq!(err.line(), 1);
    assert!(err.to_string().contains("odd number"), "got: {err}");

    let err = parse_text("bad.edn", "\"unterminated").expect_err("unterminated string must fail");
    assert!(err.to_string().contains("unterminated"), "got: {err}");

    let err = parse_text("bad.edn", "1 2").expect_err("trailing form must fail");
    assert!(err.to_string().contains("end of input"), "got: {err}");

    let err = parse_text("bad.edn", "99999999999999999999999999").expect_err("overflow must fail");
    assert!(err.to_string().contains("out of 64-bit"), "got: {err}");

    let err = parse_text("bad.edn", r"\uD800").expect_err("surrogate code point must fail");
    assert!(err.to_string().contains("invalid unicode"), "got: {err}");

    let err = parse_text("bad.edn", "   ; empty\n").expect_err("empty doc must fail");
    assert_eq!(err.line(), 2);
}
