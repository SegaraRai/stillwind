//! Lists the labels that candidate completion filters by the typed prefix.
//!
//! Utility labels come from definition names and theme tokens. Variant labels come from
//! variant names, values of functional variants, compound variants, and snippet
//! placeholders.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_design_system::{
    StillwindVariantValueType, named_domain_values, placeholder_namespaces,
    stillwind_utility_token_prefixes,
};

use crate::query::{CandidateContext, DesignSystemQuery};

impl DesignSystemQuery {
    /// Returns the utility labels that start with `prefix`.
    ///
    /// The labels include utility names without a capture, utility names built from
    /// theme tokens, and the named values of each functional family whose root matches
    /// `prefix`.
    pub(super) fn utility_vocabulary_for(&self, prefix: &str) -> BTreeSet<LeanStr> {
        let theme = self.context.design_system.theme();
        let mut vocabulary: BTreeSet<_> = self
            .context
            .design_system
            .utilities()
            .names()
            .filter(|name| !name.contains('*') && name.starts_with(prefix))
            .map(LeanStr::from)
            .collect();
        {
            let token_prefixes = stillwind_utility_token_prefixes(theme)
                .into_iter()
                .filter(|(root, _)| {
                    format_args!("{root}-").to_lean_str().starts_with(prefix)
                        || prefix.starts_with(format_args!("{root}-").to_lean_str().as_str())
                })
                .collect::<Vec<_>>();
            for token in theme.active_tokens() {
                for (root, namespace) in &token_prefixes {
                    if let Some(name) = token
                        .name
                        .value
                        .strip_prefix(format_args!("--{namespace}-").to_lean_str().as_str())
                    {
                        vocabulary.insert(format_args!("{root}-{name}").to_lean_str());
                    }
                }
            }
        }
        for (name, definitions) in self.context.design_system.utilities().iter() {
            if let Some(root) = name.strip_suffix('*') {
                if !root.starts_with(prefix) && !prefix.starts_with(root) {
                    continue;
                }
                for definition in definitions {
                    for value in named_domain_values(&definition.body, theme) {
                        vocabulary.insert(format_args!("{root}{value}").to_lean_str());
                    }
                }
            }
        }
        vocabulary.retain(|label| label.starts_with(prefix));
        vocabulary
    }

    /// Returns the variant labels that start with `partial`, each preceded by
    /// `variants`. `variants` is the text of the variants before `partial`, including
    /// the final `:`.
    ///
    /// Each label is offered only when `variants` followed by the label is valid in the
    /// role `context`. When `snippets` is true, the method also offers labels with an
    /// empty `[]`, but only when a filled value can resolve in that role.
    pub(super) fn variant_completion_labels(
        &self,
        variants: &str,
        partial: &str,
        context: CandidateContext,
        snippets: bool,
    ) -> BTreeSet<LeanStr> {
        let mut completions = BTreeSet::new();
        let mut variant_names = self.variant_vocabulary();
        let empty_placeholders = BTreeMap::new();
        let placeholders = if snippets {
            self.variant_placeholders()
        } else {
            &empty_placeholders
        };
        variant_names.extend(placeholders.keys().cloned());
        variant_names.extend(self.compound_variant_vocabulary(partial, &variant_names, 0));
        for variant in variant_names {
            let probe = placeholders
                .iter()
                .filter(|(label, _)| variant.ends_with(label.as_str()))
                .max_by_key(|(label, _)| label.len())
                .map_or_else(
                    || variant.clone(),
                    |(label, value)| {
                        format_args!("{}{value}", &variant[..variant.len() - label.len()])
                            .to_lean_str()
                    },
                );
            if variant.starts_with(partial)
                && self
                    .valid_input_variant(&format_args!("{variants}{probe}").to_lean_str(), context)
            {
                completions.insert(format_args!("{variants}{variant}:").to_lean_str());
            }
        }
        completions
    }

