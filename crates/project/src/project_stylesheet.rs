//! Occurrence ownership and reference-order assembly.
//!
//! [`ProjectStylesheet::replace`] retains each producer's rule occurrences, including
//! repeated content. [`ProjectStylesheet::serialize`] expands the design-system template.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_compiler::{
    GeneratedRule, MappedStylesheet, ModuleCssArtifact, StylesheetPlacement, StylesheetRenderer,
};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_semantic::ModuleId;
use stillwind_source::{SourceFile, Span};

/// Published rule occurrences ordered by the supplied producer list.
/// Producers absent from that list follow in module-identity order.
#[derive(Clone, Debug, Default)]
pub struct ProjectStylesheet {
    contributions: BTreeMap<ModuleId, ModuleCssArtifact>,
    order: Vec<ModuleId>,
}

impl ProjectStylesheet {
    /// Replaces every occurrence owned by a producer.
    pub fn replace(&mut self, module: ModuleId, artifact: &ModuleCssArtifact) {
        self.contributions.insert(module, artifact.clone());
    }

    /// Retracts one producer without changing any other producer's occurrences.
    pub fn remove(&mut self, module: &str) {
        self.contributions.remove(module);
        self.order.retain(|id| id.as_str() != module);
    }

    /// Sets the dependency order computed by the linker.
    pub(crate) fn set_order(&mut self, order: Vec<ModuleId>) {
        self.order = order;
    }

    /// Counts distinct content payloads, independently of occurrence count.
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
    pub fn module_css(&self) -> Result<String, CompileErrorKind> {
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

    /// Renders reference order through the design-system template.
    ///
    /// # Errors
    ///
    /// Returns invalid template or support errors.
    pub fn serialize(&self, renderer: &StylesheetRenderer<'_>) -> Result<String, CompileErrorKind> {
        renderer
            .render_mapped_stylesheet(
                &self.module_css_with_origins(|_, _| None),
                StylesheetPlacement::Document,
            )
            .map(|mapped| mapped.css)
    }
}

#[cfg(test)]
mod tests {
    use stillwind_compiler::RuleKey;
    use stillwind_css::CssRule;
    use stillwind_source::Fingerprint;

    use super::*;

    fn artifact(css: &str) -> ModuleCssArtifact {
        ModuleCssArtifact {
            rules: CssRule::parse(css)
                .unwrap()
                .into_iter()
                .map(|syntax| GeneratedRule {
                    syntax,
                    key: RuleKey(Fingerprint::of("untrusted")),
                    keyframes: false,
                    required_properties: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn identical_content_retains_each_cascade_occurrence_and_owner() {
        let mut sheet = ProjectStylesheet::default();
        sheet.replace("a".into(), &artifact(".x{color:red}"));
        sheet.replace("b".into(), &artifact(".x{color:blue}"));
        sheet.replace("c".into(), &artifact(".x{color:red}"));
        assert_eq!(sheet.rule_count(), 2);
        assert_eq!(
            sheet.module_css().unwrap(),
            ".x{color:red}.x{color:blue}.x{color:red}"
        );
        sheet.remove("c");
        assert_eq!(sheet.module_css().unwrap(), ".x{color:red}.x{color:blue}");
        sheet.set_order(vec!["b".into(), "a".into()]);
        assert_eq!(sheet.module_css().unwrap(), ".x{color:blue}.x{color:red}");
    }
}
