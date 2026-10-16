//! Input roles select the same resolvers that emit their compiler declarations.
//!
//! [`DesignSystemQuery::preview_candidate`] renders standalone decoded text. It cannot
//! recover surrounding source conditions; module captures provide that context.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{UtilityResolutionError, resolve_variant};
use stillwind_source::{FileId, Span, Spanned};
use stillwind_syntax::parse_candidate;

use super::DesignSystemQuery;
use crate::compiler::{
    OptionVariants, condition_paths, emit_view_transition_utilities, resolve_frame,
    resolve_options, wrap_conditions,
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
    /// Options of a keyframes use; variants, importance, and negative values are rejected.
    AnimationOptions,
    /// View-transition group options, with media and supports conditions.
    ViewTransitionOptions,
    /// View-transition phase utilities, with media and supports conditions.
    ViewTransitionUtilities,
    /// Media and supports prefixes that can contain a global definition.
    DefinitionVariants,
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
    /// Renders decoded `candidate` text under `context`, using `&` as the utility subject.
    /// Ordinary utility input accepts one candidate; declaration roles accept a token list.
    /// Definition prefixes may include trailing colons and return empty enclosing at-rules.
    /// Other roles apply the configured conditional mode and restore authored resource URLs.
    /// The result omits enclosing source conditions and document support declarations.
    /// Unsupported operands contribute no declarations. Empty output or failed
    /// conditional lowering returns `None`.
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
            let text =
                format_args!("{}:[color:red]", candidate.trim_end_matches(':')).to_lean_string();
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
    /// require every variant to resolve; definition and view-transition roles permit
    /// only media and supports conditions. Roles that forbid variants return false.
    pub(super) fn valid_input_variant(&self, text: &str, context: CandidateContext) -> bool {
        if !context.variants() {
            return false;
        }

        let Some(candidate) = parse_candidate(&format_args!("{text}:[color:red]").to_lean_string())
        else {
            return false;
        };

        if context == CandidateContext::Utility {
            return candidate.variants.iter().all(|variant| {
                resolve_variant(variant, self.context.design_system.theme()).is_ok()
            });
        }

        condition_paths(&candidate.variants, self.context.design_system.theme()).is_some()
    }
}
