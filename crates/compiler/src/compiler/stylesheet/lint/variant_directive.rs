//! `prefer-variant-directive`: nested style rules and conditional at-rules whose
//! headers repeat the CSS of a registered variant.
//!
//! Inside a style rule, `@variant <name> { body }` expands to the headers of the
//! variant around `body`. [`check_variant_directives`] reports authored headers that
//! [`VariantIndex`] maps to such a variant. The fix replaces only the headers with
//! `@variant <name>` and keeps the body, so the output selectors and their specificity
//! stay the same.
//!
//! A variant with two levels, such as `hover`, which applies `&:hover` inside
//! `@media (hover: hover)`, matches only when both levels are written, and its fix
//! replaces both headers. At-rules outside style rules are not reported, because
//! `@variant` there defines a variant. Definition bodies such as `@custom-variant` and
//! `@utility` are skipped, because a variant there could refer to itself.

use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssBlock, CssNode, CssNodeKind};
use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{
    FixAction, LintConfig, LintDiagnostic, LintFinding, LintKind, LintSettings, SourceEdit,
};

use super::analysis::{Checker, is_definition};
use super::headers::Level;
use super::variant_index::{VariantIndex, VariantMatch};
use crate::DesignSystemQuery;

/// Returns the variant index for `prefer-variant-directive`, or `None` when the rule is
/// disabled. One index serves every source of the snapshot that `query` reads.
pub(super) fn variant_directive_index(
    query: &DesignSystemQuery,
    theme: &ThemeIndex,
    settings: &LintSettings,
) -> Option<VariantIndex> {
    let Some(LintConfig::PreferVariantDirective(config)) =
        settings.get(LintKind::PreferVariantDirective)
    else {
        return None;
    };
    settings
        .enabled_for_detection(LintKind::PreferVariantDirective)
        .then(|| VariantIndex::new(query, theme, config.px_to_rem))
}

/// Reports the headers in `nodes` and their descendants that a variant of `index` can
/// replace.
pub(super) fn check_variant_directives(
    checker: &mut Checker<'_>,
    index: &VariantIndex,
    nodes: &[CssNode],
) {
    visit(checker, index, nodes, false, None);
}

/// Checks `nodes`. `in_rule` tells whether a style rule encloses them, and `covered` is
/// a node whose header an enclosing finding already replaces.
fn visit(
    checker: &mut Checker<'_>,
    index: &VariantIndex,
    nodes: &[CssNode],
    in_rule: bool,
    covered: Option<&CssNode>,
) {
    for node in nodes {
        let (block, nested) = match &node.kind {
            CssNodeKind::StyleRule { block, .. } => (block, true),
            CssNodeKind::AtRule {
                name,
                block: Some(block),
                ..
            } if name == "variant" && in_rule => (block, true),
            CssNodeKind::AtRule {
                name,
                block: Some(block),
                ..
            } if !is_definition(name) => (block, in_rule),
            _ => continue,
        };
        let inner = if in_rule && !covered.is_some_and(|covered| std::ptr::eq(covered, node)) {
            check_node(checker, index, node, block)
        } else {
            None
        };
        visit(checker, index, &block.children, nested, inner);
    }
}

/// Reports `node`, whose block is `block`, when a variant matches its header or its
/// header together with the header of its only child. Returns that child when the
/// finding covers it.
fn check_node<'a>(
    checker: &mut Checker<'_>,
    index: &VariantIndex,
    node: &'a CssNode,
    block: &'a CssBlock,
) -> Option<&'a CssNode> {
    let text = checker.source.text();
    let outer = level(text, node)?;
    if let Some(child) = only_child(text, block)
        && let Some(inner) = level(text, child)
        && let CssNodeKind::StyleRule {
            block: child_block, ..
        }
        | CssNodeKind::AtRule {
            block: Some(child_block),
            ..
        } = &child.kind
        && let Some(found) = index.find(&[outer, inner])
    {
        let header = header_range(text, child, child_block)?;
        let written = format_args!(
            "{} {{ {}",
            &text[header_range(text, node, block)?],
            &text[header]
        )
        .to_lean_str();
        // The first edit replaces both headers through the child's `{`, and the second
        // removes the child's `}` with the whitespace before the outer `}`.
        let edits = [
            (
                node.range.start..child_block.contents.start,
                format_args!("@variant {} {{", found.variant).to_lean_str(),
            ),
            (child_block.contents.end..block.contents.end, "".into()),
        ];
        report(checker, node.range.clone(), &written, &found, &edits);
        return Some(child);
    }
    let header = header_range(text, node, block)?;
    let found = index.find(&[outer])?;
    let replacement = format_args!("@variant {}", found.variant).to_lean_str();
    let written = text[header.clone()].to_lean_str();
    report(
        checker,
        header.clone(),
        &written,
        &found,
        &[(header, replacement)],
    );
    None
}

