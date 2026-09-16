//! Ownership and ordered serialization of project-wide CSS rules.
//!
//! [`ProjectStylesheet::replace`] and [`ProjectStylesheet::remove`] track producer
//! membership. Serialization keeps module-id order and each producer's rule order,
//! emitting shared rules once. Session reload policy and stylesheet coverage planning
//! belong to their callers.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_compiler::{
    CompilerOptions, GeneratedRule, Identifiers, MappedStylesheet, ModuleCssArtifact, RuleKey,
    StylesheetPlacement, StylesheetRenderer,
};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::CompileErrorKind;
use stillwind_semantic::ModuleId;
use stillwind_source::{SourceFile, Span};

/// The project-wide stylesheet assembled from published module artifacts.
///
/// Module updates retract their previous ownership without removing rules still used
/// by other modules.
#[derive(Clone, Debug, Default)]
pub struct ProjectStylesheet {
    /// Content-identity lookup of rules and their current producers.
    rules: BTreeMap<RuleKey, RuleEntry>,
    /// Rule keys per module, in that artifact's emission order, used to retract its
    /// previous contribution.
    contributions: BTreeMap<ModuleId, Vec<RuleKey>>,
}

impl ProjectStylesheet {
    /// Replaces one module's contribution with the rules of `artifact`.
    ///
    /// Rules already contributed by another module are shared, not duplicated; keys the
    /// module no longer claims are dropped when their last user goes.
    /// Publicly constructed or edited rules are re-keyed from their current content.
    pub fn replace(&mut self, module: ModuleId, artifact: &ModuleCssArtifact) {
        self.remove(module.as_str());
        let mut keys = Vec::with_capacity(artifact.rules.len());
        for rule in &artifact.rules {
            let key = rule.content_key();
            keys.push(key.clone());
            self.rules
                .entry(key.clone())
                .or_insert_with(|| {
                    let mut rule = rule.clone();
                    rule.key = key;
                    RuleEntry {
                        rule,
                        users: BTreeSet::new(),
                    }
                })
                .users
                .insert(module.clone());
        }
        self.contributions.insert(module, keys);
    }

    /// Withdraws every rule this module contributed, keeping rules that other modules
    /// still use. Unknown modules are ignored.
    pub fn remove(&mut self, module: &str) {
        if let Some(keys) = self.contributions.remove(module) {
            for key in keys {
                if let Some(entry) = self.rules.get_mut(&key) {
                    entry.users.remove(module);
                    if entry.users.is_empty() {
                        self.rules.remove(&key);
                    }
                }
            }
        }
    }

    /// Number of distinct rules currently retained.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Serializes the retained module CSS without design-system wrapper rules.
    ///
    /// This is the input to [`ProjectStylesheet::serialize`]; hosts that need the
    /// module fragment alone, such as a CSS carrier module, use it directly.
    ///
    /// # Errors
    ///
    /// Returns an error if a contribution refers to a missing rule.
    pub fn module_css(&self) -> Result<String, CompileErrorKind> {
        let mapped = self.module_css_with_origins(|_, _| None)?;
        Ok(mapped
            .keyframes
            .iter()
            .map(|frames| frames.css.as_str())
            .chain(std::iter::once(mapped.css.as_str()))
            .collect())
    }

    /// Like [`ProjectStylesheet::module_css`] but reports a source location for each
    /// rule so the renderer can emit accurate source maps.
    ///
    /// The callback receives the producing module and the rule and returns the original
    /// file and span to attribute it to. Returning `None` leaves the rule unmapped; a
    /// rule is never attributed to a location that belongs to a different file.
    ///
    /// # Errors
    ///
    /// Returns [`CompileErrorKind::MissingRule`] when a retained contribution has no rule.
    pub(crate) fn module_css_with_origins(
        &self,
        mut origin: impl FnMut(&ModuleId, &GeneratedRule) -> Option<(Arc<SourceFile>, Span)>,
    ) -> Result<MappedStylesheet, CompileErrorKind> {
        let mut emitted = BTreeSet::new();
        let mut css = MappedStylesheet::default();
        for (module, keys) in &self.contributions {
            for key in keys {
                if emitted.insert(key) {
                    let entry = self.rules.get(key).ok_or(CompileErrorKind::MissingRule)?;
                    entry.rule.append_to(&mut css, origin(module, &entry.rule));
                }
            }
        }
        Ok(css)
    }

