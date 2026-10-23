//! `swlint-*` comment directives and the filter that applies them.
//!
//! Directives are parsed out of comment bodies. Script frontends supply the comments
//! that their parser found. CSS callers use `stillwind_css::css_comment_ranges` for
//! comments of a stylesheet, so directive-shaped text inside a string or template is
//! not a directive.
//!
//! A parsed directive carries the kind, the byte offset and zero-based line of its
//! keyword, and its selected rules. An omitted list or `*` selects every rule; unknown
//! names in an explicit list select no additional rules and remain available for
//! validation.
//!
//! The compiler validates directives and filters findings after all producers have run.
//! A finding is judged by its first label, and a finding without labels is treated as
//! covering the whole file.

use std::collections::BTreeSet;
use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_source::{SourceFile, Span};

use super::{
    LintConfig, LintDiagnostic, LintFinding, LintKind, LintSettings, LintSeverity, LintSurface,
};

/// What a directive switches off, as determined by its keyword.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LintDirectiveKind {
    /// Unrecognized or malformed suppression keyword; it never changes reporting.
    Invalid,
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
    /// Rules selected by the directive; an omitted list or `*` selects every rule.
    pub rules: LintDirectiveRules,
    /// End byte offset of the directive keyword.
    pub end: usize,
    /// Explicit rule spellings, including unknown names, with their byte offsets.
    pub names: Vec<(LeanStr, usize)>,
    /// Byte range of the body of the comment that contains the directive.
    pub comment: Range<usize>,
}

/// Rules selected by one suppression directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LintDirectiveRules {
    /// Every supported rule, selected by an omitted list or `*`.
    All,
    /// Recognized names from an explicit list. An empty set selects no rules.
    Named(BTreeSet<LintKind>),
}

