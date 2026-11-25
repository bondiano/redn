//! Wadler-Lindig pretty printer for EDN ASTs.

use std::fmt::Write as _;

use crate::tagged::Tagged;
use crate::value::{TaggedValue, Value};

/// Default target line width used by [`pretty_text`].
pub const DEFAULT_LINE_WIDTH: usize = 80;

/// Target page width configuration for [`LayoutOptions`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageWidth {
    /// Wrap lines to stay within `line_width` columns and `ribbon_fraction` (in `0.0..=1.0`).
    AvailablePerLine {
        /// Maximum target columns per line.
        line_width: usize,
        /// Fraction of `line_width` available for non-indentation content.
        ribbon_fraction: f64,
    },
    /// Never break lines; lay everything out on a single line.
    Unbounded,
}

/// Layout options controlling [`pretty_text_with_options`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutOptions {
    /// Page width policy.
    pub page_width: PageWidth,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            page_width: PageWidth::AvailablePerLine {
                line_width: DEFAULT_LINE_WIDTH,
                ribbon_fraction: 1.0,
            },
        }
    }
}

/// A Wadler-Lindig pretty-printing document node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doc {
    /// Empty document.
    Empty,
    /// Literal text containing no newlines.
    Text(String),
    /// A soft break: renders as a single space when flattened, or newline + indent when broken.
    Line,
    /// Sequence of child documents.
    Concat(Vec<Doc>),
    /// Indents nested line breaks by the current output column.
    Align(Box<Doc>),
    /// Renders flat if the whole group fits on the current line; otherwise breaks its [`Doc::Line`]s.
    Group(Box<Doc>),
}

impl Doc {
    /// Constructs a text leaf.
    pub fn text(s: impl Into<String>) -> Self {
        let s = s.into();
        if s.is_empty() {
            Self::Empty
        } else {
            Self::Text(s)
        }
    }

    /// Concatenates documents into a sequence.
    pub fn concat(parts: Vec<Self>) -> Self {
        if parts.is_empty() {
            Self::Empty
        } else {
            Self::Concat(parts)
        }
    }

    /// Joins two documents with a literal space.
    pub fn space_join(left: Self, right: Self) -> Self {
        Self::Concat(vec![left, Self::text(" "), right])
    }

    /// Wraps a document so its line breaks align to the column where it begins.
    pub fn align(inner: Self) -> Self {
        Self::Align(Box::new(inner))
    }

    /// Marks a document as a flat-or-broken layout group.
    pub fn group(inner: Self) -> Self {
        Self::Group(Box::new(inner))
    }

    /// Separates items by spaces if they fit on one line, or newlines otherwise.
    pub fn sep(items: Vec<Self>) -> Self {
        if items.is_empty() {
            return Self::Empty;
        }
        let mut parts = Vec::with_capacity(items.len() * 2 - 1);
        for (idx, item) in items.into_iter().enumerate() {
            if idx > 0 {
                parts.push(Self::Line);
            }
            parts.push(item);
        }
        Self::group(Self::Concat(parts))
    }

    /// Renders this document according to `options`.
    pub fn render(&self, options: LayoutOptions) -> String {
        layout_pretty(self, options)
    }
}

/// Renders an EDN [`TaggedValue`] onto a single unbounded line.
///
/// # Examples
///
/// ```
/// use redn_ast::{parse_text, render_text};
///
/// let ast = parse_text("ex", "( 1,  2   3 )")?;
/// assert_eq!(render_text(&ast), "(1 2 3)");
/// # Ok::<(), redn_ast::ParseError>(())
/// ```
pub fn render_text(value: &TaggedValue) -> String {
    pretty_text_with_options(
        value,
        LayoutOptions {
            page_width: PageWidth::Unbounded,
        },
    )
}

/// Renders an EDN [`TaggedValue`] with default 80-column pretty-printing layout.
///
/// Collections stay on a single line when they fit within [`DEFAULT_LINE_WIDTH`]
/// columns and wrap with column alignment otherwise.
pub fn pretty_text(value: &TaggedValue) -> String {
    pretty_text_with_options(value, LayoutOptions::default())
}

/// Renders an EDN [`TaggedValue`] with custom [`LayoutOptions`].
pub fn pretty_text_with_options(value: &TaggedValue, options: LayoutOptions) -> String {
    pretty_tagged_value(value).render(options)
}

/// Converts a [`TaggedValue`] into a pretty-printable [`Doc`].
pub fn pretty_tagged_value(tv: &TaggedValue) -> Doc {
    match tv {
        Tagged::Tagged {
            namespace,
            name,
            value,
        } if namespace.is_empty() => {
            Doc::space_join(Doc::text(format!("#{name}")), pretty_value(value))
        }
        Tagged::Tagged {
            namespace,
            name,
            value,
        } => Doc::space_join(
            Doc::text(format!("#{namespace}/{name}")),
            pretty_value(value),
        ),
        Tagged::NoTag(value) => pretty_value(value),
    }
}

