//! Prepares finite canonical spellings and indexes their normalized output.
//!
//! One snapshot owns each index through its analysis cache. Numeric spacing remains
//! a computed candidate because its domain is not finite.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::{UtilityResolutionError, named_domain_values};
use stillwind_diagnostics::CanonicalUtilitiesConfig;

use super::parallel;
use super::preview::{CanonicalPreview, Normalization, SpacingUnit, dimension, spacing_properties};
use super::values::ThemeValueIndex;
use super::variants::VariantIndex;
use crate::query::QueryContext;
use crate::query::theme::ScaleReference;

#[derive(Debug, Eq, PartialEq, Hash)]
struct Signature {
    css: LeanStr,
    required: Vec<LeanStr>,
}

/// Finite spellings grouped by their output and spacing conversion policy.
#[derive(Debug, Default)]
pub(in crate::query) struct CanonicalIndex {
    values: ThemeValueIndex,
    policies: BTreeMap<Normalization, HashMap<Signature, Vec<LeanStr>>>,
    pub(super) errors: Vec<(LeanStr, UtilityResolutionError)>,
    pub(super) labels: BTreeSet<LeanStr>,
    fixed: BTreeSet<LeanStr>,
    pub(super) variants: VariantIndex,
}

impl CanonicalIndex {
    pub(in crate::query) fn prepare(query: &QueryContext) -> Self {
        let mut labels = BTreeSet::new();
        for (name, definitions) in query.design_system.utilities().iter() {
            if let Some(root) = name.strip_suffix('*') {
                for definition in definitions {
                    for value in named_domain_values(&definition.body, query.design_system.theme())
                    {
                        labels.insert(format_args!("{root}{value}").to_lean_str());
                    }
                }
            } else {
                labels.insert(name.to_lean_str());
            }
        }
        let all_labels = labels.clone();
        let fixed = query
            .design_system
            .utilities()
            .names()
            .filter(|name| !name.contains('*'))
            .map(LeanStr::from)
            .collect();
        let variants = VariantIndex::prepare(query);
        let mut index = Self {
            labels: all_labels,
            fixed,
            variants,
            values: ThemeValueIndex::prepare(query, &labels),
            ..Self::default()
        };
        let pending: Vec<_> = labels
            .into_iter()
            .filter(|label| {
                !index.values.covers(label) || query.design_system.utilities().get(label).is_some()
            })
            .collect();
        // Previews are independent of each other, so threads render them with their own
        // contexts. The index receives them in label order.
        let inputs = query.inputs();
        let previews = parallel::map_with_state(
            &pending,
            || inputs.context(),
            |context, label| context.canonical_preview(label),
        );
        for (label, preview) in pending.into_iter().zip(previews) {
            match preview {
                Ok(Some(preview)) => {
                    index.insert(query, label, &preview);
                }
                Ok(None) => {}
                Err(error) => {
                    index.errors.push(((label), error));
                }
            }
        }
        for groups in index.policies.values_mut() {
            for records in groups.values_mut() {
                records.sort_by(|left, right| rank(left, right));
            }
        }
        index
            .errors
            .sort_by(|(left, _), (right, _)| rank(left, right));
        index
    }

    fn insert(&mut self, query: &QueryContext, label: LeanStr, preview: &CanonicalPreview) {
        let options = query.canonical_options();
        let scales = query.theme_scales();
        let normalization = policy(&preview.css, &options, &scales);
        let signature = preview.signature(
            &scales,
            &normalization.properties,
            query.options.root_font_size,
        );
        if let Some(css) = signature {
            self.policies
                .entry(normalization)
                .or_default()
                .entry(Signature {
                    css,
                    required: preview.required_properties.clone(),
                })
                .or_default()
                .push(label);
        }
    }

    pub(super) fn modifier_preserves_value(&self, roots: &BTreeSet<LeanStr>, suffix: &str) -> bool {
        self.values.modifier_preserves_value(roots, suffix)
    }

    pub(super) fn context_labels(
        &self,
        roots: &BTreeSet<LeanStr>,
        negative: bool,
        preserve_value: bool,
    ) -> Vec<LeanStr> {
        let mut labels = Vec::new();
        let source = if preserve_value {
            &self.fixed
        } else {
            &self.labels
        };
        for root in roots {
            let prefix = format_args!("{}{root}-", if negative { "-" } else { "" }).to_lean_str();
            labels.extend(
                source
                    .range(prefix.clone()..)
                    .take_while(|label| label.starts_with(prefix.as_str()))
                    .cloned(),
            );
        }
        labels
    }

    pub(super) fn value_labels(
        &self,
        query: &QueryContext,
        roots: &BTreeSet<LeanStr>,
        value: &str,
    ) -> Vec<LeanStr> {
        self.values.value_labels(query, roots, value)
    }

    pub(super) fn matching(
        &self,
        query: &QueryContext,
        original: &CanonicalPreview,
    ) -> Vec<LeanStr> {
        let scales = query.theme_scales();
        let mut records = self.values.matching(query, original);
        for (normalization, signatures) in &self.policies {
            if let Some(css) = original.signature(
                &scales,
                &normalization.properties,
                query.options.root_font_size,
            ) && let Some(matches) = signatures.get(&Signature {
                css,
                required: original.required_properties.clone(),
            }) {
                records.extend(matches.iter().cloned());
            }
        }
        records.sort_by(|left, right| rank(left, right));
        records.dedup();
        records
    }
}

/// Compares two spellings by preference. A spelling without a leading `-` comes before
/// a spelling with one. A shorter spelling comes before a longer one. Spellings of the
/// same length are compared lexically.
pub(super) fn rank(left: &str, right: &str) -> Ordering {
    left.starts_with('-')
        .cmp(&right.starts_with('-'))
        .then_with(|| left.len().cmp(&right.len()))
        .then_with(|| left.cmp(right))
}

pub(super) fn policy(
    css: &str,
    options: &CanonicalUtilitiesConfig,
    scales: &[ScaleReference],
) -> Normalization {
    let mut properties = BTreeSet::new();
    if options.px_to_rem && scales.iter().any(|scale| scale.referenced_by(css)) {
        spacing_properties(css, scales, &mut properties, 0);
    }
    Normalization {
        resolve_scale: !scales.is_empty(),
        properties,
        preserve_specificity: options.preserve_specificity,
    }
}

/// Whether the value of `scale` is a rem dimension.
pub(super) fn uses_rem(scale: &ScaleReference) -> bool {
    dimension(&scale.value).is_some_and(|(_, unit)| unit == SpacingUnit::Rem)
}
