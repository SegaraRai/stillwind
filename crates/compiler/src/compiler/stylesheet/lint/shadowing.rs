//! `duplicate-definition`: definitions that a later definition with the same name
//! hides completely.
//!
//! Variant resolution tries later definitions first and falls back only after a
//! failure, so a later variant definition without a capture that resolves hides every
//! earlier one. Exact `@utility` definitions with the same name merge instead: the
//! later definition wins each property that it sets, and the earlier one still adds
//! the other properties. An earlier utility is therefore hidden only when later
//! definitions set every property that it declares.
//!
//! Definitions with a capture, such as `tab-*`, are overloads that resolution falls
//! back to, and they are not reported. Built-in definitions are not compared, so
//! redefining a built-in name such as `dark` is not reported. Definitions are ordered
//! by their position in the stylesheet after import expansion, so a definition in an
//! imported file can hide one in the importing file.

use std::collections::BTreeSet;
use std::ops::Range;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{CssNode, CssNodeKind, parse_structure};
use stillwind_design_system::{
    CssSource, DesignSystemSnapshot, ThemeIndex, exact_variant_definition_resolves,
    parse_variant_definitions,
};
use stillwind_diagnostics::{Label, LabelScope, LintDiagnostic, LintFinding};
use stillwind_source::{FileId, Span};

use super::definitions::{DefinitionKind, definitions};

/// One user definition without a capture, with its positions after import expansion.
struct Entry<'a> {
    source: &'a CssSource,
    kind: DefinitionKind,
    directive: LeanStr,
    name: Range<usize>,
    node: Range<usize>,
    /// Properties of the top-level declarations of a utility body in ASCII lowercase,
    /// or `None` when the body has other nodes or is empty.
    properties: Option<BTreeSet<LeanString>>,
    /// Whether a later definition with the same name can fail and fall back to this
    /// one. A utility that reads `--value()` or `--modifier()` can fail.
    can_fail: bool,
    /// First and last position of the definition in the expanded stylesheet. A source
    /// that is imported twice appears at two positions.
    first: usize,
    last: usize,
}

impl Entry<'_> {
    fn text(&self) -> &str {
        self.source.source.text()
    }

    fn name_text(&self) -> &str {
        &self.text()[self.name.clone()]
    }

    fn span(&self, range: Range<usize>) -> Span {
        let offset = |position: usize| u32::try_from(position).unwrap_or(u32::MAX);
        Span::new(
            self.source.source.id(),
            offset(range.start),
            offset(range.end),
        )
    }
}

/// Returns a finding for each definition in a source marked for lint that a later
/// definition hides, at the hidden definition's name and with a label at the name of
/// the latest hiding definition.
///
/// The label is omitted when the hiding definition is in a source without a document
/// URI.
pub(super) fn shadowed_definitions(snapshot: &DesignSystemSnapshot) -> Vec<LintFinding> {
    let entries = entries(snapshot);
    let mut findings = Vec::new();
    for earlier in entries.iter().filter(|entry| entry.source.lint) {
        let Some(hiding) = hiding_definition(earlier, &entries, snapshot.theme()) else {
            continue;
        };
        let mut finding = LintFinding::new(
            LintDiagnostic::DuplicateDefinition {
                directive: format_args!("@{}", earlier.directive).to_lean_str(),
                name: earlier.name_text().to_lean_str(),
            },
            earlier.span(earlier.name.clone()),
        );
        if hiding.source.source.uri().is_some() {
            finding.info.labels.push(Label {
                scope: LabelScope::Range,
                span: hiding.span(hiding.name.clone()),
                message: Some("later definition".into()),
            });
        }
        findings.push(finding);
    }
    findings
}

/// Returns the user definitions without a capture of every source of `snapshot`, with
/// their positions after import expansion. Definitions outside the expanded stylesheet
/// are skipped.
fn entries(snapshot: &DesignSystemSnapshot) -> Vec<Entry<'_>> {
    let graph = snapshot.sources();
    let mut entries = Vec::new();
    for source in &graph.sources {
        let text = source.source.text();
        if !["@utility", "@custom-variant", "@variant"]
            .iter()
            .any(|directive| text.contains(directive))
        {
            continue;
        }
        let nodes = parse_structure(text);
        for definition in definitions(&nodes, text) {
            if definition.kind == DefinitionKind::BuiltinVariant || definition.has_capture(text) {
                continue;
            }
            let positions = graph
                .mappings
                .iter()
                .filter(|mapping| mapping.file == source.source.id())
                .filter_map(|mapping| {
                    let offset = definition
                        .node
                        .range
                        .start
                        .checked_sub(mapping.source_start)?;
                    (offset < mapping.generated.len()).then_some(mapping.generated.start + offset)
                })
                .collect::<Vec<_>>();
            let (Some(&first), Some(&last)) = (positions.iter().min(), positions.iter().max())
            else {
                continue;
            };
            let block_text = definition
                .block
                .map_or("", |block| &text[block.contents.clone()]);
            entries.push(Entry {
                source,
                kind: definition.kind,
                directive: definition.directive.to_lean_str(),
                name: definition.name.clone(),
                node: definition.node.range.clone(),
                properties: definition
                    .block
                    .and_then(|block| declared_properties(&block.children, text)),
                can_fail: block_text.contains("--value(") || block_text.contains("--modifier("),
                first,
                last,
            });
        }
    }
    entries
}

