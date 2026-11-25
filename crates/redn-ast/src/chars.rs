//! Character classes used by the EDN lexer and parser.

/// ASCII decimal digits `0..=9`.
pub const DIGIT_CHARS: &str = "0123456789";

/// ASCII uppercase letters `A..=Z`.
pub const UPPER_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// ASCII lowercase letters `a..=z`.
pub const LOWER_CHARS: &str = "abcdefghijklmnopqrstuvwxyz";

/// Punctuation allowed at the start of EDN symbols and keywords.
pub const MISC_CHARS: &str = ".*<>!?$%&=+_-";

/// Characters allowed in `#tag` and `#ns/tag` identifiers.
pub const TAG_CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-";

/// Characters allowed as the first character of a keyword (after `:`).
pub const KEYWORD_INITIAL_CHARS: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.*<>!?$%&=+_-";

/// Characters allowed after the initial character of a keyword.
pub const KEYWORD_CHARS: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.*<>!?$%&=+_-0123456789/#:";

/// Characters allowed as the first character of a symbol namespace or name.
pub const SYMBOL_INITIAL_CHARS: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.*<>!?$%&=+_-";

/// Characters allowed after the initial character of a symbol namespace or name.
pub const SYMBOL_CHARS: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.*<>!?$%&=+_-0123456789#:";

/// Returns `true` if `ch` is in [`MISC_CHARS`].
pub const fn is_misc_char(ch: char) -> bool {
    matches!(
        ch,
        '.' | '*' | '<' | '>' | '!' | '?' | '$' | '%' | '&' | '=' | '+' | '_' | '-'
    )
}

/// Returns `true` if `ch` is allowed in a tag identifier.
pub const fn is_tag_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-'
}

/// Returns `true` if `ch` may start a keyword name (after `:`).
pub const fn is_keyword_initial_char(ch: char) -> bool {
    ch.is_ascii_alphabetic() || is_misc_char(ch)
}

/// Returns `true` if `ch` may appear inside a keyword name.
pub const fn is_keyword_char(ch: char) -> bool {
    is_keyword_initial_char(ch) || ch.is_ascii_digit() || matches!(ch, '/' | '#' | ':')
}

/// Returns `true` if `ch` may start a symbol namespace or name.
pub const fn is_symbol_initial_char(ch: char) -> bool {
    ch.is_ascii_alphabetic() || is_misc_char(ch)
}

/// Returns `true` if `ch` may appear inside a symbol namespace or name.
pub const fn is_symbol_char(ch: char) -> bool {
    is_symbol_initial_char(ch) || ch.is_ascii_digit() || matches!(ch, '#' | ':')
}
