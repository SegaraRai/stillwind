//! Lexical helpers for finding and splitting CSS function calls.
//!
//! Function lookup uses CSS tokens; delimiter and argument helpers scan characters.
//! Strings, comments and URL contents are never interpreted as nested function calls.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};

use crate::blocks::ScanError;

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

/// Finds `name(` at or after `from`, skipping strings, comments and URL contents.
/// This is the single-name wrapper of [`find_css_functions`].
pub fn find_css_function(value: &str, name: &str, from: usize) -> Option<usize> {
    find_css_functions(value, &[name], from)
        .first()
        .map(|found| found.start)
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
        let (nested, skip_contents, name_index) = match token {
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
        if !nested {
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
        let mut arguments_end = arguments_start;
        let _ = parser.parse_nested_block(|nested| {
            if skip_contents {
                while nested.next_including_whitespace_and_comments().is_ok() {}
            } else {
                collect_functions(nested, value, names, from, custom_only, found);
            }
            arguments_end = nested.position().byte_index();
            Ok::<_, ParseError<()>>(())
        });
        if let Some(index) = index {
            found[index].arguments.end = arguments_end;
            found[index].end =
                (value.as_bytes().get(arguments_end) == Some(&b')')).then_some(arguments_end + 1);
        }
    }
}

/// Returns the offset of the `closer` matching the `opener` at `open`.
///
/// `opener` and `closer` are supplied by the caller so one scanner serves parentheses
/// and brackets. Quotes and comments are skipped, so delimiters inside them are literal
/// text.
///
/// # Errors
///
/// Returns [`ScanError::InvalidOffset`] when `open` is past the end of `value` or
/// falls inside a multi-byte scalar, and [`ScanError::Unclosed`] when the delimiter is
/// never closed or the nesting underflows. Callers that obtain `open` from
/// `char_indices` or `find` over this same string always satisfy the offset
/// requirement.
pub fn matching_css_delimiter(
    value: &str,
    open: usize,
    opener: char,
    closer: char,
) -> Result<usize, ScanError> {
    if !value.is_char_boundary(open) {
        return Err(ScanError::InvalidOffset {
            offset: open,
            length: value.len(),
        });
    }
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    let bytes = value.as_bytes();
    let mut index = open;
    while index < bytes.len() {
        let character = bytes[index] as char;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '/' if bytes.get(index + 1) == Some(&b'*') => {
                let end = value[index + 2..].find("*/").ok_or(ScanError::Unclosed)?;
                index += end + 4;
                continue;
            }
            _ if character == opener => depth += 1,
            _ if character == closer => {
                depth = depth.checked_sub(1).ok_or(ScanError::Unclosed)?;
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    Err(ScanError::Unclosed)
}

/// Splits an argument list at its first top-level comma.
///
/// Commas inside nested parentheses, brackets, braces, quotes or escapes do not split.
/// The returned remainder keeps its leading whitespace and is `None` when there is no
/// top-level comma.
pub fn split_css_function_arguments(arguments: &str) -> (&str, Option<&str>) {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in arguments.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' | '[' | '{' => stack.push(character),
            ')' | ']' | '}' => {
                stack.pop();
            }
            ',' if stack.is_empty() => {
                return (&arguments[..index], Some(&arguments[index + 1..]));
            }
            _ => {}
        }
    }
    (arguments, None)
}

#[cfg(test)]
mod tests {
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
        assert_eq!(find_css_function(value, "--x", 0), Some(found[0].start));
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

    #[test]
    fn ignores_quoted_and_commented_functions_and_handles_unclosed_input() {
        let value = "'--theme(fake)' /* --theme(no) */ --theme(--yes)";
        assert_eq!(
            find_css_function(value, "--theme", 0),
            value.rfind("--theme")
        );
        assert_eq!(find_css_function("/* never closed", "--theme", 0), None);
        assert_eq!(
            matching_css_delimiter("(a(')') /* ) */)", 0, '(', ')'),
            Ok(15)
        );
        assert_eq!(
            matching_css_delimiter("(a", 0, '(', ')'),
            Err(ScanError::Unclosed)
        );
        assert_eq!(
            matching_css_delimiter("(a)", 4, '(', ')'),
            Err(ScanError::InvalidOffset {
                offset: 4,
                length: 3
            })
        );
        assert_eq!(
            matching_css_delimiter("é(a)", 1, '(', ')'),
            Err(ScanError::InvalidOffset {
                offset: 1,
                length: 5
            })
        );
        assert_eq!(
            matching_css_delimiter(")(a)", 0, '(', ')'),
            Err(ScanError::Unclosed)
        );
    }

    #[test]
    fn splits_only_the_first_unquoted_top_level_comma() {
        assert_eq!(
            split_css_function_arguments("var(--a, red), rgb(1, 2, 3)"),
            ("var(--a, red)", Some(" rgb(1, 2, 3)"))
        );
        assert_eq!(split_css_function_arguments("'a,b'"), ("'a,b'", None));
        assert_eq!(
            split_css_function_arguments("[a,b],x,y"),
            ("[a,b]", Some("x,y"))
        );
        assert_eq!(split_css_function_arguments(""), ("", None));
    }

    /// The recursive scanner descends into blocks, requires a complete name match, and
    /// reports closed and unclosed invocations differently.
    #[test]
    fn recursive_scan_descends_into_blocks_and_requires_complete_names() {
        let value = "@media x { --xy(1) } [--x(2)] ({--x(3)}) --x(4";
        let mut found = Vec::new();
        collect_functions(
            &mut Parser::new(value),
            value,
            &["--x"],
            0,
            true,
            &mut found,
        );
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
                find_css_function(value, name, 0),
                None,
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
            let value = format!("--x({}", text.concat());
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

        /// Splitting at the first top-level comma reconstructs the original argument
        /// list. The generated list always starts with one, so a split always happens.
        #[test]
        fn argument_splitting_reconstructs_the_input(text in proptest::collection::vec(
            proptest::sample::select(vec!["a", ",", "(", ")", "[", "]", "'", "\"", "\\", " "]),
            0..16,
        )) {
            let arguments = format!("a,{}", text.concat());
            let (head, tail) = split_css_function_arguments(&arguments);
            proptest::prop_assert!(tail.is_some());
            let tail = tail.unwrap();
            proptest::prop_assert_eq!(head, "a");
            proptest::prop_assert_eq!(format!("{head},{tail}"), arguments.as_str());
        }
    }
}
