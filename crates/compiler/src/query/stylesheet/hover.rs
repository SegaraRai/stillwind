//! Generated previews and documentation for authored CSS inputs.

use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::UtilityResolutionError;

use super::context::{Kind, context};
use super::directives::{DECLARATION_SLOTS, DIRECTIVES, RULE_SLOTS, directive_name};
use crate::query::{CandidateContext, DesignSystemQuery};

/// Semantic CSS or dialect documentation at an original UTF-8 range.
#[derive(Clone, Debug)]
pub struct StylesheetHover {
    /// Half-open UTF-8 byte range of the token or argument in the original CSS.
    pub range: Range<usize>,
    /// Generated CSS or an active theme declaration. Empty for documentation-only
    /// hovers.
    pub css: LeanStr,
    /// Markdown explaining the directive, slot, or theme reference. Empty for
    /// candidate previews.
    pub documentation: LeanStr,
}

impl DesignSystemQuery {
    /// Describes the input at the UTF-8 byte `offset` in the original `css`, using
    /// the same cursor rules as [`Self::complete_stylesheet`]. Unresolved candidates
    /// have no preview. Variant previews use `&` for the enclosing element and omit
    /// surrounding source conditions. Theme hovers show the active authored definition.
    /// The browser's cascade may override a runtime `var()` value.
    ///
    /// Returns CSS or documentation with its original source range, or `None` when the
    /// input has no context or cannot be resolved.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn hover_stylesheet(
        &self,
        css: &str,
        offset: usize,
    ) -> Result<Option<StylesheetHover>, UtilityResolutionError> {
        let Some(context) = context(css, offset) else {
            return Ok(None);
        };
        let written = &css[context.range.clone()];
        let (css, documentation): (LeanStr, LeanStr) = match context.kind {
            Kind::Directive => {
                let name = directive_name(written);
                let Some(directive) = DIRECTIVES
                    .iter()
                    .find(|directive| format_args!("@{}", directive.name).to_lean_str() == name)
                else {
                    return Ok(None);
                };
                (
                    "".into(),
                    format_args!(
                        "```css\n{}\n```\n\n{}\n\n{}",
                        directive.syntax,
                        directive.detail(),
                        directive.description
                    )
                    .to_lean_str(),
                )
            }
            Kind::DirectiveArgument { directive, .. } => {
                let Some(directive) = DIRECTIVES.iter().find(|item| item.name == directive) else {
                    return Ok(None);
                };
                (
                    "".into(),
                    format_args!("{}\n\n{}", directive.detail(), directive.description)
                        .to_lean_str(),
                )
            }
            Kind::Apply => {
                let Some(css) = self.preview_candidate(written, CandidateContext::Utility)? else {
                    return Ok(None);
                };
                (css, "".into())
            }
            Kind::Variant => {
                let Some(css) =
                    self.preview_candidate(written, CandidateContext::ElementVariants)?
                else {
                    return Ok(None);
                };
                (css, "".into())
            }
            Kind::Slot { .. } => {
                let name = written.split_whitespace().collect::<Vec<_>>().join(" ");
                let Some((_, description)) = RULE_SLOTS
                    .iter()
                    .chain(DECLARATION_SLOTS)
                    .find(|(slot, _)| *slot == name)
                else {
                    return Ok(None);
                };
                ("".into(), (*description).into())
            }
            Kind::Theme { runtime } => {
                let Some(token) = self.context.design_system.theme().custom_property(written)
                else {
                    return Ok(None);
                };
                let description = if runtime {
                    "Active theme definition. The browser's cascade can override this value."
                } else {
                    "Active theme definition used by this build-time reference."
                };
                (
                    format_args!("{}: {};", token.name.value, token.value.value).to_lean_str(),
                    description.into(),
                )
            }
        };
        Ok(Some(StylesheetHover {
            range: context.range,
            css,
            documentation,
        }))
    }
}
