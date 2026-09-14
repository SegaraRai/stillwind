//! Shared fixture and catalog helpers for the evaluation test modules.
//!
//! The helpers load the real vendored registry and the parity fixtures, expand CSSWG
//! formal syntax over property references, and resolve variants through the same code
//! path the compiler uses, so the suites never validate a simplified stand-in for
//! production behavior.

use stillwind_css::csswg_formal_syntax;
use stillwind_diagnostics::CompileError;

use super::*;
use crate::evaluation::variants::resolve::resolve_registered_variant_candidate;
use crate::{parse_builtin_utility_definitions, parse_builtin_variant_definitions, parse_theme};

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
            "theme.css",
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

/// The vendored registry CSS files and the name each is reported under.
pub(super) const REGISTRY_FILES: &[(&str, &str)] = &[
    (
        "builtin-metadata/tw-registry-metadata.css",
        include_str!("../../../../../packages/core/css/builtin-metadata/tw-registry-metadata.css"),
    ),
    (
        "builtin-metadata/tw-relationship-metadata.css",
        include_str!(
            "../../../../../packages/core/css/builtin-metadata/tw-relationship-metadata.css"
        ),
    ),
    (
        "builtin-utilities/tw-layout.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-layout.css"),
    ),
    (
        "builtin-utilities/tw-spacing-sizing.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-spacing-sizing.css"),
    ),
    (
        "builtin-utilities/tw-flex-grid.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-flex-grid.css"),
    ),
    (
        "builtin-utilities/tw-typography.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-typography.css"),
    ),
    (
        "builtin-utilities/tw-backgrounds.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-backgrounds.css"),
    ),
    (
        "builtin-utilities/tw-borders.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-borders.css"),
    ),
    (
        "builtin-utilities/tw-effects.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-effects.css"),
    ),
    (
        "builtin-utilities/tw-masks.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-masks.css"),
    ),
    (
        "builtin-utilities/tw-transforms.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-transforms.css"),
    ),
    (
        "builtin-utilities/tw-motion.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-motion.css"),
    ),
    (
        "builtin-utilities/tw-interactivity.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-interactivity.css"),
    ),
    (
        "builtin-utilities/tw-svg.css",
        include_str!("../../../../../packages/core/css/builtin-utilities/tw-svg.css"),
    ),
    (
        "builtin-variants/tw-relationship.css",
        include_str!("../../../../../packages/core/css/builtin-variants/tw-relationship.css"),
    ),
    (
        "builtin-variants/tw-structural.css",
        include_str!("../../../../../packages/core/css/builtin-variants/tw-structural.css"),
    ),
    (
        "builtin-variants/tw-state.css",
        include_str!("../../../../../packages/core/css/builtin-variants/tw-state.css"),
    ),
    (
        "builtin-variants/tw-dynamic-selector.css",
        include_str!("../../../../../packages/core/css/builtin-variants/tw-dynamic-selector.css"),
    ),
    (
        "builtin-variants/tw-conditional.css",
        include_str!("../../../../../packages/core/css/builtin-variants/tw-conditional.css"),
    ),
    (
        "builtin-variants/sw-conditional.css",
        include_str!("../../../../../packages/core/css/builtin-variants/sw-conditional.css"),
    ),
    (
        "builtin-metadata/tw-properties.css",
        include_str!("../../../../../packages/core/css/builtin-metadata/tw-properties.css"),
    ),
    (
        "builtins.css",
        include_str!("../../../../../packages/core/css/builtins.css"),
    ),
];

/// Every utility the vendored registry defines, with its originating file.
pub(super) fn registry_utilities() -> Vec<(String, CustomUtility)> {
    let mut definitions = Vec::new();
    for (file, css) in REGISTRY_FILES {
        let mut occurrences = HashMap::<String, usize>::new();
        for mut utility in parse_builtin_utility_definitions(css) {
            utility.body = utility.body.trim().to_owned();
            let occurrence = occurrences.entry(utility.name.clone()).or_default();
            *occurrence += 1;
            definitions.push((format!("{file}:{}#{occurrence}", utility.name), utility));
        }
    }
    definitions
}

