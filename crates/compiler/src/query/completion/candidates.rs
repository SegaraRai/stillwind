//! Completes one partial candidate for an input role.
//!
//! Each offered label is checked under the input role before it is returned. Labels
//! include utilities, theme values, modifiers, variants, and marker classes.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::{
    CustomUtility, DesignSystemRevision, MarkerForm, UtilityResolutionError, named_domain_values,
    placeholder_namespaces,
};
use stillwind_syntax::{CandidatePrefix, parse_candidate};

use crate::query::{CandidateContext, DesignSystemQuery};

/// A partial candidate that the user is typing in the editor.
#[derive(Clone, Debug, Default)]
pub struct CompletionInput {
    /// The text of the partial candidate before the cursor, including its variants. The
    /// caller must decode JavaScript string escapes first.
    pub prefix: LeanStr,
    /// The input role. It selects the registry, and it decides which labels are valid.
    pub context: CandidateContext,
    /// Whether to offer variant labels with an empty `[]`, which a client can turn into
    /// a snippet. When `markers` is also true, each marker `name` that accepts a
    /// modifier is also offered as `name/`.
    pub snippets: bool,
    /// Whether to offer the names of marker classes. Set it only for inputs whose
    /// candidates add classes to an element. `@apply` does not add classes, so it must
    /// not set this field.
    pub markers: bool,
}

/// One label that the editor may insert.
#[derive(Clone, Debug)]
pub struct CompletionItem {
    /// The text to insert. A variant label ends with `:`. A label that starts the
    /// modifier of a marker ends with `/`.
    pub label: LeanStr,
    /// Whether the label is a marker class name, or a marker name followed by `/` for
    /// its modifier.
    pub marker: bool,
    /// The snapshot revision for which the label is valid.
    pub revision: DesignSystemRevision,
}

impl DesignSystemQuery {
    /// Returns the labels that can continue `input.prefix`.
    ///
    /// The input role decides which labels are offered:
    ///
    /// - Most roles offer utility names and theme values. Each label must resolve under
    ///   the variant rules of the role.
    /// - The role for animation options offers option names instead of utility names.
    /// - Definition branches and `style.media()` arguments offer only variants.
    /// - The `template` option of `style.css()` offers template names.
    ///
    /// When `input.markers` is true, an ordinary utility input without variants, `!`,
    /// or `-` also offers marker class names. A marker name does not need to resolve as
    /// a utility. For a compound variant such as `group-`, the method completes its
    /// nested variant. Each label keeps the variants that `input.prefix` already
    /// contains.
    ///
    /// Returns each label once, in lexical order. Each item records the revision of
    /// this query's snapshot. A variant label ends with `:`, so the user can continue
    /// typing the candidate.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a utility definition that matches a label.
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
        let split = CandidatePrefix::variant_end(&prefix);
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
                    let start = CandidatePrefix::variant_end(text);
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

    /// Returns labels that complete a theme reference, such as `--color-red-500`, or a
    /// modifier, when the role `context` allows them. A label for a theme reference
    /// also receives the brackets or parentheses that close it.
    ///
    /// `prefix` is the complete partial candidate. `partial` is the part after its
    /// variants, and it starts at the byte offset `split` of `prefix`.
    /// `utility_partial` is `partial` without its leading `!` and `-`, which are
    /// `flags` bytes long.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a utility definition while the method checks a
    /// label.
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
        let modifier =
            CandidatePrefix::parse(partial).and_then(|prefix| prefix.last_separator('/'));
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
                    let closing = CandidatePrefix::parse(before)
                        .and_then(|prefix| prefix.completion_closers())
                        .unwrap_or_default();
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

    /// Returns the utility definitions that the registry context of `context` adds to
    /// the default definitions, in registration order. Returns no definitions when the
    /// role has no registry context.
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

    /// Returns the theme namespace prefixes, such as `--color-`, that the definitions
    /// matching `input` accept.
    ///
    /// When `modifier` is true, the method returns the namespaces of modifiers.
    /// Otherwise it returns the namespaces of values, and it also includes the
    /// namespaces of functional variants when the role `context` allows variants. Each
    /// prefix keeps its leading `--`.
    fn value_domains(
        &self,
        input: &str,
        modifier: bool,
        context: CandidateContext,
    ) -> BTreeSet<LeanStr> {
        let captures = |name: &str| {
            name.strip_suffix('*')
                .is_some_and(|root| input.starts_with(root))
        };
        let placeholder = if modifier { "--modifier" } else { "--value" };
        let mut domains = BTreeSet::new();
        if !context.options() && !context.variants_only() {
            domains.extend(
                self.context
                    .design_system
                    .utilities()
                    .iter()
                    .filter(|(name, _)| captures(name))
                    .flat_map(|(_, definitions)| definitions.iter())
                    .flat_map(|definition| placeholder_namespaces(&definition.body, placeholder)),
            );
        }
        domains.extend(
            self.context_definitions(context)
                .into_iter()
                .filter(|definition| captures(&definition.name))
                .flat_map(|definition| placeholder_namespaces(&definition.body, placeholder)),
        );
        if !modifier && context.variants() {
            domains.extend(
                self.context
                    .design_system
                    .variants()
                    .iter()
                    .filter(|(name, _)| captures(name))
                    .flat_map(|(_, definitions)| definitions.iter())
                    .flat_map(|definition| placeholder_namespaces(&definition.body, "--value")),
            );
        }
        domains
    }
}
