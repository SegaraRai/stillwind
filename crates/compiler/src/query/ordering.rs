//! Candidate comparisons use the compiler's utility and variant order.

use std::cmp::Ordering;

use stillwind_design_system::{UtilityResolutionError, compare_variant_sequences, resolve_variant};

use super::DesignSystemQuery;
use crate::candidate::CandidateResolver;

impl DesignSystemQuery {
    /// Compares two valid candidates using the compiler's variant and utility order.
    ///
    /// Returns `None` if either utility or any of its variants cannot be resolved.
    /// Callers decide how to preserve unresolved text and expression boundaries;
    /// this comparison does not rewrite source or normalize candidate spelling.
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
        let resolve = |raw| {
            Ok(CandidateResolver::resolve(raw, theme)?.filter(|resolved| {
                resolved
                    .candidate
                    .variants
                    .iter()
                    .all(|variant| resolve_variant(variant, theme).is_some())
            }))
        };
        let Some(left_resolved) = resolve(left)? else {
            return Ok(None);
        };
        let Some(right_resolved) = resolve(right)? else {
            return Ok(None);
        };
        Ok(Some(
            compare_variant_sequences(
                &left_resolved.candidate.variants,
                &right_resolved.candidate.variants,
                theme,
            )
            .then_with(|| {
                left_resolved
                    .order
                    .compare(left, &right_resolved.order, right)
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};
    use stillwind_test_support::css;

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
            let config = if mode.is_empty() {
                String::new()
            } else {
                format!("@stillwind {{utility-order:{mode};}}")
            };
            let source = SourceFile::new(
                FileId(1),
                None,
                None,
                format!("{css} {config} @stillwind-slot utilities;"),
            )
            .unwrap();
            let snapshot = DesignSystemSnapshot::from_source(
                &source,
                CssSourceName::from("test.css"),
                DesignSystemOptions::default(),
            )
            .unwrap();
            ordering_fingerprints.push(snapshot.fingerprints().ordering.clone());
            let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
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
                &CompilerOptions::default(),
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
        let theme = css::source(
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
    }
}
