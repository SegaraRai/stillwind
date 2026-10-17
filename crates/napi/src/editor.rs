//! Native editor requests share compiler resolution, lint policy, and source reporting.

use std::collections::BTreeMap;

use lean_string::LeanStr;
use napi::Error;
use napi::bindgen_prelude::Either;
use napi_derive::napi;

use stillwind_compiler::{
    CandidateContext, ColorEquivalents, CompilerOptions, CompletionInput, DesignSystemQuery,
    PreviewAnnotations, StylesheetCompletionOptions,
};
use stillwind_css::{color_rgba, format_css};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{
    FixAction, LintDiagnostic, LintFinding, LintKind, LintSurface, Reporter, SourceEdit,
    SourceRange, filter_lint_findings,
};
use stillwind_source::{FileId, SourceFile, SourceRegistry, SourceUri, Span};

use crate::options::NativeCanonicalUtilitiesOptions;
use crate::output::{BindingReport, NativeText};

/// Comments that formatted previews add after values. Omitted fields use their defaults.
#[napi(object)]
pub struct NativePreviewOptions {
    /// Adds resolved values after theme `var()` references and single-value `calc()`
    /// expressions. Defaults to true.
    pub theme_values: Option<bool>,
    /// Adds pixel equivalents for rem values. Defaults to true.
    pub pixel_equivalents: Option<bool>,
    /// `approximate`, `exact`, or false. Defaults to `approximate`.
    #[napi(ts_type = "'approximate' | 'exact' | false")]
    pub color_equivalents: Option<Either<String, bool>>,
}

impl TryFrom<NativePreviewOptions> for PreviewAnnotations {
    type Error = Error;

    fn try_from(value: NativePreviewOptions) -> napi::Result<Self> {
        let defaults = Self::default();
        Ok(Self {
            theme_values: value.theme_values.unwrap_or(defaults.theme_values),
            pixel_equivalents: value
                .pixel_equivalents
                .unwrap_or(defaults.pixel_equivalents),
            color_equivalents: match value.color_equivalents {
                None => defaults.color_equivalents,
                Some(Either::A(policy)) if policy == "approximate" => {
                    Some(ColorEquivalents::Approximate)
                }
                Some(Either::A(policy)) if policy == "exact" => Some(ColorEquivalents::Exact),
                Some(Either::B(false)) => None,
                Some(_) => {
                    return Err(Error::from_reason(
                        "colorEquivalents must be approximate, exact, or false",
                    ));
                }
            },
        })
    }
}

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
    /// Serialized as `elementVariants` for applied variants with element selectors.
    ElementVariants,
    /// Serialized as `mediaVariants` for one `style.media()` argument, whose variants
    /// contain only media conditions.
    MediaVariants,
    /// Serialized as `document` for `style.css()` inputs, whose variants select the
    /// element to style.
    Document,
    /// Serialized as `anchoredDocument` for `style.css()` inputs of a call with a
    /// `selector` option.
    AnchoredDocument,
    /// Serialized as `cssTemplate` for the `template` option of `style.css()`.
    CssTemplate,
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
            NativeCandidateContext::ElementVariants => Self::ElementVariants,
            NativeCandidateContext::MediaVariants => Self::MediaVariants,
            NativeCandidateContext::Document => Self::Document,
            NativeCandidateContext::AnchoredDocument => Self::AnchoredDocument,
            NativeCandidateContext::CssTemplate => Self::CssTemplate,
        }
    }
}

