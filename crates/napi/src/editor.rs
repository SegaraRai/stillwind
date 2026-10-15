//! Native editor requests share compiler resolution, lint policy, and source reporting.

use std::collections::BTreeMap;

use lean_string::LeanStr;
use napi::Error;
use napi_derive::napi;

use stillwind_compiler::{CandidateContext, CompilerOptions, CompletionInput, DesignSystemQuery};
use stillwind_css::{color_rgba, format_css};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{
    FixAction, LintDiagnostic, LintFinding, LintKind, Reporter, SourceEdit, SourceRange,
    filter_lint_findings,
};
use stillwind_source::{FileId, SourceFile, SourceRegistry, SourceUri, Span};

use crate::options::NativeCanonicalUtilitiesOptions;
use crate::output::{BindingReport, NativeText};

/// Selects the compiler contract of a candidate's source position.
/// Option roles use the animation-option registry and its property mappings. Frame and
/// phase utilities retain the declarations produced by ordinary utility resolution.
#[napi(string_enum = "camelCase")]
pub enum NativeCandidateContext {
    /// Serialized as `utility` for element utilities with ordinary variants.
    Utility,
    /// Serialized as `keyframesFrame`; utility variants are rejected.
    KeyframesFrame,
    /// Serialized as `animationOptions`; variants, importance, and negative values are
    /// rejected.
    AnimationOptions,
    /// Serialized as `viewTransitionOptions` for group options with media and supports
    /// conditions.
    ViewTransitionOptions,
    /// Serialized as `viewTransitionUtilities` for phase utilities with media and
    /// supports conditions.
    ViewTransitionUtilities,
    /// Serialized as `definitionVariants` for media and supports prefixes of a
    /// definition branch.
    DefinitionVariants,
}

impl From<NativeCandidateContext> for CandidateContext {
    fn from(value: NativeCandidateContext) -> Self {
        match value {
            NativeCandidateContext::Utility => Self::Utility,
            NativeCandidateContext::KeyframesFrame => Self::KeyframesFrame,
            NativeCandidateContext::AnimationOptions => Self::AnimationOptions,
            NativeCandidateContext::ViewTransitionOptions => Self::ViewTransitionOptions,
            NativeCandidateContext::ViewTransitionUtilities => Self::ViewTransitionUtilities,
            NativeCandidateContext::DefinitionVariants => Self::DefinitionVariants,
        }
    }
}

impl From<CandidateContext> for NativeCandidateContext {
    fn from(value: CandidateContext) -> Self {
        match value {
            CandidateContext::Utility => Self::Utility,
            CandidateContext::KeyframesFrame => Self::KeyframesFrame,
            CandidateContext::AnimationOptions => Self::AnimationOptions,
            CandidateContext::ViewTransitionOptions => Self::ViewTransitionOptions,
            CandidateContext::ViewTransitionUtilities => Self::ViewTransitionUtilities,
            CandidateContext::DefinitionVariants => Self::DefinitionVariants,
        }
    }
}

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
    #[napi(ts_return_type = "string | null")]
    pub fn canonical_candidate(&self, candidate: String) -> napi::Result<Option<NativeText>> {
        self.query
            .canonical_candidate(&candidate)
            .map(|value| value.map(Into::into))
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
                    replacement: (edit.replacement).into(),
                }],
            });
            findings.push(finding);
        }
        let batch = Reporter::collect(&[], &findings, &registry, self.query.lint_settings())
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(batch.diagnostics.into_iter().map(Into::into).collect())
    }

    /// Returns effective lint severities after stylesheet and project configuration.
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

    /// Renders decoded text under `context`, using `&` as the utility subject. Omitted
    /// context selects one ordinary utility candidate. Declaration roles accept a token
    /// list; definition prefixes accept trailing colons and return empty enclosing
    /// at-rules. Results omit enclosing source conditions and document support
    /// declarations. Invalid input or failed conditional lowering returns null.
    ///
    /// # Errors
    ///
    /// Returns a malformed matching utility requirement for ordinary utility input.
    #[napi(ts_return_type = "string | null")]
    pub fn preview_candidate(
        &self,
        candidate: String,
        context: Option<NativeCandidateContext>,
    ) -> napi::Result<Option<NativeText>> {
        self.query
            .preview_candidate(
                &candidate,
                context.map_or(CandidateContext::Utility, Into::into),
            )
            .map(|value| value.map(Into::into))
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Returns a complete CSS preview with line breaks and two-space indentation.
    /// Nesting and fallback declarations are retained. Invalid CSS returns an error.
    #[napi(ts_return_type = "string")]
    #[allow(clippy::unused_self)] // Formatting shares the editor query transport.
    pub fn format_css(&self, css: String) -> napi::Result<NativeText> {
        format_css(&css)
            .map(Into::into)
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Completes decoded `prefix` text using the registries and variant constraints of
    /// `context`. The prefix is one partial candidate including any preceding variants;
    /// omitted context selects ordinary utilities. Returns unique labels in lexical
    /// order, retaining existing prefixes and ending variant labels with `:`.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement while validating a matching ordinary utility
    /// label.
    #[napi(ts_return_type = "Array<string>")]
    pub fn complete_candidate(
        &self,
        prefix: String,
        context: Option<NativeCandidateContext>,
    ) -> napi::Result<Vec<NativeText>> {
        self.query
            .complete_candidate(CompletionInput {
                prefix: (prefix).into(),
                context: context.map_or(CandidateContext::Utility, Into::into),
            })
            .map(|items| items.into_iter().map(|item| item.label.into()).collect())
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Returns -1, 0, or 1 in compiler order, or null if either candidate is
    /// unresolved.
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

    /// Returns ranks for full candidate spellings in input order, with null for
    /// unresolved inputs. Equal candidates share a rank. Ranks apply only to this
    /// batch.
    ///
    /// # Errors
    ///
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
    #[napi(ts_return_type = "Record<string, string>")]
    pub fn theme_values(&self) -> BTreeMap<LeanStr, NativeText> {
        self.query
            .theme_values()
            .into_iter()
            .map(|(name, value)| (name, value.into()))
            .collect()
    }

    /// Returns the authored spacing scale, or null when no scale is active.
    #[napi(ts_return_type = "string | null")]
    pub fn spacing_scale(&self) -> Option<NativeText> {
        (self.query.spacing_scale()).map(Into::into)
    }

    /// Converts a fully determined CSS color to RGBA components in 0..=1, or null.
    #[napi]
    #[allow(clippy::unused_self)] // Color conversion shares the editor query transport.
    pub fn color_rgba(&self, value: String) -> Option<Vec<f64>> {
        color_rgba(&value).map(|components| components.to_vec())
    }

    /// Returns a concrete candidate color in RGBA, or null for contextual or ambiguous
    /// colors.
    #[napi]
    pub fn candidate_color(&self, candidate: String) -> napi::Result<Option<Vec<f64>>> {
        self.query
            .candidate_color(&candidate)
            .map(|value| value.map(|value| value.to_vec()))
            .map_err(|error| Error::from_reason(error.to_string()))
    }
}
