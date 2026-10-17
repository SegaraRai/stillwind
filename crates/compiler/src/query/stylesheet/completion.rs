//! Completion labels and replacement ranges for authored CSS.

use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::UtilityResolutionError;

use super::context::{Kind, context};
use super::directives::{DECLARATION_SLOTS, DIRECTIVES, RULE_SLOTS, directive_name};
use crate::query::{CandidateContext, CompletionInput, DesignSystemQuery};

/// One CSS completion with a range in the supplied original UTF-8 text.
#[derive(Clone, Debug)]
pub struct StylesheetCompletion {
    /// Half-open UTF-8 byte range replaced by `label`, including text after the cursor.
    /// An empty range inserts a new argument.
    pub range: Range<usize>,
    /// Literal CSS text inserted at `range`. No JavaScript escaping is needed.
    pub label: LeanStr,
    /// True for an at-rule name, which editors display as a keyword.
    pub keyword: bool,
}

impl DesignSystemQuery {
    /// Completes the CSS input at the UTF-8 `offset` in `css`.
    ///
    /// Invalid offsets and positions in comments, strings, and URLs produce no labels.
    /// The source may contain unclosed blocks and partial candidates. Utilities use the
    /// active registry. Applied variants offer only variant names without a final
    /// colon. Slot labels respect whether the cursor is inside a style rule. Theme
    /// functions offer active token names only in their first argument.
    ///
    /// Returns unique labels in lexical order. Every replacement range addresses
    /// the original `css`, including any token suffix after `offset`.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn complete_stylesheet(
        &self,
        css: &str,
        offset: usize,
    ) -> Result<Vec<StylesheetCompletion>, UtilityResolutionError> {
        let Some(context) = context(css, offset) else {
            return Ok(Vec::new());
        };
        let prefix = &css[context.range.start..offset];
        let keyword = matches!(context.kind, Kind::Directive);
        let mut labels: Vec<LeanStr> = match context.kind {
            Kind::Directive => {
                let prefix = directive_name(prefix);
                DIRECTIVES
                    .iter()
                    .map(|directive| format_args!("@{}", directive.name).to_lean_str())
                    .filter(|label| label.starts_with(prefix.as_str()))
                    .collect()
            }
            Kind::Apply | Kind::Variant => {
                let variants = matches!(context.kind, Kind::Variant);
                self.complete_candidate(CompletionInput {
                    prefix: if variants {
                        prefix.trim_end().into()
                    } else {
                        prefix.into()
                    },
                    context: if variants {
                        CandidateContext::ElementVariants
                    } else {
                        CandidateContext::Utility
                    },
                })?
                .into_iter()
                .filter_map(|item| {
                    if variants {
                        item.label.strip_suffix(':').map(Into::into)
                    } else {
                        Some(item.label)
                    }
                })
                .collect()
            }
            Kind::Slot { declarations } => {
                let mut prefix = prefix.split_whitespace().collect::<Vec<_>>().join(" ");
                if css[context.range.start..offset].ends_with(char::is_whitespace)
                    && !prefix.is_empty()
                {
                    prefix.push(' ');
                }
                let slots = if declarations {
                    DECLARATION_SLOTS
                } else {
                    RULE_SLOTS
                };
                slots
                    .iter()
                    .filter(|(name, _)| name.starts_with(&prefix))
                    .map(|(name, _)| (*name).into())
                    .collect()
            }
            Kind::Theme { .. } => self
                .context
                .design_system
                .theme()
                .active_tokens()
                .iter()
                .filter(|token| token.name.value.starts_with(prefix))
                .map(|token| token.name.value.clone())
                .collect(),
        };
        labels.sort();
        labels.dedup();
        Ok(labels
            .into_iter()
            .map(|label| StylesheetCompletion {
                range: context.range.clone(),
                label,
                keyword,
            })
            .collect())
    }
}
