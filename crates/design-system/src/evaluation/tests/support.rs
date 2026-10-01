//! Shared fixture and catalog helpers for the evaluation test modules.
//!
//! The helpers load the real vendored registry and the parity fixtures, expand CSSWG
//! formal syntax over property references, and resolve variants through the same code
//! path the compiler uses, so the suites never validate a simplified stand-in for
//! production behavior.

use stillwind_css::csswg_formal_syntax;
use stillwind_diagnostics::CompileError;
use stillwind_source::CssSourceName;

use super::*;
use crate::parse_theme;

/// The formal-syntax terms the catalog treats as primitives.
pub(super) const SIMPLE_CSSWG_TERMS: &[&str] = &[
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
pub(super) fn property_references(syntax: &str) -> Vec<&str> {
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
    visited: &mut BTreeSet<String>,
) -> Option<String> {
    if !visited.insert(property.to_owned()) {
        return Some(String::new());
    }
    let syntax = csswg_formal_syntax(property)?;
    let mut expanded = syntax.to_owned();
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

/// Rows of a markdown catalog section, up to the next heading.
pub(super) fn catalog_section<'a>(markdown: &'a str, heading: &str) -> Vec<&'a str> {
    let start = markdown
        .find(heading)
        .and_then(|start| {
            markdown[start..]
                .find("```text\n")
                .map(|offset| start + offset + 8)
        })
        .expect("catalog section must contain candidates");
    markdown[start..]
        .split_once("\n```")
        .expect("catalog section must close")
        .0
        .split_whitespace()
        .collect()
}

/// Parses a fixture buffer wrapped in the standard package CSS.
pub(super) fn parse_fixture_theme(source: &SourceFile) -> Result<ThemeIndex, CompileError> {
    parse_theme(
        &crate::css_fixture::source(source.id(), stillwind_fixture_css(source.text())).unwrap(),
        &CssSourceName::from("test.css"),
    )
}

/// Runs an upstream parity fixture against Stillwind's registry, mirroring
/// `tools/tailwind/stillwind-css.ts`.
fn stillwind_fixture_css(css: &str) -> String {
    // Only upstream fixture inputs select the registry explicitly; the other
    // fixtures keep relying on the default Stillwind entry point.
    if !css.contains("tailwindcss") {
        return css.to_owned();
    }
    let selected = css
        .replace(
            "@import \"tailwindcss/theme\"",
            "@import \"stillwind/theme.css\"",
        )
        .replace(
            "@import 'tailwindcss/theme'",
            "@import 'stillwind/theme.css'",
        )
        .replace(
            "@import \"tailwindcss/preflight\"",
            "@import \"stillwind/preflight.css\"",
        )
        .replace(
            "@import 'tailwindcss/preflight'",
            "@import 'stillwind/preflight.css'",
        )
        .replace(
            "@import \"tailwindcss/utilities\"",
            "@import \"stillwind/utilities.css\"",
        )
        .replace(
            "@import 'tailwindcss/utilities'",
            "@import 'stillwind/utilities.css'",
        )
        .replace("@import \"tailwindcss\"", "@import \"stillwind\"")
        .replace("@import 'tailwindcss'", "@import 'stillwind'");
    // `source(...)` scopes which files Tailwind scans; the fixture candidates are
    // explicit, so the modifier does not change what this audit exercises.
    let selected = selected
        .replace(
            "@import \"stillwind\" source(none);",
            "@import \"stillwind\";",
        )
        .replace("@import 'stillwind' source(none);", "@import 'stillwind';");
    let has_entry = [
        "@import \"stillwind\"",
        "@import 'stillwind'",
        "@import \"stillwind/builtins.css\"",
        "@import 'stillwind/builtins.css'",
    ]
    .iter()
    .any(|entry| selected.contains(entry));
    if has_entry {
        selected
    } else {
        format!("{selected}\n@import \"stillwind/builtins.css\";")
    }
}

#[path = "../../../../css/test-support/assertions.rs"]
mod css_assertions;
pub(super) use css_assertions::{CssAssertions, css_tokens};
