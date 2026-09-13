//! In-file validation of ref and scope usage rules.
//!
//! These checks run after the whole module is visited, because they compare uses with
//! declarations and with each other. A rule is only decided when the relevant JSX is
//! visible in this file: a use without a JSX path, or a ref whose provider lives in
//! another module, is skipped rather than reported, since nothing here can prove a
//! violation.
//!
//! Findings use the stable `require-anchor-provider`, `require-container-provider`,
//! `valid-ref-ancestry`, `require-scope-provider`, `valid-scope-ancestry`, and
//! `scoped-selector-in-scope` rule names. These checks produce lint findings directly;
//! `swlint-*` suppression never applies to the compiler's fatal ref errors.

use std::collections::HashSet;

use stillwind_diagnostics::{LintFinding, LintKind};

use crate::state::MacroExtractor;

impl MacroExtractor<'_> {
    /// Checks every ref use whose rule can be decided from this module.
    ///
    /// Anchors are collected across the whole module rather than per JSX tree, because
    /// a ref may be attached in one element and used in a nested one. A selector use is
    /// only reported when at least one anchor path is known and none of them is an
    /// ancestor.
    pub(crate) fn validate_handles(&mut self) {
        let names = self
            .handle_uses
            .iter()
            .map(|handle_use| handle_use.name.clone())
            .collect::<HashSet<_>>();
        for name in names {
            let uses = self
                .handle_uses
                .iter()
                .filter(|handle_use| handle_use.name == name)
                .collect::<Vec<_>>();
            let is_scoped = self
                .handle_definitions
                .iter()
                .any(|definition| definition.name.value == name && definition.scope.is_some());
            let has_anchor = uses
                .iter()
                .any(|handle_use| handle_use.projection == "anchor");
            if !has_anchor {
                for handle_use in uses
                    .iter()
                    .filter(|handle_use| handle_use.projection == "position")
                {
                    self.lint_findings.push(LintFinding::new(
                        LintKind::MissingAnchor { name: name.clone() },
                        handle_use.span,
                    ));
                }
            }
            let has_container = uses
                .iter()
                .any(|handle_use| handle_use.projection == "container");
            for handle_use in uses
                .iter()
                .filter(|handle_use| handle_use.projection == "container-query" && !has_container)
            {
                self.lint_findings.push(LintFinding::new(
                    LintKind::MissingContainer { name: name.clone() },
                    handle_use.span,
                ));
            }
            let known_anchor_paths = uses
                .iter()
                .filter_map(|handle_use| {
                    (handle_use.projection == "anchor" || handle_use.projection == "ref")
                        .then_some(handle_use.jsx_path.as_ref())
                        .flatten()
                })
                .collect::<Vec<_>>();
            for handle_use in uses
                .iter()
                .filter(|handle_use| handle_use.projection == "selector")
            {
                let Some(path) = &handle_use.jsx_path else {
                    continue;
                };
                if !is_scoped
                    && !known_anchor_paths.is_empty()
                    && !known_anchor_paths
                        .iter()
                        .any(|anchor_path| is_strict_ancestor(anchor_path, path))
                {
                    self.lint_findings.push(LintFinding::new(
                        LintKind::MissingRefAncestor { name: name.clone() },
                        handle_use.span,
                    ));
                }
            }
        }

        self.validate_scoped_refs();
    }

    /// Checks that every bound scope is attached to a strict ancestor.
    ///
    /// Only scoped refs take part: an unscoped ref uses the descendant-only relation
    /// and has no provider to look for. Selector uses are checked separately from other
    /// uses because they target a different element.
    pub(crate) fn validate_scoped_refs(&mut self) {
        for (definition, scope) in self
            .handle_definitions
            .iter()
            .filter_map(|definition| definition.scope.as_ref().map(|scope| (definition, scope)))
        {
            let providers = self
                .scope_uses
                .iter()
                .filter(|use_| use_.name == scope.name.value)
                .collect::<Vec<_>>();
            if providers.is_empty() {
                self.lint_findings.push(LintFinding::new(
                    LintKind::UnattachedScope {
                        name: definition.name.value.clone(),
                        scope: scope.name.value.clone(),
                    },
                    definition.call_span,
                ));
                continue;
            }
            for reference_use in self.handle_uses.iter().filter(|use_| {
                use_.name == definition.name.value
                    && !matches!(use_.projection, "selector" | "container-query")
            }) {
                let Some(reference_path) = &reference_use.jsx_path else {
                    continue;
                };
                if !providers.iter().any(|provider| {
                    provider
                        .jsx_path
                        .as_ref()
                        .is_some_and(|path| is_strict_ancestor(path, reference_path))
                }) {
                    self.lint_findings.push(LintFinding::new(
                        LintKind::MissingScopeAncestor {
                            scope: scope.name.value.clone(),
                            name: definition.name.value.clone(),
                        },
                        reference_use.span,
                    ));
                }
            }
            for selector_use in self
                .handle_uses
                .iter()
                .filter(|use_| use_.name == definition.name.value && use_.projection == "selector")
            {
                let Some(target_path) = &selector_use.jsx_path else {
                    continue;
                };
                if !providers.iter().any(|provider| {
                    provider
                        .jsx_path
                        .as_ref()
                        .is_some_and(|path| is_strict_ancestor(path, target_path))
                }) {
                    self.lint_findings.push(LintFinding::new(
                        LintKind::SelectorOutsideScope {
                            name: definition.name.value.clone(),
                            scope: scope.name.value.clone(),
                        },
                        selector_use.span,
                    ));
                }
            }
        }
    }
}

/// True when `ancestor` is a strict prefix of `descendant`.
///
/// Equality is not ancestry, and the whole path must match, so two sibling subtrees
/// never look nested.
fn is_strict_ancestor(ancestor: &[usize], descendant: &[usize]) -> bool {
    ancestor.len() < descendant.len() && descendant.starts_with(ancestor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestry_is_strict_and_requires_the_entire_prefix() {
        assert!(is_strict_ancestor(&[], &[1]));
        assert!(is_strict_ancestor(&[1, 2], &[1, 2, 3]));
        assert!(!is_strict_ancestor(&[1, 2], &[1, 2]));
        assert!(!is_strict_ancestor(&[1, 2], &[1, 3, 2]));
        assert!(!is_strict_ancestor(&[1], &[]));
    }
}
