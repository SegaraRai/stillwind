use crate::variants::resolve::resolve_registered_variant_candidate;

use super::*;

pub(super) fn canonical_builtin_body(value: &str) -> String {
    static FORMATS: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    stillwind_css::canonical_builtin_body(
        value,
        FORMATS.get_or_init(|| {
            stillwind_css::parse_function_formats(crate::test_defaults::BUILTIN_REGISTRY_CSS)
        }),
    )
}

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

pub(super) fn expanded_csswg_syntax(
    property: &str,
    visited: &mut BTreeSet<String>,
) -> Option<String> {
    if !visited.insert(property.to_owned()) {
        return Some(String::new());
    }
    let syntax = stillwind_css::csswg_formal_syntax(property)?;
    let mut expanded = syntax.to_owned();
    for referenced_property in property_references(syntax) {
        expanded.push(' ');
        expanded.push_str(&expanded_csswg_syntax(referenced_property, visited)?);
    }
    Some(expanded)
}

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

pub(super) fn has_complex_csswg_grammar(syntax: &str, terms: &BTreeSet<&str>) -> bool {
    syntax.contains("&&")
        || syntax.contains("||")
        || terms.iter().any(|term| !SIMPLE_CSSWG_TERMS.contains(term))
}

pub(super) fn theme() -> ThemeIndex {
    parse_fixture_theme(&SourceFile::new(
            FileId(1),
            "theme.css",
            "@stillwind-marker group; @stillwind-marker peer; @theme { --color-brand: red; --breakpoint-sm: 40rem; --breakpoint-md: 48rem; --breakpoint-lg: 64rem; --breakpoint-xl: 80rem; --breakpoint-2xl: 96rem; }",
        ).unwrap())
        .unwrap()
}

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

pub(super) const REGISTRY_FILES: &[(&str, &str)] = &[
    (
        "registry-metadata.css",
        include_str!("../../../../packages/core/css/builtins/registry-metadata.css"),
    ),
    (
        "relationship-metadata.css",
        include_str!("../../../../packages/core/css/builtins/relationship-metadata.css"),
    ),
    (
        "layout-utilities.css",
        include_str!("../../../../packages/core/css/builtins/layout-utilities.css"),
    ),
    (
        "spacing-sizing-utilities.css",
        include_str!("../../../../packages/core/css/builtins/spacing-sizing-utilities.css"),
    ),
    (
        "flex-grid-utilities.css",
        include_str!("../../../../packages/core/css/builtins/flex-grid-utilities.css"),
    ),
    (
        "typography-utilities.css",
        include_str!("../../../../packages/core/css/builtins/typography-utilities.css"),
    ),
    (
        "backgrounds-utilities.css",
        include_str!("../../../../packages/core/css/builtins/backgrounds-utilities.css"),
    ),
    (
        "borders-utilities.css",
        include_str!("../../../../packages/core/css/builtins/borders-utilities.css"),
    ),
    (
        "effects-utilities.css",
        include_str!("../../../../packages/core/css/builtins/effects-utilities.css"),
    ),
    (
        "masks-utilities.css",
        include_str!("../../../../packages/core/css/builtins/masks-utilities.css"),
    ),
    (
        "transforms-utilities.css",
        include_str!("../../../../packages/core/css/builtins/transforms-utilities.css"),
    ),
    (
        "motion-utilities.css",
        include_str!("../../../../packages/core/css/builtins/motion-utilities.css"),
    ),
    (
        "interactivity-utilities.css",
        include_str!("../../../../packages/core/css/builtins/interactivity-utilities.css"),
    ),
    (
        "svg-utilities.css",
        include_str!("../../../../packages/core/css/builtins/svg-utilities.css"),
    ),
    (
        "relationship-variants.css",
        include_str!("../../../../packages/core/css/builtins/relationship-variants.css"),
    ),
    (
        "structural-variants.css",
        include_str!("../../../../packages/core/css/builtins/structural-variants.css"),
    ),
    (
        "state-variants.css",
        include_str!("../../../../packages/core/css/builtins/state-variants.css"),
    ),
    (
        "dynamic-selector-variants.css",
        include_str!("../../../../packages/core/css/builtins/dynamic-selector-variants.css"),
    ),
    (
        "conditional-variants.css",
        include_str!("../../../../packages/core/css/builtins/conditional-variants.css"),
    ),
    (
        "properties.css",
        include_str!("../../../../packages/core/css/builtins/properties.css"),
    ),
    (
        "builtins.css",
        include_str!("../../../../packages/core/css/builtins.css"),
    ),
];

pub(super) fn registry_utilities() -> Vec<(String, CustomUtility)> {
    let mut definitions = Vec::new();
    for (file, css) in REGISTRY_FILES {
        let mut occurrences = HashMap::<String, usize>::new();
        for mut utility in stillwind_css::parse_builtin_utility_definitions(css) {
            utility.body = canonical_builtin_body(&utility.body);
            let occurrence = occurrences.entry(utility.name.clone()).or_default();
            *occurrence += 1;
            definitions.push((format!("{file}:{}#{occurrence}", utility.name), utility));
        }
    }
    definitions
}

pub(super) fn registry_variants() -> Vec<(String, CustomVariant)> {
    let mut definitions = Vec::new();
    for (file, css) in REGISTRY_FILES {
        let mut occurrences = HashMap::<String, usize>::new();
        for variant in stillwind_css::parse_builtin_variant_definitions(css) {
            let occurrence = occurrences.entry(variant.name.clone()).or_default();
            *occurrence += 1;
            definitions.push((format!("{file}:{}#{occurrence}", variant.name), variant));
        }
    }
    definitions
}

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
                .filter(|(index, (_, custom))| Some(*index) != excluded && custom.wildcard)
                .find_map(|(_, (_, custom))| resolve(custom))
        })
}

pub(super) fn parse_fixture_theme(
    source: &SourceFile,
) -> Result<ThemeIndex, stillwind_css::ThemeError> {
    stillwind_css::parse_theme(
        &crate::css_fixture::source(source.id(), source.path(), source.text()).unwrap(),
    )
}