impl From<CandidateContext> for NativeCandidateContext {
    fn from(value: CandidateContext) -> Self {
        match value {
            CandidateContext::Utility => Self::Utility,
            CandidateContext::Document => Self::Document,
            CandidateContext::AnchoredDocument => Self::AnchoredDocument,
            CandidateContext::KeyframesFrame => Self::KeyframesFrame,
            CandidateContext::AnimationOptions => Self::AnimationOptions,
            CandidateContext::ViewTransitionOptions => Self::ViewTransitionOptions,
            CandidateContext::ViewTransitionUtilities => Self::ViewTransitionUtilities,
            CandidateContext::DefinitionVariants => Self::DefinitionVariants,
            CandidateContext::ElementVariants => Self::ElementVariants,
            CandidateContext::MediaVariants => Self::MediaVariants,
            CandidateContext::CssTemplate => Self::CssTemplate,
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

/// A CSS completion whose replacement range counts original UTF-8 bytes.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetCompletion {
    /// Inclusive byte offset of the replaced input.
    pub start: u32,
    /// Exclusive byte offset of the replaced input.
    pub end: u32,
    /// Literal replacement text.
    pub label: String,
    /// Whether the replacement names an at-rule.
    pub keyword: bool,
    /// Audience and syntax provenance for a directive, omitted for other inputs.
    pub detail: Option<String>,
    /// Lexical key that ranks a directive by location, audience, and provenance.
    pub sort_text: Option<String>,
}

/// Editor settings that control at-rule discovery without changing compiler output.
#[napi(object)]
#[derive(Default)]
pub struct NativeStylesheetCompletionOptions {
    /// Includes registry metadata where it is useful. Defaults to false.
    pub registry_directives: Option<bool>,
}

impl From<NativeStylesheetCompletionOptions> for StylesheetCompletionOptions {
    fn from(value: NativeStylesheetCompletionOptions) -> Self {
        Self {
            registry_directives: value.registry_directives.unwrap_or(false),
        }
    }
}

/// CSS or Markdown documentation at an original UTF-8 range.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetHover {
    /// Inclusive byte offset of the described input.
    pub start: u32,
    /// Exclusive byte offset of the described input.
    pub end: u32,
    /// Generated CSS, or empty for documentation-only hovers.
    pub css: String,
    /// Markdown description, or empty for candidate previews.
    pub documentation: String,
}

/// A half-open UTF-8 byte range in an original stylesheet.
#[napi(object)]
pub struct NativeStylesheetRange {
    /// Inclusive original UTF-8 byte boundary.
    pub start: u32,
    /// Exclusive original UTF-8 byte boundary.
    pub end: u32,
}

/// A concrete sRGB color at an original CSS utility or reference.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetColor {
    /// Inclusive original UTF-8 byte boundary.
    pub start: u32,
    /// Exclusive original UTF-8 byte boundary.
    pub end: u32,
    /// sRGB red component in 0..1.
    pub red: f64,
    /// sRGB green component in 0..1.
    pub green: f64,
    /// sRGB blue component in 0..1.
    pub blue: f64,
    /// Effective opacity in 0..1.
    pub alpha: f64,
    /// Original utility spelling. Omitted for a reference replaced by a CSS literal.
    pub candidate: Option<String>,
}

/// Extracts definitions and imports for an editor-only load of incomplete CSS.
/// Ordinary compilation and lint must continue to use the original source.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn editor_css_source(source: String) -> String {
    stillwind_compiler::editor_css_source(&source).into()
}

/// Candidate operations for one immutable design system and explicitly supplied source buffers.
#[napi]
pub struct NativeEditorQuery {
    query: DesignSystemQuery,
}

#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
impl NativeEditorQuery {
    /// Returns concrete colors with original byte ranges, including incomplete CSS
    /// blocks. Runtime references use the active theme definition.
    ///
    /// # Errors
    ///
    /// Returns malformed utility requirements or offsets exceeding uint32.
    #[napi]
    pub fn stylesheet_colors(&self, source: String) -> napi::Result<Vec<NativeStylesheetColor>> {
        self.query
            .stylesheet_colors(&source)
            .map_err(|error| Error::from_reason(error.to_string()))?
            .into_iter()
            .map(|color| {
                Ok(NativeStylesheetColor {
                    start: u32::try_from(color.range.start)
                        .map_err(|error| Error::from_reason(error.to_string()))?,
                    end: u32::try_from(color.range.end)
                        .map_err(|error| Error::from_reason(error.to_string()))?,
                    red: color.rgba[0],
                    green: color.rgba[1],
                    blue: color.rgba[2],
                    alpha: color.rgba[3],
                    candidate: color.candidate.map(Into::into),
                })
            })
            .collect()
    }

