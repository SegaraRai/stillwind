//! Shares value indexes across families that copy theme values into declarations.
//!
//! The lookup deliberately permits extra candidates. The caller verifies their full
//! CSS and registration requirements before returning a replacement.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::{
    CssMinifyOptions, Declaration, find_css_functions, fold_calc_expressions, minify_css,
    take_property_requirements,
};

use super::super::preview::{SpacingUnit, convert_pixel_dimensions, dimension};
use crate::query::QueryContext;
use crate::query::completion::value_namespaces;

#[derive(Debug, Default)]
pub(super) struct DirectValues {
    families: BTreeMap<LeanStr, BTreeSet<LeanStr>>,
    search: BTreeMap<LeanStr, BTreeSet<LeanStr>>,
    modifiers: BTreeSet<LeanStr>,
    values: HashMap<LeanStr, Vec<LeanStr>>,
    covered: BTreeSet<LeanStr>,
}

impl DirectValues {
    pub(super) fn prepare(query: &QueryContext) -> Self {
        let mut index = Self::default();
        let mut valid_tokens = BTreeSet::new();
        for (name, definitions) in query.design_system.utilities().iter() {
            let Some(root) = name.strip_suffix("-*") else {
                continue;
            };
            if definitions
                .iter()
                .all(|definition| modifier_preserves_value(&definition.body))
            {
                index.modifiers.insert(root.to_lean_str());
            }
            index.search.insert(
                root.to_lean_str(),
                definitions
                    .iter()
                    .flat_map(|definition| value_namespaces(&definition.body, "--value"))
                    .collect(),
            );
            if let Some(namespaces) = copied_namespaces(
                definitions
                    .iter()
                    .map(|definition| definition.body.as_str()),
            ) {
                index.families.insert(root.to_lean_str(), namespaces);
            }
        }
        index.modifiers.retain(|root| {
            index
                .search
                .get(root)
                .is_some_and(|namespaces| modifier_branches_are_local(root, namespaces, query))
        });
        for (name, group) in query.design_system.tokens().iter() {
            let Some(token) = group.first() else {
                continue;
            };
            let Some(value) = query.resolve_theme_values(&token.value.value) else {
                continue;
            };
            let keys = value_keys(&value, query);
            if keys.is_empty() {
                continue;
            }

            valid_tokens.insert(name.to_lean_str());
            for key in keys {
                index
                    .values
                    .entry(key)
                    .or_default()
                    .push(name.to_lean_str());
            }
        }
        for (root, namespaces) in &index.families {
            let own = format_args!("{root}-").to_lean_str();
            let foreign: Vec<_> = query
                .design_system
                .utilities()
                .iter()
                .filter_map(|(name, definitions)| {
                    let prefix = name.strip_suffix('*')?;
                    (prefix != own && (own.starts_with(prefix) || prefix.starts_with(own.as_str())))
                        .then_some((prefix, definitions))
                })
                .collect();
            for namespace in namespaces {
                for token in query.design_system.tokens().names_with_prefix(namespace) {
                    let Some(capture) = token.strip_prefix((namespace).as_str()) else {
                        continue;
                    };
                    let label = format_args!("{root}-{capture}").to_lean_str();
                    if !valid_tokens.contains(token) {
                        continue;
                    }
                    if foreign.iter().all(|(prefix, definitions)| {
                        let Some(capture) = label.strip_prefix(*prefix) else {
                            return true;
                        };
                        definitions.iter().all(|definition| {
                            rejects_named_capture(&definition.body, capture, query)
                        })
                    }) {
                        index.covered.insert(label);
                    }
                }
            }
        }
        index
    }

    pub(super) fn modifier_preserves_value(&self, roots: &BTreeSet<LeanStr>, suffix: &str) -> bool {
        suffix
            .strip_prefix('/')
            .and_then(|value| value.parse::<f64>().ok())
            .is_some_and(|value| value.is_finite() && value > 0.0)
            && roots.iter().all(|root| self.modifiers.contains(root))
    }

