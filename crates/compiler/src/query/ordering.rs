//! Candidate ordering reuses prepared keys for comparisons and relative ranks.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::{
    ThemeIndex, UtilityOrder, UtilityResolutionError, VariantSequenceOrder, resolve_utility,
    resolve_variant,
};

use super::DesignSystemQuery;

impl DesignSystemQuery {
    /// Compares candidates using the compiler's variant and utility order.
    /// Returns `None` if either candidate cannot resolve. Each valid candidate's key
    /// and each unresolved spelling are retained for this immutable query.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn compare_candidates(
        &self,
        left: &str,
        right: &str,
    ) -> Result<Option<Ordering>, UtilityResolutionError> {
        let theme = self.context.design_system.theme();
        let Some(left_key) = self.ordering.key(left, theme)? else {
            return Ok(None);
        };
        let Some(right_key) = self.ordering.key(right, theme)? else {
            return Ok(None);
        };
        Ok(Some(left_key.compare(left, &right_key, right)))
    }

    /// Assigns relative ranks to `candidates` using one batch of prepared keys.
    /// Output positions match the input. Unresolved candidates return `None`, and
    /// candidates with equal ordering keys and spellings share a rank. Ranks apply
    /// only to this batch and cannot be compared with ranks from another call.
    ///
    /// # Errors
    ///
    /// Returns the first malformed matching utility requirement in input order.
    pub fn candidate_ranks(
        &self,
        candidates: &[&str],
    ) -> Result<Vec<Option<usize>>, UtilityResolutionError> {
        let theme = self.context.design_system.theme();
        let mut valid = Vec::new();
        for (index, raw) in candidates.iter().enumerate() {
            if let Some(key) = self.ordering.key(raw, theme)? {
                valid.push((index, *raw, key));
            }
        }
        valid.sort_by(|(_, left, a), (_, right, b)| a.compare(left, b, right));
        let mut ranks = vec![None; candidates.len()];
        let mut rank = 0;
        for (position, (index, raw, key)) in valid.iter().enumerate() {
            if position > 0 {
                let (_, previous, previous_key) = &valid[position - 1];
                if previous_key.compare(previous, key, raw) != Ordering::Equal {
                    rank += 1;
                }
            }
            ranks[*index] = Some(rank);
        }
        Ok(ranks)
    }
}

/// Retains candidate keys without holding another reference to their owning snapshot.
#[derive(Clone, Debug, Default)]
pub(super) struct CandidateOrdering {
    keys: RefCell<HashMap<LeanStr, Option<Arc<CandidateOrder>>>>,
}

impl CandidateOrdering {
    fn key(
        &self,
        raw: &str,
        theme: &ThemeIndex,
    ) -> Result<Option<Arc<CandidateOrder>>, UtilityResolutionError> {
        if let Some(key) = self.keys.borrow().get(raw) {
            return Ok(key.clone());
        }
        let key = resolve_utility(raw, theme)?
            .ok()
            .filter(|resolved| {
                resolved
                    .candidate
                    .variants
                    .iter()
                    .all(|variant| resolve_variant(variant, theme).is_ok())
            })
            .map(|resolved| {
                Arc::new(CandidateOrder {
                    variants: VariantSequenceOrder::new(&resolved.candidate.variants, theme),
                    utility: resolved.order,
                })
            });
        self.keys
            .borrow_mut()
            .insert(raw.to_lean_str(), key.clone());
        Ok(key)
    }
}

/// Holds the resolved metadata needed to compare one candidate.
#[derive(Debug)]
struct CandidateOrder {
    variants: VariantSequenceOrder,
    utility: UtilityOrder,
}

