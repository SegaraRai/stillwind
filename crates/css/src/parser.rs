//! Shared CSS token traversal, explicit block boundaries, and syntax validation.
//!
//! `parse_css_block` records whether cssparser consumed a real closing delimiter. The
//! complete validator rejects implicit EOF closure; the recovering validator retains
//! CSS's EOF recovery while still rejecting malformed tokens and mismatched delimiters.

use std::ops::Range;

use cssparser::{BasicParseErrorKind, ParseError, ParseErrorKind, Parser, Token};
use lean_string::LeanStr;

/// Reason a delimiter lookup cannot return a closing offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScanError {
    /// The offset is outside the input or splits a UTF-8 scalar.
    #[error("scan offset {offset} is not a character boundary within {length} bytes")]
    InvalidOffset {
        /// Requested byte offset.
        offset: usize,
        /// Input length in bytes.
        length: usize,
    },
    /// The opener is absent, the block is unclosed, or its contents are malformed.
    #[error("delimiter has no matching closer")]
    Unclosed,
}

/// A malformed CSS token or block, positioned in the supplied input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{kind} at byte {offset}")]
pub struct CssSyntaxError {
    /// Byte offset of the malformed token or opening delimiter.
    pub offset: usize,
    /// Structural reason for rejecting the input.
    pub kind: CssSyntaxErrorKind,
}

/// Structural failures shared by CSS fragments and complete definitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CssSyntaxErrorKind {
    /// A token contains a malformed string or URL.
    #[error("invalid CSS token")]
    InvalidToken,
    /// A closing delimiter does not close the current block.
    #[error("unexpected closing delimiter `{0}`")]
    UnexpectedCloser(char),
    /// A block or literal reaches EOF before its closing delimiter.
    #[error("expected closing delimiter `{0}`")]
    Unclosed(char),
    /// The parser cannot safely descend further.
    #[error("CSS nesting limit exceeded")]
    NestingLimit,
}

/// Source coordinates of a block whose opening token was already consumed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssBlockRange {
    /// Bytes between the delimiters, ending at EOF for an unclosed block.
    pub contents: Range<usize>,
    /// Byte offset of a real closing delimiter.
    pub close: Option<usize>,
}

