//! Editor requests for candidates: canonical spellings, previews, completion, marker
//! classes, ordering, and colors.

use std::cmp::Ordering;

use napi::Error;
use napi::bindgen_prelude::Either;
use napi_derive::napi;

use stillwind_compiler::{CandidateContext, ColorEquivalents, CompletionInput, PreviewAnnotations};
use stillwind_css::{color_rgba, format_css};
use stillwind_design_system::MarkerOrigin;

use super::NativeEditorQuery;
use super::query::reason;
use crate::options::NativeCanonicalUtilitiesOptions;
use crate::output::NativeText;

/// The input role of a candidate, which depends on where the candidate is written in
/// the source. The role decides which utilities and variants are valid.
///
/// Keyframe frames, keyframes uses, and view transitions resolve utilities through
/// their own registry context.
#[napi(string_enum = "camelCase")]
pub enum NativeCandidateContext {
    /// Serialized as `utility`. This is an ordinary utility on an element, which can
    /// have ordinary variants.
    Utility,
    /// Serialized as `keyframesFrame`. A utility in a keyframe frame cannot have
    /// variants.
    KeyframesFrame,
    /// Serialized as `animationOptions`. An animation option cannot have variants, `!`,
    /// or a negative value.
    AnimationOptions,
    /// Serialized as `viewTransitionUtilities`. A view transition utility applies to
    /// pseudo-elements, and it can have only media and supports conditions as variants.
    ViewTransitionUtilities,
    /// Serialized as `definitionVariants`. These are the media and supports variants in
    /// front of a definition branch.
    DefinitionVariants,
    /// Serialized as `elementVariants`. These are applied variants that select
    /// elements.
    ElementVariants,
    /// Serialized as `mediaVariants`. This is one argument of `style.media()`, which
    /// can contain only media conditions.
    MediaVariants,
    /// Serialized as `document`. This is an input of `style.css()`. Its variants select
    /// the element to style.
    Document,
    /// Serialized as `anchoredDocument`. This is an input of a `style.css()` call that
    /// has a `selector` option.
    AnchoredDocument,
    /// Serialized as `cssTemplate`. This is the `template` option of `style.css()`.
    CssTemplate,
}

impl From<NativeCandidateContext> for CandidateContext {
    fn from(value: NativeCandidateContext) -> Self {
        match value {
            NativeCandidateContext::Utility => Self::Utility,
            NativeCandidateContext::KeyframesFrame => Self::KeyframesFrame,
            NativeCandidateContext::AnimationOptions => Self::AnimationOptions,
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
            CandidateContext::ViewTransitionUtilities => Self::ViewTransitionUtilities,
            CandidateContext::DefinitionVariants => Self::DefinitionVariants,
            CandidateContext::ElementVariants => Self::ElementVariants,
            CandidateContext::MediaVariants => Self::MediaVariants,
            CandidateContext::CssTemplate => Self::CssTemplate,
        }
    }
}

/// One label that candidate completion offers.
#[napi(object, object_from_js = false)]
pub struct NativeCandidateCompletion {
    /// The text to insert. A variant label ends with `:`. A label that starts the
    /// modifier of a marker ends with `/`.
    pub label: String,
    /// Whether the label is a marker class name, or a marker name followed by `/` for
    /// its modifier. A marker label may have no CSS preview.
    pub marker: bool,
}

/// The kind of statement or definition that makes a class a marker.
#[napi(string_enum = "camelCase")]
pub enum NativeMarkerOriginKind {
    /// Serialized as `declared`. A `@stillwind-marker` statement declares the class.
    Declared,
    /// Serialized as `source`. A rule in a `@stillwind-marker-source` string uses the
    /// class. Hosts generate these strings from the CSS of plugins.
    Source,
    /// Serialized as `utility`. A selector in a utility definition uses the class.
    Utility,
    /// Serialized as `variant`. The selector or the body of a variant definition uses
    /// the class.
    Variant,
}

/// A statement or definition that makes a class a marker.
#[napi(object, object_from_js = false)]
pub struct NativeMarkerOrigin {
    /// The kind of the statement or definition.
    pub kind: NativeMarkerOriginKind,
    /// The name of the definition for the kinds `utility` and `variant`. A functional
    /// definition ends with `*`. The field is absent for the other kinds.
    pub name: Option<String>,
}

impl From<MarkerOrigin> for NativeMarkerOrigin {
    fn from(value: MarkerOrigin) -> Self {
        let (kind, name) = match value {
            MarkerOrigin::Declared => (NativeMarkerOriginKind::Declared, None),
            MarkerOrigin::Source => (NativeMarkerOriginKind::Source, None),
            MarkerOrigin::Utility(name) => (NativeMarkerOriginKind::Utility, Some(name)),
            MarkerOrigin::Variant(name) => (NativeMarkerOriginKind::Variant, Some(name)),
        };
        Self {
            kind,
            name: name.map(|name| name.to_string()),
        }
    }
}