impl CandidateOrder {
    fn compare(&self, left: &str, other: &Self, right: &str) -> Ordering {
        self.variants
            .compare(&other.variants)
            .then_with(|| self.utility.compare(left, &other.utility, right))
    }
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot, UtilityOrderMode};
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::{Compiler, CompilerOptions};

    /// Registration order mixes user and built-in definitions without moving an
    /// override, and the ordering fingerprint follows that mode.
    #[test]
    fn registration_order_mixes_user_and_builtin_definitions_without_moving_overrides() {
        let css = "@variant before { &:first-child { @slot; } }
        @stillwind-variant middle { &:hover { @slot; } }
        @variant after { &:last-child { @slot; } }
        @variant middle { &:focus { @slot; } }
        @utility early { color:red; }
        @stillwind-utility builtin { color:blue; }
        @utility late { color:green; }
        @utility early { color:purple; }";
        let mut ordering_fingerprints = Vec::new();
        for (mode, expected) in [
            ("authored", ["early", "builtin", "late"]),
            ("tailwind", ["builtin", "early", "late"]),
            ("", ["early", "builtin", "late"]),
        ] {
            let options = CompilerOptions {
                utility_order: if mode == "tailwind" {
                    UtilityOrderMode::Tailwind
                } else {
                    UtilityOrderMode::Authored
                },
                ..Default::default()
            };
            let source = SourceFile::new(
                FileId(1),
                None,
                None,
                (format_args!("{css} @stillwind-slot utilities;").to_lean_string()).as_str(),
            )
            .unwrap();
            let snapshot = DesignSystemSnapshot::from_source(
                &source,
                CssSourceName::from("test.css"),
                options.design_system(),
            )
            .unwrap();
            ordering_fingerprints.push(snapshot.fingerprints().ordering.clone());
            let query = DesignSystemQuery::new(snapshot, options.clone());
            let mut utilities = ["late", "builtin", "early"];
            utilities.sort_by(|a, b| query.compare_candidates(a, b).unwrap().unwrap());
            assert_eq!(utilities, expected);
            let mut variants = ["after:early", "middle:early", "before:early"];
            variants.sort_by(|a, b| query.compare_candidates(a, b).unwrap().unwrap());
            assert_eq!(variants, ["before:early", "middle:early", "after:early"]);
            assert!(
                query
                    .hover_candidate("middle:early")
                    .unwrap()
                    .unwrap()
                    .css
                    .contains(":focus")
            );
            let input = SourceFile::new(
                FileId(2),
                None,
                None,
                "import {style} from 'stillwind/react'; const value=style('late builtin early');",
            )
            .unwrap();
            let compiled = Compiler::compile_source(
                &OxcSourceAnalyzer,
                &input,
                &ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &source,
                &CssSourceName::from("stillwind.css"),
                &options,
            );
            assert!(compiled.diagnostics.is_empty(), "{compiled:#?}");
            let blue = compiled.css.find("color:blue").unwrap();
            let purple = compiled.css.find("color:purple").unwrap();
            assert_eq!(purple < blue, mode != "tailwind", "{}", compiled.css);
        }
        assert_ne!(ordering_fingerprints[0], ordering_fingerprints[1]);
        assert_eq!(ordering_fingerprints[0], ordering_fingerprints[2]);
    }

    /// Authored definitions outrank built-ins for editor ordering, and unknown
    /// candidates have no order at all.
    #[test]
    fn editor_order_uses_custom_utility_and_variant_precedence() {
        let theme = css_source(
            FileId(1),
            "@utility a { color:red; } @utility b { color:blue; }",
        )
        .unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &theme,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
        let mut candidates = ["hover:b", "b", "hover:a", "a", "flex"];
        candidates.sort_by(|left, right| query.compare_candidates(left, right).unwrap().unwrap());
        assert_eq!(candidates, ["flex", "a", "b", "hover:a", "hover:b"]);
        assert_eq!(
            query.compare_candidates("a", "a").unwrap(),
            Some(std::cmp::Ordering::Equal)
        );
        assert_eq!(query.compare_candidates("unknown:a", "a").unwrap(), None);
        assert_eq!(
            query.compare_candidates("a", "unknown-utility").unwrap(),
            None
        );
        let values = ["hover:b", "a", "b", "a", "unknown:a", "unknown-utility"];
        let ranks = query.candidate_ranks(&values).unwrap();
        assert_eq!(ranks, [Some(2), Some(0), Some(1), Some(0), None, None]);
        for (left, a) in values.iter().enumerate() {
            for (right, b) in values.iter().enumerate() {
                assert_eq!(
                    ranks[left].zip(ranks[right]).map(|(a, b)| a.cmp(&b)),
                    query.compare_candidates(a, b).unwrap()
                );
            }
        }
    }
}
