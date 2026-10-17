//! Stylesheet lint for one design-system snapshot and its effective options.

use std::ops::Range;
use std::sync::Arc;

use lean_string::{LeanString, ToLeanString};

use stillwind_css::{CssNode, CssNodeKind, parse_structure};
use stillwind_design_system::{DesignSystemSnapshot, ThemeIndex};
use stillwind_diagnostics::{
    FixAction, LintDiagnostic, LintFinding, LintKind, LintSettings, LintSurface, SourceEdit,
    SourceRange, finish_lint_findings,
};
use stillwind_source::{Fingerprint, SourceFile, Span};

use super::application::check_application;
use super::definitions::definitions;
use super::nesting::check_nesting;
use super::shadowing::{addresses, shadowed_definitions};
use super::theme_references::{check_theme_references, check_utility_namespaces};
use super::theme_values::check_theme_values;
use super::variant::check_variant_block;
use super::variant_directive::{check_variant_directives, variant_directive_index};
use crate::{CompilerOptions, DesignSystemQuery};

/// Lint findings in the stylesheets that the host marked for linting.
#[derive(Debug)]
pub struct StylesheetLint {
    /// Findings in source acceptance order, then by the start of their primary label
    /// within a source.
    /// Suppression comments are already applied, and reporters apply severities. Each
    /// primary label and each fix addresses a marked source of the snapshot.
    pub findings: Vec<LintFinding>,
    /// Effective settings after stylesheet and project configuration. Reporters use
    /// them to assign severities.
    pub lint_settings: LintSettings,
}

/// Returns the stylesheet lint of `snapshot` under `options`.
///
/// Only sources that the host marked for lint and supplied with a document URI are
/// checked. The result is computed once for each snapshot and effective options, and
/// later calls share it, including calls through clones of `snapshot`.
pub fn lint_stylesheets(
    snapshot: &DesignSystemSnapshot,
    options: &CompilerOptions,
) -> Arc<StylesheetLint> {
    let options = options.clone().resolved(snapshot);
    let key = Fingerprint::of(format_args!("stylesheet-lint/v1\0{options:?}").to_lean_string());
    snapshot.analysis(key, || analyze(snapshot, options))
}

fn analyze(snapshot: &DesignSystemSnapshot, options: CompilerOptions) -> StylesheetLint {
    let lint_settings = options.lint_settings();
    let mut findings = Vec::new();
    if lint_settings.has_enabled_rules() {
        let query = DesignSystemQuery::new(snapshot.clone(), options);
        let shadowed = if lint_settings.enabled_for_detection(LintKind::DuplicateDefinition) {
            shadowed_definitions(snapshot)
        } else {
            Vec::new()
        };
        let variant_index = variant_directive_index(&query, snapshot.theme(), &lint_settings);
        for source in snapshot
            .sources()
            .sources
            .iter()
            .filter(|source| source.lint && source.source.uri().is_some())
        {
            let mut checker = Checker {
                source: &source.source,
                theme: snapshot.theme(),
                query: &query,
                settings: &lint_settings,
                findings: Vec::new(),
            };
            let nodes = parse_structure(source.source.text());
            checker.visit(&nodes, &[LeanString::new()], false);
            if checker.enabled(LintKind::PreferSortedUtilities)
                && let Ok(edits) = query.sort_stylesheet(source.source.text(), &[])
            {
                for edit in edits {
                    let span = checker.span(edit.range);
                    let mut finding = LintFinding::new(
                        if edit.directives {
                            LintDiagnostic::UnsortedStylesheet
                        } else {
                            LintDiagnostic::UnsortedUtilities
                        },
                        span,
                    );
                    if let Some(range) = checker.edit_range(span) {
                        finding.actions.push(FixAction {
                            title: if edit.directives {
                                "Sort Stillwind CSS inputs"
                            } else {
                                "Sort Stillwind utilities"
                            }
                            .into(),
                            edits: vec![SourceEdit {
                                range,
                                replacement: edit.replacement,
                            }],
                        });
                    }
                    checker.report(finding);
                }
            }
            for definition in definitions(&nodes, source.source.text()) {
                check_nesting(&mut checker, &definition);
                check_utility_namespaces(&mut checker, &definition);
            }
            check_theme_values(&mut checker, &nodes);
            check_theme_references(&mut checker, &nodes, false);
            if let Some(index) = &variant_index {
                check_variant_directives(&mut checker, index, &nodes);
            }
            checker.report_all(
                shadowed
                    .iter()
                    .filter(|finding| addresses(finding, source.source.id()))
                    .cloned(),
            );
            let mut source_findings = checker.findings;
            source_findings
                .sort_by_key(|finding| finding.labels.first().map(|label| label.span.start));
            finish_lint_findings(
                &source.source,
                LintSurface::Stylesheet,
                &lint_settings,
                &mut source_findings,
                true,
            );
            findings.append(&mut source_findings);
        }
    }
    StylesheetLint {
        findings,
        lint_settings,
    }
}

