//! Function discovery and argument ranges from CSS tokens.
//!
//! Function lookup uses CSS tokens. Strings, comments and URL contents are never
//! interpreted as nested function calls.

use std::ops::Range;

use cssparser::{Parser, Token};

use crate::parser::{consume, parse_css_block};

/// One unescaped function invocation found during a batch scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssFunctionMatch {
    /// Index of the matching name in the caller's name list (first index for duplicates).
    pub name_index: usize,
    /// Byte offset of the function name.
    pub start: usize,
    /// Argument bytes, excluding parentheses; extends to EOF for an unclosed function.
    pub arguments: Range<usize>,
    /// Exclusive end including `)`, or `None` for an unclosed function.
    pub end: Option<usize>,
}

/// Finds all occurrences of several complete, unescaped function names in one token walk.
/// Results follow source order, including nested matches. Consumers replacing an outer
/// invocation must skip matches contained in that replacement range. `from` filters
/// results without discarding lexical context before that offset.
pub fn find_css_functions(value: &str, names: &[&str], from: usize) -> Vec<CssFunctionMatch> {
    let custom_only = names.iter().all(|name| name.starts_with("--"));
    if names.is_empty()
        || from >= value.len()
        || !value.contains('(')
        || (custom_only && !value.contains("--"))
        || (names.len() == 1 && !value.contains(names[0]))
    {
        return Vec::new();
    }
    let mut found = Vec::new();
    collect_functions(
        &mut Parser::new(value),
        value,
        names,
        from,
        custom_only,
        &mut found,
    );
    found
}

fn collect_functions(
    parser: &mut Parser<'_>,
    value: &str,
    names: &[&str],
    from: usize,
    custom_only: bool,
    found: &mut Vec<CssFunctionMatch>,
) {
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return;
        };
        let (has_block, skip_contents, name_index) = match token {
            Token::Function(function) => {
                let name_index = (start >= from && (!custom_only || function.starts_with("--")))
                    .then(|| {
                        names.iter().position(|name| {
                            function.as_ref() == *name
                                && value[start..]
                                    .strip_prefix(name)
                                    .is_some_and(|tail| tail.starts_with('('))
                        })
                    })
                    .flatten();
                (true, function.eq_ignore_ascii_case("url"), name_index)
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                (true, false, None)
            }
            _ => (false, false, None),
        };
        if !has_block {
            continue;
        }
        let arguments_start = parser.position().byte_index();
        let index = name_index.map(|name_index| {
            let index = found.len();
            found.push(CssFunctionMatch {
                name_index,
                start,
                arguments: arguments_start..arguments_start,
                end: None,
            });
            index
        });
        let result = parse_css_block(parser, |inner| {
            if skip_contents {
                consume(inner);
            } else {
                collect_functions(inner, value, names, from, custom_only, found);
            }
        });
        if let Some(index) = index
            && let Ok(((), block)) = result
        {
            found[index].arguments = block.contents;
            found[index].end = block.close.map(|close| close + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use super::*;

    #[test]
    fn batch_scan_preserves_lexical_context_and_nested_ranges() {
        let value = r"'--x(fake)' /* --y(no) */ url('--x(no)') --x(--y(1)) --y(2) --\x(3)";
        let found = find_css_functions(value, &["--y", "--x", "--y"], 0);
        assert_eq!(
            found.iter().map(|m| m.name_index).collect::<Vec<_>>(),
            [1, 0, 0]
        );
        assert_eq!(
            found
                .iter()
                .map(|m| &value[m.arguments.clone()])
                .collect::<Vec<_>>(),
            ["--y(1)", "1", "2"]
        );
        assert_eq!(&value[found[0].start..found[0].end.unwrap()], "--x(--y(1))");
        assert_eq!(
            find_css_functions(value, &["--y", "--x"], found[0].start + 1),
            found[1..]
        );
        assert_eq!(
            find_css_functions(value, &[], 0),
            [] as [CssFunctionMatch; 0]
        );
        assert_eq!(
            find_css_functions(value, &["--x"], value.len() + 1),
            [] as [CssFunctionMatch; 0]
        );
    }

    #[test]
    fn batch_scan_distinguishes_unclosed_outer_and_closed_inner_calls() {
        let value = "--x(--y(1)";
        let found = find_css_functions(value, &["--x", "--y"], 0);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].end, None);
        assert_eq!(&value[found[0].arguments.clone()], "--y(1)");
        assert_eq!(found[1].end, Some(value.len()));
        assert_eq!(&value[found[1].arguments.clone()], "1");
        let value = "--x(')' /* ) */ [a(b)] )";
        let found = find_css_functions(value, &["--x"], 0);
        assert_eq!(found[0].end, Some(value.len()));
    }

    /// The recursive scanner descends into blocks, requires a complete name match, and
    /// reports closed and unclosed invocations differently.
    #[test]
    fn recursive_scan_descends_into_blocks_and_requires_complete_names() {
        let value = "@media x { --xy(1) } [--x(2)] ({--x(3)}) --x(4";
        let found = find_css_functions(value, &["--x"], 0);
        assert_eq!(
            found
                .iter()
                .map(|m| &value[m.arguments.clone()])
                .collect::<Vec<_>>(),
            ["2", "3", "4"]
        );
        assert_eq!(&value[found[0].start..found[0].end.unwrap()], "--x(2)");
        assert_eq!(&value[found[1].start..found[1].end.unwrap()], "--x(3)");
        assert_eq!(found[2].end, None);
        assert_eq!(&value[found[0].start..found[0].start + 4], "--x(");
    }

    /// A name that only starts with a requested one is not a match, and the name
    /// comparison is case-sensitive outside `url(` handling.
    #[test]
    fn scan_does_not_match_a_longer_or_differently_cased_name() {
        let value = "--xy(1) --x(2) CALC(3) calc(4)";
        let found = find_css_functions(value, &["--x", "calc"], 0);
        assert_eq!(
            found
                .iter()
                .map(|m| &value[m.start..m.end.unwrap()])
                .collect::<Vec<_>>(),
            ["--x(2)", "calc(4)"]
        );
        for (value, name) in [
            ("--xy(1)", "--x"),
            ("CALC(1)", "calc"),
            ("--x (1)", "--x"),
            (r"\--x(1)", "--x"),
        ] {
            assert_eq!(
                find_css_functions(value, &[name], 0),
                [],
                "{value:?} must not match {name:?}"
            );
        }
    }

    proptest::proptest! {
        /// Reported matches stay inside the buffer and name a real invocation. The
        /// generated text always starts one invocation, so at least one match exists.
        #[test]
        fn reported_matches_are_well_formed(text in proptest::collection::vec(
            proptest::sample::select(vec!["(", ")", "--x", "1", " ", "'a'", "/* c */", "[", "]", "{", "}"]),
            0..24,
        )) {
            let value = format_args!("--x({}", text.concat()).to_lean_string();
            let founds = find_css_functions(&value, &["--x"], 0);
            proptest::prop_assert!(!founds.is_empty());
            for found in founds {
                proptest::prop_assert!(value.is_char_boundary(found.start));
                proptest::prop_assert!(value[found.start..].starts_with("--x("));
                proptest::prop_assert!(found.arguments.start >= found.start);
                proptest::prop_assert!(found.arguments.end <= value.len());
                proptest::prop_assert!(found.arguments.start <= found.arguments.end);
                if let Some(end) = found.end {
                    proptest::prop_assert!(end <= value.len());
                    proptest::prop_assert!(value[found.start..end].ends_with(')'));
                }
            }
        }

    }
}