impl LintDirectiveRules {
    /// Returns whether this selection includes `kind`.
    pub fn contains(&self, kind: LintKind) -> bool {
        match self {
            Self::All => true,
            Self::Named(rules) => rules.contains(&kind),
        }
    }
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
/// Primary spans must refer to `source`, and `directives` must be the directives of
/// `source` in source order. File identities and bounds are not validated. A directive
/// cannot suppress validation or unused findings about itself. Only the first label's
/// start offset participates. A missing label uses offset zero. Secondary labels and
/// messages do not affect suppression.
pub fn filter_lint_findings(
    source: &SourceFile,
    directives: &[LintDirective],
    findings: &mut Vec<LintFinding>,
) {
    findings.retain(|finding| suppressing_directive(source, finding, directives).is_none());
}

pub(crate) fn suppressing_directive(
    source: &SourceFile,
    finding: &LintFinding,
    directives: &[LintDirective],
) -> Option<usize> {
    if directives.is_empty() {
        return None;
    }
    let offset = finding
        .labels
        .first()
        .map_or(0, |label| label.span.start.0 as usize);
    let line = source.text()[..offset.min(source.text().len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let mut active = None;
    let mut local = None;
    for (index, directive) in directives.iter().enumerate() {
        if matches!(
            finding.kind(),
            LintKind::InvalidLintSuppression | LintKind::UnusedLintSuppression
        ) && (offset == directive.offset
            || directive
                .names
                .iter()
                .any(|(_, name_offset)| *name_offset == offset))
        {
            continue;
        }
        if !directive.rules.contains(finding.kind()) {
            continue;
        }
        match directive.kind {
            LintDirectiveKind::Disable if directive.offset <= offset => {
                if active.is_none() {
                    active = Some(index);
                }
            }
            LintDirectiveKind::Enable if directive.offset <= offset => {
                active = None;
            }
            LintDirectiveKind::DisableLine if directive.line == line => {
                local = local.or(Some(index));
            }
            LintDirectiveKind::DisableNextLine if directive.line + 1 == line => {
                local = local.or(Some(index));
            }
            _ => {}
        }
    }
    active.or(local)
}

/// Removes the findings that `settings` never reports: every finding when no rule is
/// enabled, and the `invalid-marker-use` findings whose marker class the rule's
/// `ignoredClasses` lists. Returns whether any rule is enabled.
fn remove_unreported_findings(settings: &LintSettings, findings: &mut Vec<LintFinding>) -> bool {
    if !settings.has_enabled_rules() {
        findings.clear();
        return false;
    }
    if let Some(LintConfig::InvalidMarkerUse(config)) = settings.get(LintKind::InvalidMarkerUse)
        && !config.ignored_classes.is_empty()
    {
        findings.retain(|finding| match &finding.diagnostic {
            LintDiagnostic::InvalidMarkerUse { class, .. } => config.reports_class(class),
            _ => true,
        });
    }
    true
}

/// Validates suppression comments, detects unused disables after complete analysis, and
/// filters findings. Findings about the marker classes that `invalid-marker-use`
/// ignores are removed first, so they neither report nor count as suppressed.
/// `complete` must be false when analysis stopped before all producers ran. Only
/// enabled rules or rules with reported findings participate in unused checks.
/// Validation findings do not make their own suppression directives count as used.
///
/// `directives` must be the directives of `source` in source order, and `surface` is
/// the kind of `source`. A rule name that does not check `surface` is invalid, and only
/// rules that check it participate in unused checks.
pub fn finish_lint_findings(
    source: &SourceFile,
    surface: LintSurface,
    directives: &[LintDirective],
    settings: &LintSettings,
    findings: &mut Vec<LintFinding>,
    complete: bool,
) {
    if !remove_unreported_findings(settings, findings) {
        return;
    }
    let mut additional = Vec::new();
    if settings.enabled_for_detection(LintKind::InvalidLintSuppression) {
        for directive in directives {
            if directive.kind == LintDirectiveKind::Invalid {
                additional.push(LintFinding::new(
                    LintDiagnostic::InvalidLintSuppression {
                        detail: format_args!(
                            "unknown directive `{}`",
                            &source.text()[directive.offset..directive.end]
                        )
                        .to_lean_str(),
                    },
                    directive_span(source, directive.offset, directive.end),
                ));
            }
            for (name, offset) in &directive.names {
                let detail = match LintKind::from_name(name) {
                    _ if name == "*" => continue,
                    None => format_args!("unknown rule `{name}`").to_lean_str(),
                    Some(kind) if !kind.checks(surface) => {
                        format_args!("rule `{name}` does not check {}", surface_noun(surface))
                            .to_lean_str()
                    }
                    Some(_) => continue,
                };
                additional.push(LintFinding::new(
                    LintDiagnostic::InvalidLintSuppression { detail },
                    directive_span(source, *offset, *offset + name.len()),
                ));
            }
        }
    }
    if complete && settings.enabled_for_detection(LintKind::UnusedLintSuppression) {
        let mut used = BTreeSet::new();
        let mut executed = BTreeSet::new();
        for finding in findings.iter() {
            if settings.reporting_severity(finding.kind()) == LintSeverity::Off {
                continue;
            }
            executed.insert(finding.kind());
            if let Some(index) = suppressing_directive(source, finding, directives) {
                used.insert((index, finding.kind()));
            }
        }
        for (index, directive) in directives.iter().enumerate() {
            if matches!(
                directive.kind,
                LintDirectiveKind::Enable | LintDirectiveKind::Invalid
            ) {
                continue;
            }
            let eligible = |kind: LintKind| {
                !matches!(
                    kind,
                    LintKind::InvalidLintSuppression
                        | LintKind::UnusedLintSuppression
                        | LintKind::PreferSortedUtilities
                ) && kind.checks(surface)
                    && settings.reporting_severity(kind) != LintSeverity::Off
                    && (settings.enabled_for_detection(kind) || executed.contains(&kind))
            };
            match &directive.rules {
                LintDirectiveRules::All
                    if LintKind::ALL.iter().copied().any(eligible)
                        && !used.iter().any(|(used_index, _)| *used_index == index) =>
                {
                    additional.push(LintFinding::new(
                        LintDiagnostic::UnusedLintSuppression { rule: "*".into() },
                        directive_span(source, directive.offset, directive.end),
                    ));
                }
                LintDirectiveRules::All => {}
                LintDirectiveRules::Named(rules) => {
                    for kind in rules.iter().copied().filter(|kind| eligible(*kind)) {
                        if used.contains(&(index, kind)) {
                            continue;
                        }
                        let offset = directive
                            .names
                            .iter()
                            .find(|(name, _)| name == kind.name())
                            .map_or(directive.offset, |(_, offset)| *offset);
                        additional.push(LintFinding::new(
                            LintDiagnostic::UnusedLintSuppression {
                                rule: kind.name().into(),
                            },
                            directive_span(source, offset, offset + kind.name().len()),
                        ));
                    }
                }
            }
        }
    }
    findings.extend(additional);
    filter_lint_findings(source, directives, findings);
}

/// Returns the plural noun for sources of `surface`, as used in finding details.
const fn surface_noun(surface: LintSurface) -> &'static str {
    match surface {
        LintSurface::Script => "scripts",
        LintSurface::Stylesheet => "stylesheets",
    }
}

fn directive_span(source: &SourceFile, start: usize, end: usize) -> Span {
    match (u32::try_from(start), u32::try_from(end)) {
        (Ok(start), Ok(end)) => Span::new(source.id(), start, end),
        _ => source.full_span(),
    }
}

/// Parses the `swlint-*` directives in the comments of `source`, in source order.
///
/// Each range in `comments` is the body of one comment without its delimiters, in UTF-8
/// byte offsets of `source`. The caller obtains the ranges from a parser for the
/// language of `source`, so text in strings, templates, regular expressions, and markup
/// is never a directive. Ranges must lie on character boundaries of `source` and must
/// not overlap, and they may be in any order.
///
/// A rule list ends at `--`, which lets the directive carry a trailing human
/// explanation; rules are separated by whitespace or commas. A comment yields at most
/// one directive, and only from its first `swlint-` occurrence. Unrecognized keywords
/// are retained as invalid directives without changing reporting. There is no
/// requirement that the keyword start the comment: prose containing a recognized
/// directive spelling is parsed too.
pub fn lint_directives_in_comments(
    source: &str,
    comments: impl IntoIterator<Item = Range<usize>>,
) -> Vec<LintDirective> {
    let mut comments = comments.into_iter().collect::<Vec<_>>();
    comments.sort_by_key(|comment| comment.start);
    comments
        .into_iter()
        .filter_map(|comment| {
            let offset = comment.start;
            let body = &source[comment.clone()];
            let relative = body.find("swlint-")?;
            let directive_offset = offset + relative;
            let rest = &body[relative + "swlint-".len()..];
            let keyword_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let keyword = &rest[..keyword_end];
            let kind = match keyword {
                "disable-next-line" => LintDirectiveKind::DisableNextLine,
                "disable-line" => LintDirectiveKind::DisableLine,
                "disable" => LintDirectiveKind::Disable,
                "enable" => LintDirectiveKind::Enable,
                _ => LintDirectiveKind::Invalid,
            };
            let end = directive_offset + "swlint-".len() + keyword.len();
            let rest = &rest[keyword_end..];
            let mut names = Vec::new();
            let mut cursor = 0;
            for name in rest
                .split(|character: char| character.is_whitespace() || character == ',')
                .filter(|name| !name.is_empty())
            {
                if name == "--" {
                    break;
                }
                let relative = rest[cursor..].find(name)? + cursor;
                names.push((name.to_lean_str(), end + relative));
                cursor = relative + name.len();
            }
            let rules = if kind == LintDirectiveKind::Invalid {
                names.clear();
                LintDirectiveRules::Named(BTreeSet::new())
            } else if names.is_empty() || names.iter().any(|(name, _)| name == "*") {
                LintDirectiveRules::All
            } else {
                LintDirectiveRules::Named(
                    names
                        .iter()
                        .filter_map(|(name, _)| LintKind::from_name(name))
                        .collect(),
                )
            };
            let line = source[..directive_offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count();
            Some(LintDirective {
                kind,
                offset: directive_offset,
                line,
                rules,
                end,
                names,
                comment,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use stillwind_css::css_comment_ranges;
    use stillwind_source::{FileId, Span};

    use super::super::LintConfig;
    use super::*;
    use crate::{Label, LabelScope, LintDiagnostic};

    /// Returns the directives of `text`, whose `//` and `/* */` comments are all real
    /// comments.
    fn script_directives(text: &str) -> Vec<LintDirective> {
        let mut comments = Vec::new();
        let mut cursor = 0;
        while let Some(relative) = text[cursor..].find('/') {
            let opener = cursor + relative;
            let start = opener + 2;
            let end = if text[opener..].starts_with("//") {
                text[start..]
                    .find('\n')
                    .map_or(text.len(), |end| start + end)
            } else if text[opener..].starts_with("/*") {
                start + text[start..].find("*/").unwrap()
            } else {
                cursor = opener + 1;
                continue;
            };
            comments.push(start..end);
            cursor = end;
        }
        lint_directives_in_comments(text, comments)
    }

    #[test]
    fn directives_retain_rule_and_line_information() {
        let text = "x\n// swlint-disable-next-line missing-ref-attachment,ineffective-anchor-scope -- reason\nx\n/* swlint-enable missing-ref-attachment */";
        let block = text.find("/*").unwrap();
        // Comments may arrive out of source order.
        let directives = lint_directives_in_comments(
            text,
            [
                block + 2..text.len() - 2,
                text.find("//").unwrap() + 2..block - 3,
            ],
        );
        assert_eq!(directives.len(), 2);
        assert_eq!(directives[0].kind, LintDirectiveKind::DisableNextLine);
        assert_eq!(directives[0].line, 1);
        assert_eq!(
            directives[0].rules,
            LintDirectiveRules::Named(
                [
                    LintKind::MissingRefAttachment,
                    LintKind::IneffectiveAnchorScope
                ]
                .into()
            )
        );
        assert_eq!(directives[1].kind, LintDirectiveKind::Enable);
    }

    #[test]
    fn unknown_rule_lists_do_not_select_all_rules() {
        for directive in ["disable", "disable-line", "disable-next-line"] {
            let text = if directive == "disable-line" {
                format_args!("x // swlint-{directive} unknown-rule").to_lean_string()
            } else {
                format_args!("// swlint-{directive} unknown-rule\nx").to_lean_string()
            };
            let source = SourceFile::new(FileId(1), None, None, text.as_str()).unwrap();
            let start = u32::try_from(source.text().find('x').unwrap()).unwrap();
            let span = Span::new(FileId(1), start, start + 1);
            let mut findings = vec![LintFinding::new(
                LintDiagnostic::MissingRefAttachment { name: "x".into() },
                span,
            )];
            let directives = script_directives(source.text());
            assert_eq!(
                directives[0].rules,
                LintDirectiveRules::Named(BTreeSet::new())
            );
            filter_lint_findings(&source, &directives, &mut findings);
            assert_eq!(findings.len(), 1);
        }
        let directives = script_directives(
            "// swlint-disable unknown-rule,missing-ref-attachment\n// swlint-enable unknown-rule,*",
        );
        assert_eq!(
            directives[0].rules,
            LintDirectiveRules::Named([LintKind::MissingRefAttachment].into())
        );
        assert_eq!(directives[1].rules, LintDirectiveRules::All);
    }

    #[test]
    fn disable_next_line_does_not_suppress_other_rules_or_later_lines() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "// swlint-disable-next-line missing-ref-attachment\nx\ny",
        )
        .unwrap();
        let x = u32::try_from(source.text().find("\nx").unwrap() + 1).unwrap();
        let mut findings = vec![
            LintFinding::new(
                LintDiagnostic::MissingRefAttachment { name: "x".into() },
                Span::new(FileId(1), x, x + 1),
            ),
            LintFinding::new(
                LintDiagnostic::IneffectiveAnchorScope { detail: "x".into() },
                Span::new(FileId(1), x, x + 1),
            ),
            LintFinding::new(
                LintDiagnostic::MissingRefAttachment { name: "x".into() },
                Span::new(FileId(1), x + 2, x + 3),
            ),
        ];
        filter_lint_findings(&source, &script_directives(source.text()), &mut findings);
        assert_eq!(
            findings.iter().map(LintFinding::code).collect::<Vec<_>>(),
            vec!["ineffective-anchor-scope", "missing-ref-attachment"]
        );
    }

    #[test]
    fn disable_enable_and_same_line_directives_have_bounded_effects() {
        for (text, expected) in [
            (
                "// swlint-disable missing-ref-attachment\nx\n// swlint-enable missing-ref-attachment\ny",
                vec![
                    "ineffective-anchor-scope",
                    "missing-ref-attachment",
                    "ineffective-anchor-scope",
                ],
            ),
            (
                "// swlint-disable *\nx\n// swlint-enable\ny",
                vec!["missing-ref-attachment", "ineffective-anchor-scope"],
            ),
            (
                "x // swlint-disable-line missing-ref-attachment\ny",
                vec![
                    "ineffective-anchor-scope",
                    "missing-ref-attachment",
                    "ineffective-anchor-scope",
                ],
            ),
        ] {
            let source = SourceFile::new(FileId(1), None, None, text).unwrap();
            let mut findings = ['x', 'y']
                .into_iter()
                .flat_map(|c| {
                    let start = u32::try_from(text.find(c).unwrap()).unwrap();
                    ["missing-ref-attachment", "ineffective-anchor-scope"].map(|rule| {
                        LintFinding::new(
                            if rule == "missing-ref-attachment" {
                                LintDiagnostic::MissingRefAttachment { name: "x".into() }
                            } else {
                                LintDiagnostic::IneffectiveAnchorScope { detail: "x".into() }
                            },
                            Span::new(FileId(1), start, start + 1),
                        )
                    })
                })
                .collect();
            filter_lint_findings(&source, &script_directives(source.text()), &mut findings);
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
            "// swlint-disable-next-line missing-ref-attachment\nx\ny",
        )
        .unwrap();
        let x = u32::try_from(source.text().find("\nx").unwrap() + 1).unwrap();
        let primary = Span::new(source.id(), x, x + 1);
        let secondary = Span::new(source.id(), x + 2, x + 3);
        let mut suppressed = LintFinding::new(
            LintDiagnostic::MissingRefAttachment { name: "a".into() },
            primary,
        );
        suppressed.labels.push(Label {
            scope: LabelScope::Range,
            span: secondary,
            message: None,
        });
        let retained = LintFinding::new(
            LintDiagnostic::IneffectiveAnchorScope { detail: "a".into() },
            primary,
        );
        let mut findings = vec![suppressed, retained.clone()];
        filter_lint_findings(&source, &script_directives(source.text()), &mut findings);
        assert_eq!(findings, vec![retained]);
    }

    #[test]
    fn validation_reports_exact_unknown_names_and_malformed_keywords() {
        let source = SourceFile::new(FileId(1), None, None,
            "// swlint-disable-line *,typo -- allowed explanation\n// swlint-disabled no-important\nx").unwrap();
        let mut findings = Vec::new();
        finish_lint_findings(
            &source,
            LintSurface::Script,
            &script_directives(source.text()),
            &LintSettings::with_defaults(),
            &mut findings,
            true,
        );
        let text = findings
            .iter()
            .map(|finding| source.slice(finding.labels[0].span).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(text, ["typo", "swlint-disabled"]);
    }

    #[test]
    fn named_enable_restores_a_rule_under_a_wildcard_disable() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "// swlint-disable\n// swlint-enable no-important\nx",
        )
        .unwrap();
        let offset = u32::try_from(source.text().rfind('x').unwrap()).unwrap();
        let span = Span::new(source.id(), offset, offset + 1);
        let mut findings = vec![
            LintFinding::new(
                LintDiagnostic::ImportantDeclaration {
                    property: "color".into(),
                },
                span,
            ),
            LintFinding::new(LintDiagnostic::ClassProperty, span),
        ];
        filter_lint_findings(&source, &script_directives(source.text()), &mut findings);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind(), LintKind::NoImportant);
    }

    #[test]
    fn unused_checks_report_each_unused_rule_and_do_not_mask_themselves() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "// swlint-disable-next-line no-important,no-class-property\nx\n// swlint-disable\ny",
        )
        .unwrap();
        let mut settings = LintSettings::with_defaults();
        settings.insert(LintConfig::new(LintKind::NoImportant, LintSeverity::Warn));
        settings.insert(LintConfig::NoClassProperty(LintSeverity::Warn));
        settings.insert(LintConfig::UnusedLintSuppression(LintSeverity::Warn));
        let offset = u32::try_from(source.text().find("\nx").unwrap() + 1).unwrap();
        let mut findings = vec![LintFinding::new(
            LintDiagnostic::ImportantDeclaration {
                property: "color".into(),
            },
            Span::new(source.id(), offset, offset + 1),
        )];
        let directives = script_directives(source.text());
        finish_lint_findings(
            &source,
            LintSurface::Script,
            &directives,
            &settings,
            &mut findings,
            true,
        );
        let rules = findings
            .iter()
            .filter_map(|finding| match &finding.diagnostic {
                LintDiagnostic::UnusedLintSuppression { rule } => Some(rule.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(rules, ["no-class-property", "*"]);
        let mut partial = Vec::new();
        finish_lint_findings(
            &source,
            LintSurface::Script,
            &directives,
            &settings,
            &mut partial,
            false,
        );
        assert_eq!(partial, [] as [LintFinding; 0]);
    }

    #[test]
    fn stylesheets_have_only_block_comments_and_quoted_strings() {
        let css = r#".a { background: url(//x/swlint-disable); content: '/* swlint-enable */'; }
                   .b\/* swlint-disable */ { content: "\"/* swlint-enable */"; }
                   /* swlint-disable-next-line unknown-utility -- reason */
.b{}"#;
        let directives = lint_directives_in_comments(css, css_comment_ranges(css));
        assert_eq!(directives.len(), 1);
        assert_eq!(directives[0].kind, LintDirectiveKind::DisableNextLine);
        assert_eq!(directives[0].line, 2);
        let directives = lint_directives_in_comments(
            "`/* swlint-disable */`",
            css_comment_ranges("`/* swlint-disable */`"),
        );
        assert_eq!(directives.len(), 1);
    }

    #[test]
    fn stylesheet_directives_reject_rules_that_do_not_check_stylesheets() {
        let text = "/* swlint-disable missing-ref-attachment unknown-utility */
.a{}";
        let source = SourceFile::new(FileId(1), None, None, text).unwrap();
        let mut settings = LintSettings::with_defaults();
        settings.insert(LintConfig::MissingRefAttachment(LintSeverity::Warn));
        settings.insert(LintConfig::UnusedLintSuppression(LintSeverity::Warn));
        let mut findings = Vec::new();
        finish_lint_findings(
            &source,
            LintSurface::Stylesheet,
            &lint_directives_in_comments(source.text(), css_comment_ranges(source.text())),
            &settings,
            &mut findings,
            true,
        );
        let details = findings
            .iter()
            .map(|finding| match &finding.diagnostic {
                LintDiagnostic::InvalidLintSuppression { detail } => detail.to_lean_string(),
                LintDiagnostic::UnusedLintSuppression { rule } => {
                    format_args!("unused {rule}").to_lean_string()
                }
                diagnostic => panic!("{diagnostic:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            details,
            [
                "rule `missing-ref-attachment` does not check stylesheets",
                "unused unknown-utility"
            ]
        );
    }
}
