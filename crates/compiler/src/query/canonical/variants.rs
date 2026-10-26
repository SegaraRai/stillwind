//! Indexes the variants of a snapshot by the CSS that they produce around a test
//! declaration.
//!
//! A named variant is indexed by its complete test CSS. A functional variant is indexed
//! by a template: the test CSS for a test value, split around the text that the test
//! value produces. A template turns an arbitrary selector variant, such as `[p_&]`, back
//! into the functional variant that produces it, such as `in-[p]`. All names and
//! selector forms come from the variant definitions of the snapshot.

use std::collections::{BTreeSet, HashMap};

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_design_system::{StillwindVariantValueType, VariantApplication, resolve_variant};
use stillwind_syntax::{parse_candidate, print_arbitrary_value};

use super::selectors::selector_signatures;
use crate::query::QueryContext;

// Canonical comparison renders this declaration, so it must resolve even when the
// stylesheet defines no named utilities.
const PROBE: &str = "[--sw-canonical-probe:initial]";

/// Test values for the capture of a functional variant, with the text that each value
/// produces in the CSS. Each test value must be valid for its value type.
fn capture_probes(
    value_type: Option<StillwindVariantValueType>,
) -> &'static [(&'static str, &'static str)] {
    match value_type {
        Some(StillwindVariantValueType::Variant) => &[
            ("[.sw-canonical-probe]", ".sw-canonical-probe"),
            ("[@media_(width>=717px)]", "(width>=717px)"),
        ],
        Some(StillwindVariantValueType::Nth) => &[("[717]", "717")],
        Some(
            StillwindVariantValueType::Aria
            | StillwindVariantValueType::Data
            | StillwindVariantValueType::SupportsCondition,
        ) => &[("[sw-canonical-probe]", "sw-canonical-probe")],
        Some(StillwindVariantValueType::IfCondition) | None => &[],
    }
}

/// The test CSS of one functional variant, split around the text that its test value
/// produces.
#[derive(Debug)]
pub(super) struct VariantTemplate {
    /// The variant name before its capture, such as `in-`.
    pub(super) prefix: LeanStr,
    /// The variant name after its capture. It is usually empty.
    pub(super) suffix: LeanStr,
    /// The value type of the capture.
    pub(super) value_type: Option<StillwindVariantValueType>,
    /// The test value as written in the capture, such as `[.sw-canonical-probe]`.
    pub(super) probe: &'static str,
    /// The text that `probe` produces in the CSS, such as `.sw-canonical-probe`.
    pub(super) token: &'static str,
    /// The normalized test CSS before `token`.
    pub(super) before: LeanStr,
    /// The normalized test CSS after `token`.
    pub(super) after: LeanStr,
}

/// A named variant that selects a related element, such as `*` for the children of the
/// element. A variant applied after it applies to the related element.
#[derive(Debug)]
pub(super) struct VariantRelation {
    /// The name of the variant, such as `*`.
    pub(super) name: LeanStr,
    /// The normalized test CSS before the selector of the related element, such as `&>`
    /// for `&>*`.
    pub(super) before: LeanStr,
}

/// The variants of a snapshot, indexed by the CSS that each variant produces around a
/// test declaration.
#[derive(Debug, Default)]
pub(super) struct VariantIndex {
    /// Maps the normalized test CSS to the names of the variants that produce it.
    signatures: HashMap<LeanStr, Vec<LeanStr>>,
    /// Names of the variants whose CSS depends on where the utility is used. Every
    /// lookup returns these names, because their test CSS cannot be compared.
    contextual: Vec<LeanStr>,
    /// Templates of the functional variants, in registration order.
    pub(super) templates: Vec<VariantTemplate>,
    /// Named variants that select a related element.
    pub(super) relations: Vec<VariantRelation>,
}

