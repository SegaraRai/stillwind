//! A byte-level CSS tokenizer that follows CSS Syntax Level 3.
//!
//! [`CssLexer`] tracks only the token boundaries that decide where names, strings,
//! comments, and URLs start and end. It does not build tokens: a [`CssLexerHandler`]
//! observes each state change and the end of input, and keeps what it needs. Use it
//! for scans over large CSS text where building cssparser tokens costs more than the
//! scan needs.
//!
//! The tokenizer works on bytes because every token boundary that matters is an ASCII
//! byte. Bytes of non-ASCII characters are name code points, so a name never ends
//! inside a character.

/// Tokenizer state of [`CssLexer`]. Offsets are UTF-8 byte offsets of the input.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CssLexerState {
    /// Between tokens, or inside a token that cannot contain a name, such as a
    /// delimiter.
    #[default]
    Data,
    /// Inside a comment, after `/*`.
    Comment,
    /// Inside a string that `quote` opened.
    String { quote: u8 },
    /// Inside an unquoted URL or the remnants of a bad URL, after `url(` and
    /// whitespace.
    Url,
    /// Inside the numeric part of a number, percentage, or dimension.
    Number,
    /// Inside a name that starts at `start`, with escapes in their authored spelling.
    Name { start: usize, kind: NameKind },
}

/// Token that a name belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NameKind {
    /// An ident token, or the name of a function token when `(` follows the name.
    Ident,
    /// A hash token without its `#`.
    Hash,
    /// An at-keyword without its `@`.
    AtKeyword,
    /// The unit of a dimension token.
    Unit,
}

/// Observes the state changes of a [`CssLexer`].
pub(crate) trait CssLexerHandler {
    /// Runs when the state changes from `from` to `to` at byte `offset` of `css`.
    ///
    /// A token that leaves `from` ends at `offset`, so `from` describes the token that
    /// just ended. `from` and `to` always differ.
    fn on_transition(&mut self, css: &str, offset: usize, from: CssLexerState, to: CssLexerState);

    /// Runs once after the last byte of `css`, with the state that the end of input
    /// closes.
    fn on_end(&mut self, css: &str, state: CssLexerState);
}

/// A CSS tokenizer that reports state changes to `handler`.
#[derive(Debug)]
pub(crate) struct CssLexer<H> {
    /// Current tokenizer state.
    state: CssLexerState,
    /// Observer of state changes.
    handler: H,
}

impl<H: CssLexerHandler> CssLexer<H> {
    /// Creates a lexer in the [`CssLexerState::Data`] state.
    pub(crate) fn new(handler: H) -> Self {
        Self {
            state: CssLexerState::Data,
            handler,
        }
    }

    /// Tokenizes all of `css` and returns the handler after its
    /// [`CssLexerHandler::on_end`] call.
    pub(crate) fn run(mut self, css: &str) -> H {
        let mut offset = 0;
        while offset < css.len() {
            offset += self.step(css, offset);
        }
        self.handler.on_end(css, self.state);
        self.handler
    }

    /// Changes the state to `state` at `offset` and reports a change to the handler.
    fn enter(&mut self, css: &str, offset: usize, state: CssLexerState) {
        let from = self.state;
        if from != state {
            self.state = state;
            self.handler.on_transition(css, offset, from, state);
        }
    }