/// Returns the header of `node` as authored, or `None` for a node that is neither a
/// style rule nor an at-rule with a block.
fn level<'a>(text: &'a str, node: &'a CssNode) -> Option<Level<'a>> {
    match &node.kind {
        CssNodeKind::StyleRule { selector, .. } => Some(Level::Rule(&text[selector.clone()])),
        CssNodeKind::AtRule {
            name,
            prelude,
            block: Some(_),
        } => Some(Level::AtRule(name, &text[prelude.clone()])),
        _ => None,
    }
}

/// Returns the only node of `block` when nothing but whitespace surrounds it and both
/// blocks are closed, so that removing the child's braces keeps every other byte.
fn only_child<'a>(text: &str, block: &'a CssBlock) -> Option<&'a CssNode> {
    let [child] = block.children.as_slice() else {
        return None;
    };
    let closed = |end: usize| text[end..].starts_with('}');
    let blank = |range: Range<usize>| text[range].trim().is_empty();
    (closed(block.contents.end)
        && closed(child.range.end - 1)
        && blank(block.contents.start..child.range.start)
        && blank(child.range.end..block.contents.end))
    .then_some(child)
}

/// Returns the range from the start of `node` through the end of its header, without
/// the whitespace before the `{` that opens `block`.
fn header_range(text: &str, node: &CssNode, block: &CssBlock) -> Option<Range<usize>> {
    let open = block.contents.start.checked_sub(1)?;
    let header = text.get(node.range.start..open)?;
    Some(node.range.start..node.range.start + header.trim_end().len())
}

/// Records a finding at `range` whose fix applies `edits`, in increasing order.
fn report(
    checker: &mut Checker<'_>,
    range: Range<usize>,
    header: &str,
    found: &VariantMatch<'_>,
    edits: &[(Range<usize>, LeanStr)],
) {
    let mut finding = LintFinding::new(
        LintDiagnostic::PreferVariantDirective {
            header: header.into(),
            variant: found.variant.into(),
            px_to_rem: found.px_to_rem,
        },
        checker.span(range),
    );
    let edits = edits
        .iter()
        .map(|(range, replacement)| {
            Some(SourceEdit {
                range: checker.edit_range(checker.span(range.clone()))?,
                replacement: replacement.clone(),
            })
        })
        .collect::<Option<Vec<_>>>();
    if let Some(edits) = edits {
        finding.actions.push(FixAction {
            title: format_args!("Write `@variant {}`", found.variant).to_lean_str(),
            edits,
        });
    }
    checker.report(finding);
}

#[cfg(test)]
mod tests {
    use lightningcss::printer::PrinterOptions;
    use lightningcss::stylesheet::{MinifyOptions, ParserOptions, StyleSheet};
    use lightningcss::targets::{Browsers, Targets};

    use stillwind_diagnostics::{LintKind, LintOverrides, LintSeverity, VariantDirectiveOverride};

    use super::super::super::authored_directives::expand_authored_directives;
    use super::super::lint_stylesheets;
    use super::super::tests::{findings, fixed, snapshot};
    use crate::CompilerOptions;
    use crate::identifiers::{
        DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierContext, IdentifierMode,
    };

    /// Returns options that enable the rule, with `px_to_rem` as its option.
    fn options(px_to_rem: bool) -> CompilerOptions {
        CompilerOptions {
            lint: LintOverrides {
                prefer_variant_directive: Some(VariantDirectiveOverride {
                    severity: Some(LintSeverity::Warn),
                    px_to_rem: Some(px_to_rem),
                }),
                ..LintOverrides::default()
            },
            ..CompilerOptions::default()
        }
    }

    /// Returns the findings of the rule for `css`.
    fn directives(css: &str, px_to_rem: bool) -> Vec<(LintKind, String, Option<String>)> {
        findings(css, &options(px_to_rem))
            .into_iter()
            .filter(|(kind, ..)| *kind == LintKind::PreferVariantDirective)
            .collect()
    }

