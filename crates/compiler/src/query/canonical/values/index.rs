//! Prepares CSS templates from the shared evaluator and indexes concrete theme values.
//!
//! Families that copy values share the namespace indexes in `direct`. Other families
//! use CSS templates after checking each candidate's utility IR. Overrides and
//! transformations that change that IR remain in the finite CSS index.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::find_css_functions;
use stillwind_design_system::{
    ResolvedUtility, ThemeTokenMode, UtilityResolutionError, builtin_property_definitions,
    resolve_utility,
};
use stillwind_diagnostics::CanonicalUtilitiesConfig;
use stillwind_source::ReplaceText;

use super::super::index::{policy, rank};
use super::super::preview::{CanonicalPreview, Normalization, normalized_signature};
use super::direct::DirectValues;
use crate::query::QueryContext;
use crate::query::completion::value_namespaces;

#[derive(Debug, Default)]
pub(in super::super) struct ThemeValueIndex {
    groups: BTreeMap<Normalization, ValueMatches>,
    covered: BTreeSet<LeanStr>,
    direct: DirectValues,
}

impl ThemeValueIndex {
    pub(in super::super) fn prepare(query: &QueryContext, labels: &BTreeSet<LeanStr>) -> Self {
        let mut index = Self {
            direct: DirectValues::prepare(query),
            ..Self::default()
        };
        let mut preparation = ValuePreparation::new(query);
        for (name, definitions) in query.design_system.utilities().iter() {
            let Some(root) = name.strip_suffix('*') else {
                continue;
            };
            let namespaces: BTreeSet<_> = definitions
                .iter()
                .flat_map(|definition| value_namespaces(&definition.body, "--value"))
                .collect();
            for namespace in namespaces {
                preparation.namespace(&mut index, root, &namespace, labels);
            }
        }
        for groups in index.groups.values_mut() {
            for names in groups.signatures.values_mut() {
                names.sort_by(|left, right| rank(left, right));
            }
        }
        index
    }

    pub(in super::super) fn modifier_preserves_value(
        &self,
        roots: &BTreeSet<LeanStr>,
        suffix: &str,
    ) -> bool {
        self.direct.modifier_preserves_value(roots, suffix)
    }

    pub(in super::super) fn covers(&self, label: &str) -> bool {
        self.covered.contains(label) || self.direct.covers(label)
    }

    pub(in super::super) fn value_labels(
        &self,
        query: &QueryContext,
        roots: &BTreeSet<LeanStr>,
        value: &str,
    ) -> Vec<LeanStr> {
        self.direct.matching(query, roots, value)
    }

    pub(in super::super) fn matching(
        &self,
        query: &QueryContext,
        original: &CanonicalPreview,
    ) -> Vec<LeanStr> {
        let scale = query.spacing_scale();
        let mut labels = Vec::new();
        for (normalization, signatures) in &self.groups {
            if let Some(signature) = original.signature(
                scale.as_deref(),
                &normalization.properties,
                query.options.root_font_size,
            ) && let Some(names) = signatures
                .signatures
                .get(&(signature, original.required_properties.clone()))
            {
                labels.extend(names.iter().cloned());
            }
        }
        labels
    }
}

struct ValuePreparation<'a> {
    query: &'a QueryContext,
    options: CanonicalUtilitiesConfig,
    scale: Option<LeanStr>,
    values: BTreeMap<LeanStr, Option<LeanStr>>,
    registered: Vec<LeanStr>,
    required_values: BTreeMap<LeanStr, Vec<bool>>,
    resolved: BTreeMap<LeanStr, Result<Option<ResolvedUtility>, UtilityResolutionError>>,
    signatures: HashMap<(LeanStr, Normalization), Option<LeanStr>>,
}