    /// Consumes input at `offset`, which must be less than `css.len()`, and returns the
    /// number of bytes consumed.
    ///
    /// A return value of zero means that the state changed and the byte at `offset`
    /// must be read again. Every state that returns zero changes to a state that
    /// consumes the byte, so the scan always advances.
    fn step(&mut self, css: &str, offset: usize) -> usize {
        let bytes = css.as_bytes();
        let at = |index: usize| bytes.get(index).copied();
        let byte = bytes[offset];
        match self.state {
            CssLexerState::Data => self.data(css, offset),
            CssLexerState::Comment => {
                if byte == b'*' && at(offset + 1) == Some(b'/') {
                    self.enter(css, offset + 2, CssLexerState::Data);
                    2
                } else {
                    1
                }
            }
            CssLexerState::String { quote } => match byte {
                b'\\' => match (at(offset + 1), at(offset + 2)) {
                    (Some(b'\r'), Some(b'\n')) => 3,
                    (Some(next), _) if next.is_ascii_hexdigit() => escape_len(bytes, offset),
                    (Some(_), _) => 2,
                    (None, _) => 1,
                },
                // An unescaped newline ends a bad string and is read again as
                // whitespace.
                b'\n' | b'\r' | 0x0c => {
                    self.enter(css, offset, CssLexerState::Data);
                    0
                }
                _ => {
                    if byte == quote {
                        self.enter(css, offset + 1, CssLexerState::Data);
                    }
                    1
                }
            },
            CssLexerState::Url => match byte {
                b')' => {
                    self.enter(css, offset + 1, CssLexerState::Data);
                    1
                }
                b'\\' if valid_escape(bytes, offset) => 2,
                _ => 1,
            },
            CssLexerState::Number => {
                if byte.is_ascii_digit()
                    || byte == b'.' && at(offset + 1).is_some_and(|next| next.is_ascii_digit())
                {
                    1
                } else if byte == b'%' {
                    self.enter(css, offset + 1, CssLexerState::Data);
                    1
                } else {
                    let state = if starts_ident(bytes, offset) {
                        CssLexerState::Name {
                            start: offset,
                            kind: NameKind::Unit,
                        }
                    } else {
                        CssLexerState::Data
                    };
                    self.enter(css, offset, state);
                    0
                }
            }
            CssLexerState::Name { start, kind } => {
                if is_name(byte) {
                    1
                } else if byte == b'\\' && valid_escape(bytes, offset) {
                    match at(offset + 1) {
                        Some(next) if next.is_ascii_hexdigit() => escape_len(bytes, offset),
                        // Bytes after the first one of a non-ASCII character are name
                        // bytes.
                        Some(_) => 2,
                        None => 1,
                    }
                } else if kind == NameKind::Ident && byte == b'(' && is_url(&css[start..offset]) {
                    self.url_open(css, offset)
                } else {
                    self.enter(css, offset, CssLexerState::Data);
                    0
                }
            }
        }
    }

    /// Starts the token at `offset` and returns the number of bytes consumed.
    fn data(&mut self, css: &str, offset: usize) -> usize {
        let bytes = css.as_bytes();
        let at = |index: usize| bytes.get(index).copied();
        match bytes[offset] {
            b'/' if at(offset + 1) == Some(b'*') => {
                self.enter(css, offset, CssLexerState::Comment);
                2
            }
            quote @ (b'"' | b'\'') => {
                self.enter(css, offset, CssLexerState::String { quote });
                1
            }
            b'#' if at(offset + 1).is_some_and(is_name) || valid_escape(bytes, offset + 1) => {
                let state = CssLexerState::Name {
                    start: offset + 1,
                    kind: NameKind::Hash,
                };
                self.enter(css, offset, state);
                1
            }
            b'@' if starts_ident(bytes, offset + 1) => {
                let state = CssLexerState::Name {
                    start: offset + 1,
                    kind: NameKind::AtKeyword,
                };
                self.enter(css, offset, state);
                1
            }
            b'<' if bytes[offset + 1..].starts_with(b"!--") => 4,
            _ if starts_number(bytes, offset) => {
                self.enter(css, offset, CssLexerState::Number);
                1
            }
            b'-' if bytes[offset + 1..].starts_with(b"->") => 3,
            _ if starts_ident(bytes, offset) => {
                let state = CssLexerState::Name {
                    start: offset,
                    kind: NameKind::Ident,
                };
                self.enter(css, offset, state);
                0
            }
            _ => 1,
        }
    }

    /// Reads the `(` of a `url` function at `offset` and the whitespace after it, and
    /// returns the number of bytes consumed. A quoted argument makes it an ordinary
    /// function, and any other argument starts an unquoted URL.
    fn url_open(&mut self, css: &str, offset: usize) -> usize {
        let bytes = css.as_bytes();
        let whitespace = bytes[offset + 1..]
            .iter()
            .take_while(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c))
            .count();
        if matches!(bytes.get(offset + 1 + whitespace), Some(b'"' | b'\'')) {
            self.enter(css, offset, CssLexerState::Data);
            1
        } else {
            self.enter(css, offset, CssLexerState::Url);
            1 + whitespace
        }
    }
}

/// Returns the length of the hexadecimal escape whose backslash is at `offset`: the
/// backslash, up to six digits, and one whitespace after them, where CRLF is one
/// whitespace.
fn escape_len(bytes: &[u8], offset: usize) -> usize {
    let digits = bytes[offset + 1..]
        .iter()
        .take(6)
        .take_while(|byte| byte.is_ascii_hexdigit())
        .count();
    let end = offset + 1 + digits;
    1 + digits
        + match (bytes.get(end), bytes.get(end + 1)) {
            (Some(b'\r'), Some(b'\n')) => 2,
            (Some(b' ' | b'\t' | b'\n' | b'\r' | 0x0c), _) => 1,
            _ => 0,
        }
}