/// Returns the ASCII-lowercase properties of `children` when every node is a
/// declaration, or `None` for an empty body or a body with other nodes.
fn declared_properties(children: &[CssNode], text: &str) -> Option<BTreeSet<LeanString>> {
    if children.is_empty() {
        return None;
    }
    children
        .iter()
        .map(|node| match &node.kind {
            CssNodeKind::Declaration { property, .. } => {
                Some(text[property.clone()].to_ascii_lowercase().to_lean_string())
            }
            _ => None,
        })
        .collect()
}

/// Returns the latest definition among `entries` that hides `earlier` completely, as
/// the module documentation describes, or `None` when `earlier` is still used.
fn hiding_definition<'e, 'a>(
    earlier: &Entry<'a>,
    entries: &'e [Entry<'a>],
    theme: &ThemeIndex,
) -> Option<&'e Entry<'a>> {
    let later = entries.iter().filter(|entry| {
        entry.kind == earlier.kind
            && entry.first > earlier.last
            && entry.name_text() == earlier.name_text()
    });
    match earlier.kind {
        DefinitionKind::Variant => later
            .filter(|entry| {
                parse_variant_definitions(&entry.text()[entry.node.clone()])
                    .first()
                    .is_some_and(|(_, custom)| exact_variant_definition_resolves(custom, theme))
            })
            .max_by_key(|entry| entry.first),
        DefinitionKind::Utility => {
            let later = later.filter(|entry| !entry.can_fail).collect::<Vec<_>>();
            let covered = later
                .iter()
                .filter_map(|entry| entry.properties.as_ref())
                .flatten()
                .collect::<BTreeSet<_>>();
            earlier
                .properties
                .as_ref()
                .filter(|properties| properties.iter().all(|name| covered.contains(name)))
                .and_then(|_| later.into_iter().max_by_key(|entry| entry.first))
        }
        DefinitionKind::BuiltinVariant => None,
    }
}

/// Returns whether `finding` addresses the source `file`.
pub(super) fn addresses(finding: &LintFinding, file: FileId) -> bool {
    finding
        .labels
        .first()
        .is_some_and(|label| label.span.file == file)
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;

    use super::super::lint_stylesheets;
    use super::super::tests::{default_findings, finding, snapshot};
    use crate::CompilerOptions;

    #[test]
    fn hidden_definitions_are_reported_at_the_earlier_one() {
        assert_eq!(
            default_findings(
                "@custom-variant hv (&:hover);\
                 @custom-variant hv (&:focus);\
                 @utility tab { tab-size: 4; }\
                 @utility tab { tab-size: 8; }\
                 @custom-variant blk { &:hover { @slot; } }\
                 @variant blk { &:focus { @slot; } }"
            ),
            [
                finding(LintKind::DuplicateDefinition, "hv"),
                finding(LintKind::DuplicateDefinition, "tab"),
                finding(LintKind::DuplicateDefinition, "blk"),
            ]
        );
        let options = CompilerOptions::default();
        let snapshot = snapshot(
            "@utility tab { tab-size: 4; } @utility tab { tab-size: 8; }",
            &options,
        );
        let lint = lint_stylesheets(&snapshot, &options);
        let source = &snapshot.sources().sources[0].source;
        let labels = &lint.findings[0].labels;
        assert_eq!(source.slice(labels[1].span).unwrap(), "tab");
        assert!(labels[1].span.start > labels[0].span.start);
    }

    #[test]
    fn overloads_partial_overrides_and_built_ins_are_not_reported() {
        for css in [
            "@utility tab-* { tab-size: --value(integer); } @utility tab-* { tab-size: --value([length]); }",
            "@utility tab { tab-size: 4; color: red; } @utility tab { tab-size: 8; }",
            "@utility tab { tab-size: 4; } @utility tab { &:hover { tab-size: 8; } }",
            "@custom-variant hv (&:hover); @custom-variant hv (color:);",
            "@custom-variant dark (&:where(.dark, .dark *));",
        ] {
            assert_eq!(default_findings(css), [], "{css}");
        }
    }
}
