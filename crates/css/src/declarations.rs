//! Lossless boundaries and priority edits for CSS declaration fragments.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};

fn is_block(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
    )
}

fn consume_block(parser: &mut Parser<'_>) {
    let _ = parser.parse_nested_block(|nested| {
        while nested.next_including_whitespace_and_comments().is_ok() {}
        Ok::<_, ParseError<()>>(())
    });
}

/// Returns declaration ranges excluding their top-level terminating semicolons.
/// Strings, comments, URLs and nested blocks do not introduce boundaries.
pub fn declaration_ranges(css: &str) -> Vec<Range<usize>> {
    let mut parser = Parser::new(css);
    let mut ranges = Vec::new();
    let mut start = 0;
    loop {
        let position = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Semicolon) => {
                ranges.push(start..position);
                start = parser.position().byte_index();
            }
            Ok(token) if is_block(token) => consume_block(&mut parser),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    if start < css.len() {
        ranges.push(start..css.len());
    }
    ranges
}

/// Adds priority to each nonempty declaration, preserving values and existing priority.
pub fn important_declarations(css: &str) -> String {
    let mut output = String::new();
    for range in declaration_ranges(css) {
        let declaration = &css[range];
        let mut parser = Parser::new(declaration);
        let mut previous_bang = false;
        let mut priority = false;
        let mut has_colon = false;
        let mut last_end = 0;
        while let Ok(token) = parser.next() {
            priority = previous_bang
                && matches!(token, Token::Ident(name) if name.eq_ignore_ascii_case("important"));
            previous_bang = matches!(token, Token::Delim('!'));
            has_colon |= matches!(token, Token::Colon);
            if is_block(token) {
                consume_block(&mut parser);
            }
            last_end = parser.position().byte_index();
        }
        if !has_colon {
            output.push_str(declaration);
            continue;
        }
        // Insert before trailing whitespace/comments, including a final line
        // comment-like spelling that CSS itself treats as ordinary tokens.
        output.push_str(&declaration[..last_end]);
        if !priority {
            output.push_str("!important");
        }
        output.push_str(&declaration[last_end..]);
        output.push(';');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_preserves_nested_values_and_existing_markers() {
        assert_eq!(
            important_declarations(
                "content:\"a;b\";background:url(data:image/svg+xml;a);color:red ! IMPORTANT /* keep */;"
            ),
            "content:\"a;b\"!important;background:url(data:image/svg+xml;a)!important;color:red ! IMPORTANT /* keep */;"
        );
        assert_eq!(
            important_declarations("content:'!important';"),
            "content:'!important'!important;"
        );
        assert_eq!(important_declarations(""), "");
    }
}
