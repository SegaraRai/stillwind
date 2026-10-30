//! Input roles select the same resolvers that emit their compiler declarations.
//!
//! [`DesignSystemQuery::preview_candidate`] renders standalone decoded text. It cannot
//! recover surrounding source conditions; module captures provide that context.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{
    DocumentTargetFailure, RegistryContext, UtilityResolutionError, document_target,
    resolve_variant,
};
use stillwind_diagnostics::LintSettings;
use stillwind_source::{FileId, Span, Spanned};
use stillwind_syntax::parse_candidate;

use super::DesignSystemQuery;
use super::resolution::candidate_identifiers;
use crate::CssContribution;
use crate::compiler::{
    condition_paths, emit_document_style, emit_view_transition_utilities, expand_authored_variant,
    media_argument_query, resolve_frame, resolve_options, wrap_conditions,
};

/// The syntax and emission contract of an editor input.
/// Keyframe frames, keyframes uses, and view transitions resolve utilities through the
/// registry context that `Self::registry_context` returns.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub enum CandidateContext {
    /// Element utilities, with ordinary variant prefixes.
    #[default]
    Utility,
    /// Declarations inside a keyframe; utility variants are rejected.
    KeyframesFrame,
    /// Options of a keyframes use; variants, importance, and negative values are
    /// rejected.
    AnimationOptions,
    /// View-transition pseudo-element utilities, with media and supports conditions.
    ViewTransitionUtilities,
    /// Media and supports prefixes that can contain a global definition.
    DefinitionVariants,
    /// Applied variant sequences with element selectors and conditional at-rules.
    ElementVariants,
    /// One `style.media()` argument: variants whose definitions contain only media
    /// conditions, with an optional final `:`.
    MediaVariants,
    /// Utilities of a document-level style, such as a `style.css()` input, which has no
    /// generated class. The first variant that selects an element, such as `host:` or
    /// `[:root]:`, must come before any variant that refers to `&`.
    Document,
    /// The `template` option of `style.css()`: the name of a `@stillwind-css-template`.
    CssTemplate,
    /// Utilities of a document-level style with a default element, such as a
    /// `style.css()` input with a `selector` option. A candidate whose variants select
    /// no element styles the default element, which previews show as `&`.
    AnchoredDocument,
}

impl CandidateContext {
    /// The registry context whose definitions this role resolves, or `None` for the
    /// default definitions.
    pub(super) const fn registry_context(self) -> Option<RegistryContext> {
        match self {
            Self::KeyframesFrame => Some(RegistryContext::Keyframe),
            Self::AnimationOptions => Some(RegistryContext::Animation),
            Self::ViewTransitionUtilities => Some(RegistryContext::ViewTransition),
            _ => None,
        }
    }

    /// Whether this role accepts only the definitions of its registry context, so
    /// default utility names are not suggested.
    pub(super) const fn options(self) -> bool {
        matches!(self, Self::AnimationOptions)
    }

    /// Whether this role accepts only variants, without a utility or option.
    pub(super) const fn variants_only(self) -> bool {
        matches!(
            self,
            Self::DefinitionVariants | Self::MediaVariants | Self::ElementVariants
        )
    }

    /// Whether this role permits variant prefixes. Allowed roles still validate each
    /// prefix through their own resolver.
    pub(super) const fn variants(self) -> bool {
        !matches!(
            self,
            Self::KeyframesFrame | Self::AnimationOptions | Self::CssTemplate
        )
    }
}

impl DesignSystemQuery {
    /// Renders the wrappers of `candidate` under the variant constraints of `context`.
    /// A trailing colon is optional. Unsupported variants or roles return `None`.
    /// The preview retains declarations introduced by variants and omits utility
    /// output and surrounding source conditions.
    pub fn preview_variant(&self, candidate: &str, context: CandidateContext) -> Option<LeanStr> {
        if !self.valid_input_variant(candidate.trim_end_matches(':'), context) {
            return None;
        }
        let context = match context {
            CandidateContext::Utility
            | CandidateContext::Document
            | CandidateContext::AnchoredDocument => CandidateContext::ElementVariants,
            CandidateContext::ViewTransitionUtilities => CandidateContext::DefinitionVariants,
            other => other,
        };
        self.preview_variants(candidate, context)
    }