/// Walks one source and collects its findings.
pub(super) struct Checker<'a> {
    /// Source being checked. Every span addresses it.
    pub(super) source: &'a Arc<SourceFile>,
    /// Theme of the snapshot that loaded the source.
    pub(super) theme: &'a ThemeIndex,
    /// Query of the same snapshot, for canonical spellings and candidate order.
    pub(super) query: &'a DesignSystemQuery,
    /// Effective settings. Disabled rules do no detection work.
    pub(super) settings: &'a LintSettings,
    /// Distinct findings in detection order.
    findings: Vec<LintFinding>,
}

impl Checker<'_> {
    /// Returns whether detection for `kind` is enabled.
    pub(super) fn enabled(&self, kind: LintKind) -> bool {
        self.settings.enabled_for_detection(kind)
    }

    /// Returns the span of the byte `range` of the source.
    pub(super) fn span(&self, range: Range<usize>) -> Span {
        let offset = |position: usize| u32::try_from(position).unwrap_or(u32::MAX);
        Span::new(self.source.id(), offset(range.start), offset(range.end))
    }

    /// Returns the editable range of `span`, or `None` when it does not address the
    /// source.
    pub(super) fn edit_range(&self, span: Span) -> Option<SourceRange> {
        SourceRange::from_source(Arc::clone(self.source), span).ok()
    }

    /// Records `finding` unless an equal finding was already recorded. Equal findings
    /// arise when one candidate is checked under several enclosing variant sequences.
    pub(super) fn report(&mut self, finding: LintFinding) {
        if !self.findings.contains(&finding) {
            self.findings.push(finding);
        }
    }

    /// Records each of `findings` as [`Self::report`] does.
    pub(super) fn report_all(&mut self, findings: impl IntoIterator<Item = LintFinding>) {
        for finding in findings {
            self.report(finding);
        }
    }

    /// Checks `nodes` and their descendants.
    ///
    /// `prefixes` holds one variant prefix, such as `print:hover:`, for each sequence
    /// of the enclosing `@variant` blocks. The empty prefix means no enclosing block. A
    /// style rule starts a new element, so its contents start with the empty prefix
    /// again. `in_rule` tells whether a style rule encloses `nodes`. Outside one, a
    /// definition at-rule such as `@utility` or `@variant` is skipped.
    fn visit(&mut self, nodes: &[CssNode], prefixes: &[LeanString], in_rule: bool) {
        for node in nodes {
            match &node.kind {
                CssNodeKind::Apply { candidates } => {
                    check_application(self, candidates, prefixes);
                }
                CssNodeKind::StyleRule { block, .. } => {
                    self.visit(&block.children, &[LeanString::new()], true);
                }
                CssNodeKind::AtRule {
                    name,
                    prelude,
                    block: Some(block),
                } if in_rule && name == "variant" => {
                    let inner = check_variant_block(self, prelude.clone(), prefixes);
                    self.visit(&block.children, &inner, true);
                }
                CssNodeKind::AtRule {
                    name,
                    block: Some(block),
                    ..
                } if in_rule || !is_definition(name) => {
                    self.visit(&block.children, prefixes, in_rule);
                }
                _ => {}
            }
        }
    }
}

/// Returns whether the at-rule `name` defines design-system semantics when it appears
/// outside a style rule.
pub(super) fn is_definition(name: &str) -> bool {
    name.starts_with("stillwind-")
        || matches!(
            name,
            "theme" | "utility" | "custom-variant" | "variant" | "tailwind"
        )
}