/// The marker class that a candidate adds to the element.
#[napi(object, object_from_js = false)]
pub struct NativeAddedMarker {
    /// The decoded class name. For a marker that accepts a modifier, the name includes
    /// a named modifier.
    pub class: String,
    /// The statements and definitions that make the name a marker. The list has at
    /// least one entry and no duplicates.
    pub origins: Vec<NativeMarkerOrigin>,
}

/// The comments that a formatted preview adds after values. An omitted field uses its
/// default.
#[napi(object)]
pub struct NativePreviewOptions {
    /// Whether to add the resolved value after each theme `var()` reference and after
    /// each `calc()` with one value. The default is true.
    pub theme_values: Option<bool>,
    /// Whether to add the pixel value after each `rem` value. The default is true.
    pub pixel_equivalents: Option<bool>,
    /// How to add a color value after each color: `approximate`, `exact`, or false for
    /// no color value. The default is `approximate`.
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

/// Returns the text inside an arbitrary value or modifier decoded as Tailwind decodes
/// it.
///
/// An `_` becomes a space, and `\_` becomes `_`. Underscores stay in `url()` and in the
/// first argument of `var()` and `theme()`. Operators inside math functions such as
/// `calc()` receive spaces around them.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn decode_arbitrary_value(value: String) -> String {
    stillwind_syntax::decode_arbitrary_value(&value).to_string()
}

#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
impl NativeEditorQuery {
    /// Returns the canonical spelling of `candidate`, using the lint options of the
    /// project. Returns null when `candidate` is already canonical, or when no
    /// replacement produces the same CSS.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition.
    #[napi(ts_return_type = "string | null")]
    pub fn canonical_candidate(&self, candidate: String) -> napi::Result<Option<NativeText>> {
        self.query
            .canonical_candidate(&candidate)
            .map(|value| value.map(Into::into))
            .map_err(reason)
    }

    /// Returns the canonical spellings of `candidates`, the candidates of one source
    /// container such as one class string.
    ///
    /// When the `collapse` option of the query is enabled, several candidates with the
    /// same variants and `!` may become one utility. Candidates that do not change keep
    /// their order.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition.
    #[napi(ts_return_type = "string[]")]
    pub fn canonical_candidates(&self, candidates: Vec<String>) -> napi::Result<Vec<NativeText>> {
        let candidates: Vec<_> = candidates.into_iter().map(Into::into).collect();
        self.query
            .canonical_candidates(&candidates)
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(reason)
    }

    /// Returns the options of the `prefer-canonical-utilities` rule that compilation
    /// and the reporter use. Every field is set.
    #[napi]
    pub fn canonical_options(&self) -> NativeCanonicalUtilitiesOptions {
        let options = self.query.canonical_options();
        NativeCanonicalUtilitiesOptions {
            severity: Some(options.severity.as_str().into()),
            px_to_rem: Some(options.px_to_rem),
            collapse: Some(options.collapse),
            logical_to_physical: Some(options.logical_to_physical),
            preserve_specificity: Some(options.preserve_specificity),
        }
    }

