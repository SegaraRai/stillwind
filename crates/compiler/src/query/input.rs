//! Input roles select the same resolvers that emit their compiler declarations.
//!
//! [`DesignSystemQuery::preview_candidate`] renders standalone decoded text. It cannot
//! recover surrounding source conditions; module captures provide that context.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{
    DocumentTargetFailure, UtilityResolutionError, document_target, resolve_variant,
};
use stillwind_diagnostics::LintSettings;
use stillwind_source::{FileId, Span, Spanned};
use stillwind_syntax::parse_candidate;

use super::DesignSystemQuery;
use super::resolution::candidate_identifiers;
use crate::compiler::{
    OptionVariants, condition_paths, emit_document_style, emit_view_transition_utilities,
    resolve_frame, resolve_options, wrap_conditions,
};

/// The syntax and emission contract of an editor input.
/// Option roles use the animation-option registry and its registered property mappings.
/// Frame and phase utilities keep the declarations produced by utility resolution.
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
    /// View-transition group options, with media and supports conditions.
    ViewTransitionOptions,
    /// View-transition phase utilities, with media and supports conditions.
    ViewTransitionUtilities,
    /// Media and supports prefixes that can contain a global definition.
    DefinitionVariants,
    /// Utilities of a document-level style, which has no generated class. The first
    /// variant that selects an element, such as `host:` or `[:root]:`, must come before
    /// any variant that refers to `&`. Only tests use this role until the
    /// document-level API is finalized.
    Document,
}

impl CandidateContext {
    /// Whether this role uses the animation-option registry instead of utility definitions.
    pub(super) const fn options(self) -> bool {
        matches!(self, Self::AnimationOptions | Self::ViewTransitionOptions)
    }

    /// Whether this role permits variant prefixes. Allowed roles still validate each
    /// prefix through their own resolver.
    pub(super) const fn variants(self) -> bool {
        !matches!(self, Self::KeyframesFrame | Self::AnimationOptions)
    }
}

impl DesignSystemQuery {
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

        let theme = self.context.design_system.theme();
        if context == CandidateContext::DefinitionVariants {
            // Only the parsed variants contribute to the preview. The required utility
            // suffix is discarded without resolving it.
            let text = format_args!("{}:utility", candidate.trim_end_matches(':')).to_lean_string();
            let Some(parsed) = parse_candidate(&text) else {
                return Ok(None);
            };
            let Some(paths) = condition_paths(&parsed.variants, theme) else {
                return Ok(None);
            };
            return Ok(Some(
                paths.iter().map(|path| wrap_conditions("", path)).collect(),
            ));
        }

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
                let (declaration, _) =
                    resolve_frame(&input, theme, &mut diagnostics, &mut lint_findings);
                if declaration.is_empty() {
                    LeanString::new()
                } else {
                    format_args!("&{{{declaration}}}").to_lean_string()
                }
            }
            CandidateContext::AnimationOptions | CandidateContext::ViewTransitionOptions => {
                let variants = if context == CandidateContext::AnimationOptions {
                    OptionVariants::Rejected
                } else {
                    OptionVariants::Conditions
                };
                let resolved = resolve_options(
                    &input,
                    variants,
                    theme,
                    &mut diagnostics,
                    &mut lint_findings,
                );
                let mut css = LeanString::new();
                if !resolved.declaration.is_empty() {
                    css.push_str("&{");
                    css.push_str(&resolved.declaration);
                    css.push('}');
                }
                for option in resolved.conditioned {
                    for path in option.paths {
                        css.push_str(&wrap_conditions(
                            &format_args!("&{{{}}}", option.declaration).to_lean_string(),
                            &path,
                        ));
                    }
                }
                css
            }
            CandidateContext::Document => emit_document_style(
                &input,
                theme,
                &LintSettings::default(),
                &candidate_identifiers(true),
                &mut diagnostics,
                &mut lint_findings,
            )
            .unwrap_or_default(),
            CandidateContext::ViewTransitionUtilities => {
                let mut css = LeanString::new();
                emit_view_transition_utilities(
                    &mut css,
                    "&",
                    &input,
                    theme,
                    &mut diagnostics,
                    &mut Vec::new(),
                    &mut lint_findings,
                );
                css
            }
            CandidateContext::Utility | CandidateContext::DefinitionVariants => return Ok(None),
        };
        Ok(if diagnostics.is_empty() && !css.is_empty() {
            self.context.preview_declarations(&css)
        } else {
            None
        })
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

        // The candidate grammar requires a utility suffix. Validation below resolves
        // only its variants.
        let Some(candidate) = parse_candidate(&format_args!("{text}:utility").to_lean_string())
        else {
            return false;
        };

        let theme = self.context.design_system.theme();
        if matches!(
            context,
            CandidateContext::Utility | CandidateContext::Document
        ) {
            let Some(applications) = candidate
                .variants
                .iter()
                .map(|variant| resolve_variant(variant, theme).ok())
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            return context == CandidateContext::Utility
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
    use crate::CompilerOptions;

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
        }
    }
}
