//! Shares value indexes across families that copy theme values into declarations.
//!
//! The lookup deliberately permits extra candidates. The caller verifies their full CSS
//! and registration requirements before returning a replacement.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use cssparser::Token;
use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{
    CssMinifyOptions, Declaration, find_css_functions, fold_calc_expressions, minify_css,
    split_once_top_level, split_top_level,
};
use stillwind_design_system::{NumericDomain, placeholder_namespaces, take_property_requirements};

use super::super::index::uses_rem;
use super::super::preview::convert_pixel_dimensions;
use crate::query::QueryContext;

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
                    .flat_map(|definition| placeholder_namespaces(&definition.body, "--value"))
                    .collect(),
            );
            // An overload that rejects every theme label cannot change the indexed CSS.
            if let Some(namespaces) = copied_namespaces(
                definitions
                    .iter()
                    .filter(|definition| {
                        may_accept_theme_label(&definition.body, &index.search[root], query)
                    })
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

/// Checks whether the utility body `body` may accept a label built from an active token
/// in one of `namespaces`. Returns false only when `body` rejects every such token.
fn may_accept_theme_label(
    body: &str,
    namespaces: &BTreeSet<LeanStr>,
    query: &QueryContext,
) -> bool {
    namespaces.iter().any(|namespace| {
        query
            .design_system
            .tokens()
            .names_with_prefix(namespace)
            .any(|token| {
                let capture = token.strip_prefix(namespace.as_str()).unwrap_or(token);
                !rejects_named_capture(body, capture, query)
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
            let domains: BTreeSet<_> = placeholder_namespaces(value, "--value")
                .into_iter()
                .collect();
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
        let value = compat_color_value(value).unwrap_or(value);
        let calls = find_css_functions(value, &["--value"], 0);
        if calls.is_empty() {
            continue;
        }
        if calls.len() != 1 || calls[0].start != 0 || calls[0].end != Some(value.len()) {
            return false;
        }
        preserves = true;
    }
    preserves
}

/// Extracts the color operand when its alpha comes directly from the slash modifier.
/// Only positive numeric modifiers use this proof. Theme-based modifier
/// domains and a second alpha can erase color differences, so they remain unproven.
fn compat_color_value(value: &str) -> Option<&str> {
    let arguments = single_function_arguments(value, "--compat-color")?;
    let (color, alpha) = split_once_top_level(arguments, |token| matches!(token, Token::Comma));
    let alpha = single_function_arguments(alpha?.trim(), "--alpha")?;
    let modifier = single_function_arguments(alpha.trim(), "--modifier")?;
    modifier.trim().is_empty().then_some(color.trim())
}

/// Returns the arguments when `value` consists of one complete call to `name`.
fn single_function_arguments<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let call = find_css_functions(value, &[name], 0).into_iter().next()?;
    (call.start == 0 && call.end == Some(value.len())).then(|| &value[call.arguments])
}

fn rejects_named_capture(body: &str, capture: &str, query: &QueryContext) -> bool {
    let functions = find_css_functions(
        body,
        &["--value", "--value-map", "--stillwind-builtin-shadow-color"],
        0,
    );
    !functions.is_empty()
        && functions.iter().all(|function| {
            if function.name_index != 0 {
                return false;
            }
            let arguments = &body[function.arguments.clone()];
            split_top_level(arguments, |token| matches!(token, Token::Comma))
                .into_iter()
                .all(|argument| {
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
                    let numeric = capture.trim_end_matches('%').parse::<f64>().is_ok();
                    if let Some(namespace) = query
                        .design_system
                        .theme()
                        .namespaces()
                        .scale_domain(argument)
                    {
                        return !numeric
                            && namespace
                                .scale
                                .as_ref()
                                .is_none_or(|scale| !scale.keys.contains_key(capture));
                    }
                    if let Some(numeric) = NumericDomain::parse(argument) {
                        return numeric.resolve(capture).is_none();
                    }
                    match argument {
                        "--fraction()" => !capture.contains('/'),
                        "--stillwind-builtin-width()" | "--stillwind-builtin-height()" => ![
                            "full", "screen", "svw", "lvw", "dvw", "svh", "lvh", "dvh", "min",
                            "max", "fit", "lh",
                        ]
                        .contains(&capture),
                        argument if argument.starts_with("--") => false,
                        _ => capture != argument,
                    }
                })
        })
}

/// Builds candidate lookup keys by minifying `value` for each of [`KEY_PROPERTIES`].
///
/// Adds a pixel-to-rem form when the query permits it and one of its theme scales uses
/// rem. These keys can admit extra candidates, so callers must compare complete CSS and
/// registration requirements before accepting a replacement.
fn value_keys(value: &str, query: &QueryContext) -> BTreeSet<LeanStr> {
    let options = query.canonical_options();
    let mut values = vec![fold_calc_expressions(value, None)];
    if options.px_to_rem && query.theme_scales().iter().any(uses_rem) {
        let converted = convert_pixel_dimensions(value, query.options.root_font_size.get());
        if converted != values[0] {
            values.push(converted);
        }
    }
    values
        .iter()
        .flat_map(|value| minified_keys(value))
        .collect()
}

/// The properties whose minification gives the keys of a value. Minification depends on
/// the receiving property, so these declarations give keys for colors, lengths, and
/// untyped token values.
const KEY_PROPERTIES: [&str; 4] = ["color", "width", "font-family", "--sw-value"];

/// Returns `&{property:value}` minified for each property of [`KEY_PROPERTIES`].
///
/// The function minifies one rule that contains all declarations. It uses the
/// separate minification of [`separately_minified_keys`] when the rule cannot be split
/// back into one declaration for each property in order, such as when `value` contains
/// `;` or when the minifier drops a declaration.
fn minified_keys(value: &str) -> Vec<LeanStr> {
    let mut rule = String::from("&{");
    for (index, property) in KEY_PROPERTIES.iter().enumerate() {
        if index > 0 {
            rule.push(';');
        }
        rule.push_str(property);
        rule.push(':');
        rule.push_str(value);
    }
    rule.push('}');
    if let Ok(output) = minify_css(&rule, &CssMinifyOptions::default())
        && let Some(body) = output
            .css
            .strip_prefix("&{")
            .and_then(|body| body.strip_suffix('}'))
    {
        let declarations: Vec<_> = split_top_level(body, |token| matches!(token, Token::Semicolon))
            .into_iter()
            .map(str::trim)
            .filter(|declaration| !declaration.is_empty())
            .collect();
        if declarations.len() == KEY_PROPERTIES.len()
            && declarations
                .iter()
                .zip(KEY_PROPERTIES)
                .all(|(declaration, property)| {
                    declaration
                        .strip_prefix(property)
                        .is_some_and(|rest| rest.starts_with(':'))
                })
        {
            return declarations
                .into_iter()
                .map(|declaration| format_args!("&{{{declaration}}}").to_lean_str())
                .collect();
        }
    }
    separately_minified_keys(value)
}

/// Returns `&{property:value}` minified separately for each property of
/// [`KEY_PROPERTIES`]. A property whose rule cannot be minified gives no key.
fn separately_minified_keys(value: &str) -> Vec<LeanStr> {
    KEY_PROPERTIES
        .into_iter()
        .filter_map(|property| {
            minify_css(
                &format_args!("&{{{property}:{value}}}").to_lean_str(),
                &CssMinifyOptions::default(),
            )
            .ok()
            .map(|output| LeanStr::from(output.css))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::{css_source, with_defaults};

    use super::*;

    #[test]
    fn combined_minification_matches_separate_minification() {
        let source = css_source(FileId(1), with_defaults("")).unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let mut values: Vec<String> = snapshot
            .tokens()
            .iter()
            .filter_map(|(_, group)| group.first())
            .map(|token| token.value.value.to_string())
            .collect();
        values.extend(
            [
                "red",
                "#FF0000",
                "1.50rem",
                "calc(var(--spacing) * 4)",
                "0 1px 3px 0 rgb(0 0 0 / 0.1)",
                "\"Inter\", ui-sans-serif",
                "a;b",
                "a}b",
                "}",
                "",
                "!important",
                "url(a.png)",
            ]
            .map(String::from),
        );
        for value in values {
            assert_eq!(
                minified_keys(&value),
                separately_minified_keys(&value),
                "{value}"
            );
        }
    }
}
