//! Completion catalogs and theme domains use the active compiler registry.

use std::collections::BTreeSet;

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::find_css_functions;
use stillwind_design_system::{
    BuiltinVariantValueType, DesignSystemRevision, ThemeNamespace, ThemeTokenKind,
    UtilityResolutionError, builtin_utility_token_prefixes,
};

use super::{CandidateContext, DesignSystemQuery, last_top_level};

/// A partially typed candidate, as spelled in the editor.
#[derive(Clone, Debug)]
pub struct CompletionInput {
    /// Decoded text of one partial candidate before the cursor, including its variant
    /// prefix. JavaScript string escapes must already be decoded.
    pub prefix: LeanStr,
    /// Input role used to validate labels and select their registry.
    pub context: CandidateContext,
}

/// One label the editor may insert.
#[derive(Clone, Debug)]
pub struct CompletionItem {
    /// Text to insert, ending in `:` for a variant.
    pub label: LeanStr,
    /// Snapshot revision the label is valid for.
    pub revision: DesignSystemRevision,
}

impl DesignSystemQuery {
    /// Offers labels that continue `input.prefix`.
    ///
    /// The input role selects utility or animation-option names and their theme tokens.
    /// Definition branches and `style.media()` arguments offer only variant prefixes,
    /// and the `template` option of `style.css()` offers template names. Every label
    /// must resolve under the role's variant constraints. Compound variants complete
    /// their nested argument, and the existing variant prefix is retained.
    ///
    /// Returns unique labels in lexical order, each pinned to this query's
    /// design-system revision. Variant labels end in `:` so the caller can continue the
    /// input.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn complete_candidate(
        &self,
        input: CompletionInput,
    ) -> Result<Vec<CompletionItem>, UtilityResolutionError> {
        let prefix = input.prefix;
        let context = input.context;
        if context == CandidateContext::CssTemplate {
            return Ok(self
                .context
                .design_system
                .theme()
                .css_templates()
                .keys()
                .filter(|name| name.starts_with(prefix.as_str()))
                .map(|name| CompletionItem {
                    label: name.clone(),
                    revision: self.context.design_system.revision(),
                })
                .collect());
        }
        let split = last_top_level(&prefix, ':').map_or(0, |index| index + 1);
        let (variants, partial) = prefix.split_at(split);
        let utility_partial = partial.trim_start_matches('!').trim_start_matches('-');
        let utility_flags = &partial[..partial.len() - utility_partial.len()];
        let mut vocabulary = if context.options() || context.variants_only() {
            BTreeSet::new()
        } else {
            self.utility_vocabulary_for(utility_partial)
        };
        if context.options() {
            let theme = self.context.design_system.theme();
            for definition in theme.registry().animation_options() {
                if let Some(root) = definition.name.strip_suffix('*') {
                    for namespace in value_namespaces(&definition.body, "--value") {
                        for token in theme.active_tokens() {
                            if let Some(value) = token.name.value.strip_prefix(namespace.as_str()) {
                                vocabulary.insert(format_args!("{root}{value}").to_lean_str());
                            }
                        }
                    }
                } else {
                    vocabulary.insert(definition.name.to_lean_str());
                }
            }
        }
        let mut completions: BTreeSet<LeanStr> = BTreeSet::new();
        completions.extend(self.complete_theme_values(
            &prefix,
            split,
            partial,
            utility_partial,
            utility_flags.len(),
            context,
        )?);
        for label in vocabulary
            .into_iter()
            .filter(|label| label.starts_with(utility_partial))
        {
            let label = format_args!("{variants}{utility_flags}{label}").to_lean_str();
            if !context.variants_only() && self.valid_input(&label, context)? {
                completions.insert(label);
            }
        }
        let mut variant_names = self.variant_vocabulary();
        variant_names.extend(self.compound_variant_vocabulary(partial, &variant_names, 0));
        for variant in variant_names {
            if variant.starts_with(partial)
                && self.valid_input_variant(
                    &format_args!("{variants}{variant}").to_lean_str(),
                    context,
                )
            {
                completions.insert(format_args!("{variants}{variant}:").to_lean_str());
            }
        }
        Ok(completions
            .into_iter()
            .map(|label| CompletionItem {
                label,
                revision: self.context.design_system.revision(),
            })
            .collect())
    }

    /// Completes theme references and modifiers permitted by `context`. `split` is the
    /// UTF-8 byte start of `partial` in `prefix`, and `utility_partial` omits `flags`
    /// bytes of leading importance or negative markers. All slices must describe that
    /// prefix. Returns unique complete labels, adding closing delimiters for theme
    /// references.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement while validating an ordinary utility label.
    fn complete_theme_values(
        &self,
        prefix: &str,
        split: usize,
        partial: &str,
        utility_partial: &str,
        flags: usize,
        context: CandidateContext,
    ) -> Result<BTreeSet<LeanStr>, UtilityResolutionError> {
        let theme = self.context.design_system.theme();
        let mut completions = BTreeSet::new();
        let modifier = last_top_level(partial, '/');
        let input = modifier.map_or(partial, |index| &partial[index + 1..]);
        let domain_input = modifier.map_or(utility_partial, |index| &partial[flags..index]);
        let domains = self.value_domains(domain_input, modifier.is_some(), context);
        if let Some(index) = input.find("--") {
            let fragment = &input[index..];
            for token in theme.active_tokens() {
                if token.name.value.starts_with(fragment)
                    && domains
                        .iter()
                        .any(|namespace| token.name.value.starts_with((namespace).as_str()))
                {
                    let before = &prefix[..prefix.len() - input.len() + index];
                    let closing = completion_closers(before);
                    let label = format_args!("{before}{}{closing}", token.name.value).to_lean_str();
                    if !context.variants_only() && self.valid_input(&label, context)? {
                        completions.insert(label);
                    } else if modifier.is_none() && self.valid_input_variant(&label, context) {
                        completions.insert(format_args!("{label}:").to_lean_str());
                    }
                }
            }
        }
        if let Some(index) = modifier {
            let before = &prefix[..=split + index];
            for token in theme.active_tokens() {
                for namespace in &domains {
                    if let Some(name) = token.name.value.strip_prefix((namespace).as_str())
                        && name.starts_with(input)
                    {
                        let label = format_args!("{before}{name}").to_lean_str();
                        if !context.variants_only() && self.valid_input(&label, context)? {
                            completions.insert(label);
                        }
                    }
                }
            }
        }
        Ok(completions)
    }

    /// Returns theme namespace prefixes used by matching definitions for `input` and
    /// `context`. `modifier` selects modifier namespaces; value queries also include
    /// variant namespaces when the role permits variants. Prefixes keep their `--`
    /// spelling.
    fn value_domains(
        &self,
        input: &str,
        modifier: bool,
        context: CandidateContext,
    ) -> BTreeSet<LeanStr> {
        let mut domains = self
            .context
            .design_system
            .utilities()
            .iter()
            .filter(|(name, _)| {
                !context.options()
                    && !context.variants_only()
                    && name
                        .strip_suffix('*')
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
        if context.options() {
            domains.extend(
                self.context
                    .design_system
                    .theme()
                    .registry()
                    .animation_options()
                    .iter()
                    .filter(|definition| {
                        definition
                            .name
                            .strip_suffix('*')
                            .is_some_and(|root| input.starts_with(root))
                    })
                    .flat_map(|definition| {
                        value_namespaces(
                            &definition.body,
                            if modifier { "--modifier" } else { "--value" },
                        )
                    }),
            );
        }
        if !modifier && context.variants() {
            domains.extend(
                self.context
                    .design_system
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
            let token_prefixes = builtin_utility_token_prefixes(theme)
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
                    for namespace in value_namespaces(&definition.body, "--value") {
                        for token in theme.active_tokens() {
                            if let Some(value) = token.name.value.strip_prefix(namespace.as_str()) {
                                vocabulary.insert(format_args!("{root}{value}").to_lean_str());
                            }
                        }
                    }
                }
            }
        }
        vocabulary.retain(|label| label.starts_with(prefix));
        vocabulary
    }

    /// Returns every variant spelling that completion offers without a compound
    /// argument: names without a capture, wildcard names with each matching theme
    /// value, and breakpoint names.
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
                    for namespace in value_namespaces(&definition.body, "--value") {
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
        names: &BTreeSet<LeanStr>,
        depth: usize,
    ) -> BTreeSet<LeanStr> {
        let mut labels = BTreeSet::new();
        if depth > 64 {
            return labels;
        }
        for (_, definitions) in self.context.design_system.variants().iter() {
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
                        .map(|child| format_args!("{prefix}{child}{suffix}").to_lean_str()),
                );
            }
        }
        labels
    }
}