    /// Returns the CSS that the decoded `candidate` produces in the role `context`,
    /// with `&` as the selector of the element. An omitted `context` means one ordinary
    /// utility.
    ///
    /// Roles for declarations accept a list of candidates. A definition prefix may end
    /// with `:`, and its preview contains the at-rules with empty bodies. The preview
    /// does not contain the conditions around the source, or the support declarations
    /// of document-level styles. Returns null when the input is invalid, or when the
    /// compiler cannot write a condition in the configured mode.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a matching utility definition,
    /// but only for ordinary utility input.
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
            .map_err(reason)
    }

    /// Returns the closest spelling of the decoded `candidate` that resolves when
    /// `candidate` fails by a likely typo. Returns null for a valid candidate and when
    /// no spelling is close enough.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a matching utility definition.
    #[napi]
    pub fn suggest_candidate(&self, candidate: String) -> napi::Result<Option<String>> {
        self.query
            .suggest_candidate(&candidate)
            .map(|value| value.map(|value| value.to_string()))
            .map_err(reason)
    }

    /// Returns the CSS that the variants of the decoded `candidate` add: their wrappers
    /// and the declarations that the variants themselves add. The final `:` is
    /// optional. An omitted `context` means ordinary utilities. Returns null when the
    /// role or the variants are not supported.
    #[napi(ts_return_type = "string | null")]
    pub fn preview_variant(
        &self,
        candidate: String,
        context: Option<NativeCandidateContext>,
    ) -> Option<NativeText> {
        self.query
            .preview_variant(
                &candidate,
                context.map_or(CandidateContext::Utility, Into::into),
            )
            .map(Into::into)
    }

    /// Returns `css` formatted for display, with line breaks and an indentation of two
    /// spaces. Nested rules and fallback declarations stay in the result.
    ///
    /// When `preview` is given, the result also contains comments with resolved values,
    /// as `NativePreviewOptions` describes. An omitted field of `preview` uses its
    /// default. When `preview` is omitted, the result has no comments.
    ///
    /// # Errors
    ///
    /// Returns an error when `css` is invalid, or when `colorEquivalents` has an
    /// invalid value.
    #[napi(ts_return_type = "string")]
    pub fn format_css(
        &self,
        css: String,
        preview: Option<NativePreviewOptions>,
    ) -> napi::Result<NativeText> {
        let formatted = format_css(&css).map_err(reason)?;
        Ok(match preview {
            Some(preview) => self
                .query
                .annotate_preview(&formatted, preview.try_into()?)
                .into(),
            None => formatted.into(),
        })
    }

    /// Returns the labels that can continue the decoded `prefix` in the role `context`.
    /// `prefix` is one partial candidate, including the variants before it. An omitted
    /// `context` means ordinary utilities.
    ///
    /// The labels are unique and in lexical order. Each label keeps the variants of
    /// `prefix`. A variant label ends with `:`. An ordinary utility prefix without
    /// variants, `!`, or `-` also receives marker class names.
    ///
    /// When `snippets` is true, variants that accept an arbitrary value are also
    /// offered with an empty `[]`, and each marker `name` that accepts a modifier is
    /// also offered as `name/`. The default of `snippets` is false.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition that matches
    /// a label.
    #[napi]
    pub fn complete_candidate(
        &self,
        prefix: String,
        context: Option<NativeCandidateContext>,
        snippets: Option<bool>,
    ) -> napi::Result<Vec<NativeCandidateCompletion>> {
        self.query
            .complete_candidate(CompletionInput {
                prefix: (prefix).into(),
                context: context.map_or(CandidateContext::Utility, Into::into),
                snippets: snippets.unwrap_or(false),
                markers: true,
            })
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| NativeCandidateCompletion {
                        label: item.label.to_string(),
                        marker: item.marker,
                    })
                    .collect()
            })
            .map_err(reason)
    }

    /// Returns the marker class that the decoded `candidate` adds to the element in an
    /// input of the role `context`. Returns null when it adds no marker class. An
    /// omitted `context` means ordinary utilities, which is the only role that adds
    /// marker classes.
    ///
    /// `candidate` must be the complete candidate, including its variants. A candidate
    /// with variants adds no marker class. A ref condition of the surrounding style
    /// object also prevents the class, but this method does not check it.
    #[napi]
    pub fn added_marker(
        &self,
        candidate: String,
        context: Option<NativeCandidateContext>,
    ) -> Option<NativeAddedMarker> {
        self.query
            .added_marker(
                &candidate,
                context.map_or(CandidateContext::Utility, Into::into),
            )
            .map(|marker| NativeAddedMarker {
                class: marker.class.to_string(),
                origins: marker.origins.into_iter().map(Into::into).collect(),
            })
    }

    /// Compares `left` and `right` in the order of the compiled CSS. Returns -1 when
    /// `left` comes first, 1 when `right` comes first, and 0 when their order is the
    /// same. Returns null when either candidate does not resolve.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition.
    #[napi]
    pub fn compare_candidates(&self, left: String, right: String) -> napi::Result<Option<i32>> {
        self.query
            .compare_candidates(&left, &right)
            .map(|order| {
                order.map(|order| match order {
                    Ordering::Less => -1,
                    Ordering::Equal => 0,
                    Ordering::Greater => 1,
                })
            })
            .map_err(reason)
    }

    /// Returns a rank for each of `candidates`, in input order. A lower rank comes
    /// first in the compiled CSS. Equal candidates share a rank. A candidate that does
    /// not resolve receives null. Ranks are valid only within one call.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition, or when a
    /// rank does not fit in `u32`.
    #[napi]
    pub fn candidate_ranks(&self, candidates: Vec<String>) -> napi::Result<Vec<Option<u32>>> {
        self.query
            .candidate_ranks(&candidates.iter().map(String::as_str).collect::<Vec<_>>())
            .map_err(reason)?
            .into_iter()
            .map(|rank| rank.map(u32::try_from).transpose())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::from_reason("Too many candidates in one sorting batch"))
    }

    /// Returns the utility keys of host plugins and the `@variant ` requests that
    /// appear in `text`. `text` can be CSS or source code.
    #[napi]
    pub fn dynamic_candidates(&self, text: String) -> Vec<String> {
        self.query
            .dynamic_candidates(&text)
            .into_iter()
            .map(|value| value.to_string())
            .collect()
    }

    /// Returns the red, green, blue, and alpha components of the CSS color `value`,
    /// each from 0 to 1. Returns null when `value` is not a color whose components are
    /// fully known.
    #[napi]
    #[allow(clippy::unused_self)] // Color conversion shares the editor query transport.
    pub fn color_rgba(&self, value: String) -> Option<Vec<f64>> {
        color_rgba(&value).map(|components| components.to_vec())
    }

    /// Returns the red, green, blue, and alpha components of the one color that
    /// `candidate` sets, each from 0 to 1. Returns null when the utility sets more than
    /// colors, or when its color does not resolve or is ambiguous.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition.
    #[napi]
    pub fn candidate_color(&self, candidate: String) -> napi::Result<Option<Vec<f64>>> {
        self.query
            .candidate_color(&candidate)
            .map(|value| value.map(|value| value.to_vec()))
            .map_err(reason)
    }
}
