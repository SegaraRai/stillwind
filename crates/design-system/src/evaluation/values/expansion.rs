//! Bounded expansion of compiler-owned CSS functions.

use std::collections::BTreeSet;

use stillwind_css::{CssFunctionMatch, find_css_functions};

/// Skips tokenization when every requested function name is absent.
/// Escaped identifiers still require the CSS parser to decode their spelling.
pub(in super::super) fn matching_functions(input: &str, names: &[&str]) -> Vec<CssFunctionMatch> {
    if !input.contains('\\') && !names.iter().any(|name| input.contains(name)) {
        Vec::new()
    } else {
        find_css_functions(input, names, 0)
    }
}

/// Single-family wrapper of [`expand_placeholders`].
pub(in super::super) fn expand_placeholder(
    body: &mut String,
    name: &str,
    mut resolve: impl FnMut(&str) -> Option<String>,
) -> Option<()> {
    expand_placeholders(body, &[name], |_, arguments| resolve(arguments))
}

/// Expands families in `names` order, recursively expanding replacements of the
/// current family. Later families see those replacements; earlier families are
/// never revisited. Duplicate names are ignored after their first occurrence.
/// Absent families share a token scan. Failure leaves `body` intact.
/// Cycles, depth, work and output size are bounded independently for each family.
pub(in super::super) fn expand_placeholders(
    body: &mut String,
    names: &[&str],
    mut resolve: impl FnMut(&str, &str) -> Option<String>,
) -> Option<()> {
    let mut matches = matching_functions(body, names);
    // Limits apply only to executable expansion, not large authored strings/data URLs.
    if matches.is_empty() {
        return Some(());
    }
    let mut working = body.clone();
    for (phase, name) in names.iter().enumerate() {
        if !matches.iter().any(|found| found.name_index == phase) {
            continue;
        }
        let mut expansion = Expansion {
            name,
            remaining: 4096,
            active: BTreeSet::new(),
        };
        working = expansion.expand_matches(
            &working,
            matches.iter().filter(|found| found.name_index == phase),
            &mut |arguments| resolve(name, arguments),
        )?;
        if phase + 1 < names.len() {
            matches = matching_functions(&working, names);
        }
    }
    *body = working;
    Some(())
}

/// Recursion guard and work budget for one CSS function family.
struct Expansion<'a> {
    /// CSS function name for this expansion phase, without parentheses.
    name: &'a str,
    /// Remaining invocation-expansion budget, decremented once per executed replacement.
    remaining: usize,
    /// Exact invocation text on the recursive expansion path, used to reject cycles;
    /// not a completed-result cache.
    active: BTreeSet<String>,
}