impl VariantIndex {
    /// Builds the index from the variants of the snapshot of `query`.
    pub(super) fn prepare(query: &QueryContext) -> Self {
        let mut index = Self::default();
        let exact: BTreeSet<_> = query
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
        for name in &exact {
            let name = name.as_str();
            let raw = format_args!("{name}:{PROBE}").to_lean_string();
            if let Some(parsed) = parse_candidate(&raw)
                && parsed
                    .variants
                    .first()
                    .and_then(|variant| resolve_variant(variant, query.design_system.theme()).ok())
                    .is_some_and(|application| contextual(&application))
            {
                index.contextual.push(name.to_lean_str());
            }
            if let Ok(Some(preview)) = query.canonical_preview(&raw)
                && let Some(signature) =
                    preview.signature(&[], &BTreeSet::new(), query.options.root_font_size)
            {
                index
                    .signatures
                    .entry(signature)
                    .or_default()
                    .push(name.to_lean_str());
            }
            if let Some(css) = test_css(query, name)
                && let Some(before) = css
                    .strip_suffix(&format!("*{{{}}}", probe_declaration()))
                    .filter(|before| relation_start(before))
            {
                index.relations.push(VariantRelation {
                    name: name.to_lean_str(),
                    before: before.into(),
                });
            }
        }
        let mut seen = BTreeSet::new();
        for definition in query
            .design_system
            .variants()
            .iter()
            .flat_map(|(_, definitions)| definitions)
        {
            let Some((prefix, suffix)) = definition
                .wildcard_parts()
                .filter(|(prefix, _)| !prefix.is_empty())
            else {
                continue;
            };
            for &(probe, token) in capture_probes(definition.value_type) {
                if !seen.insert((prefix.to_owned(), suffix.to_owned(), probe)) {
                    continue;
                }
                let Some(css) = test_css(query, &format!("{prefix}{probe}{suffix}")) else {
                    continue;
                };
                let Some((before, after)) = css.split_once(token) else {
                    continue;
                };
                index.templates.push(VariantTemplate {
                    prefix: prefix.into(),
                    suffix: suffix.into(),
                    value_type: definition.value_type,
                    probe,
                    token,
                    before: before.into(),
                    after: after.into(),
                });
            }
        }
        index
    }

    /// Returns the names of the variants that may replace `variant`. The caller must
    /// still check each replacement.
    ///
    /// The result contains every variant whose CSS depends on context, and every
    /// variant whose test CSS equals the test CSS of `variant`. Returns an empty list
    /// when `variant` is already a registered name, or when it is a bare theme name
    /// that no functional variant takes over.
    pub(super) fn matching(&self, query: &QueryContext, variant: &str) -> Vec<LeanStr> {
        if query.design_system.variants().get(variant).is_some()
            || unshadowed_bare_variant(query, variant)
        {
            return Vec::new();
        }
        let mut names: BTreeSet<_> = self.contextual.iter().cloned().collect();
        names.extend(self.named(query, variant));
        names.into_iter().collect()
    }

    /// Returns the names of the named variants whose test CSS equals the test CSS of
    /// `variant`.
    pub(super) fn named(&self, query: &QueryContext, variant: &str) -> Vec<LeanStr> {
        query
            .canonical_preview(&format_args!("{variant}:{PROBE}").to_lean_string())
            .ok()
            .flatten()
            .and_then(|preview| {
                preview.signature(&[], &BTreeSet::new(), query.options.root_font_size)
            })
            .and_then(|signature| self.signatures.get(&signature).cloned())
            .unwrap_or_default()
    }
}

impl VariantTemplate {
    /// Returns the variant spellings that this template gives for the normalized test
    /// CSS `css`, or no spellings when `css` does not have the form of the template.
    ///
    /// For a capture of variant type, `child` receives the captured child variant and
    /// returns its spellings, and the result wraps each spelling in this variant. For
    /// other value types, the result contains the bracket form and, when the captured
    /// value is a plain name, the named form.
    pub(super) fn spellings(
        &self,
        css: &str,
        mut child: impl FnMut(&str) -> Vec<String>,
    ) -> Vec<String> {
        let Some(captured) = css
            .strip_prefix(self.before.as_str())
            .and_then(|rest| rest.strip_suffix(self.after.as_str()))
            .filter(|captured| !captured.is_empty())
        else {
            return Vec::new();
        };
        let (prefix, suffix) = (&self.prefix, &self.suffix);
        if self.value_type == Some(StillwindVariantValueType::Variant) {
            // Tailwind keeps a relative selector, a child combinator, and several
            // attribute selectors in one compound as an arbitrary variant.
            if captured.starts_with(['>', '+', '~'])
                || captured.contains('>')
                || captured.contains("][")
            {
                return Vec::new();
            }
            let value = self
                .probe
                .strip_prefix('[')
                .and_then(|probe| probe.strip_suffix(']'))
                .unwrap_or(self.probe);
            let value =
                stillwind_syntax::decode_arbitrary_value(value).replace(self.token, captured);
            let written = format!("[{}]", print_arbitrary_value(&value));
            let names = child(&written);
            if names.is_empty() {
                return vec![format!("{prefix}{written}{suffix}")];
            }
            return names
                .into_iter()
                .map(|name| format!("{prefix}{name}{suffix}"))
                .collect();
        }
        let attribute = matches!(
            self.value_type,
            Some(StillwindVariantValueType::Aria | StillwindVariantValueType::Data)
        );
        // The value of one attribute selector cannot contain another selector.
        if attribute && captured.contains(['[', ']']) {
            return Vec::new();
        }
        let mut output = Vec::new();
        if self.value_type == Some(StillwindVariantValueType::Aria)
            && let Some(name) = aria_true_name(captured)
        {
            output.push(format!("{prefix}{name}{suffix}"));
        }
        if simple_name(captured) {
            output.push(format!("{prefix}{captured}{suffix}"));
        }
        output.push(format!(
            "{prefix}[{}]{suffix}",
            print_arbitrary_value(captured)
        ));
        output
    }
}

