//! Source-located property supply checks before class allocation erases use boundaries.

use std::collections::BTreeSet;

use cssparser::Parser;

use stillwind_css::{declaration_ranges, properties_interfere};
use stillwind_diagnostics::{Label, LintFinding, LintKind};
use stillwind_source::Span;

use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::CallEmitter;

pub(super) struct PropertyWrite {
    properties: Vec<String>,
    provider: bool,
    span: Span,
    guard: Vec<GuardStep>,
    gate: Option<String>,
}

impl CallEmitter<'_> {
    /// Retains declarations targeting the element for later overlap checks.
    /// Arbitrary selector targets are excluded when their overlap cannot be established.
    pub(super) fn record_property_writes(
        &mut self,
        contribution: &LiteralContribution,
        provider: bool,
        span: Span,
        guard: &[GuardStep],
        gate: Option<&str>,
    ) {
        let properties = contribution
            .utilities
            .iter()
            .filter(|utility| provider || utility.selector.is_none())
            .flat_map(|utility| {
                declaration_ranges(&utility.declaration)
                    .into_iter()
                    .filter_map(|range| {
                        let mut parser = Parser::new(&utility.declaration[range]);
                        parser.expect_ident().ok().map(ToString::to_string)
                    })
            })
            .collect::<Vec<_>>();
        if !properties.is_empty() {
            self.property_writes.push(PropertyWrite {
                properties,
                provider,
                span,
                guard: guard.to_vec(),
                gate: gate.map(str::to_owned),
            });
        }
    }

    /// Proves two distinct marker gates cannot be satisfied by this call's attachments.
    fn exclusive_markers(&self, a: &str, b: &str) -> bool {
        let a_sites = self
            .attachment_sets
            .iter()
            .enumerate()
            .filter(|(_, (names, _))| names.iter().any(|name| name == a))
            .collect::<Vec<_>>();
        let b_sites = self
            .attachment_sets
            .iter()
            .enumerate()
            .filter(|(_, (names, _))| names.iter().any(|name| name == b))
            .collect::<Vec<_>>();
        !a_sites.is_empty()
            && !b_sites.is_empty()
            && a_sites.iter().all(|(ai, (_, ag))| {
                b_sites.iter().all(|(bi, (_, bg))| {
                    ai == bi
                        || ag.iter().any(|a| {
                            bg.iter()
                                .any(|b| a.test == b.test && a.when_true != b.when_true)
                        })
                })
            })
    }

    /// Warns once per conflicting source pair; no winner is inferred from CSS order.
    pub(super) fn report_provider_conflicts(&mut self) {
        let mut reported = BTreeSet::new();
        for (index, current) in self.property_writes.iter().enumerate() {
            for previous in &self.property_writes[..index] {
                if !(current.provider || previous.provider) || current.span == previous.span {
                    continue;
                }
                if current.guard.iter().any(|a| {
                    previous
                        .guard
                        .iter()
                        .any(|b| a.test == b.test && a.when_true != b.when_true)
                }) {
                    continue;
                }
                if let (Some(a), Some(b)) = (&current.gate, &previous.gate)
                    && a != b
                    && self.exclusive_markers(a, b)
                {
                    continue;
                }
                let mut properties = Vec::new();
                for a in &current.properties {
                    for b in &previous.properties {
                        if properties_interfere(a, b) {
                            let pair = if a == b {
                                a.clone()
                            } else {
                                format!("{a} and {b}")
                            };
                            if !properties.contains(&pair) {
                                properties.push(pair);
                            }
                        }
                    }
                }
                if !properties.is_empty()
                    && reported.insert((previous.span.start.0, current.span.start.0))
                {
                    let mut finding = LintFinding::new(
                        LintKind::ProviderConflict {
                            properties: properties.join(", "),
                        },
                        current.span,
                    );
                    finding.info.labels.push(Label {
                        span: previous.span,
                        message: Some("overlapping property source".into()),
                    });
                    self.lint_findings.push(finding);
                }
            }
        }
    }
}