    /// Renders the full project stylesheet: module CSS plus the design system's own
    /// rules, wrappers, and authored directives.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent contributions or invalid assembled CSS.
    pub fn serialize(
        &self,
        snapshot: &DesignSystemSnapshot,
        options: &CompilerOptions,
        identifiers: &Identifiers,
    ) -> Result<String, CompileErrorKind> {
        StylesheetRenderer::new(snapshot, options, identifiers)
            .render_mapped_stylesheet(
                &self.module_css_with_origins(|_, _| None)?,
                StylesheetPlacement::Document,
            )
            .map(|css| css.css)
    }
}

/// One generated rule plus the set of producers that depend on it.
///
/// The rule body is shared; only the membership varies per module. Removing a producer
/// therefore never removes a rule another producer still needs.
#[derive(Clone, Debug)]
struct RuleEntry {
    /// Shared generated rule retained while at least one module uses it.
    rule: GeneratedRule,
    /// Module identities currently contributing this rule; set order is module-id order.
    users: BTreeSet<ModuleId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use stillwind_source::Fingerprint;

    #[test]
    fn support_metadata_participates_in_ownership_and_survives_removal() {
        let plain = GeneratedRule {
            key: RuleKey(Fingerprint::of("stale")),
            prelude: ".x".into(),
            body: Some("transform:var(--x)".into()),
            keyframes: false,
            required_properties: Vec::new(),
        };
        let mut supported = plain.clone();
        supported.required_properties.push("--x".into());
        let mut stylesheet = ProjectStylesheet::default();
        stylesheet.replace("plain".into(), &ModuleCssArtifact { rules: vec![plain] });
        stylesheet.replace(
            "supported".into(),
            &ModuleCssArtifact {
                rules: vec![supported],
            },
        );
        assert_eq!(stylesheet.rules.len(), 2);
        assert_eq!(
            stylesheet
                .module_css_with_origins(|_, _| None)
                .unwrap()
                .required_properties,
            ["--x"]
        );
        stylesheet.remove("supported");
        assert_eq!(stylesheet.rules.len(), 1);
        assert!(
            stylesheet
                .module_css_with_origins(|_, _| None)
                .unwrap()
                .required_properties
                .is_empty()
        );
    }

    #[test]
    fn missing_rule_is_a_serialization_error() {
        let mut stylesheet = ProjectStylesheet::default();
        stylesheet
            .contributions
            .insert("x".into(), vec![RuleKey(Fingerprint::of("missing"))]);
        assert!(matches!(
            stylesheet.module_css(),
            Err(CompileErrorKind::MissingRule)
        ));
    }

    #[test]
    fn removing_one_module_does_not_drop_rules_shared_by_another() {
        let artifact = ModuleCssArtifact {
            rules: vec![GeneratedRule {
                keyframes: false,
                required_properties: Vec::new(),
                key: RuleKey(Fingerprint::of(".x{color:red;}")),
                prelude: ".x".into(),
                body: Some("color:red;".into()),
            }],
        };
        let mut stylesheet = ProjectStylesheet::default();
        stylesheet.replace("a".into(), &artifact);
        stylesheet.replace("b".into(), &artifact);
        assert_eq!(stylesheet.rule_count(), 1);
        stylesheet.remove("a");
        assert_eq!(stylesheet.module_css().unwrap(), ".x{color:red;}");
        stylesheet.remove("b");
        assert_eq!(stylesheet.rule_count(), 0);
        stylesheet.remove("absent");
        assert_eq!(stylesheet.module_css().unwrap(), "");
    }
}