    /// Renders decoded `candidate` text under `context`, using `&` as the utility
    /// subject. Ordinary utility input accepts one candidate; declaration roles and
    /// document-level styles accept a token list. A document-level style uses the
    /// element that its variants select instead of `&`, and omits candidates that
    /// select none. Definition prefixes may include trailing colons and return empty
    /// enclosing at-rules. Other roles apply the configured conditional mode and
    /// restore authored resource URLs. The result omits enclosing source conditions and
    /// document support declarations. Unsupported operands contribute no declarations.
    /// Empty output or failed conditional lowering returns `None`.
    ///
    /// # Errors
    ///
    /// Returns a malformed matching utility requirement for ordinary candidates.
    pub fn preview_candidate(
        &self,
        candidate: &str,
        context: CandidateContext,
    ) -> Result<Option<LeanStr>, UtilityResolutionError> {
        if context == CandidateContext::Utility {
            return self
                .hover_candidate(candidate)
                .map(|value| value.map(|value| value.css));
        }
        if context.variants_only() {
            return Ok(self.preview_variants(candidate, context));
        }
        if context == CandidateContext::CssTemplate {
            return Ok(self.preview_template(candidate));
        }
        if context == CandidateContext::AnchoredDocument {
            return self.preview_anchored(candidate);
        }

        let theme = self.context.design_system.theme();
        let input = Spanned::new(
            LeanStr::from(candidate),
            Span::new(
                FileId(0),
                0,
                u32::try_from(candidate.len()).unwrap_or(u32::MAX),
            ),
        );
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();
        let css = match context {
            CandidateContext::KeyframesFrame => {
                let (declaration, _, _) = resolve_frame(
                    &input,
                    theme,
                    &LintSettings::default(),
                    &mut diagnostics,
                    &mut lint_findings,
                );
                if declaration.is_empty() {
                    LeanString::new()
                } else {
                    format_args!("&{{{declaration}}}").to_lean_string()
                }
            }
            CandidateContext::AnimationOptions => {
                let (declaration, _) =
                    resolve_options(&input, theme, &mut diagnostics, &mut lint_findings);
                if declaration.is_empty() {
                    LeanString::new()
                } else {
                    format_args!("&{{{declaration}}}").to_lean_string()
                }
            }
            CandidateContext::Document => emit_document_style(
                &input,
                theme,
                &LintSettings::default(),
                self.context.options.use_css_if,
                &candidate_identifiers(true),
                &mut diagnostics,
                &mut lint_findings,
            )
            .unwrap_or_default(),
            CandidateContext::ViewTransitionUtilities => {
                let mut contribution = CssContribution::default();
                emit_view_transition_utilities(
                    &mut contribution,
                    "&",
                    &input,
                    theme,
                    &LintSettings::default(),
                    &mut diagnostics,
                    &mut lint_findings,
                );
                contribution.css.into()
            }
            CandidateContext::Utility
            | CandidateContext::CssTemplate
            | CandidateContext::AnchoredDocument
            | CandidateContext::DefinitionVariants
            | CandidateContext::ElementVariants
            | CandidateContext::MediaVariants => return Ok(None),
        };
        Ok(if diagnostics.is_empty() && !css.is_empty() {
            self.context.preview_declarations(&css)
        } else {
            None
        })
    }

    /// Renders `candidate` as a document-level utility, or with `&` as its element when
    /// its variants select none.
    ///
    /// # Errors
    ///
    /// Returns a malformed matching utility requirement.
    fn preview_anchored(&self, candidate: &str) -> Result<Option<LeanStr>, UtilityResolutionError> {
        let document = self.preview_candidate(candidate, CandidateContext::Document)?;
        if document.is_some() {
            return Ok(document);
        }
        self.preview_candidate(candidate, CandidateContext::Utility)
    }

    /// Returns the outline of the `@stillwind-css-template` named `name`, or `None`
    /// when the stylesheet defines no such template.
    fn preview_template(&self, name: &str) -> Option<LeanStr> {
        self.context
            .design_system
            .theme()
            .css_templates()
            .get(name)
            .map(|template| template.outline().into())
    }

    /// Renders the wrappers that a variant-only input applies to a rule, or returns
    /// `None` when the input does not resolve under `context`.
    fn preview_variants(&self, candidate: &str, context: CandidateContext) -> Option<LeanStr> {
        let theme = self.context.design_system.theme();
        if context == CandidateContext::ElementVariants {
            let css = expand_authored_variant(
                candidate.trim_end_matches(':'),
                "&{}",
                theme,
                &candidate_identifiers(true),
            )
            .ok()
            .filter(|css| !css.is_empty())?;
            return self.context.preview_declarations(&css);
        }
        if context == CandidateContext::MediaVariants {
            return media_argument_query(candidate, theme)
                .map(|query| format_args!("@media {query}{{}}").to_lean_string().into());
        }
        // Only the parsed variants contribute to the preview. The required utility
        // suffix is discarded without resolving it.
        let text = format_args!("{}:utility", candidate.trim_end_matches(':')).to_lean_string();
        let parsed = parse_candidate(&text)?;
        let paths = condition_paths(&parsed.variants, theme)?;
        Some(paths.iter().map(|path| wrap_conditions("", path)).collect())
    }

