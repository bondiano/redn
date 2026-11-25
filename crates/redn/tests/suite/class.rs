use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::fmt::Debug;

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use redn::{
    DecodeResult, Error, FromEdn, List, Tagged, TaggedValue, ToEdn, Value, alt, decode_text,
    encode_text, from_edn, map_get_keyword, map_get_string, map_get_symbol, map_get_symbol_ns,
    render_text, to_edn_tagged, vec_get, with_integral, with_map, with_no_tag, with_tagged,
    with_textual,
};
#[cfg(feature = "uuid")]
use uuid::Uuid;

fn codec_example<T>(encoded: &str, value: &T)
where
    T: ToEdn + FromEdn + PartialEq + Debug,
{
    let ast = value.to_edn();
    let roundtrip = from_edn::<T>(&ast).expect("from_edn roundtrip failed");
    assert_eq!(&roundtrip, value, "AST roundtrip mismatch for {encoded}");
    assert_eq!(render_text(&ast), encoded);
    assert_eq!(encode_text(value), encoded);

    let from_text = decode_text::<T>("<codecExample>", encoded).expect("decode_text failed");
    assert_eq!(&from_text, value, "text roundtrip mismatch for {encoded}");
}

#[test]
fn prop_nil_unit() {
    codec_example("nil", &());
}

#[test]
fn prop_bool_true() {
    codec_example("true", &true);
}

#[test]
fn prop_bool_false() {
    codec_example("false", &false);
}

#[test]
fn prop_text() {
    codec_example(
        r#""stringly \"typed\" languages""#,
        &"stringly \"typed\" languages".to_owned(),
    );
}

#[test]
fn prop_char_unicode_hex() {
    codec_example(r"\uA650", &'Ꙑ');
}

#[test]
fn prop_char_named_ws() {
    codec_example(r"\space", &' ');
}

#[test]
fn prop_char_basic() {
    codec_example(r"\n", &'n');
}

#[test]
fn prop_int() {
    codec_example("42", &42_i64);
}

#[test]
fn prop_double() {
    codec_example("42.0", &42.0_f64);
}

fn hetero_items() -> Vec<Value> {
    vec![
        1_i64.to_ednv(),
        true.to_ednv(),
        Value::symbol("sort-of", "hetero"),
    ]
}

#[test]
fn prop_nil_maybe() {
    codec_example::<Option<Infallible>>("nil", &None);
    // Tagged nil also decodes to None.
    let tagged_nil = Tagged::new("my", "nil", Value::Nil);
    assert_eq!(from_edn::<Option<i64>>(&tagged_nil), Ok(None));
}

#[test]
fn prop_list_empty() {
    codec_example("()", &List::<Infallible>::new());
}

#[test]
fn prop_list() {
    codec_example("(1 true sort-of/hetero)", &List(hetero_items()));
}

#[test]
fn prop_vector_empty() {
    codec_example("[]", &Vec::<Infallible>::new());
}

#[test]
fn prop_vector() {
    codec_example("[1 true sort-of/hetero]", &hetero_items());
}

#[test]
fn prop_set_empty() {
    codec_example("#{}", &BTreeSet::<Infallible>::new());
}

#[test]
fn prop_set() {
    let set: BTreeSet<Value> = hetero_items().into_iter().collect();
    codec_example("#{true sort-of/hetero 1}", &set);
}

#[test]
#[expect(
    clippy::zero_sized_map_values,
    reason = "verifies empty map encoding with unit key and value types"
)]
fn prop_map_empty() {
    codec_example("{}", &BTreeMap::<(), ()>::new());
}

#[test]
fn prop_map() {
    let map = BTreeMap::from([
        (Value::keyword("the-answer"), Value::floating(42.0)),
        (Value::symbol("the", "question"), Value::Character('?')),
    ]);
    codec_example(r"{the/question \? :the-answer 42.0}", &map);
}

#[test]
fn prop_tuple_2() {
    codec_example("[true nil]", &(true, ()));
}

#[test]
fn prop_tuple_3() {
    codec_example(r"[true nil \space]", &(true, (), ' '));
}

#[test]
fn prop_tuple_4() {
    codec_example(
        r"[true nil \space ()]",
        &(true, (), ' ', List::<Infallible>::new()),
    );
}

