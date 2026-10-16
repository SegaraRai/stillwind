//! Theme construction and CSSWG syntax helpers for evaluation tests.
//!
//! [`theme`] parses package CSS through the crate's fixture parser. The syntax
//! helpers expand CSSWG property references for comparisons with builtin domains.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::csswg_formal_syntax;

use super::*;
use crate::test_support::parse_fixture_theme;

pub(super) fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
    super::super::resolve_utility(raw, theme).unwrap().ok()
}

pub(super) fn resolve_animation_option(raw: &str, theme: &ThemeIndex) -> Option<LeanString> {
    super::super::resolve_animation_option(raw, theme).unwrap()
}

/// The formal-syntax terms the catalog treats as primitives.
const SIMPLE_CSSWG_TERMS: &[&str] = &[
    "angle",
    "angle-percentage",
    "color",
    "easing-function",
    "image",
    "integer",
    "length",
    "length-percentage",
    "line-width",
    "number",
    "number-percentage",
    "percentage",
    "position",
    "ratio",
    "time",
    "url",
];

/// Property names a formal syntax references through `<'property'>`.
fn property_references(syntax: &str) -> Vec<&str> {
    let mut references = Vec::new();
    let mut remainder = syntax;
    while let Some(start) = remainder.find("<'") {
        let after_start = &remainder[start + 2..];
        let Some(end) = after_start.find("'>") else {
            break;
        };
        references.push(&after_start[..end]);
        remainder = &after_start[end + 2..];
    }
    references
}

/// Expands a property's formal syntax with the syntaxes it references.
///
/// `visited` breaks reference cycles; `None` means an unknown property or a reference
/// the CSS data does not describe.
pub(super) fn expanded_csswg_syntax(
    property: &str,
    visited: &mut BTreeSet<LeanStr>,
) -> Option<LeanString> {
    if !visited.insert(property.to_lean_str()) {
        return Some(LeanString::new());
    }
    let syntax = csswg_formal_syntax(property)?;
    let mut expanded = syntax.to_lean_string();
    for referenced_property in property_references(syntax) {
        expanded.push(' ');
        expanded.push_str(&expanded_csswg_syntax(referenced_property, visited)?);
    }
    Some(expanded)
}

/// Splits a CSS value into the formal-syntax terms it mentions.
pub(super) fn formal_terms(syntax: &str) -> BTreeSet<&str> {
    let mut terms = BTreeSet::new();
    let mut remainder = syntax;
    while let Some(start) = remainder.find('<') {
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find('>') else {
            break;
        };
        if after_start.starts_with('\'') {
            remainder = &after_start[end + 1..];
            continue;
        }
        let term = after_start[..end]
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
            .next()
            .unwrap_or_default();
        if !term.is_empty() {
            terms.insert(term);
        }
        remainder = &after_start[end + 1..];
    }
    terms
}

/// Reads a property's own value domains without following references.
pub(super) fn direct_value_domains(value: &str) -> Vec<ValueType> {
    if !value.trim_start().starts_with("--value(") {
        return Vec::new();
    }
    let mut domains = Vec::new();
    let mut remainder = value;
    while let Some(start) = remainder.find('[') {
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find(']') else {
            break;
        };
        if let Some(domain) = ValueType::from_hint(&after_start[..end]) {
            domains.push(domain);
        }
        remainder = &after_start[end + 1..];
    }
    domains
}

/// Whether a formal syntax uses constructs the simple-term model does not cover.
pub(super) fn has_complex_csswg_grammar(syntax: &str, terms: &BTreeSet<&str>) -> bool {
    syntax.contains("&&")
        || syntax.contains("||")
        || terms.iter().any(|term| !SIMPLE_CSSWG_TERMS.contains(term))
}

/// A theme parsed from the standard package, for tests that need built-ins.
pub(super) fn theme() -> ThemeIndex {
    parse_fixture_theme(&SourceFile::new(
            FileId(1),
            None,
            None,
            "@stillwind-marker group; @stillwind-marker peer; @theme { --color-brand: red; --breakpoint-sm: 40rem; --breakpoint-md: 48rem; --breakpoint-lg: 64rem; --breakpoint-xl: 80rem; --breakpoint-2xl: 96rem; }",
        ).unwrap())
        .unwrap()
}
