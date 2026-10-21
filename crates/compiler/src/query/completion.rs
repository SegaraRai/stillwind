//! Completion catalogs and theme domains use the active compiler registry.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::{CssNode, CssNodeKind, find_css_functions, parse_structure};
use stillwind_design_system::{
    BuiltinVariantValueType, CustomUtility, DesignSystemRevision, MarkerForm,
    UtilityResolutionError, builtin_utility_token_prefixes, named_domain_values,
};
use stillwind_syntax::parse_candidate;

use super::{CandidateContext, DesignSystemQuery, last_top_level};

/// A partially typed candidate, as spelled in the editor.
#[derive(Clone, Debug, Default)]
pub struct CompletionInput {
    /// Decoded text of one partial candidate before the cursor, including its variant
    /// prefix. JavaScript string escapes must already be decoded.
    pub prefix: LeanStr,
    /// Input role used to validate labels and select their registry.
    pub context: CandidateContext,
    /// Includes variant labels with an empty `[]` for a client to turn into a snippet.
    /// With `markers`, it also includes `name/` for each modifiable marker `name`.
    pub snippets: bool,
    /// Includes the names of marker classes. Set it only for inputs whose candidates
    /// add classes to an element, which excludes `@apply`.
    pub markers: bool,
}

/// One label the editor may insert.
#[derive(Clone, Debug)]
pub struct CompletionItem {
    /// Text to insert, ending in `:` for a variant and in `/` for the modifier of a
    /// modifiable marker.
    pub label: LeanStr,
    /// Whether the label adds a marker class to the element, or ends in `/` and starts
    /// the modifier of a modifiable marker.
    pub marker: bool,
    /// Snapshot revision the label is valid for.
    pub revision: DesignSystemRevision,
}

impl DesignSystemQuery {
    /// Offers labels that continue `input.prefix`.
    ///
    /// The input role selects utility or animation-option names and their theme tokens.
    /// With `input.markers`, an ordinary utility input without variants, `!`, or `-`
    /// also offers marker class names, which need not resolve as utilities. Definition
    /// branches and `style.media()` arguments offer only variant prefixes, and the
    /// `template` option of `style.css()` offers template names. Every other label must
    /// resolve under the role's variant constraints. Compound variants complete their
    /// nested argument, and the existing variant prefix is retained.
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
        let snippets = input.snippets;
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
                    marker: false,
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
        let theme = self.context.design_system.theme();
        for definition in self.context_definitions(context) {
            if let Some(root) = definition.name.strip_suffix('*') {
                for value in named_domain_values(&definition.body, theme) {
                    vocabulary.insert(format_args!("{root}{value}").to_lean_str());
                }
            } else {
                vocabulary.insert(definition.name.to_lean_str());
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
        completions.extend(self.variant_completion_labels(variants, partial, context, snippets));
        let mut markers = BTreeSet::new();
        if input.markers
            && context == CandidateContext::Utility
            && variants.is_empty()
            && utility_flags.is_empty()
        {
            for (name, form) in self.marker_names(partial) {
                if snippets && form == MarkerForm::Modifiable {
                    markers.insert(format_args!("{name}/").to_lean_str());
                }
                markers.insert(name);
            }
            completions.extend(markers.iter().cloned());
        }
        let used: BTreeSet<_> = parse_candidate(&format_args!("{variants}utility").to_lean_str())
            .into_iter()
            .flat_map(|candidate| candidate.variants)
            .map(|variant| variant.raw)
            .collect();
        Ok(completions
            .into_iter()
            .filter(|label| {
                label.strip_suffix(':').is_none_or(|text| {
                    let start = last_top_level(text, ':').map_or(0, |index| index + 1);
                    !used.contains(&text[start..])
                })
            })
            .map(|label| CompletionItem {
                marker: markers.contains(&label),
                label,
                revision: self.context.design_system.revision(),
            })
            .collect())
    }

    /// Returns variant labels matching `partial` after the enclosing `variants`.
    /// `context` validates the complete chain. `snippets` includes empty brackets whose
    /// filled values can resolve under that role.
    fn variant_completion_labels(
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

    /// Returns the definitions that the registry context of `context` adds to the
    /// default definitions, in registration order. Roles without a registry context
    /// add none.
    fn context_definitions(&self, context: CandidateContext) -> Vec<Arc<CustomUtility>> {
        let Some(registry_context) = context.registry_context() else {
            return Vec::new();
        };
        let theme = self.context.design_system.theme();
        let defaults = theme
            .registry()
            .utilities()
            .iter()
            .chain(theme.utilities())
            .map(Arc::as_ptr)
            .collect::<HashSet<_>>();
        let scoped = theme.context(registry_context);
        scoped
            .registry()
            .utilities()
            .iter()
            .chain(scoped.utilities())
            .filter(|definition| !defaults.contains(&Arc::as_ptr(definition)))
            .cloned()
            .collect()
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
        domains.extend(
            self.context_definitions(context)
                .into_iter()
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
                    for value in named_domain_values(&definition.body, theme) {
                        vocabulary.insert(format_args!("{root}{value}").to_lean_str());
                    }
                }
            }
        }
        vocabulary.retain(|label| label.starts_with(prefix));
        vocabulary
    }

