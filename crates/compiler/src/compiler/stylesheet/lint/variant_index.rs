//! Reverse index from the CSS that `@variant` expands to, to variant spellings.
//!
//! [`VariantIndex::new`] expands each variant spelling that completion offers around a
//! probe declaration, in the same way that `@variant` expands it inside a style rule.
//! A spelling is indexed only when its expansion is one chain of one or two nested
//! headers around the unchanged probe. Variants that emit several rules, contribute
//! declarations, or guard declarations at runtime have no such chain, because the
//! authored headers alone cannot express them.

use std::collections::HashMap;

use lean_string::LeanStr;

use stillwind_css::{CssNodeKind, parse_structure};
use stillwind_design_system::{ThemeIndex, resolve_variant};

use super::super::authored_directives::{parse_variant_sequence, render_authored_variant_path};
use super::headers::{Header, Level, header_key};
use crate::DesignSystemQuery;
use crate::identifiers::{DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierContext, IdentifierMode};

/// Custom property that marks the body of an expansion.
const PROBE_PROPERTY: &str = "--sw-variant-probe";

/// Variant spellings indexed by the comparable headers of their expansions.
pub(super) struct VariantIndex {
    /// Spellings by headers compared without unit conversion.
    exact: HashMap<Vec<Header>, LeanStr>,
    /// Spellings by headers whose `@media` lengths are converted to `px`. It is empty
    /// unless the rule option `pxToRem` is enabled.
    converted: HashMap<Vec<Header>, LeanStr>,
}

/// A variant whose `@variant` block applies the same CSS as authored headers.
pub(super) struct VariantMatch<'a> {
    /// Variant spelling, such as `sm`.
    pub(super) variant: &'a str,
    /// Whether the headers match only after `px` lengths are read as `rem`.
    pub(super) px_to_rem: bool,
}

impl VariantIndex {
    /// Indexes the variant spellings of `query`, whose design system owns `theme`.
    ///
    /// When several spellings expand to the same headers, the shortest spelling wins,
    /// and lexical order breaks ties, as `prefer-canonical-utilities` ranks them. With
    /// `px_to_rem`, [`Self::find`] also matches `px` lengths with `rem` lengths.
    pub(super) fn new(query: &DesignSystemQuery, theme: &ThemeIndex, px_to_rem: bool) -> Self {
        let identifiers = IdentifierContext::new(
            GeneratorSet::default(),
            IdentifierMode::Release,
            DEFAULT_HASH_LENGTH,
        );
        let mut spellings = query.variant_vocabulary().into_iter().collect::<Vec<_>>();
        spellings.sort_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
        let mut index = Self {
            exact: HashMap::new(),
            converted: HashMap::new(),
        };
        for spelling in spellings {
            let Some(css) = expansion(&spelling, theme, &identifiers) else {
                continue;
            };
            let Some((exact, converted)) = probe_keys(&css, px_to_rem) else {
                continue;
            };
            if let Some(key) = exact {
                index.exact.entry(key).or_insert_with(|| spelling.clone());
            }
            if let Some(key) = converted {
                index.converted.entry(key).or_insert(spelling);
            }
        }
        index
    }

    /// Returns the variant whose expansion has the same headers as `levels`, outermost
    /// first, or `None` when no indexed variant matches.
    pub(super) fn find(&self, levels: &[Level<'_>]) -> Option<VariantMatch<'_>> {
        if let Some(variant) = header_key(levels, false).and_then(|key| self.exact.get(&key)) {
            return Some(VariantMatch {
                variant,
                px_to_rem: false,
            });
        }
        if self.converted.is_empty() {
            return None;
        }
        let variant = self.converted.get(&header_key(levels, true)?)?;
        Some(VariantMatch {
            variant,
            px_to_rem: true,
        })
    }
}

/// Returns the CSS that `@variant <spelling>` produces around the probe declaration, or
/// `None` when the spelling is not one variant that resolves.
fn expansion(
    spelling: &str,
    theme: &ThemeIndex,
    identifiers: &IdentifierContext,
) -> Option<String> {
    let [variant] = parse_variant_sequence(spelling)?.try_into().ok()?;
    let application = resolve_variant(&variant, theme).ok()?;
    let body = format!("{PROBE_PROPERTY}:0;");
    render_authored_variant_path(&body, std::slice::from_ref(&application), identifiers)
        .ok()
        .map(|css| css.to_string())
}

/// Keys of one expansion: without unit conversion, and with `@media` lengths converted
/// to `px` when `pxToRem` is enabled.
type ProbeKeys = (Option<Vec<Header>>, Option<Vec<Header>>);

/// Returns the keys of the headers of `css` without and with unit conversion, when
/// `css` is one chain of one or two blocks that contains only the probe declaration
/// with its value unchanged. The second key is `None` unless `px_to_rem` is set.
fn probe_keys(css: &str, px_to_rem: bool) -> Option<ProbeKeys> {
    let root = parse_structure(css);
    let mut levels = Vec::new();
    let mut nodes = root.as_slice();
    loop {
        let [node] = nodes else {
            return None;
        };
        match &node.kind {
            CssNodeKind::StyleRule { selector, block } => {
                levels.push(Level::Rule(&css[selector.clone()]));
                nodes = &block.children;
            }
            CssNodeKind::AtRule {
                name,
                prelude,
                block: Some(block),
            } => {
                levels.push(Level::AtRule(name, &css[prelude.clone()]));
                nodes = &block.children;
            }
            CssNodeKind::Declaration { property, value }
                if &css[property.clone()] == PROBE_PROPERTY && &css[value.clone()] == "0" =>
            {
                break;
            }
            _ => {
                return None;
            }
        }
    }
    if !(1..=2).contains(&levels.len()) {
        return None;
    }
    Some((
        header_key(&levels, false),
        px_to_rem.then(|| header_key(&levels, true)).flatten(),
    ))
}
