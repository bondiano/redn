//! Recursive-descent EDN lexer and parser.

use std::error::Error;
use std::fmt;

use crate::chars::{
    is_keyword_char, is_keyword_initial_char, is_symbol_char, is_symbol_initial_char, is_tag_char,
};
use crate::float::Float;
use crate::tagged::Tagged;
use crate::value::{TaggedValue, Value, mk_list, mk_map, mk_set, mk_vec};

/// Error returned when parsing an EDN document fails.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct ParseError {
    source_name: String,
    offset: usize,
    line: usize,
    column: usize,
    message: String,
    expected: Vec<String>,
}

impl ParseError {
    /// Returns the source name supplied to [`parse_text`] or [`Parser::new`].
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Returns the 0-based byte offset where parsing failed.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the 1-based line number where parsing failed.
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns the 1-based column number (in Unicode scalar values) where parsing failed.
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Returns the primary error description.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the list of expected tokens or constructs, if any.
    pub fn expected(&self) -> &[String] {
        &self.expected
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.source_name, self.line, self.column, self.message
        )?;
        if !self.expected.is_empty() {
            write!(f, "; expected {}", self.expected.join(", "))?;
        }
        Ok(())
    }
}

impl Error for ParseError {}

/// Parses a single-expression EDN document into a [`TaggedValue`].
///
/// `source_name` identifies the input in error messages (for example
/// `"example.edn"` or `"<stdin>"`).
///
/// # Examples
///
/// ```
/// use redn_ast::{Tagged, Value, parse_text};
///
/// let ast = parse_text("example.edn", "#ns/tag [1, 2 ; comment\n]")?;
/// assert_eq!(
///     ast,
///     Tagged::new(
///         "ns",
///         "tag",
///         Value::Vec(vec![
///             Tagged::NoTag(Value::Integer(1)),
///             Tagged::NoTag(Value::Integer(2)),
///         ]),
///     ),
/// );
/// # Ok::<(), redn_ast::ParseError>(())
/// ```
///
/// # Errors
///
/// Returns a [`ParseError`] with source name, byte offset, and 1-based line
/// and column if `source` contains invalid EDN syntax, an integer outside
/// `i64`, an odd number of map forms, or trailing tokens after the root value.
pub fn parse_text(source_name: &str, source: &str) -> Result<TaggedValue, ParseError> {
    let mut parser = Parser::new(source_name, source);
    parser.parse_doc()
}

/// Stateful EDN parser exposing both full-document and primitive element parsers.
#[derive(Debug, Clone)]
pub struct Parser<'src> {
    source_name: String,
    input: &'src str,
    pos: usize,
}

impl<'src> Parser<'src> {
    /// Creates a new parser positioned at the beginning of `input`.
    pub fn new(source_name: impl Into<String>, input: &'src str) -> Self {
        Self {
            source_name: source_name.into(),
            input,
            pos: 0,
        }
    }

