//! Plugin paths retain their original CSS ranges without loading a design system.

use std::ops::Range;

use cssparser::{Parser, Token};
use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssNodeKind, parse_structure};

/// A quoted path in a top-level `@plugin` statement, including an unclosed path.
#[derive(Clone, Debug)]
pub struct StylesheetPluginPath {
    /// Original UTF-8 range of the string contents, excluding its quotes.
    pub range: Range<usize>,
    /// Decoded CSS string used for module resolution.
    pub specifier: LeanStr,
    /// Opening quote, either `'` or `"`, used when writing a replacement.
    pub quote: char,
    /// Whether the original string has a closing quote.
    pub closed: bool,
    /// Decoded text before the cursor, present only when the cursor is inside this
    /// path.
    pub prefix: Option<LeanStr>,
}

/// Finds quoted plugin paths without resolving imports or executing plugins.
/// `offset`, when present, is an original UTF-8 cursor boundary. Invalid boundaries
/// do not select a prefix. Comments, other strings, and nested rules are excluded.
pub fn stylesheet_plugin_paths(css: &str, offset: Option<usize>) -> Vec<StylesheetPluginPath> {
    let mut paths = Vec::new();
    for node in parse_structure(css) {
        let CssNodeKind::AtRule { name, prelude, .. } = node.kind else {
            continue;
        };
        if name != "plugin" {
            continue;
        }
        let mut parser = Parser::new(&css[prelude.clone()]);
        let path = loop {
            let start = parser.position().byte_index();
            match parser.next_including_whitespace_and_comments().cloned() {
                Ok(Token::WhiteSpace(_) | Token::Comment(_)) => {}
                Ok(Token::QuotedString(value)) => {
                    break Some((prelude.start + start, value.to_lean_str()));
                }
                _ => {
                    break None;
                }
            }
        };
        let Some((start, specifier)) = path else {
            continue;
        };
        let Some(quote @ ('\'' | '"')) = css.get(start..).and_then(|text| text.chars().next())
        else {
            continue;
        };
        let end = prelude.start + parser.position().byte_index();
        let escaped = css[start..end.saturating_sub(1)]
            .bytes()
            .rev()
            .take_while(|byte| *byte == b'\\')
            .count()
            % 2
            == 1;
        let closed =
            end > start + 1 && css.as_bytes().get(end - 1) == css.as_bytes().get(start) && !escaped;
        let range = start + 1..end - usize::from(closed);
        let prefix = offset
            .filter(|offset| {
                css.is_char_boundary(*offset) && range.start <= *offset && *offset <= range.end
            })
            .and_then(|offset| {
                let mut parser = Parser::new(&css[start..offset]);
                match parser.next().ok()? {
                    Token::QuotedString(value) => Some(value.to_lean_str()),
                    _ => None,
                }
            });
        paths.push(StylesheetPluginPath {
            range,
            specifier,
            quote,
            closed,
            prefix,
        });
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_paths_decode_css_and_keep_original_ranges_in_incomplete_input() {
        let css = r#"/* 🦀 @plugin 'ignored'; */
.a { content: '@plugin "ignored"'; @plugin 'nested'; }
@plugin /* options */ './pl\75gin.mjs' { prefix: 'icons'; }
@plugin "./part"#;
        let paths = stylesheet_plugin_paths(css, Some(css.len()));
        assert_eq!(paths.len(), 2);
        assert_eq!(&css[paths[0].range.clone()], r"./pl\75gin.mjs");
        assert_eq!(paths[0].specifier, "./plugin.mjs");
        assert!(paths[0].closed);
        assert_eq!(paths[1].prefix.as_deref(), Some("./part"));
        assert!(!paths[1].closed);
        assert!(stylesheet_plugin_paths("'str'; @plugin invalid;", None).is_empty());
        let escaped = stylesheet_plugin_paths(r#"@plugin "./quote\""#, None);
        assert!(!escaped[0].closed);
        assert_eq!(escaped[0].specifier, "./quote\"");
    }
}
