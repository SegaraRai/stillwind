//! Indexes named variant wrappers using a declaration that stays independent of theme values.

use std::collections::{BTreeSet, HashMap};

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_design_system::{VariantApplication, resolve_utility, resolve_variant};
use stillwind_syntax::parse_candidate;

use crate::query::QueryContext;

const PROBE: &str = "[--sw-canonical-probe:initial]";

#[derive(Debug, Default)]
pub(super) struct VariantIndex {
    signatures: HashMap<LeanStr, Vec<LeanStr>>,
    names: Vec<LeanStr>,
    contextual: Vec<LeanStr>,
}

impl VariantIndex {
    pub(super) fn prepare(query: &QueryContext) -> Self {
        let mut index = Self::default();
        for name in query
            .design_system
            .variants()
            .names()
            .filter(|name| !name.contains('*'))
        {
            index.names.push(name.to_lean_str());
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
                    preview.signature(None, &BTreeSet::new(), query.options.root_font_size)
            {
                index
                    .signatures
                    .entry(signature)
                    .or_default()
                    .push(name.to_lean_str());
            }
        }
        index
    }

    pub(super) fn matching(&self, query: &QueryContext, raw: &str, variant: &str) -> Vec<LeanStr> {
        let Some(parsed) = parse_candidate(raw) else {
            return Vec::new();
        };
        if parsed.variants.len() > 1
            || resolve_utility(raw, query.design_system.theme())
                .ok()
                .and_then(Result::ok)
                .is_some_and(|utility| {
                    utility.selector.is_some()
                        || !utility.nested.is_empty()
                        || !utility.additional.is_empty()
                })
        {
            return self.names.clone();
        }
        let mut names: BTreeSet<_> = self.contextual.iter().cloned().collect();
        if let Ok(Some(preview)) =
            query.canonical_preview(&format_args!("{variant}:{PROBE}").to_lean_string())
            && let Some(signature) =
                preview.signature(None, &BTreeSet::new(), query.options.root_font_size)
            && let Some(matches) = self.signatures.get(&signature)
        {
            names.extend(matches.iter().cloned());
        }
        names.into_iter().collect()
    }
}

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