    /// Returns the current byte offset within the input.
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// Returns the unparsed suffix of the input.
    pub fn remaining(&self) -> &'src str {
        &self.input[self.pos..]
    }

    /// Parses leading whitespace, a single [`TaggedValue`], and end of input.
    pub fn parse_doc(&mut self) -> Result<TaggedValue, ParseError> {
        self.drop_ws();
        let value = self.parse_tagged()?;
        self.drop_ws();
        if let Some(ch) = self.peek() {
            return Err(self.error(
                format!("unexpected character {ch:?}"),
                vec!["end of input".to_owned()],
            ));
        }
        Ok(value)
    }

    /// Skips whitespace, commas (treated as whitespace in EDN), and `;` line comments.
    pub fn drop_ws(&mut self) {
        loop {
            let start = self.pos;
            while let Some(ch) = self.peek() {
                if ch.is_whitespace() || ch == ',' {
                    self.advance(ch);
                } else {
                    break;
                }
            }
            if self.peek() == Some(';') {
                while let Some(ch) = self.peek() {
                    self.advance(ch);
                    if ch == '\n' {
                        break;
                    }
                }
            }
            if self.pos == start {
                break;
            }
        }
    }

    /// Parses a tagged or untagged EDN value.
    pub fn parse_tagged(&mut self) -> Result<TaggedValue, ParseError> {
        self.skip_discards()?;
        if self.peek() == Some('#') {
            let saved = self.pos;
            if let Some((namespace, name)) = self.try_parse_tag_header() {
                self.drop_ws();
                let value = self.parse_value()?;
                return Ok(Tagged::Tagged {
                    namespace,
                    name,
                    value,
                });
            }
            self.pos = saved;
        }
        let value = self.parse_value()?;
        Ok(Tagged::NoTag(value))
    }

    /// Parses a single untagged EDN [`Value`], skipping any leading `#_` discards.
    pub fn parse_value(&mut self) -> Result<Value, ParseError> {
        self.skip_discards()?;
        if let Some(val) = self.try_parse_nil() {
            return Ok(val);
        }
        if let Some(val) = self.try_parse_bool() {
            return Ok(val);
        }
        if let Some(res) = self.try_parse_number() {
            return res;
        }
        match self.peek() {
            Some('\\') => self.parse_character(),
            Some('"') => self.parse_string(),
            Some(':') => self.parse_keyword(),
            Some('#' | '{' | '[' | '(') => self.parse_collections(),
            Some('/') => self.parse_symbol(),
            Some(ch) if is_symbol_initial_char(ch) => self.parse_symbol(),
            Some(ch) => Err(self.error(
                format!("unexpected character {ch:?}"),
                vec!["EDN value".to_owned()],
            )),
            None => Err(self.error("unexpected end of input", vec!["EDN value".to_owned()])),
        }
    }

    /// Parses and discards a `#_` discard form followed by the discarded element.
    pub fn parse_discard(&mut self) -> Result<(), ParseError> {
        if !self.remaining().starts_with("#_") {
            return Err(self.error("expected discard reader macro", vec!["#_".to_owned()]));
        }
        self.pos += 2;
        self.drop_ws();
        let _ = self.parse_tagged()?;
        Ok(())
    }

    /// Parses `nil`.
    pub fn parse_nil(&mut self) -> Result<Value, ParseError> {
        self.try_parse_nil()
            .ok_or_else(|| self.error("expected nil", vec!["nil".to_owned()]))
    }

    /// Parses `true` or `false`.
    pub fn parse_bool(&mut self) -> Result<Value, ParseError> {
        self.try_parse_bool().ok_or_else(|| {
            self.error(
                "expected boolean",
                vec!["true".to_owned(), "false".to_owned()],
            )
        })
    }

    /// Parses an EDN floating-point or 64-bit signed integer literal.
    pub fn parse_number(&mut self) -> Result<Value, ParseError> {
        self.try_parse_number()
            .unwrap_or_else(|| Err(self.error("expected number", vec!["number".to_owned()])))
    }

    /// Parses a `:keyword`.
    pub fn parse_keyword(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() != Some(':') {
            return Err(self.error("expected keyword", vec!["keyword".to_owned()]));
        }
        self.advance(':');
        let Some(first) = self.peek().filter(|&c| is_keyword_initial_char(c)) else {
            self.pos = start;
            return Err(self.error("invalid keyword", vec!["keyword".to_owned()]));
        };
        let name_start = self.pos;
        self.advance(first);
        while let Some(ch) = self.peek() {
            if is_keyword_char(ch) {
                self.advance(ch);
            } else {
                break;
            }
        }
        let name = self.input[name_start..self.pos].to_owned();
        self.drop_ws();
        Ok(Value::Keyword(name))
    }

    /// Parses a symbol (`name`, `namespace/name`, or `/`).
    pub fn parse_symbol(&mut self) -> Result<Value, ParseError> {
        if self.peek() == Some('/') {
            self.advance('/');
            self.drop_ws();
            return Ok(Value::symbol("", "/"));
        }
        let Some(first) = self.peek().filter(|&c| is_symbol_initial_char(c)) else {
            return Err(self.error("expected symbol", vec!["symbol".to_owned()]));
        };
        let first_start = self.pos;
        self.advance(first);
        while let Some(ch) = self.peek() {
            if is_symbol_char(ch) {
                self.advance(ch);
            } else {
                break;
            }
        }
        let head = &self.input[first_start..self.pos];
        if self.peek() == Some('/') {
            let slash_pos = self.pos;
            self.advance('/');
            if let Some(name_first) = self.peek().filter(|&c| is_symbol_initial_char(c)) {
                let name_start = self.pos;
                self.advance(name_first);
                while let Some(ch) = self.peek() {
                    if is_symbol_char(ch) {
                        self.advance(ch);
                    } else {
                        break;
                    }
                }
                let ns = head.to_owned();
                let name = self.input[name_start..self.pos].to_owned();
                self.drop_ws();
                return Ok(Value::Symbol {
                    namespace: ns,
                    name,
                });
            }
            self.pos = slash_pos;
        }
        let name = head.to_owned();
        self.drop_ws();
        Ok(Value::Symbol {
            namespace: String::new(),
            name,
        })
    }

    /// Parses a character literal (`\c`, `\newline`, `\return`, `\space`, `\tab`, or `\uXXXX`).
    pub fn parse_character(&mut self) -> Result<Value, ParseError> {
        if self.peek() != Some('\\') {
            return Err(self.error("expected character literal", vec!["character".to_owned()]));
        }
        self.advance('\\');

        if let Some(res) = self.try_parse_unicode_char() {
            let ch = res?;
            self.drop_ws();
            return Ok(Value::Character(ch));
        }

        for (name, ch) in [
            ("newline", '\n'),
            ("return", '\r'),
            ("space", ' '),
            ("tab", '\t'),
        ] {
            if self.remaining().starts_with(name) {
                self.pos += name.len();
                self.drop_ws();
                return Ok(Value::Character(ch));
            }
        }

        match self.peek() {
            Some(ch) if is_printable_char(ch) => {
                self.advance(ch);
                self.drop_ws();
                Ok(Value::Character(ch))
            }
            Some(ch) => Err(self.error(
                format!("non-printable character {ch:?} in character literal"),
                vec!["printable character".to_owned()],
            )),
            None => Err(self.error(
                "unexpected end of input in character literal",
                vec!["character".to_owned()],
            )),
        }
    }

    /// Parses a double-quoted EDN string literal.
    pub fn parse_string(&mut self) -> Result<Value, ParseError> {
        if self.peek() != Some('"') {
            return Err(self.error("expected string", vec!["string".to_owned()]));
        }
        self.advance('"');
        let mut out = String::new();
        loop {
            match self.peek() {
                Some('"') => {
                    self.advance('"');
                    self.drop_ws();
                    return Ok(Value::String(out));
                }
                Some('\\') => {
                    self.advance('\\');
                    let Some(escaped) = self.peek() else {
                        return Err(self.error(
                            "unexpected end of input in string escape",
                            vec!["escape character".to_owned()],
                        ));
                    };
                    self.advance(escaped);
                    let decoded = match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        other => other,
                    };
                    out.push(decoded);
                }
                Some(ch) => {
                    self.advance(ch);
                    out.push(ch);
                }
                None => {
                    return Err(self.error("unterminated string literal", vec!["\"".to_owned()]));
                }
            }
        }
    }

    /// Parses an EDN collection: list `(...)`, vector `[...]`, map `{...}`, or set `#{...}`.
    pub fn parse_collections(&mut self) -> Result<Value, ParseError> {
        let Some(open) = self.peek() else {
            return Err(self.error("unexpected end of input", vec!["collection".to_owned()]));
        };
        let value = match open {
            '#' => {
                let saved = self.pos;
                self.advance('#');
                if self.peek() != Some('{') {
                    self.pos = saved;
                    return Err(self.error("expected set literal", vec!["#{".to_owned()]));
                }
                self.advance('{');
                let items = self.parse_items_until('}')?;
                mk_set(items)
            }
            '{' => {
                self.advance('{');
                let pairs = self.parse_pairs_until('}')?;
                mk_map(pairs)
            }
            '[' => {
                self.advance('[');
                let items = self.parse_items_until(']')?;
                mk_vec(items)
            }
            '(' => {
                self.advance('(');
                let items = self.parse_items_until(')')?;
                mk_list(items)
            }
            other => {
                return Err(self.error(
                    format!("unexpected character {other:?}"),
                    vec![
                        "(".to_owned(),
                        "[".to_owned(),
                        "{".to_owned(),
                        "#{".to_owned(),
                    ],
                ));
            }
        };
        self.drop_ws();
        Ok(value)
    }

    fn skip_discards(&mut self) -> Result<(), ParseError> {
        while self.remaining().starts_with("#_") {
            self.parse_discard()?;
        }
        Ok(())
    }

    fn try_parse_tag_header(&mut self) -> Option<(String, String)> {
        let saved = self.pos;
        if self.peek() != Some('#') {
            return None;
        }
        self.advance('#');
        let first = self.take_tag_ident()?;
        if self.peek() == Some('/') {
            let after_first = self.pos;
            self.advance('/');
            if let Some(second) = self.take_tag_ident() {
                return Some((first, second));
            }
            self.pos = after_first;
        }
        if first.is_empty() {
            self.pos = saved;
            return None;
        }
        Some((String::new(), first))
    }

    fn take_tag_ident(&mut self) -> Option<String> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if is_tag_char(ch) {
                self.advance(ch);
            } else {
                break;
            }
        }
        if self.pos == start {
            None
        } else {
            Some(self.input[start..self.pos].to_owned())
        }
    }

    fn try_parse_reserved_word(&mut self, word: &str) -> bool {
        if !self.remaining().starts_with(word) {
            return false;
        }
        let after = &self.remaining()[word.len()..];
        if let Some(next) = after.chars().next()
            && (is_symbol_char(next) || next == '/')
        {
            return false;
        }
        self.pos += word.len();
        self.drop_ws();
        true
    }

    fn try_parse_nil(&mut self) -> Option<Value> {
        self.try_parse_reserved_word("nil").then_some(Value::Nil)
    }

    fn try_parse_bool(&mut self) -> Option<Value> {
        if self.try_parse_reserved_word("true") {
            Some(Value::Boolean(true))
        } else if self.try_parse_reserved_word("false") {
            Some(Value::Boolean(false))
        } else {
            None
        }
    }

    fn try_parse_number(&mut self) -> Option<Result<Value, ParseError>> {
        if let Some(res) = self.try_parse_floating() {
            return Some(res);
        }
        self.try_parse_integer()
    }

    fn try_parse_floating(&mut self) -> Option<Result<Value, ParseError>> {
        let saved = self.pos;
        if matches!(self.peek(), Some('+' | '-'))
            && let Some(sign) = self.peek()
        {
            self.advance(sign);
        }
        let digits_start = self.pos;
        self.skip_ascii_digits();
        if self.pos == digits_start {
            self.pos = saved;
            return None;
        }

        let mut is_float = false;
        if self.peek() == Some('.') {
            let dot_pos = self.pos;
            self.advance('.');
            let frac_start = self.pos;
            self.skip_ascii_digits();
            if self.pos == frac_start {
                self.pos = dot_pos;
            } else {
                is_float = true;
            }
        }

        if matches!(self.peek(), Some('e' | 'E')) {
            let exp_pos = self.pos;
            if let Some(exp_ch) = self.peek() {
                self.advance(exp_ch);
            }
            if matches!(self.peek(), Some('+' | '-'))
                && let Some(sign) = self.peek()
            {
                self.advance(sign);
            }
            let exp_digits_start = self.pos;
            self.skip_ascii_digits();
            if self.pos == exp_digits_start {
                self.pos = exp_pos;
            } else {
                is_float = true;
            }
        }

        if !is_float {
            self.pos = saved;
            return None;
        }

        let slice = &self.input[saved..self.pos];
        if let Ok(num) = slice.parse::<f64>() {
            self.drop_ws();
            Some(Ok(Value::Floating(Float::new(num))))
        } else {
            self.pos = saved;
            Some(Err(self.error(
                format!("invalid floating-point literal {slice:?}"),
                vec!["floating-point number".to_owned()],
            )))
        }
    }

    fn try_parse_integer(&mut self) -> Option<Result<Value, ParseError>> {
        let saved = self.pos;
        if matches!(self.peek(), Some('+' | '-'))
            && let Some(sign) = self.peek()
        {
            self.advance(sign);
        }
        let digits_start = self.pos;
        self.skip_ascii_digits();
        if self.pos == digits_start {
            self.pos = saved;
            return None;
        }

        let slice = &self.input[saved..self.pos];
        if let Ok(num) = slice.parse::<i64>() {
            self.drop_ws();
            Some(Ok(Value::Integer(num)))
        } else {
            self.pos = saved;
            Some(Err(self.error(
                format!("integer literal out of 64-bit signed range: {slice}"),
                vec!["64-bit integer".to_owned()],
            )))
        }
    }

    fn try_parse_unicode_char(&mut self) -> Option<Result<char, ParseError>> {
        if self.peek() != Some('u') {
            return None;
        }
        let saved = self.pos;
        self.advance('u');
        let hex_start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_hexdigit() {
                self.advance(ch);
            } else {
                break;
            }
        }
        if self.pos == hex_start {
            self.pos = saved;
            return None;
        }
        let hex = &self.input[hex_start..self.pos];
        if let Some(ch) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
            Some(Ok(ch))
        } else {
            self.pos = saved;
            Some(Err(self.error(
                format!("invalid unicode code point \\u{hex}"),
                vec!["hex-encoded unicode character".to_owned()],
            )))
        }
    }

    fn parse_items_until(&mut self, closing: char) -> Result<Vec<TaggedValue>, ParseError> {
        let mut items = Vec::new();
        loop {
            self.drop_ws();
            if self.peek() == Some(closing) {
                self.advance(closing);
                return Ok(items);
            }
            if self.peek().is_none() {
                return Err(self.error(
                    "unexpected end of input in collection",
                    vec![closing.to_string()],
                ));
            }
            items.push(self.parse_tagged()?);
        }
    }

    fn parse_pairs_until(
        &mut self,
        closing: char,
    ) -> Result<Vec<(TaggedValue, TaggedValue)>, ParseError> {
        let mut pairs = Vec::new();
        loop {
            self.drop_ws();
            if self.peek() == Some(closing) {
                self.advance(closing);
                return Ok(pairs);
            }
            if self.peek().is_none() {
                return Err(self.error("unexpected end of input in map", vec![closing.to_string()]));
            }
            let key = self.parse_tagged()?;
            self.drop_ws();
            if self.peek() == Some(closing) {
                return Err(self.error(
                    "map literal contains an odd number of forms",
                    vec!["map value".to_owned()],
                ));
            }
            let val = self.parse_tagged()?;
            pairs.push((key, val));
        }
    }

    fn skip_ascii_digits(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.advance(ch);
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self, ch: char) {
        self.pos += ch.len_utf8();
    }

    fn error(&self, message: impl Into<String>, expected: Vec<String>) -> ParseError {
        let (line, column) = line_and_column(self.input, self.pos);
        ParseError {
            source_name: self.source_name.clone(),
            offset: self.pos,
            line,
            column,
            message: message.into(),
            expected,
        }
    }
}

fn is_printable_char(ch: char) -> bool {
    !ch.is_control() && !matches!(ch, '\u{2028}' | '\u{2029}')
}

fn line_and_column(input: &str, offset: usize) -> (usize, usize) {
    let prefix = &input[..offset.min(input.len())];
    let mut line = 1_usize;
    let mut column = 1_usize;
    for ch in prefix.chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}
