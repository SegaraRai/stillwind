//! Completion labels and replacement ranges for authored CSS.

use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::UtilityResolutionError;

use super::context::{Context, Kind, context};
use super::directives::{DECLARATION_SLOTS, DIRECTIVES, RULE_SLOTS, directive_name};
use crate::query::{CandidateContext, CompletionInput, DesignSystemQuery, last_top_level};

/// Editor discovery settings. These options do not alter compilation or stylesheet
/// lint.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StylesheetCompletionOptions {
    /// Offers registry metadata in addition to authoring directives. Defaults to false.
    /// Required local syntax remains available inside matching definitions.
    pub registry_directives: bool,
    /// Includes empty arbitrary-variant placeholders for clients that support snippets.
    pub snippets: bool,
    /// Finite host plugin candidates. Variant labels end in `:`; no callback runs during discovery.
    pub catalog: Vec<LeanStr>,
}

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
    /// True when this item inserts a variant or a variant prefix.
    pub variant: bool,
    /// Audience and provenance for at-rule names. Other inputs omit this detail.
    pub detail: Option<LeanStr>,
    /// Lexical sort key that ranks at-rule names by location, audience, and provenance.
    /// Other inputs retain lexical label order.
    pub sort_text: Option<LeanStr>,
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
    /// `options` controls editor discovery independently of compiler settings.
    /// Returns unique labels, with directive names ordered by location, audience, and
    /// provenance and other labels ordered lexically. Every replacement range addresses
    /// the original `css`, including any token suffix after `offset`.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn complete_stylesheet(
        &self,
        css: &str,
        offset: usize,
        options: StylesheetCompletionOptions,
    ) -> Result<Vec<StylesheetCompletion>, UtilityResolutionError> {
        let Some(context) = context(css, offset) else {
            return Ok(Vec::new());
        };
        let prefix = &css[context.range.start..offset];
        let keyword = matches!(context.kind, Kind::Directive);
        let mut labels: Vec<LeanStr> = match context.kind {
            Kind::Directive => return Ok(directive_completions(&context, prefix, &options)),
            Kind::DirectiveArgument { directive, first } => {
                let theme = self.context.design_system.theme();
                let values: Vec<LeanStr> = match directive {
                    "stillwind-require-properties" => self
                        .context
                        .design_system
                        .properties()
                        .names()
                        .map(Into::into)
                        .collect(),
                    "stillwind-intrinsic" if first => ["builtin-if-variant", "builtin-not-variant"]
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    "stillwind-format" if first => {
                        theme.declaration_formats().keys().cloned().collect()
                    }
                    "stillwind-format" => theme
                        .declaration_formats()
                        .values()
                        .flat_map(|value| value.split_whitespace().map(Into::into))
                        .collect(),
                    _ => Vec::new(),
                };
                values
                    .into_iter()
                    .filter(|label| label.starts_with(prefix))
                    .collect()
            }
            Kind::Apply | Kind::Variant => {
                let variants = matches!(context.kind, Kind::Variant);
                let mut labels =
                    self.stylesheet_candidate_labels(prefix, variants, options.snippets)?;
                labels.extend(catalog_labels(prefix, variants, options.catalog));
                labels
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
                variant: matches!(context.kind, Kind::Variant)
                    || matches!(context.kind, Kind::Apply) && label.ends_with(':'),
                range: context.range.clone(),
                label,
                keyword,
                detail: None,
                sort_text: None,
            })
            .collect())
    }

    /// Completes `prefix` as an applied utility or variant. Variant labels omit the
    /// trailing colon. `snippets` enables empty brackets for arbitrary variant values.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    fn stylesheet_candidate_labels(
        &self,
        prefix: &str,
        variants: bool,
        snippets: bool,
    ) -> Result<Vec<LeanStr>, UtilityResolutionError> {
        Ok(self
            .complete_candidate(CompletionInput {
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
                snippets,
            })?
            .into_iter()
            .filter_map(|item| {
                if variants {
                    item.label.strip_suffix(':').map(Into::into)
                } else {
                    Some(item.label)
                }
            })
            .collect())
    }
}

/// Completes host candidates only inside an applied utility or variant input.
fn catalog_labels(prefix: &str, variants: bool, catalog: Vec<LeanStr>) -> Vec<LeanStr> {
    let split = last_top_level(prefix, ':').map_or(0, |index| index + 1);
    let (before, partial) = prefix.split_at(split);
    catalog
        .into_iter()
        .filter_map(|name| {
            let name = if variants {
                name.strip_suffix(':')?
            } else {
                name.as_str()
            };
            name.starts_with(partial)
                .then(|| format_args!("{before}{name}").to_lean_str())
        })
        .take(200)
        .collect()
}

/// Offers names for the captured cursor positions and discovery settings.
/// `prefix` is the original at-rule token before the cursor. Returned ranges copy
/// `context.range`, and sort keys retain the same order in LSP clients.
fn directive_completions(
    context: &Context,
    prefix: &str,
    options: &StylesheetCompletionOptions,
) -> Vec<StylesheetCompletion> {
    let prefix = directive_name(prefix);
    let mut directives: Vec<_> = DIRECTIVES
        .iter()
        .filter(|directive| directive.visible(context.positions, options.registry_directives))
        .filter_map(|directive| {
            let label = format_args!("@{}", directive.name).to_lean_str();
            label
                .starts_with(prefix.as_str())
                .then(|| StylesheetCompletion {
                    range: context.range.clone(),
                    sort_text: Some(
                        format_args!("{:02}-{label}", directive.priority(context.positions))
                            .to_lean_str(),
                    ),
                    detail: Some(directive.detail()),
                    label,
                    keyword: true,
                    variant: false,
                })
        })
        .collect();
    directives.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
    directives
}
