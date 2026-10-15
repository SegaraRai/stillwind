//! Lexical resource discovery; resolving a URL belongs to the host.
//!
//! [`relative_css_urls`] reports every `url()` and `image-set()` string that looks like
//! a relative resource, together with the byte range of the expression it came from.
//! Nothing is resolved, normalized or checked against the filesystem: the host owns
//! resolution and the decision of which imports to exclude.
//!
//! Discovery is token-based, so a `url(` inside a string or comment is not a resource;
//! absolute URLs, fragments, `data:` and any value containing a colon are skipped as
//! non-relative.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, ToLeanString};

/// One relative resource reference found in CSS text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssUrl {
    /// The entire `url()` expression, or the string in `image-set()`.
    pub range: Range<usize>,
    /// Decoded specifier text, with CSS escapes resolved by the tokenizer.
    pub specifier: LeanStr,
    /// True when the reference was written as a quoted string (`image-set` candidates),
    /// false for `url()`.
    pub string: bool,
}

/// Finds relative resource references without interpreting strings or comments as CSS.
/// The caller excludes import declarations before using these as assets.
///
/// Results are in source order and their ranges refer to `css` itself, so a caller that
/// passed a transformed string must not reuse them against the original.
pub fn relative_css_urls(css: &str) -> Vec<CssUrl> {
    let mut output = Vec::new();
    scan(&mut Parser::new(css), false, &mut output);
    output
}

/// True for a specifier this crate considers relative.
///
/// Empty values, values starting with `/` or `#`, and values containing `:` (schemes,
/// `data:`) are not relative and are never reported.
fn relative(value: &str) -> bool {
    !value.is_empty() && !value.starts_with(['/', '#']) && !value.contains(':')
}

/// Walks one token stream, recording URL tokens and descending into blocks.
///
/// `image_set` marks the argument list of `image-set()` (including the `-webkit-`
/// spelling), where bare strings are candidates too.
fn scan(parser: &mut Parser<'_>, image_set: bool, output: &mut Vec<CssUrl>) {
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            break;
        };
        let (value, string) = match token {
            Token::UnquotedUrl(value) => (Some(value.to_lean_string()), false),
            Token::QuotedString(value) if image_set => (Some(value.to_lean_string()), true),
            Token::Function(name) if name.eq_ignore_ascii_case("url") => {
                let value = parser
                    .parse_nested_block(|nested| {
                        let value = nested.expect_string_cloned()?.to_lean_string();
                        nested.expect_exhausted()?;
                        Ok::<_, ParseError<()>>(value)
                    })
                    .ok();
                (value, false)
            }
            Token::Function(name) => {
                let images = name.eq_ignore_ascii_case("image-set")
                    || name.eq_ignore_ascii_case("-webkit-image-set");
                let _ = parser.parse_nested_block(|nested| {
                    scan(nested, images, output);
                    Ok::<_, ParseError<()>>(())
                });
                (None, false)
            }
            Token::CurlyBracketBlock | Token::SquareBracketBlock | Token::ParenthesisBlock => {
                let _ = parser.parse_nested_block(|nested| {
                    scan(nested, false, output);
                    Ok::<_, ParseError<()>>(())
                });
                (None, false)
            }
            _ => (None, false),
        };
        if let Some(value) = value.filter(|value| relative(value)) {
            output.push(CssUrl {
                range: start..parser.position().byte_index(),
                specifier: (value).into(),
                string,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_resources_from_strings_comments_and_absolute_urls() {
        let css = r#"/* url(fake) */ .x { content: 'url(fake)'; background: image-set('a.png' 1x, url("b.png") 2x, url(c.png) 3x); mask:url(#mask); x:url(data:image/png,foo); y:url(/root.png); z:url(https://a/x); }"#;
        let urls = relative_css_urls(css);
        assert_eq!(
            urls.iter()
                .map(|url| url.specifier.as_str())
                .collect::<Vec<_>>(),
            ["a.png", "b.png", "c.png"]
        );
        assert_eq!(&css[urls[0].range.clone()], "'a.png'");
        assert_eq!(&css[urls[1].range.clone()], "url(\"b.png\")");
        assert_eq!(&css[urls[2].range.clone()], "url(c.png)");

        // An escaped space is part of the specifier, not a separator.
        let escaped = relative_css_urls(r"a{background:url('./a\20 b.svg')}");
        assert_eq!(escaped[0].specifier, "./a b.svg");
    }
}
