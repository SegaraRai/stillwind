//! Keeps each producer's CSS rules and assembles them in the requested order.
//!
//! [`ProjectStylesheet::replace`] retains each producer's rule occurrences, including
//! repeated content. [`ProjectStylesheet::serialize`] expands the design-system template.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_compiler::{
    GeneratedRule, MappedStylesheet, ModuleCssArtifact, RenderedStylesheet, StylesheetPlacement,
    StylesheetRenderError, StylesheetRenderer,
};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::{ModuleId, SourceFile, Span};

/// Published rule occurrences ordered by the supplied producer list.
/// Producers absent from that list follow in module-identity order.
#[derive(Clone, Debug, Default)]
pub struct ProjectStylesheet {
    /// `Arc` shares rules between session clones, so cloning does not copy every rule.
    /// Replacing a producer's rules leaves other stylesheet clones unchanged.
    contributions: BTreeMap<ModuleId, Arc<ModuleCssArtifact>>,
    /// The linker sets this order. Other producers follow in module-identity order.
    order: Vec<ModuleId>,
}

impl ProjectStylesheet {
    /// Replaces every occurrence owned by a producer.
    ///
    /// `module` identifies the owner. This method copies `artifact`, including repeated
    /// rules, and keeps the existing order. Later changes to `artifact` do not change
    /// the stored rules.
    pub fn replace(&mut self, module: ModuleId, artifact: &ModuleCssArtifact) {
        self.contributions
            .insert(module, Arc::new(artifact.clone()));
    }

    /// Retracts one producer without changing any other producer's occurrences.
    pub fn remove(&mut self, module: &ModuleId) {
        self.contributions.remove(module);
        self.order.retain(|id| id != module);
    }

    /// Sets the dependency order computed by the linker.
    pub(crate) fn set_order(&mut self, order: Vec<ModuleId>) {
        self.order = order;
    }

    /// Returns the number of distinct rule contents. Repeated rules count once.
    pub fn rule_count(&self) -> usize {
        self.contributions
            .values()
            .flat_map(|artifact| artifact.rules.iter().map(GeneratedRule::content_key))
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// Serializes keyframes followed by other rules, before template expansion.
    ///
    /// # Errors
    ///
    /// No errors are currently produced.
    pub fn module_css(&self) -> Result<String, CompileDiagnostic> {
        let mapped = self.module_css_with_origins(|_, _| None);
        Ok(mapped
            .keyframes
            .iter()
            .map(|frames| frames.css.as_str())
            .chain(std::iter::once(mapped.css.as_str()))
            .collect())
    }

    pub(crate) fn module_css_with_origins(
        &self,
        mut origin: impl FnMut(&ModuleId, &GeneratedRule) -> Option<(Arc<SourceFile>, Span)>,
    ) -> MappedStylesheet {
        let mut output = MappedStylesheet::default();
        let mut modules = BTreeSet::new();
        for module in self.order.iter().chain(self.contributions.keys()) {
            if !modules.insert(module) {
                continue;
            }
            if let Some(artifact) = self.contributions.get(module) {
                for rule in &artifact.rules {
                    rule.append_to(&mut output, origin(module, rule));
                }
            }
        }
        output
    }

    /// Uses `renderer` to assemble the rules in order within the design-system
    /// template. Returns mapped CSS and naming fallbacks without changing this stylesheet.
    ///
    /// # Errors
    ///
    /// Returns an error if the template or required support rules are invalid.
    pub fn serialize(
        &self,
        renderer: &StylesheetRenderer<'_>,
    ) -> Result<RenderedStylesheet, StylesheetRenderError> {
        renderer.render_mapped_stylesheet(
            &self.module_css_with_origins(|_, _| None),
            StylesheetPlacement::Document,
        )
    }
}

#[cfg(test)]
mod tests {
    use stillwind_css::CssRule;

    use super::*;

    fn artifact(css: &str) -> ModuleCssArtifact {
        ModuleCssArtifact {
            rules: CssRule::parse(css)
                .unwrap()
                .into_iter()
                .map(|syntax| GeneratedRule::new(syntax, false, Vec::new()))
                .collect(),
        }
    }

    #[test]
    fn identical_content_retains_each_cascade_occurrence_and_owner() {
        let mut sheet = ProjectStylesheet::default();
        sheet.replace(ModuleId::from("a"), &artifact(".x{color:red}"));
        sheet.replace(ModuleId::from("b"), &artifact(".x{color:blue}"));
        sheet.replace(ModuleId::from("c"), &artifact(".x{color:red}"));
        assert_eq!(sheet.rule_count(), 2);
        assert_eq!(
            sheet.module_css().unwrap(),
            ".x{color:red}.x{color:blue}.x{color:red}"
        );
        sheet.remove(&ModuleId::from("c"));
        assert_eq!(sheet.module_css().unwrap(), ".x{color:red}.x{color:blue}");
        sheet.set_order(vec![ModuleId::from("b"), ModuleId::from("a")]);
        assert_eq!(sheet.module_css().unwrap(), ".x{color:blue}.x{color:red}");
    }

    #[test]
    fn publication_copies_rules_and_retracts_each_owner_independently() {
        let mut artifact = ModuleCssArtifact {
            rules: vec![GeneratedRule::new(
                CssRule {
                    range: 0..0,
                    prelude: ".x".into(),
                    body: Some("color:red;".into()),
                },
                false,
                Vec::new(),
            )],
        };
        let mut sheet = ProjectStylesheet::default();
        sheet.replace(ModuleId::from("a"), &artifact);
        artifact.rules[0] = GeneratedRule::new(
            CssRule {
                range: 0..0,
                prelude: ".x".into(),
                body: Some("color:blue;".into()),
            },
            false,
            Vec::new(),
        );
        sheet.replace(ModuleId::from("b"), &artifact);
        assert_eq!(sheet.rule_count(), 2);
        assert!(sheet.module_css().unwrap().contains("color:red;"));
        assert!(sheet.module_css().unwrap().contains("color:blue;"));
        sheet.remove(&ModuleId::from("a"));
        assert_eq!(sheet.rule_count(), 1);
        assert!(!sheet.module_css().unwrap().contains("color:red;"));
    }
}