impl<'a> ValuePreparation<'a> {
    fn new(query: &'a QueryContext) -> Self {
        let options = query.canonical_options();
        let scale = query.spacing_scale();
        let values: BTreeMap<_, _> = query
            .design_system
            .tokens()
            .iter()
            .filter_map(|(name, group)| {
                group.first().map(|token| {
                    (
                        name.to_lean_str(),
                        query.resolve_theme_values(&token.value.value),
                    )
                })
            })
            .collect();
        let authored = query.design_system.theme();
        let registered: Vec<_> = authored
            .properties()
            .iter()
            .chain(builtin_property_definitions(authored))
            .map(|property| property.name.clone())
            .collect();
        let required_values: BTreeMap<_, _> = query
            .design_system
            .tokens()
            .iter()
            .filter_map(|(name, group)| {
                let token = group.first()?;
                let value = if token.mode == ThemeTokenMode::Inline {
                    token.value.value.clone()
                } else {
                    format_args!("var({name})").to_lean_str()
                };
                Some((
                    name.to_lean_str(),
                    registered
                        .iter()
                        .map(|name| value.contains((name).as_str()))
                        .collect::<Vec<_>>(),
                ))
            })
            .collect();
        Self {
            query,
            options,
            scale: (scale),
            values,
            registered,
            required_values,
            resolved: BTreeMap::new(),
            signatures: HashMap::new(),
        }
    }

    fn namespace(
        &mut self,
        index: &mut ThemeValueIndex,
        root: &str,
        namespace: &str,
        labels: &BTreeSet<LeanStr>,
    ) {
        let query = self.query;
        let mut template = None;
        for token_name in query.design_system.tokens().names_with_prefix(namespace) {
            let Some(token) = query
                .design_system
                .tokens()
                .get(token_name)
                .and_then(|group| group.first())
            else {
                continue;
            };
            let Some(capture) = token_name.strip_prefix(namespace) else {
                continue;
            };
            let label = format_args!("{root}{capture}").to_lean_str();
            if !labels.contains(&label) || index.covers(&label) {
                continue;
            }
            let actual = self.resolved.entry(label.clone()).or_insert_with(|| {
                resolve_utility(&label, query.design_system.theme()).map(Result::ok)
            });
            let Ok(Some(actual)) = actual else {
                continue;
            };
            if template.is_none()
                && token.mode != ThemeTokenMode::Inline
                && let Ok(Some(preview)) = query.canonical_preview(&label)
                && preview
                    .css
                    .contains(format_args!("var({token_name})").to_lean_string().as_str())
            {
                template = Some(CssTemplate::new(
                    query,
                    token_name,
                    actual.clone(),
                    (preview.css.clone()).into(),
                    &self.registered,
                ));
            }
            let Some(template) = &template else {
                continue;
            };
            let value = if token.mode == ThemeTokenMode::Inline {
                token.value.value.clone()
            } else {
                format_args!("var({token_name})").to_lean_str()
            };
            if !template.matches(actual, &value) {
                continue;
            }
            let css = template.css.replace_text(&template.variable, &value);
            let required_properties: Vec<_> = self
                .registered
                .iter()
                .zip(&template.required)
                .zip(&self.required_values[token_name])
                .filter(|((_, fixed), captured)| **fixed || **captured)
                .map(|((name, _), _)| name.clone())
                .collect();
            let resolved_css = template.expanded.as_ref().and_then(|css| {
                self.values
                    .get(token_name)?
                    .as_ref()
                    .map(|value| css.replace_text(&template.placeholder, value))
            });
            let normalization = policy(&css, &self.options, self.scale.as_deref());
            let signature = resolved_css.as_ref().and_then(|css| {
                self.signatures
                    .entry((css.as_str().into(), normalization.clone()))
                    .or_insert_with(|| {
                        normalized_signature(
                            css,
                            self.scale.as_deref(),
                            &normalization.properties,
                            self.query.options.root_font_size,
                        )
                    })
                    .clone()
            });
            if let Some(signature) = signature {
                index
                    .groups
                    .entry(normalization)
                    .or_default()
                    .signatures
                    .entry((signature, required_properties))
                    .or_default()
                    .push(label.clone());
            }
            index.covered.insert(label);
        }
    }
}

