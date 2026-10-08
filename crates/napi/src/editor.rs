//! Native editor requests share compiler resolution, lint policy, and source reporting.

use std::collections::BTreeMap;

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::{CompilerOptions, CompletionInput, DesignSystemQuery};
use stillwind_css::{color_rgba, format_css};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{
    FixAction, LintDiagnostic, LintFinding, LintKind, Reporter, SourceEdit, SourceRange,
    filter_lint_findings,
};
use stillwind_source::{FileId, SourceFile, SourceRegistry, SourceUri, Span};

use crate::options::NativeCanonicalUtilitiesOptions;
use crate::output::BindingReport;

/// A source operation detected by a host's syntax index and reported through the common lint policy.
#[napi(object)]
pub struct NativeEditorLintEdit {
    /// Original inclusive UTF-8 byte boundary.
    pub start: u32,
    /// Original exclusive UTF-8 byte boundary.
    pub end: u32,
    /// Replacement text for this source operation.
    pub replacement: String,
}

/// Candidate operations for one immutable design system and explicitly supplied source buffers.
#[napi]
pub struct NativeEditorQuery {
    query: DesignSystemQuery,
}

impl NativeEditorQuery {
    pub(crate) fn new(snapshot: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        Self {
            query: DesignSystemQuery::new(snapshot, options),
        }
    }
}

#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
impl NativeEditorQuery {
    /// Returns a verified canonical replacement using the effective project lint options, or null.
    #[napi]
    pub fn canonical_candidate(&self, candidate: String) -> napi::Result<Option<String>> {
        self.query
            .canonical_candidate(&candidate)
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Returns the resolved canonical lint policy used by compilation and Reporter.
    #[napi]
    pub fn canonical_options(&self) -> NativeCanonicalUtilitiesOptions {
        let options = self.query.canonical_options();
        NativeCanonicalUtilitiesOptions {
            severity: Some(options.severity.as_str().into()),
            px_to_rem: Some(options.px_to_rem),
            root_font_size: Some(options.root_font_size.get()),
        }
    }

    /// Reports sorting suggestions through the effective typed settings and common Reporter.
    /// URI and text must identify the exact original document addressed by all byte ranges.
    #[napi]
    pub fn report_sort_edits(
        &self,
        uri: String,
        text: String,
        edits: Vec<NativeEditorLintEdit>,
    ) -> napi::Result<Vec<BindingReport>> {
        let mut registry = SourceRegistry::default();
        let uri = SourceUri::parse(uri).map_err(|error| Error::from_reason(error.to_string()))?;
        let source = registry
            .register(Some(uri), None, text)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let mut findings = Vec::new();
        for edit in edits {
            let span = Span::new(source.id(), edit.start, edit.end);
            let range = SourceRange::from_span(&registry, span)
                .map_err(|error| Error::from_reason(error.to_string()))?;
            let mut finding = LintFinding::new(LintDiagnostic::SortUtilities, span);
            finding.actions.push(FixAction {
                title: "Sort Stillwind utilities".into(),
                edits: vec![SourceEdit {
                    range,
                    replacement: edit.replacement,
                }],
            });
            findings.push(finding);
        }
        let batch = Reporter::collect(&[], &findings, &registry, self.query.lint_settings())
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(batch.diagnostics.into_iter().map(Into::into).collect())
    }

    /// Returns effective lint severities after stylesheet and project configuration.
    #[napi]
    pub fn lint_settings(&self) -> BTreeMap<String, String> {
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

    /// Checks source suppression for an editor lint at an original UTF-8 byte range.
    /// Returns an error for unknown rules or invalid source bounds.
    #[napi]
    #[allow(clippy::unused_self)] // Suppression is exposed through the same editor query transport.
    pub fn lint_suppressed(
        &self,
        source: String,
        start: u32,
        end: u32,
        rule: String,
    ) -> napi::Result<bool> {
        let diagnostic = match rule.as_str() {
            "canonical-utilities" => LintDiagnostic::CanonicalUtilities,
            "sort-utilities" => LintDiagnostic::SortUtilities,
            _ => return Err(Error::from_reason("unknown editor lint rule")),
        };
        if start > end || usize::try_from(end).unwrap_or(usize::MAX) > source.len() {
            return Err(Error::from_reason("invalid editor lint range"));
        }
        let source = SourceFile::new(FileId(0), None, None, source)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let mut findings = vec![LintFinding::new(
            diagnostic,
            Span::new(FileId(0), start, end),
        )];
        filter_lint_findings(&source, &mut findings);
        Ok(findings.is_empty())
    }

    /// Renders a valid candidate with the main selector replaced by `&`. Invalid candidates return null.
    #[napi]
    pub fn hover_candidate(&self, candidate: String) -> napi::Result<Option<String>> {
        self.query
            .hover_candidate(&candidate)
            .map(|result| result.map(|result| result.css))
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Returns a complete CSS preview with line breaks and two-space indentation.
    /// Nesting and fallback declarations are retained. Invalid CSS returns an error.
    #[napi]
    #[allow(clippy::unused_self)] // Formatting shares the editor query transport.
    pub fn format_css(&self, css: String) -> napi::Result<String> {
        format_css(&css).map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Completes utilities and variants against a partially typed candidate.
    #[napi]
    pub fn complete_candidate(&self, prefix: String) -> napi::Result<Vec<String>> {
        self.query
            .complete_candidate(CompletionInput { prefix })
            .map(|items| items.into_iter().map(|item| item.label).collect())
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Returns -1, 0, or 1 in compiler order, or null if either candidate is unresolved.
    #[napi]
    pub fn compare_candidates(&self, left: String, right: String) -> napi::Result<Option<i32>> {
        self.query
            .compare_candidates(&left, &right)
            .map(|order| {
                order.map(|order| match order {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                })
            })
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Returns ranks for full candidate spellings in input order, with null for unresolved inputs.
    /// Equal candidates share a rank. Ranks apply only to this batch.
    ///
    /// # Errors
    /// Returns malformed matching utility requirements or a rank that exceeds `u32`.
    #[napi]
    pub fn candidate_ranks(&self, candidates: Vec<String>) -> napi::Result<Vec<Option<u32>>> {
        self.query
            .candidate_ranks(&candidates.iter().map(String::as_str).collect::<Vec<_>>())
            .map_err(|error| Error::from_reason(error.to_string()))?
            .into_iter()
            .map(|rank| rank.map(u32::try_from).transpose())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::from_reason("Too many candidates in one sorting batch"))
    }

    /// Returns active theme values, keyed by their full custom-property names.
    #[napi]
    pub fn theme_values(&self) -> BTreeMap<String, String> {
        self.query.theme_values().into_iter().collect()
    }

    /// Returns the authored spacing scale, or null when no scale is active.
    #[napi]
    pub fn spacing_scale(&self) -> Option<String> {
        self.query.spacing_scale()
    }

    /// Converts a fully determined CSS color to RGBA components in 0..=1, or null.
    #[napi]
    #[allow(clippy::unused_self)] // Color conversion shares the editor query transport.
    pub fn color_rgba(&self, value: String) -> Option<Vec<f64>> {
        color_rgba(&value).map(|components| components.to_vec())
    }

    /// Returns a concrete candidate color in RGBA, or null for contextual or ambiguous colors.
    #[napi]
    pub fn candidate_color(&self, candidate: String) -> napi::Result<Option<Vec<f64>>> {
        self.query
            .candidate_color(&candidate)
            .map(|value| value.map(|value| value.to_vec()))
            .map_err(|error| Error::from_reason(error.to_string()))
    }
}