impl Expansion<'_> {
    fn expand_matches<'a>(
        &mut self,
        input: &str,
        matches: impl Iterator<Item = &'a CssFunctionMatch>,
        resolve: &mut impl FnMut(&str) -> Option<String>,
    ) -> Option<String> {
        const MAX_BYTES: usize = 1_048_576;
        if input.len() > MAX_BYTES || self.active.len() >= 64 {
            return None;
        }
        let mut output = String::with_capacity(input.len());
        let mut cursor = 0;
        for found in matches {
            let start = found.start;
            // Replacing an outer invocation consumes any nested matches in its arguments.
            if start < cursor {
                continue;
            }
            self.remaining = self.remaining.checked_sub(1)?;
            let end = found.end?;
            let invocation = &input[start..end];
            if !self.active.insert(invocation.to_owned()) {
                return None;
            }
            let replacement = resolve(&input[found.arguments.clone()])?;
            let nested = matching_functions(&replacement, &[self.name]);
            let expanded = self.expand_matches(&replacement, nested.iter(), resolve)?;
            self.active.remove(invocation);
            if output
                .len()
                .checked_add(start - cursor)?
                .checked_add(expanded.len())?
                > MAX_BYTES
            {
                return None;
            }
            output.push_str(&input[cursor..start]);
            output.push_str(&expanded);
            cursor = end;
        }
        if output.len().checked_add(input.len() - cursor)? > MAX_BYTES {
            return None;
        }
        output.push_str(&input[cursor..]);
        Some(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_expansion_obeys_family_order_and_does_not_revisit_earlier_families() {
        let mut body = "--b(--a(1)) --a(2)".to_owned();
        let mut calls = Vec::new();
        assert_eq!(
            expand_placeholders(&mut body, &["--a", "--b"], |name, arg| {
                calls.push((name.to_owned(), arg.to_owned()));
                Some(if name == "--b" {
                    format!("--a({arg})")
                } else {
                    arg.to_owned()
                })
            }),
            Some(())
        );
        assert_eq!(body, "--a(1) 2");
        assert_eq!(
            calls,
            [
                ("--a".into(), "1".into()),
                ("--a".into(), "2".into()),
                ("--b".into(), "1".into())
            ]
        );
        let mut body = "--a(1)".to_owned();
        expand_placeholders(&mut body, &["--absent", "--a", "--b"], |name, arg| {
            Some(if name == "--a" {
                format!("--b({arg})")
            } else {
                "done".into()
            })
        })
        .unwrap();
        assert_eq!(body, "done");
    }

    #[test]
    fn batch_failure_is_transactional_and_outer_replacements_consume_nested_calls() {
        let original = "--a(1) --b(2)";
        let mut body = original.to_owned();
        assert_eq!(
            expand_placeholders(&mut body, &["--a", "--b"], |name, _| (name == "--a")
                .then(|| "ok".into())),
            None
        );
        assert_eq!(body, original);
        let mut body = "--a(--a(1)) --a(2)".to_owned();
        let mut calls = Vec::new();
        expand_placeholder(&mut body, "--a", |arg| {
            calls.push(arg.to_owned());
            Some("ok".into())
        })
        .unwrap();
        assert_eq!(body, "ok ok");
        assert_eq!(calls, ["--a(1)", "2"]);
        let mut body = "--a(--b(1)".to_owned();
        assert_eq!(
            expand_placeholder(&mut body, "--a", |_| Some("ok".into())),
            None
        );
    }

    #[test]
    fn rejects_cycles_and_amplification_but_allows_shared_subexpressions() {
        for value in ["--x(a)", "--x(b)"] {
            let mut body = value.to_owned();
            assert_eq!(
                expand_placeholder(&mut body, "--x", |arg| Some(
                    if arg == "a" { "--x(b)" } else { "--x(a)" }.into()
                )),
                None
            );
        }
        let mut body = "--x(0)".into();
        assert_eq!(
            expand_placeholder(&mut body, "--x", |arg| {
                let next: usize = arg.parse::<usize>().ok()? + 1;
                Some(format!("--x({next})--x({next})"))
            }),
            None
        );
        let mut body = "--x(a) --x(a) '--x(a)' /* --x(a) */".into();
        assert_eq!(
            expand_placeholder(&mut body, "--x", |arg| Some(
                if arg == "a" { "--x(b)" } else { "1" }.into()
            )),
            Some(())
        );
        assert_eq!(body, "1 1 '--x(a)' /* --x(a) */");
    }

    /// Each invocation is replaced in source order, surrounding text is kept, and the
    /// remaining budget is spent once per executed replacement.
    #[test]
    fn expand_matches_replaces_in_source_order_and_spends_the_budget() {
        let mut expansion = Expansion {
            name: "--x",
            remaining: 3,
            active: BTreeSet::new(),
        };
        let input = "pre --x(1) --x(2) post";
        let matches = find_css_functions(input, &["--x"], 0);
        let output = expansion
            .expand_matches(input, matches.iter(), &mut |arguments| {
                Some(arguments.to_owned())
            })
            .unwrap();
        assert_eq!(output, "pre 1 2 post");
        assert_eq!(expansion.remaining, 1);
        assert!(expansion.active.is_empty());
    }

    /// An exhausted budget, an unclosed invocation, and a missing replacement all fail
    /// the whole expansion instead of returning partial output; the caller only sees
    /// `None`, so these cases cannot be told apart from the outside.
    #[test]
    fn expand_matches_rejects_exhausted_budget_and_unusable_invocations() {
        let input = "--x(1) --x(2)";
        let matches = find_css_functions(input, &["--x"], 0);
        let mut expansion = Expansion {
            name: "--x",
            remaining: 1,
            active: BTreeSet::new(),
        };
        assert!(
            expansion
                .expand_matches(input, matches.iter(), &mut |arguments| Some(
                    arguments.to_owned()
                ))
                .is_none()
        );
        let unclosed = "--x(1";
        let matches = find_css_functions(unclosed, &["--x"], 0);
        let mut expansion = Expansion {
            name: "--x",
            remaining: 4,
            active: BTreeSet::new(),
        };
        assert!(
            expansion
                .expand_matches(unclosed, matches.iter(), &mut |_| Some(String::new()))
                .is_none()
        );
        let input = "--x(1)";
        let matches = find_css_functions(input, &["--x"], 0);
        let mut expansion = Expansion {
            name: "--x",
            remaining: 4,
            active: BTreeSet::new(),
        };
        assert!(
            expansion
                .expand_matches(input, matches.iter(), &mut |_| None)
                .is_none()
        );
    }

    /// A replacement that re-invokes the same invocation is a cycle, while an outer
    /// invocation consumes the nested matches inside its own arguments.
    #[test]
    fn expand_matches_rejects_recursion_and_consumes_nested_matches() {
        let input = "--x(a)";
        let matches = find_css_functions(input, &["--x"], 0);
        let mut expansion = Expansion {
            name: "--x",
            remaining: 8,
            active: BTreeSet::new(),
        };
        assert!(
            expansion
                .expand_matches(input, matches.iter(), &mut |arguments| Some(format!(
                    "--x({arguments})"
                )))
                .is_none()
        );
        let input = "--x(--x(1))";
        let matches = find_css_functions(input, &["--x"], 0);
        let mut expansion = Expansion {
            name: "--x",
            remaining: 8,
            active: BTreeSet::new(),
        };
        let mut calls = 0;
        let output = expansion
            .expand_matches(input, matches.iter(), &mut |_| {
                calls += 1;
                Some("R".to_owned())
            })
            .unwrap();
        assert_eq!(output, "R");
        assert_eq!(calls, 1);
    }

    /// Oversized input and a full recursion stack are rejected before any work. The
    /// literals below are the implementation limits (1 MiB, 64 frames) plus one, since
    /// the guards are private to `expand_matches`.
    #[test]
    fn expand_matches_rejects_oversized_input_and_a_full_stack() {
        let long = "a".repeat(1_048_577);
        let mut expansion = Expansion {
            name: "--x",
            remaining: 4,
            active: BTreeSet::new(),
        };
        assert!(
            expansion
                .expand_matches(&long, std::iter::empty(), &mut |_| Some(String::new()))
                .is_none()
        );
        let mut expansion = Expansion {
            name: "--x",
            remaining: 4,
            active: (0..64).map(|index| index.to_string()).collect(),
        };
        assert!(
            expansion
                .expand_matches("--x(1)", std::iter::empty(), &mut |_| Some(String::new()))
                .is_none()
        );
    }
}