    pub(super) fn covers(&self, label: &str) -> bool {
        self.covered.contains(label)
    }

    pub(super) fn matching(
        &self,
        query: &QueryContext,
        roots: &BTreeSet<LeanStr>,
        value: &str,
    ) -> Vec<LeanStr> {
        let Some(value) = query.resolve_theme_values(value) else {
            return Vec::new();
        };
        let names: BTreeSet<_> = value_keys(&value, query)
            .into_iter()
            .flat_map(|key| self.values.get(&key).into_iter().flatten())
            .collect();
        let mut labels = Vec::new();
        for root in roots {
            let Some(namespaces) = self.search.get(root) else {
                continue;
            };
            for name in &names {
                for namespace in namespaces {
                    if let Some(capture) = name.strip_prefix((namespace).as_str()) {
                        labels.push(format_args!("{root}-{capture}").to_lean_str());
                    }
                }
            }
        }
        labels
    }
}

fn modifier_branches_are_local(
    root: &str,
    namespaces: &BTreeSet<LeanStr>,
    query: &QueryContext,
) -> bool {
    let own = format_args!("{root}-").to_lean_str();
    query
        .design_system
        .utilities()
        .iter()
        .all(|(name, definitions)| {
            let Some(prefix) = name.strip_suffix('*') else {
                return true;
            };
            if prefix == own || (!own.starts_with(prefix) && !prefix.starts_with(own.as_str())) {
                return true;
            }
            namespaces.iter().all(|namespace| {
                query
                    .design_system
                    .tokens()
                    .names_with_prefix(namespace)
                    .all(|token| {
                        let label = format_args!(
                            "{own}{}",
                            token.strip_prefix((namespace).as_str()).unwrap_or(token)
                        )
                        .to_lean_str();
                        let Some(capture) = label.strip_prefix(prefix) else {
                            return true;
                        };
                        definitions.iter().all(|definition| {
                            rejects_named_capture(&definition.body, capture, query)
                        })
                    })
            })
        })
}

fn copied_namespaces<'a>(mut bodies: impl Iterator<Item = &'a str>) -> Option<BTreeSet<LeanStr>> {
    let mut namespaces = BTreeSet::new();
    let mut captured = BTreeSet::new();
    let mut fixed = BTreeSet::new();
    let valid = bodies.all(|text| {
        let Ok((body, _)) = take_property_requirements(text) else {
            return false;
        };
        let modifiers = find_css_functions(&body, &["--modifier"], 0);
        if !modifiers.is_empty() && body.match_indices("--modifier(").count() == modifiers.len() {
            return true;
        }
        let Some(declarations) = Declaration::parse(&body) else {
            return false;
        };
        let mut properties = BTreeSet::new();
        for declaration in declarations {
            if !properties.insert(declaration.property.clone()) {
                return false;
            }
            let Some((_, value)) = declaration.css.split_once(':') else {
                return false;
            };
            let value = value.trim().trim_end_matches(';').trim();
            let functions = find_css_functions(
                value,
                &[
                    "--value",
                    "--value-literal",
                    "--value-map",
                    "--modifier",
                    "--modifier-map",
                    "--modifier-none",
                    "--alpha",
                    "--compat-color",
                    "--stillwind-builtin-shadow-color",
                    "--stillwind-theme-suffix",
                ],
                0,
            );
            if functions.is_empty() {
                fixed.insert(declaration.property);
                continue;
            }
            if functions.len() != 1
                || functions[0].name_index != 0
                || functions[0].start != 0
                || functions[0].end != Some(value.len())
            {
                return false;
            }
            let domains: BTreeSet<_> = value_namespaces(value, "--value").into_iter().collect();
            if domains.is_empty() || !captured.insert(declaration.property) {
                return false;
            }
            namespaces.extend(domains);
        }
        true
    });
    (valid && !namespaces.is_empty() && captured.is_disjoint(&fixed)).then_some(namespaces)
}