/// Converts an untagged [`Value`] into a pretty-printable [`Doc`].
pub fn pretty_value(value: &Value) -> Doc {
    match value {
        Value::Nil => Doc::text("nil"),
        Value::Boolean(b) => Doc::text(if *b { "true" } else { "false" }),
        Value::Character(ch) => Doc::text(format_character(*ch)),
        Value::Symbol { namespace, name } if namespace.is_empty() => Doc::text(name.clone()),
        Value::Symbol { namespace, name } => Doc::text(format!("{namespace}/{name}")),
        Value::Keyword(ident) => Doc::text(format!(":{ident}")),
        Value::Integer(int) => Doc::text(int.to_string()),
        Value::Floating(float) => Doc::text(float.to_string()),
        Value::String(text) => Doc::text(format!("\"{}\"", escape_text(text))),
        Value::List(items) => {
            enclose_sep("(", ")", items.iter().map(pretty_tagged_value).collect())
        }
        Value::Vec(items) => enclose_sep("[", "]", items.iter().map(pretty_tagged_value).collect()),
        Value::Set(items) => {
            enclose_sep("#{", "}", items.iter().map(pretty_tagged_value).collect())
        }
        Value::Map(pairs) => {
            let entries = pairs
                .iter()
                .map(|(k, v)| Doc::space_join(pretty_tagged_value(k), pretty_tagged_value(v)))
                .collect();
            enclose_sep("{", "}", entries)
        }
    }
}

/// Escapes special characters inside an EDN string literal body.
pub fn escape_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out
}

fn enclose_sep(open: &str, close: &str, items: Vec<Doc>) -> Doc {
    Doc::concat(vec![
        Doc::text(open),
        Doc::align(Doc::sep(items)),
        Doc::text(close),
    ])
}

fn format_character(ch: char) -> String {
    match ch {
        '\n' => "\\newline".to_owned(),
        '\t' => "\\tab".to_owned(),
        '\r' => "\\return".to_owned(),
        ' ' => "\\space".to_owned(),
        other if other.is_ascii_graphic() && !matches!(other, '\\' | '\'') => {
            format!("\\{other}")
        }
        other => {
            let mut out = String::with_capacity(6);
            let _ = write!(out, "\\u{:04X}", u32::from(other));
            out
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

fn layout_pretty(doc: &Doc, options: LayoutOptions) -> String {
    let mut out = String::new();
    let mut col = 0_usize;
    let initial_mode = match options.page_width {
        PageWidth::Unbounded => Mode::Flat,
        PageWidth::AvailablePerLine { .. } => Mode::Break,
    };
    let mut stack: Vec<(usize, Mode, &Doc)> = vec![(0, initial_mode, doc)];

    while let Some((indent, mode, node)) = stack.pop() {
        match node {
            Doc::Empty => {}
            Doc::Text(s) => {
                out.push_str(s);
                col += s.chars().count();
            }
            Doc::Line => match mode {
                Mode::Flat => {
                    out.push(' ');
                    col += 1;
                }
                Mode::Break => {
                    out.push('\n');
                    for _ in 0..indent {
                        out.push(' ');
                    }
                    col = indent;
                }
            },
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    stack.push((indent, mode, part));
                }
            }
            Doc::Align(inner) => {
                stack.push((col, mode, inner));
            }
            Doc::Group(inner) => {
                let chosen = match (mode, options.page_width) {
                    (Mode::Flat, _) | (_, PageWidth::Unbounded) => Mode::Flat,
                    (
                        Mode::Break,
                        PageWidth::AvailablePerLine {
                            line_width,
                            ribbon_fraction,
                        },
                    ) => {
                        let rem = available_width(line_width, ribbon_fraction, col, indent);
                        if fits(rem, col, &stack, (indent, Mode::Flat, inner)) {
                            Mode::Flat
                        } else {
                            Mode::Break
                        }
                    }
                };
                stack.push((indent, chosen, inner));
            }
        }
    }

    out
}

fn available_width(line_width: usize, ribbon_fraction: f64, col: usize, indent: usize) -> isize {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "line_width is a screen column count where f64 rounding is exact"
    )]
    let ribbon_width = (line_width as f64 * ribbon_fraction.clamp(0.0, 1.0)).round() as usize;
    let columns_left = line_width.saturating_sub(col);
    let current_ribbon = col.saturating_sub(indent);
    let ribbon_left = ribbon_width.saturating_sub(current_ribbon);
    let remaining = columns_left.min(ribbon_left);
    isize::try_from(remaining).unwrap_or(isize::MAX)
}

fn fits(
    mut remaining: isize,
    mut col: usize,
    rest_stack: &[(usize, Mode, &Doc)],
    first: (usize, Mode, &Doc),
) -> bool {
    let mut local_stack = vec![first];
    let mut rest_idx = rest_stack.len();

    while remaining >= 0 {
        let next = if let Some(item) = local_stack.pop() {
            item
        } else if rest_idx > 0 {
            rest_idx -= 1;
            rest_stack[rest_idx]
        } else {
            return true;
        };

        let (indent, mode, node) = next;
        match node {
            Doc::Empty => {}
            Doc::Text(s) => {
                let len = s.chars().count();
                let len_signed = isize::try_from(len).unwrap_or(isize::MAX);
                remaining -= len_signed;
                col += len;
                if remaining < 0 {
                    return false;
                }
            }
            Doc::Line => match mode {
                Mode::Flat => {
                    remaining -= 1;
                    col += 1;
                    if remaining < 0 {
                        return false;
                    }
                }
                Mode::Break => return true,
            },
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    local_stack.push((indent, mode, part));
                }
            }
            Doc::Align(inner) => {
                local_stack.push((col, mode, inner));
            }
            Doc::Group(inner) => {
                local_stack.push((indent, mode, inner));
            }
        }
    }

    false
}