    /// Checks decoded `text` under its role. Ordinary utilities require failure-free
    /// resolution; declaration roles require a successful preview. No source state
    /// changes.
    ///
    /// # Errors
    ///
    /// Returns a malformed matching utility requirement for ordinary utility input.
    pub(super) fn valid_input(
        &self,
        text: &str,
        context: CandidateContext,
    ) -> Result<bool, UtilityResolutionError> {
        if context == CandidateContext::Utility {
            return Ok(self.resolve_candidate(text)?.failures.is_empty());
        }

        self.preview_candidate(text, context)
            .map(|css| css.is_some())
    }

    /// Checks a decoded variant prefix without a trailing colon. Ordinary utilities
    /// require every variant to resolve. A document-level style also rejects a variant
    /// that refers to `&` before an earlier variant selects an element, so its first
    /// variant must select one or be an at-rule condition. Definition and
    /// view-transition roles permit only media and supports conditions. Roles that
    /// forbid variants return false.
    pub(super) fn valid_input_variant(&self, text: &str, context: CandidateContext) -> bool {
        if !context.variants() {
            return false;
        }
        if context == CandidateContext::MediaVariants {
            return media_argument_query(text, self.context.design_system.theme()).is_some();
        }

        // The candidate grammar requires a utility suffix. Validation below resolves
        // only its variants.
        let Some(candidate) = parse_candidate(&format_args!("{text}:utility").to_lean_string())
        else {
            return false;
        };

        let theme = self.context.design_system.theme();
        if matches!(
            context,
            CandidateContext::Utility
                | CandidateContext::ElementVariants
                | CandidateContext::Document
                | CandidateContext::AnchoredDocument
        ) {
            let Some(applications) = candidate
                .variants
                .iter()
                .map(|variant| resolve_variant(variant, theme).ok())
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            return context != CandidateContext::Document
                || !matches!(
                    document_target(&applications),
                    Err(DocumentTargetFailure::BeforeTarget(_))
                );
        }

        condition_paths(&candidate.variants, self.context.design_system.theme()).is_some()
    }
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::{CssSourceName, FileId, SourceFile};

    use super::*;
    use crate::{CompilerOptions, CompletionInput};

    #[test]
    fn variant_inputs_resolve_without_evaluating_utility_definitions() {
        for utilities in [
            "",
            "@utility utility { @stillwind-require-properties invalid; display:none; }",
        ] {
            let source = SourceFile::new(
                FileId(1),
                None,
                None,
                format!(
                    "@custom-variant hovered (&:hover); \
                     @custom-variant wide {{ @media (width >= 40rem) {{ @slot; }} }} \
                     {utilities}"
                ),
            )
            .unwrap();
            let snapshot = DesignSystemSnapshot::from_source(
                &source,
                CssSourceName::from("test.css"),
                DesignSystemOptions::default(),
            )
            .unwrap();
            let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());

            assert!(query.valid_input_variant("hovered", CandidateContext::Utility));
            assert!(query.valid_input_variant("wide", CandidateContext::DefinitionVariants));
            assert_eq!(
                query
                    .preview_candidate("wide:", CandidateContext::DefinitionVariants)
                    .unwrap()
                    .as_deref(),
                Some("@media (width >= 40rem){}")
            );
            assert!(query.valid_input_variant("wide", CandidateContext::MediaVariants));
            assert!(!query.valid_input_variant("hovered", CandidateContext::MediaVariants));
            for argument in ["wide", "wide:"] {
                assert_eq!(
                    query
                        .preview_candidate(argument, CandidateContext::MediaVariants)
                        .unwrap()
                        .as_deref(),
                    Some("@media (width >= 40rem){}")
                );
            }
            assert_eq!(
                query
                    .preview_candidate("wide:hovered", CandidateContext::MediaVariants)
                    .unwrap(),
                None
            );
            let labels = query
                .complete_candidate(CompletionInput {
                    prefix: "w".into(),
                    context: CandidateContext::MediaVariants,
                    snippets: false,
                    markers: false,
                })
                .unwrap()
                .into_iter()
                .map(|item| item.label)
                .collect::<Vec<_>>();
            assert_eq!(labels, ["wide:"]);
        }
    }
}