fn modifier_preserves_value(body: &str) -> bool {
    if !body.contains("--modifier") {
        return !body.contains("--fraction()");
    }
    let Ok((body, _)) = take_property_requirements(body) else {
        return false;
    };
    let Some(declarations) = Declaration::parse(&body) else {
        return false;
    };
    let mut preserves = false;
    let mut properties = BTreeSet::new();
    for declaration in declarations {
        if !properties.insert(declaration.property.clone()) {
            return false;
        }
        let Some((_, value)) = declaration.css.split_once(':') else {
            return false;
        };
        let value = value.trim().trim_end_matches(';').trim();
        let calls = find_css_functions(value, &["--value", "--value-literal"], 0);
        if calls.is_empty() {
            continue;
        }
        if calls.len() != 1 {
            return false;
        }
        if calls[0].name_index == 0 && calls[0].start == 0 && calls[0].end == Some(value.len()) {
            preserves = true;
        } else {
            let compact: LeanString = value
                .chars()
                .filter(|character| !character.is_ascii_whitespace())
                .collect();
            let source = &value[calls[0].start..calls[0].end.unwrap_or(value.len())];
            let source: LeanString = source
                .chars()
                .filter(|character| !character.is_ascii_whitespace())
                .collect();
            if calls[0].name_index != 1
                || compact
                    != format_args!("--compat-color({source},--alpha(--modifier()))").to_lean_str()
            {
                return false;
            }
            if value_namespaces(value, "--value-literal")
                .iter()
                .any(|namespace| namespace != "--color-")
            {
                return false;
            }
            preserves = true;
        }
    }
    preserves
}

fn rejects_named_capture(body: &str, capture: &str, query: &QueryContext) -> bool {
    let functions = find_css_functions(
        body,
        &[
            "--value",
            "--value-literal",
            "--value-map",
            "--stillwind-builtin-shadow-color",
        ],
        0,
    );
    !functions.is_empty()
        && functions.iter().all(|function| {
            if function.name_index != 0 {
                return false;
            }
            let arguments = &body[function.arguments.clone()];
            arguments.split(',').all(|argument| {
                let argument = argument.trim();
                if let Some(prefix) = argument
                    .strip_suffix('*')
                    .filter(|prefix| prefix.starts_with("--"))
                {
                    return query
                        .design_system
                        .tokens()
                        .get(&format_args!("{prefix}{capture}").to_lean_str())
                        .is_none();
                }
                if argument.starts_with('[') {
                    return true;
                }
                if argument.starts_with(['\'', '"']) {
                    return false;
                }
                match argument {
                    "number" | "integer" | "percentage" | "--spacing()" => {
                        capture != "px" && capture.trim_end_matches('%').parse::<f64>().is_err()
                    }
                    "--fraction()" => !capture.contains('/'),
                    "--stillwind-builtin-width()" | "--stillwind-builtin-height()" => ![
                        "auto", "full", "screen", "svw", "lvw", "dvw", "svh", "lvh", "dvh", "min",
                        "max", "fit", "lh",
                    ]
                    .contains(&capture),
                    argument if argument.starts_with("--") => false,
                    _ => capture != argument,
                }
            })
        })
}

fn value_keys(value: &str, query: &QueryContext) -> BTreeSet<LeanStr> {
    let options = query.canonical_options();
    let mut values = vec![fold_calc_expressions(value, None)];
    if options.px_to_rem
        && query
            .spacing_scale()
            .as_deref()
            .and_then(dimension)
            .is_some_and(|(_, unit)| unit == SpacingUnit::Rem)
    {
        values.push(convert_pixel_dimensions(
            value,
            query.options.root_font_size.get(),
        ));
    }
    values
        .into_iter()
        .flat_map(|value| {
            ["color", "width", "--sw-value"]
                .into_iter()
                .filter_map(move |property| {
                    minify_css(
                        &format_args!("&{{{property}:{value}}}").to_lean_str(),
                        &CssMinifyOptions::default(),
                    )
                    .ok()
                    .map(|output| LeanStr::from(output.css))
                })
        })
        .collect()
}
