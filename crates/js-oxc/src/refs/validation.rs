//! In-file validation of ref usage rules: ancestry and boundary containment.
//!
//! These checks run after the whole module is visited, because they compare uses with
//! declarations and with each other. A rule is only decided when the relevant JSX is
//! visible in this file: a use without a JSX path, or a ref whose provider lives in
//! another module, is skipped rather than reported, since nothing here can prove a
//! violation.
//!
//! Findings use the stable `require-anchor-provider`, `require-container-provider`,
//! `valid-ref-ancestry`, `require-boundary-provider`, `valid-boundary-ancestry`, and
//! `bounded-selector-in-boundary` rule names. These checks produce lint findings
//! directly; `swlint-*` suppression never applies to the compiler's fatal ref errors.

use std::collections::HashSet;

use stillwind_diagnostics::{LintFinding, LintKind};
use stillwind_ir::{ConditionalBranches, StyleInput};

use super::RefUseKind;
use crate::state::MacroExtractor;

impl MacroExtractor<'_> {
    /// Checks every ref use whose rule can be decided from this module.
    ///
    /// Anchors are collected across the whole module rather than per JSX tree, because
    /// a ref may be attached in one element and used in a nested one. A selector use is
    /// only reported when at least one anchor path is known and none of them is an
    /// ancestor.
    pub(crate) fn validate_refs(&mut self) {
        let names = self
            .ref_uses
            .iter()
            .map(|ref_use| ref_use.identity.clone())
            .collect::<HashSet<_>>();
        for identity in names {
            let uses = self
                .ref_uses
                .iter()
                .filter(|ref_use| ref_use.identity == identity)
                .collect::<Vec<_>>();
            let name = uses[0].name.clone();
            let has_boundary = self.ref_definitions.iter().any(|definition| {
                (
                    definition.module_id.clone(),
                    definition.binding.value.clone(),
                    definition.binding.span,
                ) == identity
                    && definition.boundary.is_some()
            });
            let has_anchor = uses
                .iter()
                .any(|ref_use| ref_use.projection == RefUseKind::AsAnchor);
            if !has_anchor {
                for ref_use in uses
                    .iter()
                    .filter(|ref_use| ref_use.projection == RefUseKind::PositionAnchor)
                {
                    self.lint_findings.push(LintFinding::new(
                        LintKind::MissingAnchor { name: name.clone() },
                        ref_use.span,
                    ));
                }
            }
            let has_container = uses
                .iter()
                .any(|ref_use| ref_use.projection == RefUseKind::AsContainer);
            for ref_use in uses.iter().filter(|ref_use| {
                ref_use.projection == RefUseKind::ContainerQueryVariant && !has_container
            }) {
                self.lint_findings.push(LintFinding::new(
                    LintKind::MissingContainer { name: name.clone() },
                    ref_use.span,
                ));
            }
            for provider in uses.iter().filter(|use_| {
                matches!(
                    use_.projection,
                    RefUseKind::AsAnchor | RefUseKind::AsContainer
                )
            }) {
                let owning_call = self.style_calls.iter().find(|call| {
                    call.span.start <= provider.span.start && call.span.end >= provider.span.end
                });
                let attached = uses.iter().any(|use_| {
                    use_.projection == RefUseKind::Marker
                        && (provider.jsx_path.is_some() && provider.jsx_path == use_.jsx_path
                            || owning_call.is_some_and(|call| {
                                call.span.start <= use_.span.start && call.span.end >= use_.span.end
                            }))
                });
                let uncertain =
                    owning_call.is_some_and(|call| call.inputs.iter().any(has_composition));
                if !attached && !uncertain && owning_call.is_some() {
                    self.lint_findings.push(LintFinding::new(
                        LintKind::MissingRefAttachment { name: name.clone() },
                        provider.span,
                    ));
                }
            }
            let known_anchor_paths = uses
                .iter()
                .filter_map(|ref_use| {
                    (ref_use.projection == RefUseKind::Marker)
                        .then_some(ref_use.jsx_path.as_ref())
                        .flatten()
                })
                .collect::<Vec<_>>();
            for ref_use in uses
                .iter()
                .filter(|ref_use| ref_use.projection == RefUseKind::SelectorVariant)
            {
                let Some(path) = &ref_use.jsx_path else {
                    continue;
                };
                if !has_boundary
                    && !known_anchor_paths.is_empty()
                    && !known_anchor_paths
                        .iter()
                        .any(|anchor_path| is_strict_ancestor(anchor_path, path))
                {
                    self.lint_findings.push(LintFinding::new(
                        LintKind::MissingRefAncestor { name: name.clone() },
                        ref_use.span,
                    ));
                }
            }
        }

        self.validate_bounded_refs();
    }

    /// Checks that every bounded ref's boundary is attached to a strict ancestor.
    ///
    /// Only bounded refs take part: an unbounded ref uses the descendant-only relation
    /// and has no boundary to look for. A boundary is provided by an element that
    /// attaches the boundary ref, so its attachments are the provider paths. Selector
    /// uses are checked separately from other uses because they target a different
    /// element.
    pub(crate) fn validate_bounded_refs(&mut self) {
        for (definition, boundary) in self.ref_definitions.iter().filter_map(|definition| {
            definition
                .boundary
                .as_ref()
                .map(|boundary| (definition, boundary))
        }) {
            let providers = self
                .ref_uses
                .iter()
                .filter(|use_| {
                    use_.identity
                        == (
                            boundary.module_id.clone(),
                            boundary.binding.value.clone(),
                            boundary.binding.span,
                        )
                        && use_.projection == RefUseKind::Marker
                })
                .collect::<Vec<_>>();
            if providers.is_empty() {
                self.lint_findings.push(LintFinding::new(
                    LintKind::UnattachedBoundary {
                        name: definition.binding.value.clone(),
                        boundary: boundary.binding.value.clone(),
                    },
                    definition.call_span,
                ));
                continue;
            }
            for reference_use in self.ref_uses.iter().filter(|use_| {
                use_.identity
                    == (
                        definition.module_id.clone(),
                        definition.binding.value.clone(),
                        definition.binding.span,
                    )
                    && matches!(
                        use_.projection,
                        RefUseKind::Marker | RefUseKind::AsAnchor | RefUseKind::AsContainer
                    )
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
                        LintKind::MissingBoundaryAncestor {
                            boundary: boundary.binding.value.clone(),
                            name: definition.binding.value.clone(),
                        },
                        reference_use.span,
                    ));
                }
            }
            for selector_use in self.ref_uses.iter().filter(|use_| {
                use_.identity
                    == (
                        definition.module_id.clone(),
                        definition.binding.value.clone(),
                        definition.binding.span,
                    )
                    && use_.projection == RefUseKind::SelectorVariant
            }) {
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
                        LintKind::SelectorOutsideBoundary {
                            name: definition.binding.value.clone(),
                            boundary: boundary.binding.value.clone(),
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
/// Equality is not ancestry, and the whole path must match, so two sibling boundaries
/// never look nested.
fn is_strict_ancestor(ancestor: &[usize], descendant: &[usize]) -> bool {
    ancestor.len() < descendant.len() && descendant.starts_with(ancestor)
}

/// Composition can supply an attachment that this module's JSX inspection cannot prove absent.
fn has_composition(input: &StyleInput) -> bool {
    match input {
        StyleInput::Composition { .. } => true,
        StyleInput::Array(inputs) => inputs.iter().any(has_composition),
        StyleInput::Conditional { branches, .. } => match branches {
            ConditionalBranches::And { consequent } => consequent.iter().any(has_composition),
            ConditionalBranches::Ternary {
                consequent,
                alternate,
            } => consequent.iter().chain(alternate).any(has_composition),
        },
        _ => false,
    }
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