    /// Returns every variant spelling that completion offers without a compound
    /// argument: names without a capture, wildcard names with each matching theme
    /// value, and the token keys that bare variants such as breakpoints accept.
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

    fn variant_placeholders(&self) -> &BTreeMap<LeanStr, LeanStr> {
        self.variant_placeholders.get_or_init(|| {
            let mut placeholders = BTreeMap::new();
            let mut patterns: BTreeSet<_> = self
                .context
                .design_system
                .variants()
                .iter()
                .flat_map(|(_, definitions)| definitions)
                .filter_map(|definition| definition.wildcard_parts())
                .filter(|(prefix, _)| !prefix.is_empty())
                .map(|(prefix, suffix)| (prefix.to_owned(), suffix.to_owned()))
                .collect();
            patterns.insert((String::new(), String::new()));
            for (prefix, suffix) in patterns {
                for value in [
                    "&:hover",
                    "1",
                    "1px",
                    "display:grid",
                    "pressed",
                    "--flag",
                    "@media_(width>1px)",
                ] {
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

/// Summarizes generated CSS for one selected completion. Ordinary declarations take
/// precedence over custom properties. A preview without declarations shows its rule
/// wrappers. The result contains at most 160 characters, including a final ellipsis.
pub fn completion_detail(css: &str) -> Option<LeanStr> {
    fn collect(
        css: &str,
        nodes: &[CssNode],
        ordinary: &mut Vec<String>,
        custom: &mut Vec<String>,
        wrappers: &mut Vec<String>,
    ) {
        for node in nodes {
            match &node.kind {
                CssNodeKind::AtRule { name, .. } if name == "property" => continue,
                CssNodeKind::Declaration { property, value } => {
                    let property = &css[property.clone()];
                    let output = if property.starts_with("--") {
                        &mut *custom
                    } else {
                        &mut *ordinary
                    };
                    let text = format!("{property}: {};", &css[value.clone()]);
                    if !output.contains(&text) {
                        output.push(text);
                    }
                }
                CssNodeKind::StyleRule { selector, .. } => {
                    let selector = &css[selector.clone()];
                    if selector != "&" {
                        wrappers.push(selector.into());
                    }
                }
                CssNodeKind::AtRule { name, prelude, .. } => {
                    wrappers.push(format!("@{name} {}", &css[prelude.clone()]));
                }
                CssNodeKind::Apply { .. } => {}
            }
            collect(css, node.children(), ordinary, custom, wrappers);
        }
    }
    let (mut ordinary, mut custom, mut wrappers) = (Vec::new(), Vec::new(), Vec::new());
    collect(
        css,
        &parse_structure(css),
        &mut ordinary,
        &mut custom,
        &mut wrappers,
    );
    let values = if !ordinary.is_empty() {
        ordinary
    } else if !custom.is_empty() {
        custom
    } else {
        wrappers
    };
    if values.is_empty() {
        return None;
    }
    let text = values.join(" ");
    Some(if text.chars().count() > 160 {
        format_args!("{}…", text.chars().take(159).collect::<String>()).to_lean_str()
    } else {
        text.into()
    })
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
    fn variant_completions_offer_valid_placeholders_and_skip_used_variants() {
        let source = css_source(FileId(1), "@import 'stillwind';").unwrap();
        let query = DesignSystemQuery::new(
            DesignSystemSnapshot::from_source(
                &source,
                "theme.css".into(),
                DesignSystemOptions::default(),
            )
            .unwrap(),
            CompilerOptions::default(),
        );
        let complete = |prefix: &str, context, snippets| {
            query
                .complete_candidate(CompletionInput {
                    prefix: prefix.into(),
                    context,
                    snippets,
                    markers: false,
                })
                .unwrap()
                .into_iter()
                .map(|item| item.label)
                .collect::<Vec<_>>()
        };
        let labels = complete("hover:", CandidateContext::Utility, true);
        assert!(!labels.iter().any(|label| label == "hover:hover:"));
        assert!(labels.iter().any(|label| label == "hover:nth-[]:"));
        assert!(labels.iter().any(|label| label == "hover:group-[]:"));
        assert!(
            complete("group-nth-", CandidateContext::Utility, true)
                .iter()
                .any(|label| label == "group-nth-[]:")
        );
        assert!(
            !complete("nth-", CandidateContext::Utility, false)
                .iter()
                .any(|label| label.contains("[]"))
        );
        assert_eq!(
            complete("nth-", CandidateContext::MediaVariants, true),
            [] as [LeanStr; 0]
        );
        assert!(
            complete("min-", CandidateContext::MediaVariants, true)
                .iter()
                .any(|label| label == "min-[]:")
        );
        assert!(
            query
                .preview_variant("hover:", CandidateContext::KeyframesFrame)
                .is_none()
        );
        assert!(
            query
                .preview_variant("hover:", CandidateContext::Document)
                .is_none()
        );
        assert!(
            query
                .preview_variant("hover:", CandidateContext::Utility)
                .unwrap()
                .contains("&:hover")
        );
    }

    #[test]
    fn completion_details_preserve_declaration_values_and_describe_empty_wrappers() {
        assert_eq!(
            completion_detail("& { --metadata: blue; content: 'a;b'; padding: 1rem; }"),
            Some("content: 'a;b'; padding: 1rem;".into())
        );
        assert_eq!(
            completion_detail("@media (hover: hover) { &:hover {} }"),
            Some("@media (hover: hover) &:hover".into())
        );
        assert_eq!(
            completion_detail(
                "@property --color { syntax: '<color>'; inherits: false; initial-value: red; } & { --paint: red; }"
            ),
            Some("--paint: red;".into())
        );
    }

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
                    snippets: false,
                    markers: false,
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
                    snippets: false,
                    markers: false,
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
                    snippets: false,
                    markers: false,
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
                    snippets: false,
                    markers: false,
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

    #[test]
    fn utility_completions_offer_marker_names_only_where_they_add_the_class() {
        let source = css_source(
            FileId(1),
            "@stillwind-marker menu (modifiable);
            @utility card-title { .card & { color: red; } }
            @utility box { color: blue; }",
        )
        .unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
        let items = |prefix: &str, markers: bool, snippets: bool| {
            query
                .complete_candidate(CompletionInput {
                    prefix: prefix.into(),
                    context: CandidateContext::Utility,
                    snippets,
                    markers,
                })
                .unwrap()
                .into_iter()
                .map(|item| (item.label.to_string(), item.marker))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            items("card", true, false),
            [("card".into(), true), ("card-title".into(), false)]
        );
        assert_eq!(
            items("menu", true, true),
            [("menu".into(), true), ("menu/".into(), true)]
        );
        assert_eq!(items("menu", true, false), [("menu".into(), true)]);
        assert_eq!(items("card", false, false), [("card-title".into(), false)]);
        for prefix in ["!card", "-card"] {
            assert!(
                items(prefix, true, false).iter().all(|(_, marker)| !marker),
                "{prefix}"
            );
        }
    }
}
