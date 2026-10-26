//! Editor requests that report lint findings and check suppression comments.
//!
//! The requests check that each comment range lies inside the source text before they
//! read suppression directives.

use std::collections::BTreeMap;

use lean_string::LeanStr;
use napi::Error;
use napi_derive::napi;

use stillwind_diagnostics::{
    FixAction, LintDiagnostic, LintDirective, LintFinding, LintKind, LocatedDiagnostic, Reporter,
    SourceEdit, SourceRange, filter_lint_findings, lint_directives_in_comments,
};
use stillwind_source::{FileId, SourceFile, SourceRegistry, SourceUri, Span};

use super::NativeEditorQuery;
use super::query::reason;
use crate::output::{BindingReport, NativeText};
use crate::semantic::NativeLintComment;

/// One source edit that the host found with its own syntax index. The host reports it
/// through the common lint settings.
#[napi(object)]
pub struct NativeEditorLintEdit {
    /// Inclusive start of the edited range, as a UTF-8 byte offset in the original
    /// source.
    pub start: u32,
    /// Exclusive end of the edited range, as a UTF-8 byte offset in the original
    /// source.
    pub end: u32,
    /// The text that replaces the range.
    pub replacement: String,
}

#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
impl NativeEditorQuery {
    /// Returns reports for the sorting edits `edits`, using the lint settings of this
    /// query. Each edit becomes an `unsorted-utilities` finding with a fix. The reports
    /// respect the severity of the rule and the suppression comments of the source.
    ///
    /// `uri` and `text` must identify the original document that the byte ranges of
    /// `edits` refer to. `lint_comments` must come from a scan of `text`.
    ///
    /// # Errors
    ///
    /// Returns an error when `uri` is invalid, when a comment range or an edit range
    /// does not fit `text`, or when the reporter rejects a finding.
    #[napi]
    pub fn report_sort_edits(
        &self,
        uri: String,
        text: String,
        edits: Vec<NativeEditorLintEdit>,
        lint_comments: Vec<NativeLintComment>,
    ) -> napi::Result<Vec<BindingReport>> {
        let directives = lint_directives(&text, &lint_comments)?;
        let mut registry = SourceRegistry::default();
        let uri = SourceUri::parse(uri).map_err(reason)?;
        let source = registry.register(Some(uri), None, text).map_err(reason)?;
        let mut findings = Vec::new();
        for edit in edits {
            let span = Span::new(source.id(), edit.start, edit.end);
            let range = SourceRange::from_span(&registry, span).map_err(reason)?;
            let mut finding = LintFinding::new(LintDiagnostic::UnsortedUtilities, span);
            finding.actions.push(FixAction {
                title: "Sort Stillwind utilities".into(),
                edits: vec![SourceEdit {
                    range,
                    replacement: (edit.replacement).into(),
                }],
            });
            findings.push(finding);
        }
        let mut reporter =
            Reporter::new(self.query.lint_settings()).with_lint_directives(source.id(), directives);
        for finding in &findings {
            let located = LocatedDiagnostic::from_lint(finding, &registry).map_err(reason)?;
            reporter.report(&located).map_err(reason)?;
        }
        Ok(reporter
            .finish()
            .diagnostics
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Returns the severity of each lint rule, keyed by rule name. The severities
    /// combine the compiler defaults, the stylesheet, and the project configuration.
    #[napi(ts_return_type = "Record<string, string>")]
    pub fn lint_settings(&self) -> BTreeMap<LeanStr, NativeText> {
        let settings = self.query.lint_settings();
        LintKind::ALL
            .iter()
            .map(|&kind| {
                (
                    kind.name().into(),
                    settings.reporting_severity(kind).as_str().into(),
                )
            })
            .collect()
    }

    /// Checks whether a suppression comment in `source` disables the editor lint `rule`
    /// at the UTF-8 byte range from `start` to `end`. `rule` must be
    /// `prefer-canonical-utilities` or `prefer-sorted-utilities`. `lint_comments` must
    /// come from a scan of `source`.
    ///
    /// # Errors
    ///
    /// Returns an error when `rule` is not one of those rules, when the range does not
    /// fit `source`, or when a comment range is invalid.
    #[napi]
    #[allow(clippy::unused_self)] // Suppression is exposed through the same editor query transport.
    pub fn lint_suppressed(
        &self,
        source: String,
        start: u32,
        end: u32,
        rule: String,
        lint_comments: Vec<NativeLintComment>,
    ) -> napi::Result<bool> {
        let diagnostic = match rule.as_str() {
            "prefer-canonical-utilities" => LintDiagnostic::NonCanonicalUtility {
                utility: "".into(),
                replacement: "".into(),
            },
            "prefer-sorted-utilities" => LintDiagnostic::UnsortedUtilities,
            _ => return Err(Error::from_reason("unknown editor lint rule")),
        };
        if start > end || usize::try_from(end).unwrap_or(usize::MAX) > source.len() {
            return Err(Error::from_reason("invalid editor lint range"));
        }
        let directives = lint_directives(&source, &lint_comments)?;
        let source = SourceFile::new(FileId(0), None, None, source).map_err(reason)?;
        let mut findings = vec![LintFinding::new(
            diagnostic,
            Span::new(FileId(0), start, end),
        )];
        filter_lint_findings(&source, &directives, &mut findings);
        Ok(findings.is_empty())
    }
}

/// Returns the lint directives in the comments `comments` of `source`.
///
/// # Errors
///
/// Returns an error when a comment range lies outside `source`, or when it starts or
/// ends inside a UTF-8 character.
fn lint_directives(
    source: &str,
    comments: &[NativeLintComment],
) -> napi::Result<Vec<LintDirective>> {
    let ranges = comments
        .iter()
        .map(|comment| {
            let range = comment.start as usize..comment.end as usize;
            source
                .get(range.clone())
                .map(|_| range)
                .ok_or_else(|| Error::from_reason("invalid lint comment range"))
        })
        .collect::<napi::Result<Vec<_>>>()?;
    Ok(lint_directives_in_comments(source, ranges))
}