    /// Returns every variant spelling that completion offers without a compound prefix.
    ///
    /// The spellings include variant names without a capture, each functional variant
    /// combined with each matching theme value, and the token keys that bare variants
    /// such as breakpoints accept.
    pub(crate) fn variant_vocabulary(&self) -> BTreeSet<LeanStr> {
        let theme = self.context.design_system.theme();
        let mut variant_names: BTreeSet<_> = self
            .context
            .design_system
            .variants()
            .iter()
            .filter(|(_, definitions)| {
                definitions
                    .iter()
                    .any(|definition| definition.wildcard.is_none())
            })
            .map(|(name, _)| name.to_lean_str())
            .collect();
        for (_, definitions) in self.context.design_system.variants().iter() {
            for definition in definitions {
                if let Some((prefix, suffix)) = definition.wildcard_parts() {
                    for namespace in placeholder_namespaces(&definition.body, "--value") {
                        for token in theme.active_tokens() {
                            if let Some(value) = token.name.value.strip_prefix(namespace.as_str()) {
                                variant_names
                                    .insert(format_args!("{prefix}{value}{suffix}").to_lean_str());
                            }
                        }
                    }
                }
            }
        }
        for (_, definitions) in self.context.design_system.variants().iter() {
            for namespace in definitions.iter().filter_map(|definition| {
                definition
                    .sort
                    .as_ref()
                    .filter(|sort| sort.bare)?
                    .key
                    .as_deref()?
                    .strip_prefix("--")?
                    .strip_suffix("-*")
            }) {
                variant_names.extend(
                    theme
                        .tokens_in(namespace)
                        .into_iter()
                        .map(|(key, _)| key.to_lean_str()),
                );
            }
        }
        variant_names
    }

    /// Returns a map from snippet labels to test values. Each key is a variant prefix
    /// with an empty `[]`, such as `data-[]`. Its value is the same variant with a
    /// filled value that resolves in element context.
    ///
    /// The map contains only variants that accept an arbitrary value. The test values
    /// come from `CustomVariant::sample_captures`, and an arbitrary variant such as
    /// `[]` uses the samples of a variant capture. The query computes the map on the
    /// first call and reuses it.
    fn variant_placeholders(&self) -> &BTreeMap<LeanStr, LeanStr> {
        self.variant_placeholders.get_or_init(|| {
            let mut placeholders = BTreeMap::new();
            let mut patterns = BTreeMap::<_, BTreeSet<LeanString>>::new();
            for definition in self
                .context
                .design_system
                .variants()
                .iter()
                .flat_map(|(_, definitions)| definitions)
            {
                if let Some((prefix, suffix)) = definition
                    .wildcard_parts()
                    .filter(|(prefix, _)| !prefix.is_empty())
                {
                    patterns
                        .entry((prefix.to_owned(), suffix.to_owned()))
                        .or_default()
                        .extend(definition.sample_captures());
                }
            }
            patterns.insert(
                (String::new(), String::new()),
                StillwindVariantValueType::Variant
                    .sample_captures()
                    .iter()
                    .map(ToLeanString::to_lean_string)
                    .collect(),
            );
            for ((prefix, suffix), samples) in patterns {
                for value in samples {
                    let probe = format_args!("{prefix}[{value}]{suffix}").to_lean_str();
                    if self.valid_input_variant(&probe, CandidateContext::ElementVariants) {
                        placeholders
                            .insert(format_args!("{prefix}[]{suffix}").to_lean_str(), probe);
                        break;
                    }
                }
            }
            placeholders
        })
    }

    /// Returns compound variant labels that continue `partial`. Each label places one
    /// of `names` inside a compound prefix, such as `group-hover` for `group-` and
    /// `hover`.
    ///
    /// When `partial` already contains a compound prefix, the method completes the
    /// nested variant in the same way. `depth` is the current nesting level, and the
    /// method stops after 64 levels.
    fn compound_variant_vocabulary(
        &self,
        partial: &str,
        names: &BTreeSet<LeanStr>,
        depth: usize,
    ) -> BTreeSet<LeanStr> {
        let mut labels = BTreeSet::new();
        if depth > 64 {
            return labels;
        }
        for (_, definitions) in self.context.design_system.variants().iter() {
            for definition in definitions {
                if definition.value_type != Some(StillwindVariantValueType::Variant) {
                    continue;
                }
                let Some((prefix, suffix)) = definition
                    .wildcard_parts()
                    .filter(|(prefix, _)| !prefix.is_empty())
                else {
                    continue;
                };
                let remaining = if let Some(remaining) = partial.strip_prefix(prefix) {
                    remaining
                } else if prefix.starts_with(partial) {
                    ""
                } else {
                    continue;
                };
                let mut children: BTreeSet<_> = names
                    .iter()
                    .filter(|name| name.starts_with(remaining))
                    .cloned()
                    .collect();
                if !remaining.is_empty() {
                    children.extend(self.compound_variant_vocabulary(remaining, names, depth + 1));
                }
                labels.extend(
                    children
                        .into_iter()
                        .map(|child| format_args!("{prefix}{child}{suffix}").to_lean_str()),
                );
            }
        }
        labels
    }
}