#[cfg(feature = "chrono")]
#[test]
fn prop_tagged_utctime() {
    let dt: DateTime<Utc> = "1985-04-12T23:20:50.52Z"
        .parse()
        .expect("valid RFC 3339 timestamp");
    codec_example(r#"#inst "1985-04-12T23:20:50.52Z""#, &dt);

    // Also accepts untagged RFC 3339 strings and "UTC" suffix timestamps.
    let untagged: DateTime<Utc> =
        decode_text("test", r#""1985-04-12T23:20:50.52Z""#).expect("untagged inst");
    assert_eq!(untagged, dt);

    let utc_suffix: DateTime<Utc> =
        decode_text("test", r#"#inst "1985-04-12T23:20:50.52UTC""#).expect("UTC suffix inst");
    assert_eq!(utc_suffix, dt);
}

#[cfg(feature = "uuid")]
#[test]
fn prop_tagged_uuid() {
    let id = Uuid::parse_str("f81d4fae-7dec-11d0-a765-00a0c91e6bf6").expect("valid uuid");
    codec_example(r#"#uuid "f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#, &id);

    let untagged: Uuid =
        decode_text("test", r#""f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#).expect("untagged uuid");
    assert_eq!(untagged, id);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Person {
    first: String,
    last: String,
}

impl ToEdn for Person {
    fn to_edn(&self) -> TaggedValue {
        let map = BTreeMap::from([
            (Value::keyword("first"), self.first.to_edn()),
            (Value::keyword("last"), self.last.to_edn()),
        ]);
        to_edn_tagged("myapp", "Person", &map)
    }
}

impl FromEdn for Person {
    fn parse_edn(value: &TaggedValue) -> DecodeResult<Self> {
        with_tagged("myapp", "Person", value, |inner| {
            with_map(inner, |m| {
                Ok(Self {
                    first: map_get_keyword("first", m)?,
                    last: map_get_keyword("last", m)?,
                })
            })
        })
    }
}

#[test]
fn prop_tagged_person() {
    codec_example(
        r#"#myapp/Person {:first "Fred" :last "Mertz"}"#,
        &Person {
            first: "Fred".to_owned(),
            last: "Mertz".to_owned(),
        },
    );
}

#[cfg(feature = "uuid")]
#[test]
fn prop_unexpected() {
    let uuid: Uuid = decode_text(
        "class.rs:234",
        r#"#uuid "f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#,
    )
    .expect("uuid decodes");
    assert_eq!(
        uuid,
        Uuid::parse_str("f81d4fae-7dec-11d0-a765-00a0c91e6bf6").expect("valid uuid"),
    );
}

#[test]
fn decoder_combinators_and_error_reporting() {
    // with_textual accepts String, Character, Keyword, Symbol, and reports all 4 expected kinds on failure.
    assert_eq!(
        decode_text::<String>("t", ":my-key").expect("keyword as String"),
        "my-key",
    );
    assert_eq!(
        decode_text::<String>("t", "my/sym").expect("namespaced symbol as String"),
        "my/sym",
    );
    assert_eq!(
        decode_text::<String>("t", r"\x").expect("char as String"),
        "x",
    );

    let err = with_textual(&Value::Integer(42), |s| Ok(s.to_owned())).expect_err("int not textual");
    assert_eq!(
        err.to_string(),
        "Unexpected integer\nexpected\n  string, char, keyword, symbol",
    );

    // alt accumulates expected labels across branches.
    let combined = alt::<i64>(redn::unexpected(&Value::Nil, "integer"), || {
        redn::unexpected(&Value::Nil, "double")
    })
    .expect_err("both branches fail");
    assert_eq!(combined.expected(), &["integer", "double"]);
    assert_eq!(
        combined.to_string(),
        "Unexpected nil\nexpected\n  integer, double",
    );

    // Map lookup helpers: string, symbol, namespaced symbol, missing key.
    let map_ast = redn::parse_text("m", r#"{"s" 1 sym 2 ns/sym 3}"#).expect("valid map");
    let Value::Map(map) = map_ast.value() else {
        panic!("expected map");
    };
    assert_eq!(map_get_string::<i64>("s", map), Ok(1));
    assert_eq!(map_get_symbol::<i64>("sym", map), Ok(2));
    assert_eq!(map_get_symbol_ns::<i64>("ns", "sym", map), Ok(3));
    let missing = map_get_keyword::<i64>("absent", map).expect_err("missing key");
    assert_eq!(missing.to_string(), "key not found: :absent");

    // vec_get bounds error.
    let items = [Tagged::NoTag(Value::Integer(10))];
    assert_eq!(vec_get::<i64>(0, &items), Ok(10));
    let oob = vec_get::<i64>(2, &items).expect_err("out of bounds");
    assert_eq!(oob.to_string(), "expected vector with at least 3 elements");

    // with_no_tag rejects tagged values; with_tagged reports wrong tag clearly.
    let tagged = Tagged::new("foo", "bar", Value::Nil);
    assert_eq!(
        with_no_tag(&tagged, |_| Ok(()))
            .expect_err("reject tag")
            .to_string(),
        "no tag expected, got #foo/bar",
    );
    assert_eq!(
        with_tagged("baz", "qux", &tagged, |_| Ok(()))
            .expect_err("wrong tag")
            .to_string(),
        "unexpected tag. expecting: #baz/qux; got: #foo/bar",
    );

    // with_integral rejects overflow cleanly.
    let overflow = with_integral::<u8, _>(&Value::Integer(300), Ok).expect_err("u8 overflow");
    assert!(
        overflow.to_string().contains("out of range"),
        "got: {overflow}"
    );

    // Error wraps both parse and decode failures.
    let parse_err = decode_text::<i64>("e", "{").expect_err("parse failure");
    assert!(matches!(parse_err, Error::Parse(_)));
    let decode_err = decode_text::<i64>("e", "nil").expect_err("decode failure");
    assert!(matches!(decode_err, Error::Decode(_)));
}
