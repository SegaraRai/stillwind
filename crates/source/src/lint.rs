use std::collections::BTreeSet;

use super::diagnostics::LintFinding;
use super::text::SourceFile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LintDirectiveKind {
    Disable,
    Enable,
    DisableLine,
    DisableNextLine,
}

#[derive(Clone, Debug)]
pub struct LintDirective {
    pub kind: LintDirectiveKind,
    pub offset: usize,
    pub line: usize,
    pub rules: Vec<String>,
}

/// Removes lint findings suppressed by `swlint-disable*` comments.
///
/// The source span is deliberately used after all analysis has completed, so
/// findings produced by different compiler passes share the same suppression
/// semantics. Both line and block comments are accepted.
pub fn filter_lint_findings(source: &SourceFile, findings: &mut Vec<LintFinding>) {
    let directives = collect_lint_directives(source.text());
    if directives.is_empty() {
        return;
    }
    let line_starts = source
        .text()
        .match_indices('\n')
        .map(|(offset, _)| offset + 1)
        .chain(std::iter::once(source.text().len()))
        .collect::<Vec<_>>();
    findings.retain(|finding| {
        let span = finding
            .labels
            .first()
            .map_or(source.full_span(), |label| label.span);
        let finding_offset = span.start.0 as usize;
        let finding_line = line_starts.partition_point(|start| *start <= finding_offset);
        let mut disabled = BTreeSet::new();
        let mut disabled_all = false;
        let mut suppressed = false;
        for directive in &directives {
            let matches_rule = |rules: &[String]| {
                rules.is_empty() || rules.iter().any(|rule| rule == "*" || rule == finding.rule)
            };
            match directive.kind {
                LintDirectiveKind::DisableLine if directive.line == finding_line => {
                    suppressed |= matches_rule(&directive.rules);
                }
                LintDirectiveKind::DisableNextLine if directive.line + 1 == finding_line => {
                    suppressed |= matches_rule(&directive.rules);
                }
                LintDirectiveKind::Disable if directive.offset <= finding_offset => {
                    if matches_rule(&directive.rules) {
                        if directive.rules.is_empty()
                            || directive.rules.iter().any(|rule| rule == "*")
                        {
                            disabled_all = true;
                        } else {
                            disabled.extend(directive.rules.iter().cloned());
                        }
                    }
                }
                LintDirectiveKind::Enable if directive.offset <= finding_offset => {
                    if directive.rules.is_empty() || directive.rules.iter().any(|rule| rule == "*")
                    {
                        disabled_all = false;
                        disabled.clear();
                    } else {
                        for rule in &directive.rules {
                            disabled.remove(rule);
                        }
                    }
                }
                _ => {}
            }
        }
        !(suppressed || disabled_all || disabled.contains(finding.rule))
    });
}

pub fn collect_lint_directives(source: &str) -> Vec<LintDirective> {
    let bytes = source.as_bytes();
    let mut comments = Vec::new();
    let mut cursor = 0;
    let mut quote = None;
    while cursor + 1 < bytes.len() {
        if let Some(delimiter) = quote {
            if bytes[cursor] == b'\\' {
                cursor = (cursor + 2).min(bytes.len());
            } else {
                if bytes[cursor] == delimiter {
                    quote = None;
                }
                cursor += 1;
            }
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'"' | b'`') {
            quote = Some(bytes[cursor]);
            cursor += 1;
            continue;
        }
        if bytes[cursor] == b'/' && bytes[cursor + 1] == b'/' {
            let start = cursor + 2;
            cursor = source[cursor..]
                .find('\n')
                .map_or(bytes.len(), |offset| cursor + offset);
            comments.push((start, &source[start..cursor]));
        } else if bytes[cursor] == b'/' && bytes[cursor + 1] == b'*' {
            let start = cursor + 2;
            cursor = source[cursor + 2..]
                .find("*/")
                .map_or(bytes.len(), |offset| cursor + 2 + offset);
            comments.push((start, &source[start..cursor]));
            cursor = (cursor + 2).min(bytes.len());
        } else {
            cursor += 1;
        }
    }
    comments
        .into_iter()
        .filter_map(|(offset, body)| {
            let relative = body.find("swlint-")?;
            let directive_offset = offset + relative;
            let rest = &body[relative + "swlint-".len()..];
            let (kind, rest) = [
                ("disable-next-line", LintDirectiveKind::DisableNextLine),
                ("disable-line", LintDirectiveKind::DisableLine),
                ("disable", LintDirectiveKind::Disable),
                ("enable", LintDirectiveKind::Enable),
            ]
            .into_iter()
            .find_map(|(name, kind)| rest.strip_prefix(name).map(|rest| (kind, rest)))?;
            let rules = rest
                .split(|character: char| character.is_whitespace() || character == ',')
                .take_while(|rule| *rule != "--")
                .filter(|rule| !rule.is_empty())
                .map(str::to_owned)
                .collect();
            let line = source[..directive_offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count();
            Some(LintDirective {
                kind,
                offset: directive_offset,
                line,
                rules,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{FileId, Span};

    #[test]
    fn directives_ignore_strings_and_retain_rule_and_line_information() {
        let directives = collect_lint_directives(
            "'// swlint-disable fake'\n// swlint-disable-next-line a,b -- reason\nx\n/* swlint-enable a */",
        );
        assert_eq!(directives.len(), 2);
        assert_eq!(directives[0].kind, LintDirectiveKind::DisableNextLine);
        assert_eq!(directives[0].line, 1);
        assert_eq!(directives[0].rules, vec!["a", "b"]);
        assert_eq!(directives[1].kind, LintDirectiveKind::Enable);
    }

    #[test]
    fn disable_next_line_does_not_suppress_other_rules_or_later_lines() {
        let source =
            SourceFile::new(FileId(1), "x", "// swlint-disable-next-line a\nx\ny").unwrap();
        let x = u32::try_from(source.text().find("\nx").unwrap() + 1).unwrap();
        let mut findings = vec![
            LintFinding::new("a", "", Span::new(FileId(1), x, x + 1)),
            LintFinding::new("b", "", Span::new(FileId(1), x, x + 1)),
            LintFinding::new("a", "", Span::new(FileId(1), x + 2, x + 3)),
        ];
        filter_lint_findings(&source, &mut findings);
        assert_eq!(
            findings.iter().map(|f| f.rule).collect::<Vec<_>>(),
            vec!["b", "a"]
        );
    }

    #[test]
    fn disable_enable_and_same_line_directives_have_bounded_effects() {
        for (text, expected) in [
            (
                "// swlint-disable a\nx\n// swlint-enable a\ny",
                vec!["b", "a", "b"],
            ),
            (
                "// swlint-disable *\nx\n// swlint-enable\ny",
                vec!["a", "b"],
            ),
            ("x // swlint-disable-line a\ny", vec!["b", "a", "b"]),
        ] {
            let source = SourceFile::new(FileId(1), "x", text).unwrap();
            let mut findings = ['x', 'y']
                .into_iter()
                .flat_map(|c| {
                    let start = u32::try_from(text.find(c).unwrap()).unwrap();
                    ["a", "b"].map(|rule| {
                        LintFinding::new(rule, "", Span::new(FileId(1), start, start + 1))
                    })
                })
                .collect();
            filter_lint_findings(&source, &mut findings);
            assert_eq!(
                findings
                    .iter()
                    .map(|finding| finding.rule)
                    .collect::<Vec<_>>(),
                expected,
                "{text}"
            );
        }
    }
}