#[derive(Debug, Default)]
struct ValueMatches {
    signatures: HashMap<(LeanStr, Vec<LeanStr>), Vec<LeanStr>>,
}

#[derive(Debug)]
struct CssTemplate {
    variable: LeanStr,
    placeholder: LeanStr,
    css: LeanStr,
    expanded: Option<LeanStr>,
    utility: ResolvedUtility,
    required: Vec<bool>,
}

impl CssTemplate {
    fn new(
        query: &QueryContext,
        token_name: &str,
        utility: ResolvedUtility,
        css: LeanString,
        registered: &[LeanStr],
    ) -> Self {
        let variable = format_args!("var({token_name})").to_lean_string();
        let mut counter = 0;
        let marker = loop {
            let marker = format_args!("--sw-canonical-index-operand-{counter}").to_lean_string();
            if !css.contains(marker.as_str())
                && !query.design_system.tokens().iter().any(|(_, group)| {
                    group
                        .iter()
                        .any(|token| token.value.value.contains(marker.as_str()))
                })
            {
                break marker;
            }
            counter += 1;
        };
        let placeholder = format_args!("var({marker})").to_lean_string();
        let mut input_template = css.clone();
        let edits: Vec<_> = find_css_functions(&css, &["var"], 0)
            .into_iter()
            .filter_map(|function| {
                let end = function.end?;
                (css[function.start..end] == variable).then_some(function.start..end)
            })
            .collect();
        for range in edits.into_iter().rev() {
            input_template.replace_range(range, &placeholder);
        }
        let static_css = css.replace_text(&variable, "");
        let required = registered
            .iter()
            .map(|name| {
                static_css.contains(name.as_str())
                    || static_css.contains(query.options.rewrite_generated(name).as_str())
                    || utility.required_properties.contains(name)
            })
            .collect();
        Self {
            variable: (variable).into(),
            placeholder: (placeholder).into(),
            expanded: (query.resolve_theme_values(&input_template)),
            css: (css).into(),
            utility,
            required,
        }
    }

    /// Checks the emitted structure after substituting the captured value.
    /// Candidate spelling and source-use records remain specific to each resolution.
    fn matches(&self, actual: &ResolvedUtility, value: &str) -> bool {
        let ResolvedUtility {
            token_uses: _,
            arbitrary_uses: _,
            candidate: _,
            invalid_value: _,
            order,
            required_properties,
            builtin_order,
            declaration,
            selector,
            nested,
            additional,
        } = actual;
        if order != &self.utility.order
            || required_properties != &self.utility.required_properties
            || builtin_order != &self.utility.builtin_order
            || declaration != &self.utility.declaration.replace_text(&self.variable, value)
        {
            return false;
        }
        let expected_selector = self
            .utility
            .selector
            .as_ref()
            .map(|selector| LeanStr::from(selector.replace_text(&self.variable, value)));
        if selector != &expected_selector {
            return false;
        }
        let mut expected_nested = self.utility.nested.clone();
        for rule in &mut expected_nested {
            rule.condition = (rule.condition.replace_text(&self.variable, value)).into();
            rule.declaration = (rule.declaration.replace_text(&self.variable, value)).into();
        }
        if nested != &expected_nested {
            return false;
        }
        let mut expected_additional = self.utility.additional.clone();
        for target in &mut expected_additional {
            target.declaration = (target.declaration.replace_text(&self.variable, value)).into();
            if let Some(selector) = &mut target.selector {
                *selector = (selector.replace_text(&self.variable, value)).into();
            }
            for nested in &mut target.nested {
                nested.condition = (nested.condition.replace_text(&self.variable, value)).into();
                nested.declaration =
                    (nested.declaration.replace_text(&self.variable, value)).into();
            }
        }
        additional == &expected_additional
    }
}
