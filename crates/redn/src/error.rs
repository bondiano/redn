//! Decoding and top-level error types for EDN processing.
//!
//! # Examples
//!
//! [`alt`] merges expected labels across fallback branches:
//!
//! ```
//! use redn::{Value, alt, with_floating, with_integer};
//!
//! let err = alt(with_integer(&Value::Nil, |n| Ok(n as f64)), || {
//!     with_floating(&Value::Nil, Ok)
//! })
//! .expect_err("nil is not a number");
//!
//! assert_eq!(err.expected(), &["integer", "double"]);
//! assert_eq!(
//!     err.to_string(),
//!     "Unexpected nil\nexpected\n  integer, double",
//! );
//! ```

use std::error::Error as StdError;
use std::fmt;

use redn_ast::{ParseError, Value};

/// Result alias returned by [`FromEdn`](crate::FromEdn) and AST decoder combinators.
pub type DecodeResult<T> = Result<T, DecodeError>;

/// Error returned while decoding an EDN AST into a Rust value.
///
/// Carries a primary error message along with expected labels accumulated across
/// [`alt`] branches.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct DecodeError {
    message: String,
    expected: Vec<String>,
}

impl DecodeError {
    /// Creates a decoding error with no expected-label list.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            expected: Vec::new(),
        }
    }

    /// Creates a decoding error reporting an unexpected AST node kind and one expected label.
    pub fn unexpected(value: &Value, expected_label: impl Into<String>) -> Self {
        Self {
            message: format!("Unexpected {}", value.kind_name()),
            expected: vec![expected_label.into()],
        }
    }

    /// Creates a decoding error reporting an unexpected AST node kind and multiple expected labels.
    pub fn unexpected_any<I, S>(value: &Value, expected_labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            message: format!("Unexpected {}", value.kind_name()),
            expected: expected_labels.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the primary failure message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the accumulated expected labels, if any.
    pub fn expected(&self) -> &[String] {
        &self.expected
    }

    /// Combines two failures from an alternative branch, preserving both expected label lists
    /// and keeping the second branch's error message.
    pub fn combine(mut self, other: Self) -> Self {
        self.expected.extend(other.expected);
        Self {
            message: other.message,
            expected: self.expected,
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.expected.is_empty() {
            f.write_str(&self.message)
        } else {
            writeln!(f, "{}", self.message)?;
            writeln!(f, "expected")?;
            write!(f, "  {}", self.expected.join(", "))
        }
    }
}

impl StdError for DecodeError {}

/// Tries `first`, falling back to `second` and merging expected labels on failure.
pub fn alt<T>(first: DecodeResult<T>, second: impl FnOnce() -> DecodeResult<T>) -> DecodeResult<T> {
    match first {
        Ok(val) => Ok(val),
        Err(first_err) => match second() {
            Ok(val) => Ok(val),
            Err(second_err) => Err(first_err.combine(second_err)),
        },
    }
}

/// Top-level error returned by [`decode_text`](crate::decode_text).
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub enum Error {
    /// Syntax error while parsing EDN text into an AST.
    Parse(ParseError),
    /// Type or schema error while decoding an AST into a Rust value.
    Decode(DecodeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "{err}"),
            Self::Decode(err) => write!(f, "{err}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Parse(err) => Some(err),
            Self::Decode(err) => Some(err),
        }
    }
}

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Self {
        Self::Parse(err)
    }
}

impl From<DecodeError> for Error {
    fn from(err: DecodeError) -> Self {
        Self::Decode(err)
    }
}