/// Reads an opened block with `read`, which must consume its contents to the end. Every
/// coordinate refers to the same input as parser. No closing offset is invented when
/// EOF closes a block.
///
/// # Errors
///
/// Returns a syntax error when the nesting limit is reached or read leaves tokens.
///
/// # Panics
///
/// Panics unless the parser last returned a function or an opening block token.
pub fn parse_css_block<T>(
    parser: &mut Parser<'_>,
    read: impl FnOnce(&mut Parser<'_>) -> T,
) -> Result<(T, CssBlockRange), CssSyntaxError> {
    let start = parser.position().byte_index();
    let mut end = start;
    let result = parser.parse_nested_block(|inner| {
        let value = read(inner);
        end = inner.position().byte_index();
        Ok::<_, ParseError<()>>(value)
    });
    result
        .map(|value| {
            let close = (parser.position().byte_index() > end).then_some(end);
            (
                value,
                CssBlockRange {
                    contents: start..end,
                    close,
                },
            )
        })
        .map_err(|error| CssSyntaxError {
            offset: start.saturating_sub(1),
            kind: if matches!(
                error.kind,
                ParseErrorKind::Basic(BasicParseErrorKind::TooManyNestedBlocks)
            ) {
                CssSyntaxErrorKind::NestingLimit
            } else {
                CssSyntaxErrorKind::InvalidToken
            },
        })
}

/// Consumes the rest of a component list, allowing cssparser to skip nested blocks.
pub(crate) fn consume(parser: &mut Parser<'_>) {
    while parser.next_including_whitespace_and_comments().is_ok() {}
}

/// Consumes the block whose opening token the parser just returned.
///
/// # Errors
///
/// Returns an error when the parser cannot enter the block.
///
/// # Panics
///
/// Panics unless the parser last returned a function or an opening block token.
pub fn consume_css_block(parser: &mut Parser<'_>) -> Result<CssBlockRange, CssSyntaxError> {
    parse_css_block(parser, consume).map(|((), block)| block)
}

/// Returns the closing character for a CSS opening token.
pub(crate) fn closer(token: &Token<'_>) -> Option<char> {
    match token {
        Token::Function(_) | Token::ParenthesisBlock => Some(')'),
        Token::SquareBracketBlock => Some(']'),
        Token::CurlyBracketBlock => Some('}'),
        _ => None,
    }
}

/// Decodes CSS escapes in raw name text while preserving unescaped punctuation. Returns
/// `None` for a dangling escape or an escaped newline. The spelling may contain
/// candidate punctuation, so it need not form one CSS identifier.
pub fn decode_css_escapes(raw: &str) -> Option<LeanStr> {
    if !raw.contains('\\') {
        return Some(raw.into());
    }
    let mut output = String::with_capacity(raw.len());
    let mut cursor = 0;
    while let Some(relative) = raw[cursor..].find('\\') {
        let start = cursor + relative;
        output.push_str(&raw[cursor..start]);
        if &raw[start..] == "\\" {
            return None;
        }
        // The prefix makes the escaped text an identifier and prevents url() tokens.
        let input = format!("x{}", &raw[start..]);
        let mut parser = Parser::new(&input);
        let function = match parser.next().ok()? {
            Token::Ident(name) => {
                output.push_str(name.strip_prefix('x')?);
                false
            }
            Token::Function(name) => {
                output.push_str(name.strip_prefix('x')?);
                true
            }
            _ => return None,
        };
        let consumed = parser.position().byte_index().checked_sub(1)?;
        if consumed == 0 {
            return None;
        }
        if function {
            output.push('(');
        }
        cursor = start + consumed;
    }
    output.push_str(&raw[cursor..]);
    Some(output.into())
}

/// Returns whether `name` can stand before the `:` of a declaration. `name` is either a
/// CSS identifier or a custom property, written as CSS source text that may contain
/// escapes. The check accepts unknown properties, and it rejects `--`, which CSS
/// reserves.
#[must_use]
pub fn is_css_property_name(name: &str) -> bool {
    if name == "--" || decode_css_escapes(name).is_none() {
        return false;
    }
    let mut parser = Parser::new(name);
    matches!(
        parser.next_including_whitespace_and_comments(),
        Ok(Token::Ident(_))
    ) && parser.next_including_whitespace_and_comments().is_err()
}

/// Checks all component values and requires explicit closure of blocks and literals.
///
/// Unknown functions and custom-property values are accepted. This checks CSS syntax,
/// not a property's value grammar.
///
/// # Errors
///
/// Returns the first malformed token, unclosed block or literal, or nesting failure.
pub fn validate_css_syntax(css: &str) -> Result<(), CssSyntaxError> {
    inspect(&mut Parser::new(css), css, true)
}

/// Checks tokens and delimiter kinds while accepting CSS's implicit closure at EOF.
///
/// # Errors
///
/// Returns the first malformed token, mismatched delimiter, or nesting failure.
pub fn validate_css_syntax_recovering(css: &str) -> Result<(), CssSyntaxError> {
    inspect(&mut Parser::new(css), css, false)
}

/// Checks tokens at the current nesting level and recursively checks block contents.
/// `css` must be the complete input of `parser`, so token offsets index the same text.
/// `complete` requires authored closing delimiters, including literals closed by EOF.
/// Returns the first syntax error and consumes the rest of each enclosing block.
fn inspect(parser: &mut Parser<'_>, css: &str, complete: bool) -> Result<(), CssSyntaxError> {
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            return Ok(());
        };
        let end = parser.position().byte_index();
        let error = match &token {
            Token::BadString(_) | Token::BadUrl(_) => Some(CssSyntaxErrorKind::InvalidToken),
            Token::CloseParenthesis => Some(CssSyntaxErrorKind::UnexpectedCloser(')')),
            Token::CloseSquareBracket => Some(CssSyntaxErrorKind::UnexpectedCloser(']')),
            Token::CloseCurlyBracket => Some(CssSyntaxErrorKind::UnexpectedCloser('}')),
            Token::QuotedString(_)
                if complete && !literal_closed(&css[start..end], &css[start..=start]) =>
            {
                Some(CssSyntaxErrorKind::Unclosed(css.as_bytes()[start] as char))
            }
            Token::UnquotedUrl(_) if complete && !literal_closed(&css[start..end], ")") => {
                Some(CssSyntaxErrorKind::Unclosed(')'))
            }
            Token::Comment(_) if complete && !literal_closed(&css[start..end], "*/") => {
                Some(CssSyntaxErrorKind::Unclosed('/'))
            }
            _ => None,
        };
        let closing = closer(&token);
        if let Some(kind) = error {
            return Err(CssSyntaxError {
                offset: start,
                kind,
            });
        }
        if let Some(closing) = closing {
            let (result, block) = parse_css_block(parser, |inner| {
                let result = inspect(inner, css, complete);
                consume(inner);
                result
            })?;
            result?;
            if complete && block.close.is_none() {
                return Err(CssSyntaxError {
                    offset: start,
                    kind: CssSyntaxErrorKind::Unclosed(closing),
                });
            }
        }
    }
}

