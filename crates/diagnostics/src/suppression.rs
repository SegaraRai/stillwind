//! `swlint-*` comment directives and the filter that applies them.
//!
//! Directives are parsed out of the raw source text by scanning comments while skipping
//! string literals, so directive-shaped text inside a string or template is not a
//! directive. A parsed directive carries the kind, the byte offset and zero-based line
//! of its keyword, and the rule names it names; an empty rule list means every rule.
//!
//! The compiler filters analysis findings before lowering and filters the combined
//! findings again after emission; suppression therefore cannot depend on which pass
//! noticed a problem. A finding is judged by its first label, and a finding without
//! labels is treated as covering the whole file.

use std::collections::BTreeSet;

use stillwind_source::SourceFile;

use crate::LintFinding;

/// What a directive switches off, as determined by its keyword.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LintDirectiveKind {
    /// `swlint-disable`: the listed rules stay off until a matching `enable`.
    Disable,
    /// `swlint-enable`: re-enables the listed rules from this point on.
    Enable,
    /// `swlint-disable-line`: applies to the line the directive sits on.
    DisableLine,
    /// `swlint-disable-next-line`: applies to the following line only.
    DisableNextLine,
}

/// One parsed `swlint-*` comment.
#[derive(Clone, Debug)]
pub struct LintDirective {
    /// Which suppression form the comment used.
    pub kind: LintDirectiveKind,
    /// Byte offset of the directive keyword, used to order it against findings.
    pub offset: usize,
    /// Zero-based line containing the directive keyword.
    pub line: usize,
    /// Rule names following the keyword; empty means every rule.
    pub rules: Vec<String>,
}

/// Removes lint findings suppressed by `swlint-disable*` comments.
///
/// The source span is deliberately used after all analysis has completed, so findings
/// produced by different compiler passes share the same suppression semantics. Both
/// line and block comments are accepted.
///
/// Scope follows the directive kind: `disable-line` covers the directive's own line,
/// `disable-next-line` the following line, and `disable` / `enable` every finding at or
/// after the directive offset, applied in source order. A `disable` never reaches
/// backwards, findings not named by a rule list are kept, and this function can only
/// remove findings, never add them. Retained findings keep their original order.
///
/// Primary spans must refer to `source`; file identities and bounds are not validated.
/// Only the first label's start offset participates. A missing label uses offset zero;
/// secondary labels and messages do not affect suppression.
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
                rules.is_empty()
                    || rules
                        .iter()
                        .any(|rule| rule == "*" || rule == finding.code())
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
        !(suppressed || disabled_all || disabled.contains(finding.code()))
    });
}

/// Parses the `swlint-*` directives in `source`, in source order.
///
/// Comment bodies are scanned with string and template literals skipped, so directive
/// text inside a literal is ignored. A rule list ends at `--`, which lets the directive
/// carry a trailing human explanation; rules are separated by whitespace or commas.
///
/// A comment yields at most one directive, and only from its first `swlint-`
/// occurrence; if the text after that occurrence names no keyword, the comment
/// contributes nothing. There is no requirement that the keyword start the comment:
/// prose containing a recognized directive spelling is parsed too.
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
            // Longest keyword first: `disable` would otherwise swallow `disable-line`
            // and `disable-next-line` and never see their kind.
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
    use stillwind_source::{FileId, Span};

    use super::*;
    use crate::{Label, LintKind};

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
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "// swlint-disable-next-line require-anchor-provider\nx\ny",
        )
        .unwrap();
        let x = u32::try_from(source.text().find("\nx").unwrap() + 1).unwrap();
        let mut findings = vec![
            LintFinding::new(
                LintKind::MissingAnchor { name: "x".into() },
                Span::new(FileId(1), x, x + 1),
            ),
            LintFinding::new(
                LintKind::MissingContainer { name: "x".into() },
                Span::new(FileId(1), x, x + 1),
            ),
            LintFinding::new(
                LintKind::MissingAnchor { name: "x".into() },
                Span::new(FileId(1), x + 2, x + 3),
            ),
        ];
        filter_lint_findings(&source, &mut findings);
        assert_eq!(
            findings.iter().map(LintFinding::code).collect::<Vec<_>>(),
            vec!["require-container-provider", "require-anchor-provider"]
        );
    }

    #[test]
    fn disable_enable_and_same_line_directives_have_bounded_effects() {
        for (text, expected) in [
            (
                "// swlint-disable require-anchor-provider\nx\n// swlint-enable require-anchor-provider\ny",
                vec![
                    "require-container-provider",
                    "require-anchor-provider",
                    "require-container-provider",
                ],
            ),
            (
                "// swlint-disable *\nx\n// swlint-enable\ny",
                vec!["require-anchor-provider", "require-container-provider"],
            ),
            (
                "x // swlint-disable-line require-anchor-provider\ny",
                vec![
                    "require-container-provider",
                    "require-anchor-provider",
                    "require-container-provider",
                ],
            ),
        ] {
            let source = SourceFile::new(FileId(1), None, None, text).unwrap();
            let mut findings = ['x', 'y']
                .into_iter()
                .flat_map(|c| {
                    let start = u32::try_from(text.find(c).unwrap()).unwrap();
                    ["require-anchor-provider", "require-container-provider"].map(|rule| {
                        LintFinding::new(
                            if rule == "require-anchor-provider" {
                                LintKind::MissingAnchor { name: "x".into() }
                            } else {
                                LintKind::MissingContainer { name: "x".into() }
                            },
                            Span::new(FileId(1), start, start + 1),
                        )
                    })
                })
                .collect();
            filter_lint_findings(&source, &mut findings);
            assert_eq!(
                findings.iter().map(LintFinding::code).collect::<Vec<_>>(),
                expected,
                "{text}"
            );
        }
    }

    /// Suppression reads the primary label and the typed rule, not the message or the
    /// secondary labels.
    #[test]
    fn suppression_uses_primary_label_and_typed_rule_only() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "// swlint-disable-next-line require-anchor-provider\nx\ny",
        )
        .unwrap();
        let x = u32::try_from(source.text().find("\nx").unwrap() + 1).unwrap();
        let primary = Span::new(source.id(), x, x + 1);
        let secondary = Span::new(source.id(), x + 2, x + 3);
        let mut suppressed =
            LintFinding::new(LintKind::MissingAnchor { name: "a".into() }, primary);
        suppressed.labels.push(Label {
            span: secondary,
            message: None,
        });
        let retained = LintFinding::new(LintKind::MissingContainer { name: "a".into() }, primary);
        let mut findings = vec![suppressed, retained.clone()];
        filter_lint_findings(&source, &mut findings);
        assert_eq!(findings, vec![retained]);
    }
}