/// Every variant the vendored registry defines, with its originating file.
pub(super) fn registry_variants() -> Vec<(String, CustomVariant)> {
    let mut definitions = Vec::new();
    for (file, css) in REGISTRY_FILES {
        let mut occurrences = HashMap::<String, usize>::new();
        for variant in parse_builtin_variant_definitions(css) {
            let occurrence = occurrences.entry(variant.name.clone()).or_default();
            *occurrence += 1;
            definitions.push((format!("{file}:{}#{occurrence}", variant.name), variant));
        }
    }
    definitions
}

/// The contents of the first fenced block under a markdown heading.
pub(super) fn fenced_section<'a>(
    section: &'a str,
    heading: &str,
    language: &str,
) -> Option<&'a str> {
    let after_heading = section.split_once(heading)?.1;
    let fence = format!("```{language}");
    let after_fence = after_heading.split_once(&fence)?.1;
    after_fence.split_once("```").map(|(body, _)| body.trim())
}

/// Candidate spellings grouped by the catalog section that lists them.
pub(super) fn parity_cases(markdown: &str) -> Vec<(&str, Vec<&str>)> {
    markdown
        .split("\n## `")
        .skip(1)
        .filter_map(|section| {
            let input = fenced_section(section, "### Input CSS", "css")?;
            let candidates = fenced_section(section, "### Candidates", "text")?
                .split_whitespace()
                .collect::<Vec<_>>();
            Some((input, candidates))
        })
        .collect()
}

/// The registry definitions whose names match a candidate spelling.
pub(super) fn matched_registry_utilities<'a>(
    definitions: &'a [(String, CustomUtility)],
    lookup: &str,
) -> Vec<(usize, &'a CustomUtility)> {
    let matches = |definition: &CustomUtility| {
        definition.name == lookup
            || definition
                .name
                .strip_suffix('*')
                .is_some_and(|prefix| lookup.starts_with(prefix))
    };
    definitions
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, (_, definition))| definition.name.ends_with('*') && matches(definition))
        .map(|(index, (_, definition))| (index, definition))
        .chain(
            definitions
                .iter()
                .enumerate()
                .rev()
                .filter(|(_, (_, definition))| definition.name == lookup)
                .map(|(index, (_, definition))| (index, definition)),
        )
        .collect()
}

/// Resolves a variant through its definition, bypassing the theme wrapper.
pub(super) fn resolve_registry_variant(
    definition: &CustomVariant,
    value: &str,
    theme: &ThemeIndex,
) -> (bool, Option<VariantApplication>) {
    let candidates = registered_variant_candidates(std::slice::from_ref(definition), value);
    let matched = !candidates.is_empty();
    let resolved = candidates.into_iter().find_map(|(custom, capture)| {
        resolve_registered_variant_candidate(custom, capture, value, theme)
    });
    (matched, resolved)
}

/// Resolves every definition that can capture a variant spelling.
pub(super) fn resolve_registry_variant_branches(
    definitions: &[(String, CustomVariant)],
    excluded: Option<usize>,
    value: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    let resolve = |custom: &CustomVariant| {
        registered_variant_candidates(std::slice::from_ref(custom), value)
            .into_iter()
            .find_map(|(custom, capture)| {
                resolve_registered_variant_candidate(custom, capture, value, theme)
            })
    };
    definitions
        .iter()
        .enumerate()
        .rev()
        .filter(|(index, (_, custom))| Some(*index) != excluded && custom.name == value)
        .find_map(|(_, (_, custom))| resolve(custom))
        .or_else(|| {
            definitions
                .iter()
                .enumerate()
                .rev()
                .filter(|(index, (_, custom))| {
                    Some(*index) != excluded && custom.wildcard.is_some()
                })
                .find_map(|(_, (_, custom))| resolve(custom))
        })
}

/// Parses a fixture buffer wrapped in the standard package CSS.
pub(super) fn parse_fixture_theme(source: &SourceFile) -> Result<ThemeIndex, CompileError> {
    parse_theme(
        &crate::css_fixture::source(
            source.id(),
            source.path(),
            stillwind_fixture_css(source.text()),
        )
        .unwrap(),
    )
}

/// Runs an upstream parity fixture against Stillwind's registry, mirroring
/// `tools/lib/stillwind-fixture-css.ts`.
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