/// Reports whether a token's source text contains its literal closing delimiter.
/// Strings and URLs require an unescaped final delimiter; comments require `*/`.
pub fn literal_closed(text: &str, closing: &str) -> bool {
    if closing == "*/" {
        return text.len() >= 4 && text.ends_with(closing);
    }
    text.len() > closing.len()
        && text.strip_suffix(closing).is_some_and(|before| {
            before
                .bytes()
                .rev()
                .take_while(|byte| *byte == b'\\')
                .count()
                % 2
                == 0
        })
}

/// Finds the explicit close for the CSS block opened at `open`.
/// The requested delimiter pair must match the opening token. Malformed contents,
/// missing closure, and invalid byte offsets return a scan error.
///
/// # Errors
///
/// Returns `InvalidOffset` for an unusable byte offset and `Unclosed` for an invalid
/// delimiter pair, malformed contents, or a missing closing delimiter.
pub fn matching_css_delimiter(
    value: &str,
    open: usize,
    opener: char,
    closing: char,
) -> Result<usize, ScanError> {
    let rest = value.get(open..).ok_or(ScanError::InvalidOffset {
        offset: open,
        length: value.len(),
    })?;
    if !rest.starts_with(opener) {
        return Err(ScanError::Unclosed);
    }
    let mut parser = Parser::new(rest);
    let token = parser
        .next_including_whitespace_and_comments()
        .map_err(|_| ScanError::Unclosed)?;
    if closer(token) != Some(closing) {
        return Err(ScanError::Unclosed);
    }
    let (result, block) = parse_css_block(&mut parser, |inner| {
        let result = inspect(inner, rest, true);
        consume(inner);
        result
    })
    .map_err(|_| ScanError::Unclosed)?;
    result.map_err(|_| ScanError::Unclosed)?;
    block
        .close
        .map(|close| open + close)
        .ok_or(ScanError::Unclosed)
}

