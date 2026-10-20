//! Discovery of `--name` identifiers in CSS text by tokenization.
//!
//! [`custom_property_references`] runs [`CssLexer`] over the text and keeps only the
//! names of ident and function tokens that start with `--`. Strings, comments, URLs,
//! hash tokens, at-keywords, and dimension units are tokenized and discarded, so `--x`
//! inside them is not reported. The scan does not parse declarations or rules, so a
//! declaration such as `--x: 1` and a reference such as `var(--x)` are reported the
//! same way. This is the cheap dependency set used for activation decisions, not a
//! substitute for the AST-based classification in [`super::usage`].

use std::collections::BTreeSet;

use lean_string::{LeanStr, ToLeanStr};

use crate::lexer::{CssLexer, CssLexerHandler, CssLexerState, NameKind};

/// Returns every distinct `--name` ident or function name in `css`, in `BTreeSet`
/// order.
///
/// A name keeps its authored spelling, including escapes such as `--size-2\.5`, and the
/// scan never decodes escapes. A bare `--` is skipped. Function names count because
/// `--spacing(4)` lowers to a `var(--spacing)` reference. The scan never fails.
pub fn custom_property_references(css: &str) -> BTreeSet<LeanStr> {
    CssLexer::new(DashedIdentLexer::default()).run(css).names
}

/// Records the ident and function names that start with `--` as [`CssLexer`] ends them.
#[derive(Debug, Default)]
struct DashedIdentLexer {
    /// Recorded names with their authored spelling.
    names: BTreeSet<LeanStr>,
}

impl DashedIdentLexer {
    /// Records the name of `state` when it is a dashed ident name other than `--` that
    /// ends at `end`.
    fn record(&mut self, css: &str, state: CssLexerState, end: usize) {
        if let CssLexerState::Name {
            start,
            kind: NameKind::Ident,
        } = state
        {
            let name = &css[start..end];
            if name.len() > 2 && name.starts_with("--") && !self.names.contains(name) {
                self.names.insert(name.to_lean_str());
            }
        }
    }
}

impl CssLexerHandler for DashedIdentLexer {
    fn on_transition(&mut self, css: &str, offset: usize, from: CssLexerState, _to: CssLexerState) {
        self.record(css, from, offset);
    }

    fn on_end(&mut self, css: &str, state: CssLexerState) {
        self.record(css, state, css.len());
    }
}

#[cfg(test)]
mod tests {
    use cssparser::{ParseError, Parser, Token};

    use super::*;

    /// Collects dashed ident and function names with cssparser's tokenizer, at every
    /// block depth.
    fn cssparser_names(parser: &mut Parser<'_>, names: &mut BTreeSet<LeanStr>) {
        loop {
            let start = parser.position();
            let Ok(token) = parser.next_including_whitespace_and_comments() else {
                return;
            };
            let (name, block) = match token {
                Token::Ident(_) => (Some(parser.slice_from(start)), false),
                Token::Function(_) => (parser.slice_from(start).strip_suffix('('), true),
                Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                    (None, true)
                }
                _ => (None, false),
            };
            if let Some(name) = name.filter(|name| name.len() > 2 && name.starts_with("--")) {
                names.insert(name.into());
            }
            if block {
                let _ = parser.parse_nested_block(|nested| {
                    cssparser_names(nested, names);
                    Ok::<_, ParseError<()>>(())
                });
            }
        }
    }

    proptest::proptest! {
        /// The lexer reports the same names as cssparser's CSS Syntax tokenizer.
        #[test]
        fn names_match_the_cssparser_tokenizer(parts in proptest::collection::vec(
            proptest::sample::select(vec![
                "--", "-", "a", "1", ".", "+", "e", " ", "\n", "\r", "\\", "31", "(", ")",
                "{", "}", "[", "]", "\"", "'", "/*", "*/", "#", "@", "%", "url(", "<!--",
                "-->", "色", ";", ":", "u", "R", "l", "\\75 ", "\t", "\u{c}", "_",
            ]),
            0..24,
        )) {
            let css = parts.concat();
            let mut expected = BTreeSet::new();
            cssparser_names(&mut Parser::new(&css), &mut expected);
            proptest::prop_assert_eq!(custom_property_references(&css), expected, "{:?}", css);
        }
    }

    #[test]
    fn deduplicates_names_and_stops_at_css_delimiters() {
        assert_eq!(
            custom_property_references("var(--x, --fallback_2) --x --"),
            BTreeSet::from(["--x".into(), "--fallback_2".into()])
        );
        assert!(custom_property_references("color:red").is_empty());
        assert_eq!(
            custom_property_references(r"var(--size-2\.5) var(--a\31 b) --end\"),
            BTreeSet::from([r"--size-2\.5".into(), r"--a\31 b".into(), r"--end\".into()])
        );
    }

    #[test]
    fn names_inside_other_tokens_are_discarded() {
        let css = r#"/* --comment */ "--double" '--single \' --escaped' url(--url) URL( --spaced )
            #--hash @--at 1--unit 2.5e3--exponent .btn--class a--b <!-- -->"#;
        assert_eq!(custom_property_references(css), BTreeSet::new());
    }

    #[test]
    fn idents_and_function_names_count_after_other_tokens() {
        let css = "url(\"--quoted\") --spacing(4) calc(1-var(--a)) 1url(--unit-call) --色 x:--y;-->--z \"bad\n--after-newline";
        assert_eq!(
            custom_property_references(css),
            BTreeSet::from(
                [
                    "--a",
                    "--after-newline",
                    "--spacing",
                    "--unit-call",
                    "--y",
                    "--z",
                    "--色"
                ]
                .map(LeanStr::from)
            )
        );
    }

    #[test]
    fn hex_escapes_consume_one_whitespace_and_crlf() {
        assert_eq!(
            custom_property_references("--a\\31\r\nb --c\\31  d"),
            BTreeSet::from(["--a\\31\r\nb".into(), "--c\\31 ".into()])
        );
    }
}