/// Whether the authored name `name` decodes to `url`, ignoring ASCII case.
fn is_url(name: &str) -> bool {
    if !name.contains('\\') {
        return name.eq_ignore_ascii_case("url");
    }
    let mut decoded = String::new();
    let mut chars = name.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6
            && let Some(digit) = chars.next_if(char::is_ascii_hexdigit)
        {
            hex.push(digit);
        }
        if hex.is_empty() {
            decoded.push(chars.next().unwrap_or('\u{fffd}'));
        } else {
            chars.next_if(|next| matches!(next, ' ' | '\t' | '\n' | '\r' | '\u{c}'));
            decoded.push(
                u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .unwrap_or('\u{fffd}'),
            );
        }
        if decoded.len() > 3 {
            return false;
        }
    }
    decoded.eq_ignore_ascii_case("url")
}

/// Whether `byte` can start a name: an ASCII letter, `_`, or a byte of a non-ASCII
/// character.
fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80
}

/// Whether `byte` can continue a name.
fn is_name(byte: u8) -> bool {
    is_name_start(byte) || byte.is_ascii_digit() || byte == b'-'
}

/// Whether a backslash at `offset` starts an escape: it is followed by a byte that is
/// not a newline, or by the end of input.
fn valid_escape(bytes: &[u8], offset: usize) -> bool {
    bytes.get(offset) == Some(&b'\\')
        && !matches!(bytes.get(offset + 1), Some(b'\n' | b'\r' | 0x0c))
}

/// Whether the bytes at `offset` start an ident sequence.
fn starts_ident(bytes: &[u8], offset: usize) -> bool {
    match bytes.get(offset) {
        Some(b'-') => {
            bytes
                .get(offset + 1)
                .is_some_and(|&next| is_name_start(next) || next == b'-')
                || valid_escape(bytes, offset + 1)
        }
        Some(b'\\') => valid_escape(bytes, offset),
        Some(&byte) => is_name_start(byte),
        None => false,
    }
}

/// Whether the bytes at `offset` start a number.
fn starts_number(bytes: &[u8], offset: usize) -> bool {
    let digit = |index: usize| bytes.get(index).is_some_and(u8::is_ascii_digit);
    match bytes.get(offset) {
        Some(b'+' | b'-') => {
            digit(offset + 1) || bytes.get(offset + 1) == Some(&b'.') && digit(offset + 2)
        }
        Some(b'.') => digit(offset + 1),
        Some(byte) => byte.is_ascii_digit(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records each token that leaves a non-data state, with its kind and text.
    #[derive(Default)]
    struct Tokens(Vec<(&'static str, String)>);

    impl Tokens {
        fn push(&mut self, css: &str, start: usize, end: usize, state: CssLexerState) {
            let (label, start) = match state {
                CssLexerState::Data => return,
                CssLexerState::Comment => ("comment", start),
                CssLexerState::String { .. } => ("string", start),
                CssLexerState::Url => ("url", start),
                CssLexerState::Number => ("number", start),
                CssLexerState::Name { start, kind } => (
                    match kind {
                        NameKind::Ident => "ident",
                        NameKind::Hash => "hash",
                        NameKind::AtKeyword => "at",
                        NameKind::Unit => "unit",
                    },
                    start,
                ),
            };
            self.0.push((label, css[start..end].to_owned()));
        }
    }

    /// Tracks where the current non-data state started.
    #[derive(Default)]
    struct Recorder {
        start: usize,
        tokens: Tokens,
    }

    impl CssLexerHandler for Recorder {
        fn on_transition(
            &mut self,
            css: &str,
            offset: usize,
            from: CssLexerState,
            _to: CssLexerState,
        ) {
            self.tokens.push(css, self.start, offset, from);
            self.start = offset;
        }

        fn on_end(&mut self, css: &str, state: CssLexerState) {
            self.tokens.push(css, self.start, css.len(), state);
        }
    }

    fn tokens(css: &str) -> Vec<(&'static str, String)> {
        CssLexer::new(Recorder::default()).run(css).tokens.0
    }

    #[test]
    fn transitions_report_each_token_that_ends() {
        assert_eq!(
            tokens(r#"a{b:#c 1.5em @d url( e ) "f\"g" /* h */ 2%}"#),
            [
                ("ident", "a"),
                ("ident", "b"),
                ("hash", "c"),
                ("number", "1.5"),
                ("unit", "em"),
                ("at", "d"),
                ("ident", "url"),
                ("url", "( e )"),
                ("string", r#""f\"g""#),
                ("comment", "/* h */"),
                ("number", "2%"),
            ]
            .map(|(label, text)| (label, text.to_owned()))
        );
    }

    #[test]
    fn the_end_of_input_closes_the_last_token() {
        assert_eq!(tokens("--x"), [("ident", "--x".to_owned())]);
        assert_eq!(tokens("/* open"), [("comment", "/* open".to_owned())]);
        assert_eq!(tokens(""), []);
    }
}
