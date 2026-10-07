//! Prepares finite canonical spellings and indexes their normalized output.
//!
//! One snapshot owns each index through its analysis cache. Numeric spacing remains
//! a computed candidate because its domain is not finite.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use stillwind_design_system::UtilityResolutionError;
use stillwind_diagnostics::CanonicalUtilitiesConfig;

use super::super::completion::value_namespaces;
use super::preview::{CanonicalPreview, Normalization, dimension, spacing_properties};
use super::values::ThemeValueIndex;
use super::variants::VariantIndex;
use crate::query::QueryContext;

#[derive(Debug, Eq, PartialEq, Hash)]
struct Signature {
    css: Arc<str>,
    required: Vec<String>,
}

#[derive(Debug)]
struct IndexedUtility {
    label: String,
    preview: Arc<CanonicalPreview>,
}

/// Finite spellings grouped by their output and spacing conversion policy.
#[derive(Debug, Default)]
pub(in crate::query) struct CanonicalIndex {
    values: ThemeValueIndex,
    policies: BTreeMap<Normalization, HashMap<Signature, Vec<Arc<IndexedUtility>>>>,
    records: Vec<Arc<IndexedUtility>>,
    pub(super) errors: Vec<(String, UtilityResolutionError)>,
    pub(super) labels: BTreeSet<String>,
    fixed: BTreeSet<String>,
    pub(super) variants: VariantIndex,
}

impl CanonicalIndex {
    pub(in crate::query) fn prepare(query: &QueryContext) -> Self {
        let mut labels = BTreeSet::new();
        for (name, definitions) in query.design_system.utilities().iter() {
            if let Some(root) = name.strip_suffix('*') {
                for definition in definitions {
                    for namespace in value_namespaces(&definition.body, "--value") {
                        for token in query.design_system.tokens().names_with_prefix(&namespace) {
                            if let Some(value) = token.strip_prefix(&namespace) {
                                labels.insert(format!("{root}{value}"));
                            }
                        }
                    }
                }
            } else {
                labels.insert(name.to_owned());
            }
        }
        let all_labels = labels.clone();
        let fixed = query
            .design_system
            .utilities()
            .names()
            .filter(|name| !name.contains('*'))
            .map(str::to_owned)
            .collect();
        let variants = VariantIndex::prepare(query);
        let mut index = Self {
            labels: all_labels,
            fixed,
            variants,
            values: ThemeValueIndex::prepare(query, &labels),
            ..Self::default()
        };
        for label in labels {
            if index.values.covers(&label) && query.design_system.utilities().get(&label).is_none()
            {
                continue;
            }
            match query.canonical_preview(&label) {
                Ok(Some(preview)) => index.insert(query, label, preview),
                Ok(None) => {}
                Err(error) => index.errors.push((label, error)),
            }
        }
        index
            .records
            .sort_by(|left, right| rank(&left.label, &right.label));
        for groups in index.policies.values_mut() {
            for records in groups.values_mut() {
                records.sort_by(|left, right| rank(&left.label, &right.label));
            }
        }
        index
            .errors
            .sort_by(|(left, _), (right, _)| rank(left, right));
        index
    }

    fn insert(&mut self, query: &QueryContext, label: String, preview: Arc<CanonicalPreview>) {
        let options = query.canonical_options();
        let scale = query.spacing_scale();
        let normalization = policy(&preview, &options, scale.as_deref());
        let signature = preview.signature(scale.as_deref(), &normalization.properties, &options);
        let record = Arc::new(IndexedUtility { label, preview });
        if let Some(css) = signature {
            self.policies
                .entry(normalization)
                .or_default()
                .entry(Signature {
                    css,
                    required: record.preview.required_properties.clone(),
                })
                .or_default()
                .push(record.clone());
        }
        self.records.push(record);
    }

    pub(super) fn modifier_preserves_value(&self, roots: &BTreeSet<String>, suffix: &str) -> bool {
        self.values.modifier_preserves_value(roots, suffix)
    }

    pub(super) fn context_labels(
        &self,
        roots: &BTreeSet<String>,
        negative: bool,
        preserve_value: bool,
    ) -> Vec<String> {
        let mut labels = Vec::new();
        let source = if preserve_value {
            &self.fixed
        } else {
            &self.labels
        };
        for root in roots {
            let prefix = format!("{}{root}-", if negative { "-" } else { "" });
            labels.extend(
                source
                    .range(prefix.clone()..)
                    .take_while(|label| label.starts_with(&prefix))
                    .cloned(),
            );
        }
        labels
    }

    pub(super) fn value_labels(
        &self,
        query: &QueryContext,
        roots: &BTreeSet<String>,
        value: &str,
    ) -> Vec<String> {
        self.values.value_labels(query, roots, value)
    }

    pub(super) fn matching(
        &self,
        query: &QueryContext,
        original: &CanonicalPreview,
    ) -> Vec<String> {
        let options = query.canonical_options();
        let scale = query.spacing_scale();
        let mut records = self.values.matching(query, original);
        for (normalization, signatures) in &self.policies {
            if let Some(css) =
                original.signature(scale.as_deref(), &normalization.properties, &options)
                && let Some(matches) = signatures.get(&Signature {
                    css,
                    required: original.required_properties.clone(),
                })
            {
                records.extend(matches.iter().map(|record| record.label.clone()));
            }
        }
        records.sort_by(|left, right| rank(left, right));
        records.dedup();
        records
    }
}

pub(super) fn rank(left: &str, right: &str) -> std::cmp::Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

pub(super) fn policy(
    preview: &CanonicalPreview,
    options: &CanonicalUtilitiesConfig,
    scale: Option<&str>,
) -> Normalization {
    let mut properties = BTreeSet::new();
    if options.px_to_rem
        && scale
            .and_then(dimension)
            .is_some_and(|(_, unit)| unit == "rem")
        && (preview.css.contains("var(--spacing)") || preview.css.contains("var(--spacing-"))
    {
        spacing_properties(&preview.css, &mut properties, 0);
    }
    Normalization {
        resolve_scale: scale.is_some(),
        properties,
    }
}
