//! Validates producer diagnostics and resolves one batch under its lint policy.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_source::{
    Fingerprint, SourceError, SourceFile, SourceRegistry, SourceRegistryError, SourceUri, Span,
};

use super::{
    DiagnosticBatch, DiagnosticCause, DiagnosticSeverity, LocatedDiagnostic, LocatedLabel,
    ResolvedAction, ResolvedDiagnostic, ResolvedEdit, ResolvedLabel, ResolvedLocation,
};
use crate::lint::suppressing_directive;
use crate::{
    CompileError, FixAction, LabelScope, LintDirective, LintFinding, LintSettings, LintSeverity,
    SourceRange, collect_lint_directives,
};

/// A malformed diagnostic or an edit that no longer addresses its source revision.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    /// A document label does not cover the full source revision.
    #[error("a document label must cover its complete source")]
    InvalidDocumentLabel,
    /// A source has no host-supplied document identity.
    #[error("source {0} has no URI")]
    MissingUri(String),
    /// A label or edit has invalid byte coordinates.
    #[error("{0}")]
    Source(#[from] SourceError),
    /// A proposed edit refers to another revision of its document.
    #[error("stale edit for {0}")]
    StaleEdit(String),
    /// An action has no edits or contains overlapping edits.
    #[error("invalid fix action: {0}")]
    InvalidAction(&'static str),
}

/// Failure resolving internal labels or validating a completed diagnostic batch.
#[derive(Debug, thiserror::Error)]
pub enum AggregateError {
    /// A span names an unregistered source or invalid byte range.
    #[error("{0}")]
    Source(#[from] SourceRegistryError),
    /// Resolved diagnostics disagree about revisions or edits.
    #[error("{0}")]
    Report(#[from] ReportError),
}

/// Temporary reporter for a single analysis or update.
pub struct Reporter {
    lint_settings: LintSettings,
    revisions: BTreeMap<SourceUri, String>,
    diagnostics: Vec<ResolvedDiagnostic>,
    sources: BTreeMap<usize, ReportSource>,
}

impl Reporter {
    /// Resolves compilation errors and lint findings from one analysis in producer order.
    ///
    /// # Errors
    ///
    /// Returns an error for unregistered spans, conflicting revisions, or invalid actions.
    pub fn collect(
        errors: &[CompileError],
        findings: &[LintFinding],
        sources: &SourceRegistry,
        lint_settings: LintSettings,
    ) -> Result<DiagnosticBatch, AggregateError> {
        let mut reporter = Self::new(lint_settings);
        for error in errors {
            reporter.report(&LocatedDiagnostic::from_compile(error, sources)?)?;
        }
        for finding in findings {
            reporter.report(&LocatedDiagnostic::from_lint(finding, sources)?)?;
        }
        Ok(reporter.finish())
    }

    /// Starts a report with the lint policy in effect for this analysis.
    pub fn new(lint_settings: LintSettings) -> Self {
        Self {
            lint_settings,
            revisions: BTreeMap::new(),
            diagnostics: Vec::new(),
            sources: BTreeMap::new(),
        }
    }

    /// Validates and records one completed error or lint finding.
    ///
    /// # Errors
    ///
    /// Returns an error for conflicting source revisions or invalid edits.
    pub fn report(&mut self, diagnostic: &LocatedDiagnostic) -> Result<(), ReportError> {
        let primary_source = diagnostic.primary.range.source();
        let mut revisions = self.revisions.clone();
        self.check_revision(&mut revisions, &diagnostic.primary.range)?;
        for label in &diagnostic.related {
            self.check_revision(&mut revisions, &label.range)?;
        }
        for action in &diagnostic.actions {
            for edit in &action.edits {
                self.check_revision(&mut revisions, &edit.range)?;
            }
        }
        let primary = self.resolve_label(&diagnostic.primary)?;
        let related = diagnostic
            .related
            .iter()
            .map(|label| self.resolve_label(label))
            .collect::<Result<Vec<_>, _>>()?;
        let actions = diagnostic
            .actions
            .iter()
            .map(|action| self.resolve_action(action))
            .collect::<Result<Vec<_>, _>>()?;
        let severity = if let DiagnosticCause::Lint(lint) = &diagnostic.cause {
            let level = self.lint_settings.reporting_severity(lint.kind());
            let finding = LintFinding::new(
                lint.clone(),
                Span::new(
                    primary_source.id(),
                    diagnostic.primary.range.start(),
                    diagnostic.primary.range.end(),
                ),
            );
            let directives = &self.source_info(&diagnostic.primary.range).directives;
            if level == LintSeverity::Off
                || suppressing_directive(primary_source, &finding, directives).is_some()
            {
                return Ok(());
            }
            if level == LintSeverity::Deny {
                DiagnosticSeverity::Error
            } else {
                DiagnosticSeverity::Warning
            }
        } else {
            DiagnosticSeverity::Error
        };
        let resolved = ResolvedDiagnostic {
            code: diagnostic.cause.code().to_owned(),
            message: diagnostic.cause.message(),
            severity,
            primary,
            related,
            notes: diagnostic.notes.clone(),
            actions,
        };
        self.revisions = revisions;
        self.diagnostics.push(resolved);
        Ok(())
    }

    /// Returns the completed report batch.
    pub fn finish(self) -> DiagnosticBatch {
        DiagnosticBatch {
            diagnostics: self.diagnostics,
        }
    }

    fn check_revision(
        &mut self,
        revisions: &mut BTreeMap<SourceUri, String>,
        range: &SourceRange,
    ) -> Result<(), ReportError> {
        let source = range.source();
        let uri = source
            .uri()
            .ok_or_else(|| ReportError::MissingUri(format!("{:?}", source.id())))?;
        let revision = self.source_info(range).revision.clone();
        if let Some(previous) = revisions.insert(uri.clone(), revision.clone())
            && previous != revision
        {
            return Err(ReportError::StaleEdit(uri.as_str().to_owned()));
        }
        Ok(())
    }

    fn resolve_label(&mut self, label: &LocatedLabel) -> Result<ResolvedLabel, ReportError> {
        if label.scope == LabelScope::Document
            && (label.range.start() != 0
                || label.range.end() != label.range.source().full_span().end.0)
        {
            return Err(ReportError::InvalidDocumentLabel);
        }
        Ok(ResolvedLabel {
            scope: label.scope,
            location: self.source_info(&label.range).location(&label.range)?,
            message: label.message.clone(),
        })
    }

    fn resolve_action(&mut self, action: &FixAction) -> Result<ResolvedAction, ReportError> {
        if action.edits.is_empty() {
            return Err(ReportError::InvalidAction("an action needs an edit"));
        }
        let mut sorted = action.edits.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| {
            let x = &a.range;
            let y = &b.range;
            (x.source().uri(), x.start(), x.end()).cmp(&(y.source().uri(), y.start(), y.end()))
        });
        for pair in sorted.windows(2) {
            let left = &pair[0].range;
            let right = &pair[1].range;
            if left.source().uri() == right.source().uri()
                && (left.end() > right.start() || left.start() == right.start())
            {
                return Err(ReportError::InvalidAction("edits overlap"));
            }
        }
        let edits = action
            .edits
            .iter()
            .map(|edit| {
                Ok(ResolvedEdit {
                    location: self.source_info(&edit.range).location(&edit.range)?,
                    replacement: edit.replacement.clone(),
                })
            })
            .collect::<Result<Vec<_>, ReportError>>()?;
        Ok(ResolvedAction {
            title: action.title.clone(),
            edits,
        })
    }
    fn source_info(&mut self, range: &SourceRange) -> &mut ReportSource {
        let source = range.shared_source();
        self.sources
            .entry(Arc::as_ptr(source) as usize)
            .or_insert_with(|| ReportSource::new(Arc::clone(source)))
    }
}

struct ReportSource {
    // Retaining the source keeps its pointer key valid until reporting finishes.
    source: Arc<SourceFile>,
    revision: String,
    directives: Vec<LintDirective>,
    locations: BTreeMap<(u32, u32), ResolvedLocation>,
}

impl ReportSource {
    fn new(source: Arc<SourceFile>) -> Self {
        Self {
            revision: Fingerprint::of(source.text()).0,
            directives: collect_lint_directives(source.text()),
            source,
            locations: BTreeMap::new(),
        }
    }

    fn location(&mut self, range: &SourceRange) -> Result<ResolvedLocation, ReportError> {
        let key = (range.start(), range.end());
        if let Some(location) = self.locations.get(&key) {
            return Ok(location.clone());
        }
        let source = &self.source;
        let span = Span::new(source.id(), range.start(), range.end());
        source.slice(span)?;
        let uri = source
            .uri()
            .ok_or_else(|| ReportError::MissingUri(format!("{:?}", source.id())))?;
        let location = ResolvedLocation {
            uri: uri.clone(),
            revision: self.revision.clone(),
            start: span.start.0,
            end: span.end.0,
            start_utf16: source.utf16_offset(span.start)?,
            end_utf16: source.utf16_offset(span.end)?,
            start_position: source.position(span.start)?,
            end_position: source.position(span.end)?,
        };
        self.locations.insert(key, location.clone());
        Ok(location)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_source::{Position, SourceFile, SourceRegistry, SourceRegistryError};

    use super::*;
    use crate::{CompileDiagnostic, FixAction, LintConfig, LintDiagnostic, LintKind, SourceEdit};

    fn document(sources: &mut SourceRegistry, uri: &str, text: &str) -> Arc<SourceFile> {
        sources
            .register(Some(SourceUri::parse(uri).unwrap()), None, text)
            .unwrap()
    }

    fn range(sources: &SourceRegistry, source: &SourceFile, start: u32, end: u32) -> SourceRange {
        SourceRange::from_span(sources, Span::new(source.id(), start, end)).unwrap()
    }

    #[test]
    fn document_reports_preserve_revision_without_claiming_a_position() {
        let mut sources = SourceRegistry::default();
        let source = document(&mut sources, "untitled:component", "日本語😀\r\ntext");
        let error = CompileError::for_document(
            CompileDiagnostic::ProjectionPreparation {
                detail: "ambiguous generated copies".into(),
            },
            &source,
        );
        let batch = Reporter::collect(&[error], &[], &sources, LintSettings::default()).unwrap();
        let report = &batch.diagnostics[0];
        assert_eq!(report.primary.scope, LabelScope::Document);
        assert_eq!(
            report.primary.location.revision,
            Fingerprint::of(source.text()).0
        );
        assert_eq!(report.primary.location.end_utf16, 11);
        assert_eq!(
            report.display_text(),
            "untitled:component: SW10008: cannot prepare the component projection: ambiguous generated copies"
        );
        let mut invalid = LocatedDiagnostic::new(
            DiagnosticCause::Compile(CompileDiagnostic::JavaScriptParse),
            range(&sources, &source, 0, 3),
        );
        invalid.primary.scope = LabelScope::Document;
        assert!(matches!(
            Reporter::new(LintSettings::default()).report(&invalid),
            Err(ReportError::InvalidDocumentLabel)
        ));
    }

    #[test]
    fn report_resolves_unicode_crlf_and_cross_file_fixes() {
        let mut sources = SourceRegistry::default();
        let first = document(&mut sources, "untitled:main", "a😀\r\nβ");
        let second = document(&mut sources, "untitled:other", "xy");
        let mut finding = LocatedDiagnostic::new(
            DiagnosticCause::Lint(LintDiagnostic::MissingRefAttachment { name: "x".into() }),
            range(&sources, &first, 7, 9),
        );
        finding.related.push(LocatedLabel {
            scope: LabelScope::Range,
            range: range(&sources, &second, 0, 1),
            message: Some("other".into()),
        });
        finding.actions = vec![
            FixAction {
                title: "replace both".into(),
                edits: vec![
                    SourceEdit {
                        range: range(&sources, &first, 7, 9),
                        replacement: "z".into(),
                    },
                    SourceEdit {
                        range: range(&sources, &second, 0, 1),
                        replacement: "q".into(),
                    },
                ],
            },
            FixAction {
                title: "alternative".into(),
                edits: vec![SourceEdit {
                    range: range(&sources, &first, 7, 9),
                    replacement: "w".into(),
                }],
            },
        ];
        let mut reporter = Reporter::new(LintSettings::with_defaults());
        reporter.report(&finding).unwrap();
        let batch = reporter.finish();
        let diagnostic = &batch.diagnostics[0];
        assert_eq!(
            diagnostic.primary.location.start_position,
            Position {
                line: 1,
                utf16_column: 0
            }
        );
        assert_eq!(
            diagnostic.primary.location.end_position,
            Position {
                line: 1,
                utf16_column: 1
            }
        );
        assert_eq!(
            diagnostic.related[0].location.uri.as_str(),
            "untitled:other"
        );
        assert_eq!(diagnostic.actions.len(), 2);
        assert_eq!(diagnostic.actions[0].edits.len(), 2);
        let current = diagnostic.primary.location.revision.clone();
        assert!(
            diagnostic.actions[1]
                .verify_current(|_| Some(current.clone()))
                .is_ok()
        );
        assert!(
            diagnostic.actions[1]
                .verify_current(|_| Some("old".into()))
                .is_err()
        );
    }

    #[test]
    fn suppression_off_and_deny_apply_in_the_reporter() {
        let mut sources = SourceRegistry::default();
        let source = document(
            &mut sources,
            "untitled:lint",
            "// swlint-disable-next-line require-ref-attachment\nx",
        );
        let start = u32::try_from(source.text().len() - 1).unwrap();
        let finding = || {
            LocatedDiagnostic::new(
                DiagnosticCause::Lint(LintDiagnostic::MissingRefAttachment { name: "x".into() }),
                range(&sources, &source, start, start + 1),
            )
        };
        let kind = LintKind::RequireRefAttachment;
        let mut reporter = Reporter::new(LintSettings::with_defaults());
        reporter.report(&finding()).unwrap();
        assert_eq!(reporter.finish().diagnostics, [] as [ResolvedDiagnostic; 0]);
        let plain = document(&mut sources, "untitled:plain", "x");
        let finding = || {
            LocatedDiagnostic::new(
                DiagnosticCause::Lint(LintDiagnostic::MissingRefAttachment { name: "x".into() }),
                range(&sources, &plain, 0, 1),
            )
        };
        let mut reporter = Reporter::new(
            [LintConfig::new(kind, LintSeverity::Off)]
                .into_iter()
                .collect(),
        );
        reporter.report(&finding()).unwrap();
        assert_eq!(reporter.finish().diagnostics, [] as [ResolvedDiagnostic; 0]);
        let mut reporter = Reporter::new(
            [LintConfig::new(kind, LintSeverity::Deny)]
                .into_iter()
                .collect(),
        );
        reporter.report(&finding()).unwrap();
        assert_eq!(
            reporter.finish().diagnostics[0].severity,
            DiagnosticSeverity::Error
        );
    }

    #[test]
    fn overlapping_edits_and_conflicting_snapshots_are_rejected() {
        let mut sources = SourceRegistry::default();
        let source = document(&mut sources, "untitled:same", "abc");
        let changed = document(&mut sources, "untitled:same", "xyz");
        let mut finding = LocatedDiagnostic::new(
            DiagnosticCause::Lint(LintDiagnostic::MissingRefAttachment { name: "x".into() }),
            range(&sources, &source, 0, 1),
        );
        finding.actions.push(FixAction {
            title: "overlap".into(),
            edits: vec![
                SourceEdit {
                    range: range(&sources, &source, 0, 2),
                    replacement: "a".into(),
                },
                SourceEdit {
                    range: range(&sources, &source, 1, 3),
                    replacement: "b".into(),
                },
            ],
        });
        let mut reporter = Reporter::new(LintSettings::default());
        assert!(matches!(
            reporter.report(&finding),
            Err(ReportError::InvalidAction(_))
        ));
        let mut finding = LocatedDiagnostic::new(
            DiagnosticCause::Lint(LintDiagnostic::MissingRefAttachment { name: "x".into() }),
            range(&sources, &source, 0, 1),
        );
        finding.actions.push(FixAction {
            title: "stale".into(),
            edits: vec![SourceEdit {
                range: range(&sources, &changed, 0, 1),
                replacement: "a".into(),
            }],
        });
        assert!(matches!(
            reporter.report(&finding),
            Err(ReportError::StaleEdit(_))
        ));
    }

    #[test]
    fn independent_source_scopes_may_reuse_file_ids() {
        let mut first_project = SourceRegistry::default();
        let source = document(&mut first_project, "untitled:first", "abc");
        let mut other_project = SourceRegistry::default();
        let other = document(&mut other_project, "untitled:other", "abc");
        assert_eq!(source.id(), other.id());
        let span = Span::new(source.id(), 0, 1);
        assert_eq!(
            SourceRange::from_span(&first_project, span)
                .unwrap()
                .source()
                .uri()
                .unwrap()
                .as_str(),
            "untitled:first"
        );
        assert_eq!(
            SourceRange::from_span(&other_project, span)
                .unwrap()
                .source()
                .uri()
                .unwrap()
                .as_str(),
            "untitled:other"
        );
        let mut reporter = Reporter::new(LintSettings::default());
        for (registry, source) in [(&first_project, &source), (&other_project, &other)] {
            reporter
                .report(&LocatedDiagnostic::new(
                    DiagnosticCause::Compile(CompileDiagnostic::JavaScriptParse),
                    range(registry, source, 0, 1),
                ))
                .unwrap();
        }
        let batch = reporter.finish();
        assert_eq!(
            batch
                .diagnostics
                .iter()
                .map(|report| report.primary.location.uri.as_str())
                .collect::<Vec<_>>(),
            ["untitled:first", "untitled:other"]
        );
    }

    #[test]
    fn failed_reports_leave_the_accepted_revision_and_labels_usable() {
        let mut sources = SourceRegistry::default();
        let source = document(&mut sources, "untitled:main", "a😀\r\nβ");
        let changed = document(&mut sources, "untitled:main", "abc");
        let mut accepted = LocatedDiagnostic::new(
            DiagnosticCause::Compile(CompileDiagnostic::JavaScriptParse),
            range(&sources, &source, 1, 5),
        );
        let mut reporter = Reporter::new(LintSettings::default());
        reporter.report(&accepted).unwrap();
        let stale = LocatedDiagnostic::new(
            DiagnosticCause::Compile(CompileDiagnostic::JavaScriptParse),
            range(&sources, &changed, 1, 2),
        );
        assert!(matches!(
            reporter.report(&stale),
            Err(ReportError::StaleEdit(_))
        ));
        accepted.primary.message = Some("second report".into());
        reporter.report(&accepted).unwrap();
        let batch = reporter.finish();
        assert_eq!(batch.diagnostics.len(), 2);
        assert_eq!(
            batch.diagnostics[0].primary.location,
            batch.diagnostics[1].primary.location
        );
        assert_eq!(batch.diagnostics[0].primary.location.end_utf16, 3);
        assert_eq!(batch.diagnostics[0].primary.message, None);
        assert_eq!(
            batch.diagnostics[1].primary.message.as_deref(),
            Some("second report")
        );
    }

    #[test]
    fn internal_error_uses_only_the_issuing_registry() {
        let mut first_project = SourceRegistry::default();
        let source = document(&mut first_project, "untitled:first", "abc");
        let error = CompileError::new(
            CompileDiagnostic::FrameworkConflict,
            Span::new(source.id(), 1, 2),
        );
        let located = LocatedDiagnostic::from_compile(&error, &first_project).unwrap();
        let mut reporter = Reporter::new(LintSettings::default());
        reporter.report(&located).unwrap();
        assert_eq!(
            reporter.finish().diagnostics[0]
                .primary
                .location
                .uri
                .as_str(),
            "untitled:first"
        );
        assert!(
            matches!(LocatedDiagnostic::from_compile(&error, &SourceRegistry::default()), Err(SourceRegistryError::UnknownFile(id)) if id == source.id())
        );
        let generated = first_project.register(None, None, "abc").unwrap();
        let error = CompileError::new(
            CompileDiagnostic::FrameworkConflict,
            Span::new(generated.id(), 0, 1),
        );
        assert!(
            matches!(LocatedDiagnostic::from_compile(&error, &first_project), Err(SourceRegistryError::UnreportableSource(id)) if id == generated.id())
        );
    }
}