    /// Plans nonoverlapping source edits with captured CSS sorting targets. Protected
    /// ranges use original UTF-8 bytes. Invalid ranges reject the request.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements, invalid protected ranges, or offsets exceeding
    /// uint32.
    #[napi]
    pub fn sort_stylesheet(
        &self,
        source: String,
        protected: Option<Vec<NativeStylesheetRange>>,
    ) -> napi::Result<Vec<NativeEditorLintEdit>> {
        let protected = protected
            .unwrap_or_default()
            .into_iter()
            .map(|range| {
                let (start, end) = (range.start as usize, range.end as usize);
                if start > end || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
                    return Err(Error::from_reason("Invalid protected CSS range"));
                }
                Ok(start..end)
            })
            .collect::<napi::Result<Vec<_>>>()?;
        self.query
            .sort_stylesheet(&source, &protected)
            .map_err(|error| Error::from_reason(error.to_string()))?
            .into_iter()
            .map(|edit| {
                Ok(NativeEditorLintEdit {
                    start: u32::try_from(edit.range.start)
                        .map_err(|error| Error::from_reason(error.to_string()))?,
                    end: u32::try_from(edit.range.end)
                        .map_err(|error| Error::from_reason(error.to_string()))?,
                    replacement: edit.replacement.into(),
                })
            })
            .collect()
    }

    /// Returns the captured design-system fingerprint used by completion resolution.
    #[napi]
    pub fn revision(&self) -> String {
        self.query.revision().0.to_string()
    }

    /// Completes an authored CSS input at an original UTF-8 byte boundary. Incomplete
    /// blocks and candidates are accepted. Invalid boundaries have no answer. Optional
    /// discovery settings default to authoring syntax. They affect neither the query's
    /// compiler settings nor its hover and argument catalogs.
    #[napi]
    pub fn complete_stylesheet(
        &self,
        source: String,
        offset: u32,
        options: Option<NativeStylesheetCompletionOptions>,
    ) -> napi::Result<Vec<NativeStylesheetCompletion>> {
        self.query
            .complete_stylesheet(&source, offset as usize, options.unwrap_or_default().into())
            .map_err(|error| Error::from_reason(error.to_string()))?
            .into_iter()
            .map(|item| {
                Ok(NativeStylesheetCompletion {
                    start: u32::try_from(item.range.start)
                        .map_err(|error| Error::from_reason(error.to_string()))?,
                    end: u32::try_from(item.range.end)
                        .map_err(|error| Error::from_reason(error.to_string()))?,
                    label: item.label.into(),
                    keyword: item.keyword,
                    detail: item.detail.map(Into::into),
                    sort_text: item.sort_text.map(Into::into),
                })
            })
            .collect()
    }

    /// Returns generated CSS or dialect documentation at an original UTF-8 byte boundary.
    #[napi]
    pub fn hover_stylesheet(
        &self,
        source: String,
        offset: u32,
    ) -> napi::Result<Option<NativeStylesheetHover>> {
        self.query
            .hover_stylesheet(&source, offset as usize)
            .map_err(|error| Error::from_reason(error.to_string()))?
            .map(|hover| {
                Ok(NativeStylesheetHover {
                    start: u32::try_from(hover.range.start)
                        .map_err(|error| Error::from_reason(error.to_string()))?,
                    end: u32::try_from(hover.range.end)
                        .map_err(|error| Error::from_reason(error.to_string()))?,
                    css: hover.css.into(),
                    documentation: hover.documentation.into(),
                })
            })
            .transpose()
    }
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
            "prefer-canonical-utilities" => LintDiagnostic::NonCanonicalUtility,
            "prefer-sorted-utilities" => LintDiagnostic::UnsortedUtilities,
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
        filter_lint_findings(&source, LintSurface::Script, &mut findings);
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
    /// Nesting and fallback declarations are retained. When `preview` is supplied, the
    /// result also contains its comments with resolved values; omitted fields use their
    /// defaults. Omitted `preview` adds no comments.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid CSS or an invalid `colorEquivalents` value.
    #[napi(ts_return_type = "string")]
    pub fn format_css(
        &self,
        css: String,
        preview: Option<NativePreviewOptions>,
    ) -> napi::Result<NativeText> {
        let formatted = format_css(&css).map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(match preview {
            Some(preview) => self
                .query
                .annotate_preview(&formatted, preview.try_into()?)
                .into(),
            None => formatted.into(),
        })
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
