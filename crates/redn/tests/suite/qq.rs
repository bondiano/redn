use redn::qq::{edn, edn_list, edn_map, edn_set, edn_vec, from_edn};
use redn::{
    DecodeResult, FromEdn, Tagged, TaggedValue, Value, map_get_keyword, mk_list, mk_map, mk_set,
    mk_vec, render_text, with_map, with_tagged,
};

fn qq_items() -> Vec<TaggedValue> {
    vec![
        Tagged::NoTag(Value::Boolean(false)),
        Tagged::NoTag(Value::Integer(42)),
        Tagged::new("my", "tagged", Value::symbol("", "symbol")),
    ]
}

#[test]
fn prop_tagged_value() {
    assert_eq!(
        Tagged::new("test", "tagged", Value::Nil),
        edn!(r#" #test/tagged nil "#),
    );
}

#[test]
fn prop_list() {
    assert_eq!(
        mk_list(qq_items()),
        edn_list!(
            r#"
            false,
            42
            #my/tagged symbol
            "#
        ),
    );
}

#[test]
fn prop_vector() {
    assert_eq!(
        mk_vec(qq_items()),
        edn_vec!(
            r#"
            false,
            42
            #my/tagged symbol
            "#
        ),
    );
}

#[test]
fn prop_set() {
    assert_eq!(
        mk_set(qq_items()),
        edn_set!(
            r#"
            false,
            42
            #my/tagged symbol
            "#
        ),
    );
    assert_eq!(render_text(&Tagged::NoTag(edn_set!("a is a"))), "#{a is}",);
}

#[test]
fn prop_map() {
    let pairs = [
        (
            Tagged::NoTag(Value::symbol("this", "is")),
            Tagged::NoTag(Value::Boolean(false)),
        ),
        (
            Tagged::NoTag(Value::keyword("the-answer")),
            Tagged::NoTag(Value::Integer(42)),
        ),
        (
            Tagged::new("my", "tagged", Value::symbol("", "key")),
            Tagged::NoTag(Value::symbol("", "value")),
        ),
    ];
    assert_eq!(
        mk_map(pairs),
        edn_map!(
            r#"
            this/is false,
            :the-answer 42
            #my/tagged key value
            "#
        ),
    );
}

#[derive(Debug, PartialEq, Eq)]
struct Person {
    first: String,
    last: String,
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
fn prop_from_value() {
    assert_eq!(from_edn!((), "nil"), Ok(()));

    let inferred: () = from_edn!("nil").expect("inferred unit");
    assert_eq!(inferred, ());

    let fred = from_edn!(Person, r#"#myapp/Person {:first "Fred" :last "Mertz"}"#)
        .expect("person decodes");
    assert_eq!(
        fred,
        Person {
            first: "Fred".to_owned(),
            last: "Mertz".to_owned(),
        },
    );
}
