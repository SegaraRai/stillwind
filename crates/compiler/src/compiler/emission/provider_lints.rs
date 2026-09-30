//! Source-located property supply checks before class allocation erases use boundaries.

use std::collections::BTreeSet;

use cssparser::Parser;

use stillwind_css::{declaration_ranges, properties_interfere};
use stillwind_diagnostics::{Label, LintFinding, LintKind};
use stillwind_source::Span;

use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::CallEmitter;

/// CSS properties added by one use, recorded before class units are merged.
///
/// When a use can resolve to several receivers, their records share the same span.
/// Conflict checks use that span to avoid comparing alternatives from the same use.
pub(super) struct PropertyWrite {
    properties: Vec<String>,
    /// Whether the properties come from a ref provider or `style.anchorScope`.
    provider: bool,
    span: Span,
    /// Enclosing JavaScript conditions, outermost first.
    guard: Vec<GuardStep>,
    /// The receiver's canonical marker token, if the selector requires that marker.
    gate: Option<String>,
}

impl CallEmitter<'_> {
    /// Records the CSS property names in `contribution` for later conflict checks.
    ///
    /// `span` identifies the source use, and `guard` contains its enclosing JavaScript
    /// conditions. For provider uses, `gate` contains the receiver's canonical marker
    /// token, if the selector requires one.
    ///
    /// Ordinary utilities with selectors are skipped because the compiler cannot
    /// determine which element they affect. Provider declarations are included even
    /// when they have selectors. Malformed declarations and contributions with no
    /// properties are skipped.
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

    /// Returns whether this call's attachments prove that markers `a` and `b` cannot
    /// appear together. The caller must pass two distinct canonical marker tokens.
    ///
    /// Both markers must have a recorded attachment site. They are exclusive if every
    /// pair of sites either selects between them or lies in opposite branches of the
    /// same source condition. The check does not inspect attachments outside this call.
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

    /// Appends warnings for conflicting CSS properties after all uses and attachments
    /// have been recorded. Each pair must include a provider or an explicit scope.
    /// Shorthands count as conflicts when they affect the same property.
    ///
    /// The check skips alternatives from one use and uses whose conditions or marker
    /// requirements cannot hold together. It reports each source pair at most once
    /// per invocation and leaves CSS and its order unchanged.
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