/// Returns the byte offset of the first parenthesis or function deeper than `limit`.
/// Strings, comments, and escaped delimiters do not increase the depth. Returns `None`
/// when no group exceeds the limit or the CSS parser cannot enter a nested block.
pub fn parenthesis_nesting_overflow(css: &str, limit: usize) -> Option<usize> {
    fn walk(parser: &mut Parser<'_>, depth: usize, limit: usize) -> Option<usize> {
        loop {
            let start = parser.position().byte_index();
            let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
                return None;
            };
            let paren = matches!(token, Token::Function(_) | Token::ParenthesisBlock);
            if closer(&token).is_some() {
                let next_depth = depth + usize::from(paren);
                if next_depth > limit {
                    return Some(start);
                }
                let (overflow, _) = parse_css_block(parser, |inner| {
                    let overflow = walk(inner, next_depth, limit);
                    consume(inner);
                    overflow
                })
                .ok()?;
                if overflow.is_some() {
                    return overflow;
                }
            }
        }
    }
    walk(&mut Parser::new(css), 0, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_depth_counts_ancestors_and_ignores_literal_parentheses() {
        assert_eq!(parenthesis_nesting_overflow("f([g(1)]) h(2)", 1), Some(3));
        assert_eq!(parenthesis_nesting_overflow("f([g(1)]) h(2)", 2), None);
        assert_eq!(
            parenthesis_nesting_overflow(r"'(((' /* ((( */ url(x(\()", 0),
            None
        );
    }

    #[test]
    fn delimiter_lookup_checks_offsets_pairs_and_nested_contents() {
        for (input, open, opener, closing, expected) in [
            ("{ '}' {x} }", 0, '{', '}', Ok(10)),
            ("(a(')') /* ) */)", 0, '(', ')', Ok(15)),
            ("{x", 0, '{', '}', Err(ScanError::Unclosed)),
            ("x}", 0, '{', '}', Err(ScanError::Unclosed)),
            ("(a)", 0, '(', ']', Err(ScanError::Unclosed)),
            ("{[)]}", 0, '{', '}', Err(ScanError::Unclosed)),
            (
                "{x",
                3,
                '{',
                '}',
                Err(ScanError::InvalidOffset {
                    offset: 3,
                    length: 2,
                }),
            ),
            (
                "é{x}",
                1,
                '{',
                '}',
                Err(ScanError::InvalidOffset {
                    offset: 1,
                    length: 5,
                }),
            ),
        ] {
            assert_eq!(
                matching_css_delimiter(input, open, opener, closing),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn name_decoding_uses_css_escapes_without_requiring_a_css_identifier() {
        for (raw, decoded) in [
            (r"foo-1\/2", "foo-1/2"),
            (r"icon-[a\:b]", "icon-[a:b]"),
            (r"a\31 b", "a1b"),
            (r"a\75 rl(x)", "aurl(x)"),
        ] {
            assert_eq!(decode_css_escapes(raw).as_deref(), Some(decoded));
        }
        assert_eq!(decode_css_escapes("a\\"), None);
        assert_eq!(decode_css_escapes("a\\\n"), None);
    }

    #[test]
    fn property_names_are_single_identifiers() {
        for name in [
            "color",
            "COLOR",
            "-webkit-box-orient",
            "--x",
            "--1",
            "--色",
            "_x",
            r"--a\.b",
        ] {
            assert!(is_css_property_name(name), "{name}");
        }
        for name in [
            "",
            "-",
            "--",
            "&",
            "1",
            "-1",
            "@media(hover",
            "@media",
            "color()",
            "a b",
            " color",
            "color/**/",
            "a\\",
        ] {
            assert!(!is_css_property_name(name), "{name}");
        }
    }

    #[test]
    fn validates_token_kinds_and_explicit_closure() {
        for css in [
            r#"a{--x:"[)}";--y:{a:[b(c)]};}"#,
            "a{--x:foo\\,bar;/* ] */}",
        ] {
            assert_eq!(validate_css_syntax(css), Ok(()), "{css}");
        }
        for css in ["[)]", "]", "(x", r#""x\""#, "/*x", "url(x"] {
            assert!(validate_css_syntax(css).is_err(), "{css}");
        }
        assert!(validate_css_syntax_recovering("[)]").is_err());
        assert_eq!(validate_css_syntax_recovering("(x"), Ok(()));
    }

    #[test]
    fn distinguishes_an_outer_close_from_a_child_close_at_eof() {
        let mut parser = Parser::new("{a{}}");
        assert_eq!(parser.next(), Ok(&Token::CurlyBracketBlock));
        let ((), outer) = parse_css_block(&mut parser, |inner| {
            assert!(inner.next().is_ok());
            assert!(inner.next().is_ok());
            assert_eq!(consume_css_block(inner).unwrap().close, Some(3));
            consume(inner);
        })
        .unwrap();
        assert_eq!(outer.close, Some(4));
        assert_eq!(
            matching_css_delimiter("{a{}", 0, '{', '}'),
            Err(ScanError::Unclosed)
        );
    }
}
