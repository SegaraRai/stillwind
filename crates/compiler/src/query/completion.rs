//! Completion catalogs and theme domains use the active compiler registry.

use std::collections::BTreeSet;

use stillwind_css::find_css_functions;
use stillwind_design_system::{
    BuiltinVariantValueType, DesignSystemRevision, ThemeNamespace, ThemeTokenKind,
    UtilityResolutionError, builtin_utility_token_prefixes, resolve_variant,
};
use stillwind_syntax::parse_candidate;

use super::{DesignSystemQuery, last_top_level};

/// A partially typed candidate, as spelled in the editor.
#[derive(Clone, Debug)]
pub struct CompletionInput {
    /// Text before the cursor, variant prefix included.
    pub prefix: String,
}

/// One label the editor may insert.
#[derive(Clone, Debug)]
pub struct CompletionItem {
    /// Text to insert, ending in `:` for a variant.
    pub label: String,
    /// Snapshot revision the label is valid for.
    pub revision: DesignSystemRevision,
}

impl DesignSystemQuery {
    /// Offers labels that continue `input.prefix`.
    ///
    /// The vocabulary is the design system's utility names plus tokens rewritten
    /// through their utility prefixes, and the variant vocabulary adds breakpoint token
    /// names. Compound variants complete their nested variant argument. Utility labels
    /// must resolve as candidates. Variant labels must resolve as variants and end in
    /// `:`, where the caller can continue with a utility. The existing variant prefix
    /// is retained.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn complete_candidate(
        &self,
        input: CompletionInput,
    ) -> Result<Vec<CompletionItem>, UtilityResolutionError> {
        let prefix = input.prefix;
        let split = last_top_level(&prefix, ':').map_or(0, |index| index + 1);
        let (variants, partial) = prefix.split_at(split);
        let utility_partial = partial.trim_start_matches('!').trim_start_matches('-');
        let utility_flags = &partial[..partial.len() - utility_partial.len()];
        let vocabulary = self.utility_vocabulary_for(utility_partial);
        let mut completions: BTreeSet<String> = BTreeSet::new();
        completions.extend(self.complete_theme_values(
            &prefix,
            split,
            partial,
            utility_partial,
            utility_flags.len(),
        )?);
        for label in vocabulary
            .into_iter()
            .filter(|label| label.starts_with(utility_partial))
        {
            let label = format!("{variants}{utility_flags}{label}");
            if self.resolve_candidate(&label)?.diagnostics.is_empty() {
                completions.insert(label);
            }
        }
        let mut variant_names = self.variant_vocabulary();
        variant_names.extend(self.compound_variant_vocabulary(partial, &variant_names, 0));
        for variant in variant_names {
            if variant.starts_with(partial) && self.valid_variant(&format!("{variants}{variant}")) {
                completions.insert(format!("{variants}{variant}:"));
            }
        }
        Ok(completions
            .into_iter()
            .map(|label| CompletionItem {
                label,
                revision: self.design_system.revision(),
            })
            .collect())
    }

    fn complete_theme_values(
        &self,
        prefix: &str,
        split: usize,
        partial: &str,
        utility_partial: &str,
        flags: usize,
    ) -> Result<BTreeSet<String>, UtilityResolutionError> {
        let theme = self.design_system.theme();
        let mut completions = BTreeSet::new();
        let modifier = last_top_level(partial, '/');
        let input = modifier.map_or(partial, |index| &partial[index + 1..]);
        let domain_input = modifier.map_or(utility_partial, |index| &partial[flags..index]);
        let domains = self.value_domains(domain_input, modifier.is_some());
        if let Some(index) = input.find("--") {
            let fragment = &input[index..];
            for token in theme.active_tokens() {
                if token.name.value.starts_with(fragment)
                    && domains
                        .iter()
                        .any(|namespace| token.name.value.starts_with(namespace))
                {
                    let before = &prefix[..prefix.len() - input.len() + index];
                    let closing = completion_closers(before);
                    let label = format!("{before}{}{closing}", token.name.value);
                    if self.resolve_candidate(&label)?.diagnostics.is_empty() {
                        completions.insert(label);
                    } else if modifier.is_none() && self.valid_variant(&label) {
                        completions.insert(format!("{label}:"));
                    }
                }
            }
        }
        if let Some(index) = modifier {
            let before = &prefix[..=split + index];
            for token in theme.active_tokens() {
                for namespace in &domains {
                    if let Some(name) = token.name.value.strip_prefix(namespace)
                        && name.starts_with(input)
                    {
                        let label = format!("{before}{name}");
                        if self.resolve_candidate(&label)?.diagnostics.is_empty() {
                            completions.insert(label);
                        }
                    }
                }
            }
        }
        Ok(completions)
    }

    fn valid_variant(&self, text: &str) -> bool {
        parse_candidate(&format!("{text}:[color:red]")).is_some_and(|candidate| {
            candidate
                .variants
                .iter()
                .all(|variant| resolve_variant(variant, self.design_system.theme()).is_some())
        })
    }

    fn value_domains(&self, input: &str, modifier: bool) -> BTreeSet<String> {
        let mut domains = self
            .design_system
            .utilities()
            .iter()
            .filter(|(name, _)| {
                name.strip_suffix('*')
                    .is_some_and(|root| input.starts_with(root))
            })
            .flat_map(|(_, definitions)| definitions.iter())
            .flat_map(|definition| {
                value_namespaces(
                    &definition.body,
                    if modifier { "--modifier" } else { "--value" },
                )
            })
            .collect::<BTreeSet<_>>();
        if !modifier {
            domains.extend(
                self.design_system
                    .variants()
                    .iter()
                    .filter(|(name, _)| {
                        name.strip_suffix('*')
                            .is_some_and(|root| input.starts_with(root))
                    })
                    .flat_map(|(_, definitions)| definitions.iter())
                    .flat_map(|definition| value_namespaces(&definition.body, "--value")),
            );
        }
        domains
    }

    pub(super) fn utility_vocabulary_for(&self, prefix: &str) -> BTreeSet<String> {
        let theme = self.design_system.theme();
        let mut vocabulary: BTreeSet<_> = self
            .design_system
            .utilities()
            .names()
            .filter(|name| !name.contains('*') && name.starts_with(prefix))
            .map(str::to_owned)
            .collect();
        {
            let token_prefixes = builtin_utility_token_prefixes(theme)
                .into_iter()
                .filter(|(root, _)| {
                    format!("{root}-").starts_with(prefix)
                        || prefix.starts_with(&format!("{root}-"))
                })
                .collect::<Vec<_>>();
            for token in theme.active_tokens() {
                for (root, namespace) in &token_prefixes {
                    if let Some(name) = token.name.value.strip_prefix(&format!("--{namespace}-")) {
                        vocabulary.insert(format!("{root}-{name}"));
                    }
                }
            }
        }
        for (name, definitions) in self.design_system.utilities().iter() {
            if let Some(root) = name.strip_suffix('*') {
                if !root.starts_with(prefix) && !prefix.starts_with(root) {
                    continue;
                }
                for definition in definitions {
                    for namespace in value_namespaces(&definition.body, "--value") {
                        for token in theme.active_tokens() {
                            if let Some(value) = token.name.value.strip_prefix(&namespace) {
                                vocabulary.insert(format!("{root}{value}"));
                            }
                        }
                    }
                }
            }
        }
        vocabulary.retain(|label| label.starts_with(prefix));
        vocabulary
    }

    fn variant_vocabulary(&self) -> BTreeSet<String> {
        let theme = self.design_system.theme();
        let mut variant_names: BTreeSet<_> = self
            .design_system
            .variants()
            .iter()
            .filter(|(_, definitions)| {
                definitions
                    .iter()
                    .any(|definition| definition.wildcard.is_none())
            })
            .map(|(name, _)| name.to_owned())
            .collect();
        for (_, definitions) in self.design_system.variants().iter() {
            for definition in definitions {
                if let Some((prefix, suffix)) = definition.wildcard_parts() {
                    for namespace in value_namespaces(&definition.body, "--value") {
                        for token in theme.active_tokens() {
                            if let Some(value) = token.name.value.strip_prefix(&namespace) {
                                variant_names.insert(format!("{prefix}{value}{suffix}"));
                            }
                        }
                    }
                }
            }
        }
        {
            variant_names.extend(theme.active_tokens().iter().filter_map(
                |token| match &token.kind {
                    ThemeTokenKind::Token {
                        namespace: ThemeNamespace::Breakpoint,
                        name,
                    } => Some(name.clone()),
                    _ => None,
                },
            ));
        }
        variant_names
    }

    fn compound_variant_vocabulary(
        &self,
        partial: &str,
        names: &BTreeSet<String>,
        depth: usize,
    ) -> BTreeSet<String> {
        let mut labels = BTreeSet::new();
        if depth > 64 {
            return labels;
        }
        for (_, definitions) in self.design_system.variants().iter() {
            for definition in definitions {
                if definition.value_type != Some(BuiltinVariantValueType::Variant) {
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
                        .map(|child| format!("{prefix}{child}{suffix}")),
                );
            }
        }
        labels
    }
}

