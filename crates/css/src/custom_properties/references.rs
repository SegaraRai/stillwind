//! Discovery of `--name` identifiers in CSS text by tokenization.
//!
//! The parser tokenizes CSS with cssparser and keeps only the names of ident and
//! function tokens that start with `--`. Strings, comments, URLs, hash tokens,
//! at-keywords, and dimension units are tokenized and discarded, so `--x` inside them
//! is not reported. The scan does not parse declarations or rules, so a declaration
//! such as `--x: 1` and a reference such as `var(--x)` are reported the same way. This
//! is the cheap dependency set used for activation decisions, not a substitute for the
//! AST-based classification in [`super::usage`].

use std::collections::BTreeSet;

use cssparser::{Parser, Token};
use lean_string::{LeanStr, ToLeanStr};

use crate::parser::{closer, parse_css_block};

/// Returns every distinct `--name` ident or function name in `css`, in `BTreeSet`
/// order.
///
/// A name keeps its authored spelling, including escapes such as `--size-2\.5`, and the
/// scan never decodes escapes. A bare `--` is skipped. Function names count because
/// `--spacing(4)` lowers to a `var(--spacing)` reference. The scan never fails.
pub fn custom_property_references(css: &str) -> BTreeSet<LeanStr> {
    let mut names = BTreeSet::new();
    collect_names(&mut Parser::new(css), &mut names);
    names
}

fn collect_names(parser: &mut Parser<'_>, names: &mut BTreeSet<LeanStr>) {
    loop {
        let start = parser.position();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return;
        };
        let block = closer(token).is_some();
        let function = matches!(token, Token::Function(_));
        let ident = function || matches!(token, Token::Ident(_));
        if ident {
            let raw = parser.slice_from(start);
            let name = if function {
                raw.strip_suffix('(').unwrap_or(raw)
            } else {
                raw
            };
            if name.len() > 2 && name.starts_with("--") {
                names.insert(name.to_lean_str());
            }
        }
        if block {
            let _ = parse_css_block(parser, |inner| collect_names(inner, names));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
