//! Plugin paths and plugin catalog inputs, read without loading a design system.
//!
//! [`stylesheet_plugin_paths`] retains the original CSS ranges of plugin paths.
//! [`stylesheet_catalog_input`] selects the name that a plugin catalog completes at a
//! cursor.

use std::ops::Range;

use cssparser::{Parser, Token};
use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssNodeKind, parse_structure};
use stillwind_syntax::CandidatePrefix;

use super::context::{Kind, context};

/// Returns the text that a plugin catalog completes at the UTF-8 byte `offset` of
/// `css`, such as `icon-mdi--cl` for `@apply hover:icon-mdi--cl` with the cursor at
/// the end.
///
/// The text runs from the start of the `@apply` candidate or applied `@variant`
/// sequence at the cursor to the cursor, without its variants. Returns `None` for an
/// invalid offset and for a cursor in any other input, including comments and
/// strings.
pub fn stylesheet_catalog_input(css: &str, offset: usize) -> Option<LeanStr> {
    let context = context(css, offset)?;
    if !matches!(context.kind, Kind::Apply | Kind::Variant) {
        return None;
    }
    let prefix = css.get(context.range.start..offset)?;
    Some(prefix[CandidatePrefix::variant_end(prefix)..].to_lean_str())
}

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

    #[test]
    fn catalog_inputs_start_at_the_candidate_and_omit_its_variants() {
        let input = |css: &str| {
            let offset = css.find('|').unwrap();
            let css = css.replace('|', "");
            stylesheet_catalog_input(&css, offset).map(|input| input.to_string())
        };
        assert_eq!(
            input(".a { @apply p-4 hover:icon-mdi--cl| }").as_deref(),
            Some("icon-mdi--cl")
        );
        assert_eq!(
            input(".a { @apply content-['a_b;c'] data-[x=':']:icon-a| }").as_deref(),
            Some("icon-a")
        );
        assert_eq!(
            input(".a { @variant dark:state-op| { color: red; } }").as_deref(),
            Some("state-op")
        );
        assert_eq!(input(".a { content: 'icon-x|'; }"), None);
        assert_eq!(input("/* @apply icon-x| */"), None);
    }
}