pub(super) fn value_namespaces(body: &str, function: &str) -> Vec<String> {
    find_css_functions(body, &[function], 0)
        .into_iter()
        .flat_map(|found| body[found.arguments].split(',').collect::<Vec<_>>())
        .filter_map(|part| part.trim().strip_suffix('*').map(str::to_owned))
        .filter(|part| part.starts_with("--"))
        .collect()
}

fn completion_closers(text: &str) -> String {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for character in text.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(current) = quote {
            if current == character {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            ')' | ']' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::css;

    use super::*;
    use crate::CompilerOptions;

    #[test]
    fn completions_include_theme_values_breakpoints_and_variant_prefixes() {
        let theme = css::source(
            FileId(1),
            "@import 'stillwind'; @theme { --color-brand: red; --breakpoint-tablet: 48rem; }",
        )
        .unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &theme,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let query = DesignSystemQuery::new(snapshot.clone(), CompilerOptions::default());
        for (prefix, expected) in [
            ("bg-br", "bg-brand"),
            ("tab", "tablet:"),
            ("hov", "hover:"),
            ("hover:bg-br", "hover:bg-brand"),
            ("hover:tab", "hover:tablet:"),
            ("group-hov", "group-hover:"),
            ("group-not-fo", "group-not-focus:"),
            ("not-group-fo", "not-group-focus:"),
            ("hover:peer-fo", "hover:peer-focus:"),
            ("*", "*:"),
            ("**", "**:"),
        ] {
            let items = query
                .complete_candidate(CompletionInput {
                    prefix: prefix.into(),
                })
                .unwrap();
            assert!(
                items.iter().any(|item| item.label == expected),
                "{prefix}: {items:?}"
            );
            assert!(
                items
                    .iter()
                    .all(|item| item.label.starts_with(prefix)
                        && item.revision == snapshot.revision())
            );
            assert!(items.windows(2).all(|pair| pair[0].label < pair[1].label));
        }
        assert!(
            query
                .complete_candidate(CompletionInput {
                    prefix: "missing:bg-br".into()
                })
                .unwrap()
                .is_empty()
        );
    }
}