pub(super) fn value_namespaces(body: &str, function: &str) -> Vec<LeanStr> {
    find_css_functions(body, &[function], 0)
        .into_iter()
        .flat_map(|found| body[found.arguments].split(',').collect::<Vec<_>>())
        .filter_map(|part| part.trim().strip_suffix('*').map(LeanStr::from))
        .filter(|part| part.starts_with("--"))
        .collect()
}

fn completion_closers(text: &str) -> LeanString {
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
    use stillwind_test_support::css_source;

    use super::*;
    use crate::CompilerOptions;

    #[test]
    fn completions_include_theme_values_breakpoints_and_variant_prefixes() {
        let theme = css_source(
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
                    context: CandidateContext::Utility,
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
        for (prefix, offered, omitted) in [
            ("not-s", "not-sm:", "not-starting:"),
            ("not-", "not-print:", "not-group-print:"),
            ("not-pl", "not-placeholder-shown:", "not-placeholder:"),
            ("group-", "group-required:", "group-sm:"),
            ("has-", "has-required:", "has-print:"),
        ] {
            let labels: Vec<_> = query
                .complete_candidate(CompletionInput {
                    prefix: prefix.into(),
                    context: CandidateContext::Utility,
                })
                .unwrap()
                .into_iter()
                .map(|item| item.label)
                .collect();
            assert!(
                labels.iter().any(|label| label == offered),
                "{prefix}: {labels:?}"
            );
            assert!(
                labels.iter().all(|label| label != omitted),
                "{prefix}: {labels:?}"
            );
        }
        assert!(
            query
                .complete_candidate(CompletionInput {
                    prefix: "missing:bg-br".into(),
                    context: CandidateContext::Utility,
                })
                .unwrap()
                .is_empty()
        );
    }

    /// The first variant of a document-level style must select an element or be an
    /// at-rule condition; later positions offer every variant.
    #[test]
    fn document_completions_start_with_variants_that_select_an_element() {
        let theme = css_source(
            FileId(1),
            "@import 'stillwind'; @custom-variant shell scope(document) (:host);",
        )
        .unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &theme,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
        let labels = |prefix: &str| {
            query
                .complete_candidate(CompletionInput {
                    prefix: prefix.into(),
                    context: CandidateContext::Document,
                })
                .unwrap()
                .into_iter()
                .map(|item| item.label)
                .collect::<Vec<_>>()
        };
        for (prefix, offered, omitted) in [
            ("h", Some("host:"), "hover:"),
            ("sh", Some("shell:"), "shell:flex"),
            ("pr", Some("print:"), "print:flex"),
            ("print:h", Some("print:host:"), "print:hover:"),
            ("host:h", Some("host:hover:"), "host:hidden:"),
            ("host:hid", Some("host:hidden"), "hidden"),
            ("not-h", None, "not-hover:"),
        ] {
            let labels = labels(prefix);
            assert!(
                offered.is_none_or(|offered| labels.iter().any(|label| label == offered)),
                "{prefix}: {labels:?}"
            );
            assert!(
                labels.iter().all(|label| label != omitted),
                "{prefix}: {labels:?}"
            );
        }
        assert_eq!(labels("hid"), [] as [LeanStr; 0]);
        assert_eq!(
            query
                .preview_candidate("host:hover:flex size-full", CandidateContext::Document)
                .unwrap()
                .as_deref(),
            Some("@media (hover: hover){:host:hover{display:flex;}}")
        );
        assert_eq!(
            query
                .preview_candidate("size-full", CandidateContext::Document)
                .unwrap(),
            None
        );
    }
}