/// Returns the normalized test CSS of `variant`: the CSS around the test declaration,
/// with equivalent selectors in one spelling and without spaces around combinators.
/// Returns `None` when `variant` does not resolve.
pub(super) fn test_css(query: &QueryContext, variant: &str) -> Option<String> {
    let preview = query
        .canonical_preview(&format_args!("{variant}:{PROBE}").to_lean_string())
        .ok()??;
    Some(compact_combinators(&selector_signatures(
        &preview.css,
        false,
        0,
    )))
}

/// Returns test CSS for an arbitrary selector variant `raw` that does not resolve,
/// built from its text: the selector, with `&` added at the start when it has none,
/// followed by the test rule. Returns `None` when `raw` is not an arbitrary variant.
pub(super) fn written_test_css(raw: &str) -> Option<String> {
    let value = raw.strip_prefix('[')?.strip_suffix(']')?;
    let value = stillwind_syntax::decode_arbitrary_value(value);
    let value = value.trim();
    let selector = if value.contains('&') {
        value.to_owned()
    } else {
        format!("&{value}")
    };
    Some(compact_combinators(&format!(
        "{selector}{{{}}}",
        probe_declaration()
    )))
}

/// Returns the declaration that the test utility produces, as it appears in test CSS.
pub(super) fn probe_declaration() -> String {
    let inner = PROBE
        .strip_prefix('[')
        .and_then(|probe| probe.strip_suffix(']'))
        .unwrap_or(PROBE);
    format!("{inner};")
}

/// Checks whether `before` is `&` followed by a combinator, so that `before*` selects
/// elements related to the element.
fn relation_start(before: &str) -> bool {
    before
        .strip_prefix('&')
        .is_some_and(|combinator| matches!(combinator, ">" | "+" | "~" | " "))
}

/// Returns `css` without the spaces around the combinators `>`, `+`, and `~`.
fn compact_combinators(css: &str) -> String {
    let mut output = String::with_capacity(css.len());
    let mut characters = css.chars().peekable();
    while let Some(character) = characters.next() {
        if character == ' '
            && (output.ends_with(['>', '+', '~'])
                || characters
                    .peek()
                    .is_some_and(|next| matches!(next, '>' | '+' | '~')))
        {
            continue;
        }
        output.push(character);
    }
    output
}

/// Checks whether `value` contains only ASCII letters, digits, and `-`.
pub(super) fn simple_name(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

/// Returns the attribute name in `value` when `value` ends with `="true"`, `='true'`,
/// or `=true` and the name passes `simple_name`. Returns `None` otherwise. A variant of
/// the `[aria]` value type selects that attribute with the value `true` when it receives
/// the bare name.
pub(super) fn aria_true_name(value: &str) -> Option<&str> {
    ["=\"true\"", "='true'", "=true"]
        .into_iter()
        .find_map(|suffix| value.strip_suffix(suffix))
        .filter(|name| simple_name(name))
}

/// Checks whether `variant` is a bare theme name, such as the breakpoint name `lg`, and
/// no functional variant captures it. A functional variant that captures the name takes
/// priority over the bare variant.
fn unshadowed_bare_variant(query: &QueryContext, variant: &str) -> bool {
    let variants = query.design_system.variants();
    if variants.iter().any(|(_, definitions)| {
        definitions
            .iter()
            .any(|definition| definition.capture(variant).is_some())
    }) {
        return false;
    }
    variants
        .iter()
        .flat_map(|(_, definitions)| definitions)
        .any(|definition| {
            let Some(namespace) = definition
                .sort
                .as_ref()
                .filter(|sort| sort.bare)
                .and_then(|sort| sort.key.as_deref())
                .and_then(|key| key.strip_prefix("--"))
                .and_then(|key| key.strip_suffix("-*"))
            else {
                return false;
            };
            query
                .design_system
                .theme()
                .token_in(namespace, variant)
                .is_some()
        })
}

/// Checks whether the CSS of `application` depends on where the utility is used. This
/// is true when it adds a selector without `&`, a nested rule, or a condition.
fn contextual(application: &VariantApplication) -> bool {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().any(contextual)
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. } => !selector.contains('&'),
        VariantApplication::SelectorRule { .. }
        | VariantApplication::SelectorMediaRule { .. }
        | VariantApplication::Conditional(_) => true,
        _ => false,
    }
}