    /// Authored CSS, the text that its finding covers, and the fixed CSS.
    const EXAMPLES: &[(&str, &str, &str)] = &[
        (
            ".card { @media (width >= 40rem) { color: red; } }",
            "@media (width >= 40rem)",
            ".card { @variant sm { color: red; } }",
        ),
        (
            ".card { &:focus-visible { color: red; } }",
            "&:focus-visible",
            ".card { @variant focus-visible { color: red; } }",
        ),
        (
            ".card {\n  @media (hover: hover) {\n    &:hover { color: red; }\n  }\n}",
            "@media (hover: hover) {\n    &:hover { color: red; }\n  }",
            ".card {\n  @variant hover { color: red; }\n}",
        ),
        (
            ".card { &:hover { @media (hover: hover) { color: red; } } }",
            "&:hover { @media (hover: hover) { color: red; } }",
            ".card { @variant hover { color: red; } }",
        ),
        (
            ".card { @media (prefers-color-scheme: dark) { color: red; } }",
            "@media (prefers-color-scheme: dark)",
            ".card { @variant dark { color: red; } }",
        ),
    ];

    #[test]
    fn headers_of_registered_variants_are_replaced() {
        for &(css, covered, expected) in EXAMPLES {
            assert_eq!(
                directives(css, false),
                [fixed(LintKind::PreferVariantDirective, covered, expected)],
                "{css}"
            );
        }
    }

    #[test]
    fn equivalent_spellings_of_a_header_match() {
        for css in [
            ".card { @media (min-width: 40rem) { color: red; } }",
            ".card { @media ( WIDTH>=40rem ) { color: red; } }",
            ".card { @media (40rem <= width) { color: red; } }",
        ] {
            let found = directives(css, false);
            assert_eq!(found.len(), 1, "{css}");
            assert_eq!(
                found[0].2.as_deref(),
                Some(".card { @variant sm { color: red; } }"),
                "{css}"
            );
        }
        assert_eq!(
            directives(
                "@custom-variant grid (@supports (display: grid));\n\
                 .card { @supports ( display:grid ) { color: red; } }",
                false
            ),
            [fixed(
                LintKind::PreferVariantDirective,
                "@supports ( display:grid )",
                "@custom-variant grid (@supports (display: grid));\n\
                 .card { @variant grid { color: red; } }"
            )]
        );
    }

    #[test]
    fn headers_that_change_meaning_or_context_are_not_reported() {
        for css in [
            ".card { &:hover { color: red; } }",
            ".card { &:hover, &:focus { color: red; } }",
            ".card { :focus-visible { color: red; } }",
            ".card { @media (max-width: 40rem) { color: red; } }",
            ".card { @media (min-width: 640px) { color: red; } }",
            ".card { @media (hover: hover) { &:hover { color: red; } color: blue; } }",
            "@media (width >= 40rem) { .card { color: red; } }",
            "@custom-variant wide { .card & { @media (width >= 40rem) { @slot; } } }",
            "@utility wide { &:focus-visible { color: red; } }",
        ] {
            assert_eq!(directives(css, false), [], "{css}");
        }
    }

    #[test]
    fn px_conditions_match_rem_variants_only_with_the_option() {
        let css = ".card { @media (min-width: 640px) { color: red; } }";
        assert_eq!(directives(css, false), []);
        assert_eq!(
            directives(css, true),
            [fixed(
                LintKind::PreferVariantDirective,
                "@media (min-width: 640px)",
                ".card { @variant sm { color: red; } }"
            )]
        );
        let options = options(true);
        let lint = lint_stylesheets(&snapshot(css, &options), &options);
        let message = lint.findings[0].diagnostic.to_string();
        assert!(message.contains("default font size"), "{message}");
        let lint = lint_stylesheets(
            &snapshot(
                ".card { @media (width >= 40rem) { color: red; } }",
                &options,
            ),
            &options,
        );
        let message = lint.findings[0].diagnostic.to_string();
        assert!(!message.contains("font size"), "{message}");
    }

    #[test]
    fn fixed_css_compiles_to_the_same_selectors() {
        let options = CompilerOptions::default();
        let snapshot = snapshot("", &options);
        let identifiers = IdentifierContext::new(
            GeneratorSet::default(),
            IdentifierMode::Release,
            DEFAULT_HASH_LENGTH,
        );
        let compile = |css: &str| {
            let expanded =
                expand_authored_directives(css, snapshot.theme(), &mut Vec::new(), &identifiers)
                    .unwrap();
            let mut sheet = StyleSheet::parse(&expanded, ParserOptions::default()).unwrap();
            let targets = Targets::from(Browsers {
                chrome: Some(100 << 16),
                ..Browsers::default()
            });
            sheet
                .minify(MinifyOptions {
                    targets,
                    ..MinifyOptions::default()
                })
                .unwrap();
            sheet
                .to_css(PrinterOptions {
                    minify: true,
                    targets,
                    ..PrinterOptions::default()
                })
                .unwrap()
                .code
        };
        for &(css, _, fixed) in EXAMPLES {
            let output = compile(css);
            assert!(!output.contains('&'), "{output}");
            assert_eq!(compile(fixed), output, "{css}");
        }
    }
}
