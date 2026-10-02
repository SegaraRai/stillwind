//! Checks ref providers, attachments, and ancestry within one module.
//!
//! `validate_refs` runs after traversal so it can look for providers across all
//! recorded uses, including uses outside JSX. It checks whether a provider's ref is
//! attached in the same style call or on the same known JSX element. Composition can
//! supply attachments that are not visible here, so it suppresses that warning.
//! Ancestry checks need known JSX paths. Shared scope checks use the conditions
//! recorded at each selector use.
//!
//! These checks add lint findings. Extraction reports fatal syntax and identity
//! errors separately, and lint suppression does not affect those errors.

use std::collections::HashSet;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::{ConditionalBranches, StyleInput};

use super::RefUseKind;
use crate::state::MacroExtractor;

impl MacroExtractor<'_> {
    /// Checks ref uses after the entire module has been visited and adds lint findings.
    /// Providers are matched by declaration identity across all recorded uses.
    /// Attachments are checked using known style calls and JSX paths. Composition can
    /// prevent the check from proving that an attachment is missing.
    ///
    /// Selector ancestry is checked only when relevant marker paths are known. The
    /// function also validates shared scopes. Calling it again adds the findings again.
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
            let has_anchor = uses
                .iter()
                .any(|ref_use| ref_use.projection == RefUseKind::AsAnchor);
            if !has_anchor {
                for ref_use in uses
                    .iter()
                    .filter(|ref_use| ref_use.projection == RefUseKind::PositionAnchor)
                {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::MissingAnchor { name: name.clone() },
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
                    LintDiagnostic::MissingContainer { name: name.clone() },
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
                        LintDiagnostic::MissingRefAttachment { name: name.clone() },
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
                if ref_use.scopes.is_empty()
                    && !known_anchor_paths.is_empty()
                    && !known_anchor_paths
                        .iter()
                        .any(|anchor_path| is_strict_ancestor(anchor_path, path))
                {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::MissingRefAncestor { name: name.clone() },
                        ref_use.span,
                    ));
                }
            }
        }

        self.validate_ref_scopes();
    }

    /// Checks the scopes of each selector use. When JSX paths are known, the same
    /// scope attachment must contain both the reference and the styled target.
    /// Unknown paths do not prove an ancestry failure. Adds lint findings without
    /// rejecting extraction, and repeated calls add the same findings again.
    fn validate_ref_scopes(&mut self) {
        for selector in self
            .ref_uses
            .iter()
            .filter(|use_| use_.projection == RefUseKind::SelectorVariant)
        {
            for scope in &selector.scopes {
                let scope_name = scope
                    .refs
                    .iter()
                    .map(|definition| definition.binding.value.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ");
                let providers =
                    self.ref_uses
                        .iter()
                        .filter(|use_| {
                            use_.projection == RefUseKind::Marker
                                && scope.refs.iter().any(|definition| {
                                    use_.identity == self.ref_identity(definition)
                                })
                        })
                        .collect::<Vec<_>>();
                if providers.is_empty() {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::UnattachedRefScope {
                            name: selector.name.clone(),
                            scope: scope_name,
                        },
                        scope.prefix.span,
                    ));
                    continue;
                }
                let unknown_provider = providers.iter().any(|provider| provider.jsx_path.is_none());
                let matching = providers
                    .iter()
                    .filter(|provider| match (&provider.jsx_path, &selector.jsx_path) {
                        (Some(provider), Some(target)) => is_strict_ancestor(provider, target),
                        _ => true,
                    })
                    .collect::<Vec<_>>();
                if matching.is_empty() && !unknown_provider {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::TargetOutsideRefScope {
                            name: selector.name.clone(),
                            scope: scope_name,
                        },
                        selector.span,
                    ));
                    continue;
                }
                let references = self
                    .ref_uses
                    .iter()
                    .filter(|use_| {
                        use_.identity == selector.identity && use_.projection == RefUseKind::Marker
                    })
                    .collect::<Vec<_>>();
                let unknown_reference = references
                    .iter()
                    .any(|reference| reference.jsx_path.is_none());
                if !references.is_empty()
                    && !unknown_reference
                    && !unknown_provider
                    && !references.iter().any(|reference| {
                        reference.jsx_path.as_ref().is_some_and(|path| {
                            matching.iter().any(|provider| {
                                provider
                                    .jsx_path
                                    .as_ref()
                                    .is_some_and(|scope| is_strict_ancestor(scope, path))
                            })
                        })
                    })
                {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::RefOutsideScope {
                            name: selector.name.clone(),
                            scope: scope_name,
                        },
                        selector.span,
                    ));
                }
            }
        }
    }
}

/// True when `ancestor` is a strict prefix of `descendant`.
///
/// Equality is not ancestry, and the whole path must match, so two sibling scopes
/// never look nested.
fn is_strict_ancestor(ancestor: &[usize], descendant: &[usize]) -> bool {
    ancestor.len() < descendant.len() && descendant.starts_with(ancestor)
}

/// Returns whether `input` contains composition, including inside arrays and either
/// conditional branch. It does not predict which branch will run.
///
/// If composition is present, this call alone cannot prove a missing attachment.
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
