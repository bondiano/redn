//! Compile-time EDN literal proc-macros for `redn`.
//!
//! Each macro parses its EDN string literal at compile time using `redn-ast` and
//! expands directly into AST constructor calls, turning EDN syntax errors into
//! compile errors.

use proc_macro::TokenStream;
use proc_macro2::{Literal, Span, TokenStream as TokenStream2};
use quote::quote;
use redn_ast::{Tagged, TaggedValue, Value, parse_text};
use syn::parse::{Parse, ParseStream};
use syn::{LitStr, Token, Type, parse_macro_input};

/// Parses a single EDN element at compile time and expands to a `redn::TaggedValue`.
///
/// Syntax errors fail compilation with the parser diagnostic attached to the
/// string literal span.
///
/// ```text
/// let value = redn::qq::edn!("#myapp/Point [10 20]");
/// ```
#[proc_macro]
pub fn edn(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    match parse_text("<edn!>", &lit.value()) {
        Ok(tv) => lift_tagged_value(&tv).into(),
        Err(err) => compile_err(lit.span(), &err.to_string()).into(),
    }
}

/// Parses EDN list elements at compile time and expands to an untagged `redn::Value::List`.
///
/// Pass elements without the enclosing parentheses: `edn_list!("1 2 :a")`.
#[proc_macro]
pub fn edn_list(input: TokenStream) -> TokenStream {
    expand_collection(input, "<edn_list!>", "(", ")")
}

/// Parses EDN vector elements at compile time and expands to an untagged `redn::Value::Vec`.
///
/// Pass elements without the enclosing brackets: `edn_vec!("1 2 :a")`.
#[proc_macro]
pub fn edn_vec(input: TokenStream) -> TokenStream {
    expand_collection(input, "<edn_vec!>", "[", "]")
}

/// Parses EDN set elements at compile time and expands to an untagged `redn::Value::Set`.
///
/// Pass elements without the enclosing `#{` `}` delimiters: `edn_set!("a b a")`.
/// Duplicate elements are deduplicated at compile time.
#[proc_macro]
pub fn edn_set(input: TokenStream) -> TokenStream {
    expand_collection(input, "<edn_set!>", "#{", "}")
}

/// Parses EDN map key-value pairs at compile time and expands to an untagged `redn::Value::Map`.
///
/// Pass alternating key and value forms without the enclosing braces:
/// `edn_map!(":host \"localhost\" :port 8080")`.
#[proc_macro]
pub fn edn_map(input: TokenStream) -> TokenStream {
    expand_collection(input, "<edn_map!>", "{", "}")
}

struct FromEdnInput {
    target_type: Option<Type>,
    source: LitStr,
}

impl Parse for FromEdnInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            let source = input.parse::<LitStr>()?;
            return Ok(Self {
                target_type: None,
                source,
            });
        }
        let target_type = input.parse::<Type>()?;
        let _comma = input.parse::<Token![,]>()?;
        let source = input.parse::<LitStr>()?;
        Ok(Self {
            target_type: Some(target_type),
            source,
        })
    }
}

/// Parses EDN syntax at compile time and decodes it through `redn::FromEdn`.
///
/// Accepts either `from_edn!(r#"..."#)` (inferring the target type from context)
/// or `from_edn!(TargetType, r#"..."#)`, returning `redn::DecodeResult<TargetType>`.
#[proc_macro]
pub fn from_edn(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as FromEdnInput);
    match parse_text("<from_edn!>", &parsed.source.value()) {
        Ok(tv) => {
            let ast = lift_tagged_value(&tv);
            let expanded = if let Some(ty) = parsed.target_type {
                quote! { ::redn::from_edn::<#ty>(&#ast) }
            } else {
                quote! { ::redn::from_edn(&#ast) }
            };
            expanded.into()
        }
        Err(err) => compile_err(parsed.source.span(), &err.to_string()).into(),
    }
}

fn expand_collection(
    input: TokenStream,
    source_name: &str,
    open: &str,
    close: &str,
) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    let doc = format!("{open}{}{close}", lit.value());
    match parse_text(source_name, &doc) {
        Ok(Tagged::NoTag(value)) => lift_value(&value).into(),
        Ok(Tagged::Tagged { .. }) => compile_err(lit.span(), "unexpected tagged value").into(),
        Err(err) => compile_err(lit.span(), &err.to_string()).into(),
    }
}

fn compile_err(span: Span, msg: &str) -> TokenStream2 {
    syn::Error::new(span, msg).to_compile_error()
}

fn lift_tagged_value(tv: &TaggedValue) -> TokenStream2 {
    match tv {
        Tagged::NoTag(value) => {
            let inner = lift_value(value);
            quote! { ::redn::TaggedValue::untagged(#inner) }
        }
        Tagged::Tagged {
            namespace,
            name,
            value,
        } => {
            let inner = lift_value(value);
            quote! { ::redn::TaggedValue::new(#namespace, #name, #inner) }
        }
    }
}

fn lift_value(value: &Value) -> TokenStream2 {
    match value {
        Value::Nil => quote! { ::redn::Value::Nil },
        Value::Boolean(b) => quote! { ::redn::Value::Boolean(#b) },
        Value::String(s) => quote! { ::redn::Value::string(#s) },
        Value::Character(c) => quote! { ::redn::Value::Character(#c) },
        Value::Symbol { namespace, name } => quote! { ::redn::Value::symbol(#namespace, #name) },
        Value::Keyword(k) => quote! { ::redn::Value::keyword(#k) },
        Value::Integer(i) => {
            let lit = Literal::i64_suffixed(*i);
            quote! { ::redn::Value::Integer(#lit) }
        }
        Value::Floating(f) => {
            let raw = f.into_inner();
            let expr = if raw.is_nan() {
                quote! { ::core::primitive::f64::NAN }
            } else if raw.is_infinite() {
                if raw.is_sign_positive() {
                    quote! { ::core::primitive::f64::INFINITY }
                } else {
                    quote! { ::core::primitive::f64::NEG_INFINITY }
                }
            } else {
                let lit = Literal::f64_suffixed(raw);
                quote! { #lit }
            };
            quote! { ::redn::Value::floating(#expr) }
        }
        Value::List(items) => {
            if items.is_empty() {
                quote! { ::redn::Value::List(::std::vec::Vec::new()) }
            } else {
                let lifted = items.iter().map(lift_tagged_value);
                quote! { ::redn::Value::List(::std::vec![ #(#lifted),* ]) }
            }
        }
        Value::Vec(items) => {
            if items.is_empty() {
                quote! { ::redn::Value::Vec(::std::vec::Vec::new()) }
            } else {
                let lifted = items.iter().map(lift_tagged_value);
                quote! { ::redn::Value::Vec(::std::vec![ #(#lifted),* ]) }
            }
        }
        Value::Set(items) => {
            if items.is_empty() {
                quote! { ::redn::Value::Set(::std::collections::BTreeSet::new()) }
            } else {
                let lifted = items.iter().map(lift_tagged_value);
                quote! { ::redn::Value::Set(::std::collections::BTreeSet::from([ #(#lifted),* ])) }
            }
        }
        Value::Map(pairs) => {
            if pairs.is_empty() {
                quote! { ::redn::Value::Map(::std::collections::BTreeMap::new()) }
            } else {
                let lifted = pairs.iter().map(|(k, v)| {
                    let lk = lift_tagged_value(k);
                    let lv = lift_tagged_value(v);
                    quote! { (#lk, #lv) }
                });
                quote! { ::redn::Value::Map(::std::collections::BTreeMap::from([ #(#lifted),* ])) }
            }
        }
    }
}
