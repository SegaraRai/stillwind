//! Expansion of `@apply` and `@variant` in authored CSS.
//!
//! [`expand_authored_directives`] replaces utility applications with declarations and
//! variant blocks with selector rules, using the design system's resolution and
//! declaration-ordering rules. It appends registration requirements to a separate
//! accumulator; it does not select registrations or place output slots.
//!
//! Directives are found through [`parse_structure`], which stylesheet lint also uses,
//! so expansion and lint agree on which text is a directive. An unresolved application
//! candidate contributes nothing. A variant sequence that does not parse or resolve is
//! a false condition and contributes nothing, so a block whose sequences all fail is
//! removed with its contents.

use std::ops::Range;

use cssparser::Token;
use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_css::{
    CssBanner, CssNode, CssNodeKind, fold_nested_host_conditions, parse_structure,
    selector_selects_host, split_top_level, whitespace_separated_ranges,
};
use stillwind_design_system::{
    ThemeIndex, VariantApplication, VariantContext, compare_variant_sequences, resolve_variant,
};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_syntax::{Variant, parse_candidate};

use super::super::conditionals::guard;
use super::super::rule_boundaries::protect;
use super::super::utilities::{
    assemble_nested_declarations, order_utilities, prepend_missing_declarations,
    resolve_utility_targets,
};
use super::super::variants::{VariantStep, variant_paths};
use crate::identifiers::IdentifierContext;

/// Returns the notices of the utilities that the `@apply` statements in `css` apply,
/// including the utilities that their bodies apply in turn.
///
/// Candidates that do not resolve add no notices. [`expand_authored_directives`]
/// reports them.
pub(crate) fn applied_banners(css: &str, theme: &ThemeIndex) -> Vec<CssBanner> {
    fn visit(css: &str, theme: &ThemeIndex, seen: &mut Vec<LeanStr>, output: &mut Vec<CssBanner>) {
        fn nodes(
            css: &str,
            items: &[CssNode],
            theme: &ThemeIndex,
            seen: &mut Vec<LeanStr>,
            output: &mut Vec<CssBanner>,
        ) {
            for node in items {
                if let Some(candidates) = apply_candidates(css, node) {
                    for range in candidates {
                        let candidate = &css[range];
                        if seen.iter().any(|name| name == candidate) {
                            continue;
                        }
                        seen.push(candidate.into());
                        if let Ok(Ok(targets)) =
                            resolve_utility_targets(candidate, theme, VariantContext::Element)
                        {
                            for target in targets {
                                output.extend(target.banners);
                                visit(&target.declaration, theme, seen, output);
                            }
                        }
                    }
                }
                nodes(css, node.children(), theme, seen, output);
            }
        }
        if css.contains("@apply") {
            nodes(css, &parse_structure(css), theme, seen, output);
        }
    }
    let mut output = Vec::new();
    visit(css, theme, &mut Vec::new(), &mut output);
    output
}

/// Rewrites every `@apply` statement and `@variant` block in `css`.
///
/// Returns a new string; `css` and `theme` are not modified. Resolved applications
/// append registration names to `required_properties` in utility emission order,
/// retaining existing entries and duplicates. Applications are expanded before
/// variants, including applications introduced by utility definitions. Every resolved
/// target retains its selector and conditions. Within an application, each candidate
/// with variants becomes nested rules whose anchor is the enclosing rule, and its
/// declarations follow those of candidates without variants. Candidates are ordered
/// by variants and then by utility order.
/// Runtime guards remain unresolved `--if` expressions for the later options-aware
/// lowering pass. A `@variant` statement without a block remains in the returned text,
/// so this function is not a CSS validator.
///
/// # Errors
///
/// Returns a malformed requirement, a circular application, or the host generator's
/// failure for a conditional helper's name. Registrations already appended to
/// `required_properties` remain when expansion fails.
pub(in crate::compiler) fn expand_authored_directives(
    css: &str,
    theme: &ThemeIndex,
    required_properties: &mut Vec<LeanStr>,
    identifiers: &IdentifierContext,
) -> Result<LeanString, CompileDiagnostic> {
    let mut output = expand_applications(
        css,
        theme,
        required_properties,
        identifiers,
        &mut Vec::new(),
    )?;
    // Each pass replaces the first remaining block, and an expansion adds no variant
    // block of its own, so the loop ends. Inner blocks are expanded after the outer
    // ones, inside the rules that the outer variants produce.
    while let Some((range, prelude, body)) = first_variant_block(&parse_structure(&output)) {
        let replacement =
            expand_authored_variant(&output[prelude], &output[body], theme, identifiers)?;
        output.replace_range(range, &replacement);
    }
    fold_nested_host_rules(&mut output);
    Ok(output)
}

/// Moves conditions on a shadow host into `:host()` in the rules nested under a host.
///
/// `@apply` and `@variant` write nested rules such as `&:hover`, and a browser cannot
/// match that rule under `:host`. The pass also covers nested rules in the authored
/// CSS, because they have the same form. A top-level rule stays as written.
fn fold_nested_host_rules(css: &mut LeanString) {
    if !css.contains(":host") {
        return;
    }
    let mut edits = Vec::new();
    collect_nested_host_edits(css, &parse_structure(css), false, &mut edits);
    edits.sort_by_key(|(range, _): &(Range<usize>, LeanString)| std::cmp::Reverse(range.start));
    for (range, selector) in edits {
        css.replace_range(range, &selector);
    }
}

/// Appends the selector replacements for `nodes`. `under_host` tells whether `&` in
/// `nodes` refers to a shadow host.
fn collect_nested_host_edits(
    css: &str,
    nodes: &[CssNode],
    under_host: bool,
    edits: &mut Vec<(Range<usize>, LeanString)>,
) {
    for node in nodes {
        let nested_under_host = match &node.kind {
            CssNodeKind::StyleRule { selector, .. } => {
                let written = &css[selector.clone()];
                let folded = if under_host {
                    fold_nested_host_conditions(written)
                } else {
                    written.into()
                };
                let selects_host =
                    selector_selects_host(&folded) || (under_host && folded.trim() == "&");
                if folded != written {
                    edits.push((selector.clone(), folded));
                }
                selects_host
            }
            _ => under_host,
        };
        collect_nested_host_edits(css, node.children(), nested_under_host, edits);
    }
}

/// Returns `css` with every `@apply` statement recursively replaced using `theme`.
/// Variant blocks remain for the enclosing authored-directive pass.
///
/// `stack` contains the candidates whose definitions are currently being expanded.
/// Its contents are restored on success and failure. Resolved applications append
/// registration names to `required_properties`, retaining existing entries and
/// duplicates. `identifiers` generates names required by conditional helpers.
///
/// # Errors
///
/// Returns a malformed utility requirement, a candidate repeated in `stack`, or a
/// failure to generate a conditional helper's name. Registrations already appended
/// to `required_properties` remain on failure.
fn expand_applications(
    css: &str,
    theme: &ThemeIndex,
    required_properties: &mut Vec<LeanStr>,
    identifiers: &IdentifierContext,
    stack: &mut Vec<LeanStr>,
) -> Result<LeanString, CompileDiagnostic> {
    let mut applications = Vec::new();
    collect_applications(css, &parse_structure(css), &mut applications);
    let mut output = LeanString::new();
    let mut cursor = 0;
    for (range, candidates) in applications {
        output.push_str(&css[cursor..range.start]);
        output.push_str(&expand_application(
            candidates.iter().map(|candidate| &css[candidate.clone()]),
            theme,
            required_properties,
            identifiers,
            stack,
        )?);
        cursor = range.end;
    }
    output.push_str(&css[cursor..]);
    Ok(output)
}

/// Returns candidate ranges for an apply statement, preserving the CSS source spelling.
pub(crate) fn apply_candidates(css: &str, node: &CssNode) -> Option<Vec<Range<usize>>> {
    let CssNodeKind::AtRule {
        name,
        prelude,
        block: None,
    } = &node.kind
    else {
        return None;
    };
    if name != "apply" {
        return None;
    }
    Some(
        whitespace_separated_ranges(&css[prelude.clone()])
            .into_iter()
            .map(|range| prelude.start + range.start..prelude.start + range.end)
            .collect(),
    )
}

/// Appends the range and candidate ranges of every `@apply` statement in `nodes`, in
/// authored order.
fn collect_applications(
    css: &str,
    nodes: &[CssNode],
    applications: &mut Vec<(Range<usize>, Vec<Range<usize>>)>,
) {
    for node in nodes {
        if let Some(candidates) = apply_candidates(css, node) {
            applications.push((node.range.clone(), candidates.clone()));
        }
        collect_applications(css, node.children(), applications);
    }
}

/// Returns the range, prelude, and contents of the first `@variant` block in authored
/// order. An enclosing block comes before the blocks it contains.
fn first_variant_block(nodes: &[CssNode]) -> Option<(Range<usize>, Range<usize>, Range<usize>)> {
    nodes.iter().find_map(|node| match &node.kind {
        CssNodeKind::AtRule {
            name,
            prelude,
            block: Some(block),
        } if name == "variant" => {
            Some((node.range.clone(), prelude.clone(), block.contents.clone()))
        }
        _ => first_variant_block(node.children()),
    })
}

/// Resolves the spellings in `candidates` through `theme` and returns the CSS for one
/// `@apply` statement. Unknown candidates contribute nothing. Every resolved target
/// retains its selector and conditions, and applications within targets expand
/// recursively. Candidates are ordered by variants and then by utility order.
///
/// `stack` contains the enclosing candidates and is restored on success and failure.
/// Required registration names are appended to `required_properties` with duplicates
/// retained. `identifiers` generates names required by conditional helpers.
///
/// # Errors
///
/// Returns a malformed requirement, a circular application, or the host generator's
/// failure for a conditional helper's name. Registrations already appended to
/// `required_properties` remain on failure.
fn expand_application<'a>(
    candidates: impl Iterator<Item = &'a str>,
    theme: &ThemeIndex,
    required_properties: &mut Vec<LeanStr>,
    identifiers: &IdentifierContext,
    stack: &mut Vec<LeanStr>,
) -> Result<LeanString, CompileDiagnostic> {
    let mut utilities = Vec::new();
    for candidate in candidates {
        if stack.iter().any(|parent| parent == candidate) {
            return Err(CompileDiagnostic::CircularUtilityApplication {
                utility: candidate.into(),
            });
        }
        match resolve_utility_targets(candidate, theme, VariantContext::Element) {
            Ok(Ok(targets)) => {
                utilities.extend(targets);
            }
            Ok(Err(_)) => {}
            Err(error) => {
                return Err(CompileDiagnostic::MalformedUtilityRequirement {
                    utility: error.name,
                    offset: error.source.offset,
                });
            }
        }
    }
    // Tailwind orders applied candidates by their variant stacks before their
    // properties, so candidates without variants come first.
    let mut ordered = order_utilities(utilities.iter().collect());
    ordered
        .sort_by(|left, right| compare_variant_sequences(&left.variants, &right.variants, theme));
    let mut declarations = LeanString::new();
    for utility in ordered {
        required_properties.extend(utility.required_properties.iter().cloned());
        let body = assemble_nested_declarations::<0>(
            &utility.declaration,
            &utility.nested.iter().collect::<Vec<_>>(),
        );
        stack.push(utility.candidate.clone());
        let expanded = expand_applications(&body, theme, required_properties, identifiers, stack);
        stack.pop();
        let body = expanded?;
        let body = if let Some(selector) = &utility.selector {
            format_args!("{selector}{{{body}}}").to_lean_string()
        } else {
            body
        };
        if utility.variants.is_empty() {
            declarations.push_str(&body);
            continue;
        }
        let applications = utility
            .variants
            .iter()
            .filter_map(|variant| resolve_variant(variant, theme).ok())
            .collect::<Vec<_>>();
        declarations.push_str(&render_authored_variant_path(
            &body,
            &applications,
            identifiers,
        )?);
    }
    Ok(declarations)
}

/// Expands one `@variant <prelude> { body }` block.
///
/// Each sequence of [`variant_sequences`] expands to its own copy of the body. A
/// sequence that [`parse_variant_sequence`] rejects or whose variants do not resolve
/// contributes nothing.
///
/// # Errors
///
/// Returns the host generator's failure for a conditional helper's name.
pub(crate) fn expand_authored_variant(
    prelude: &str,
    body: &str,
    theme: &ThemeIndex,
    identifiers: &IdentifierContext,
) -> Result<LeanString, CompileDiagnostic> {
    let mut output = LeanString::new();
    for sequence in variant_sequences(prelude) {
        let Some(applications) = parse_variant_sequence(&prelude[sequence]).and_then(|variants| {
            variants
                .iter()
                .map(|variant| resolve_variant(variant, theme).ok())
                .collect::<Option<Vec<_>>>()
        }) else {
            continue;
        };
        let mut paths = vec![Vec::new()];
        for application in applications {
            let alternatives = match application {
                VariantApplication::Compound(applications) => applications,
                application => vec![application],
            };
            paths = paths
                .into_iter()
                .flat_map(|path| {
                    alternatives.iter().cloned().map(move |application| {
                        let mut path = path.clone();
                        path.push(application);
                        path
                    })
                })
                .collect();
        }
        for applications in &paths {
            output.push_str(&render_authored_variant_path(
                body,
                applications,
                identifiers,
            )?);
        }
    }
    Ok(output)
}

/// Returns the range of each comma-separated sequence of a `@variant` prelude, without
/// surrounding whitespace.
///
/// Commas inside quotes, parentheses, brackets, or braces stay part of the sequence
/// that contains them, which allows a sequence such as `[&:is(a,b)]`.
pub(crate) fn variant_sequences(prelude: &str) -> Vec<Range<usize>> {
    split_top_level(prelude, |token| matches!(token, Token::Comma))
        .into_iter()
        .map(|part| {
            let start = part.as_ptr() as usize - prelude.as_ptr() as usize;
            let trimmed_start = start + part.len() - part.trim_start().len();
            trimmed_start..trimmed_start + part.trim().len()
        })
        .collect()
}

/// Parses one `@variant` sequence, such as `hover:focus`, into its variants.
///
/// Returns `None` when the text is empty or is not a complete variant list.
/// The returned variants have not been resolved against theme definitions.
pub(crate) fn parse_variant_sequence(sequence: &str) -> Option<Vec<Variant>> {
    // The grammar requires a utility suffix. Only the variants are used, so the suffix
    // needs no utility definition.
    let candidate = parse_candidate(&format_args!("{sequence}:utility").to_lean_string())?;
    (!candidate.variants.is_empty()
        && candidate
            .variants
            .iter()
            .all(|variant| !variant.raw.is_empty()))
    .then_some(candidate.variants)
}

/// Wraps `body` in every application of one variant path.
///
/// `applications` are in authored order, and the first one is the outermost, as in
/// Tailwind: `dark:hover` puts the `dark` condition around the `:hover` rule. The last
/// step wraps `body` first. Declarations a variant contributes are prepended only when
/// the body does not already set that property, which keeps an explicit authored value
/// authoritative. Runtime conditions guard declarations recursively. Callers must
/// finalize the retained rule boundaries after all directives are expanded and lower
/// `--if` expressions using the selected compiler strategy.
///
/// One path's conditional steps share one list of gate names, so an outer step leaves
/// the gate declarations of an inner step alone. Those declarations are already
/// part of the returned text, and no caller guards that text again, so the list ends
/// with the path.
pub(in crate::compiler) fn render_authored_variant_path(
    body: &str,
    applications: &[VariantApplication],
    identifiers: &IdentifierContext,
) -> Result<LeanString, CompileDiagnostic> {
    variant_paths(applications)
        .iter()
        .map(|path| {
            path.iter()
                .rev()
                .try_fold(
                    (body.to_lean_string(), Vec::new()),
                    |(body, mut gates), application| match *application {
                        VariantStep::SelectorSuffix(suffix) => {
                            Ok((format_args!("&{suffix}{{{body}}}").to_lean_string(), gates))
                        }
                        VariantStep::SelectorCondition(condition) => Ok((
                            format_args!("&{condition}{{{body}}}").to_lean_string(),
                            gates,
                        )),
                        VariantStep::SelectorTemplate(selector)
                        | VariantStep::BareSelector { selector, .. } => {
                            Ok((format_args!("{selector}{{{body}}}").to_lean_string(), gates))
                        }
                        VariantStep::SelectorRule {
                            selector,
                            declarations,
                        } => {
                            let mut body = body;
                            prepend_missing_declarations(&mut body, declarations);
                            Ok((format_args!("{selector}{{{body}}}").to_lean_string(), gates))
                        }
                        VariantStep::SelectorMedia {
                            selector,
                            condition,
                        } => Ok((
                            format_args!("{selector}{{@media {condition}{{{body}}}}}")
                                .to_lean_string(),
                            gates,
                        )),
                        VariantStep::SelectorMediaRule {
                            selector,
                            declarations,
                            condition,
                        } => {
                            let mut body = body;
                            prepend_missing_declarations(&mut body, declarations);
                            Ok((
                                format_args!("{selector}{{@media {condition}{{{body}}}}}")
                                    .to_lean_string(),
                                gates,
                            ))
                        }
                        VariantStep::Media(condition) => Ok((
                            format_args!("@media {condition}{{{body}}}").to_lean_string(),
                            gates,
                        )),
                        VariantStep::Container(condition) => Ok((
                            format_args!("@container {condition}{{{body}}}").to_lean_string(),
                            gates,
                        )),
                        VariantStep::Supports(condition) => Ok((
                            format_args!("@supports {condition}{{{body}}}").to_lean_string(),
                            gates,
                        )),
                        VariantStep::Conditional(condition) => {
                            let guarded = guard(&body, condition, identifiers, &gates)?;
                            gates.extend(guarded.gates);
                            let rule = format_args!("&{{{}}}", guarded.css).to_lean_string();
                            Ok((protect(&rule), gates))
                        }
                        VariantStep::StartingStyle => Ok((
                            format_args!("@starting-style{{{body}}}").to_lean_string(),
                            gates,
                        )),
                    },
                )
                // The gates are already written into the body; see the function documentation.
                .map(|(body, _)| body)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot, UtilityOrderMode};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::identifiers::{
        DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierContext, IdentifierMode,
    };
    use crate::test_support::read_theme;

    /// Expands with the compiler's own helper names.
    fn expand(
        css: &str,
        theme: &ThemeIndex,
        required: &mut Vec<LeanStr>,
    ) -> Result<LeanString, CompileDiagnostic> {
        expand_authored_directives(
            css,
            theme,
            required,
            &IdentifierContext::new(
                GeneratorSet::default(),
                IdentifierMode::Release,
                DEFAULT_HASH_LENGTH,
            ),
        )
    }

    #[test]
    fn circular_applications_report_the_repeated_candidate() {
        let source = css_source(
            FileId(1),
            "@utility a { @apply b; } @utility b { @apply a; }",
        )
        .unwrap();
        let theme = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        assert_eq!(
            expand(".x { @apply a; }", theme.theme(), &mut Vec::new()),
            Err(CompileDiagnostic::CircularUtilityApplication {
                utility: "a".into()
            })
        );
    }

    #[test]
    fn expands_authored_variants_through_the_shared_registry() {
        let source = css_source(
            FileId(1),
            r#"
                @import "stillwind/theme.css" layer(theme);
                @import "stillwind/utilities.css" layer(utilities); @import "stillwind/builtins.css";
                .button {
                  @variant hover { opacity: 0.8; }
                  @variant focus { outline-width: 2px; }
                  @variant print { color: black; }
                }
            "#,
        )
        .unwrap();
        let theme = read_theme(&source);
        let expanded = expand(theme.custom_css(), &theme, &mut Vec::new()).unwrap();
        let compact = expanded
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<LeanString>();

        assert!(
            compact.contains("&:hover{@media(hover:hover){opacity:0.8;}}"),
            "{compact}"
        );
        assert!(compact.contains("&:focus{outline-width:2px;}"), "{compact}");
        assert!(compact.contains("@mediaprint{color:black;}"), "{compact}");
    }

    #[test]
    fn expands_authored_apply_in_tailwind_property_order() {
        let source = css_source(
            FileId(1),
            r#"
                @import "stillwind/theme.css" layer(theme);
                @import "stillwind/utilities.css" layer(utilities); @import "stillwind/builtins.css";
                .button { @apply px-4 bg-red-500 inline-flex; }
            "#,
        )
        .unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("stillwind.css"),
            DesignSystemOptions {
                utility_order: UtilityOrderMode::Tailwind,
                ..Default::default()
            },
        )
        .unwrap();
        let theme = snapshot.theme();
        let expanded = expand(theme.custom_css(), theme, &mut Vec::new()).unwrap();
        let compact = expanded
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<LeanString>();

        assert!(compact.contains(
            "display:inline-flex;background-color:var(--color-red-500);padding-inline:calc(var(--spacing)*4);"
        ), "actual output: {compact}");
    }

    /// Expands `body` under the package registry and returns its expansion without the
    /// package CSS and without whitespace.
    fn compact_expansion(body: &str) -> LeanString {
        let expanded = |body: &str| {
            let source = css_source(
                FileId(1),
                format_args!("@import \"stillwind\"; {body}").to_lean_string(),
            )
            .unwrap();
            let theme = read_theme(&source);
            expand(theme.custom_css(), &theme, &mut Vec::new())
                .unwrap()
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<LeanString>()
        };
        let package = expanded("");
        expanded(body)
            .strip_prefix(package.as_str())
            .unwrap()
            .to_lean_string()
    }

    #[test]
    fn unresolved_variant_sequences_discard_their_copy_of_the_body() {
        assert_eq!(
            compact_expansion(".card { @variant nope { color: red; } }"),
            ".card{}"
        );
        assert_eq!(
            compact_expansion(".card { @variant nope, print { color: red; } }"),
            ".card{@mediaprint{color:red;}}"
        );
        assert_eq!(
            compact_expansion(".card { @variant print { @variant nope { color: red; } } }"),
            ".card{@mediaprint{}}"
        );
    }

    #[test]
    fn applications_end_at_their_statement() {
        assert_eq!(compact_expansion(".btn { @apply unknown-x; }"), ".btn{}");
        assert_eq!(
            compact_expansion(".btn { @apply flex } .next { color: red; }"),
            ".btn{display:flex;}.next{color:red;}"
        );
        assert_eq!(
            compact_expansion(".btn { content: '@apply flex;'; /* @apply grid; */ }"),
            ".btn{content:'@applyflex;';/*@applygrid;*/}"
        );
    }

    #[test]
    fn applications_keep_the_variants_of_their_candidates() {
        for (body, expected) in [
            (
                ".btn { @apply hover:flex; }",
                ".btn{&:hover{@media(hover:hover){display:flex;}}}",
            ),
            (
                ".btn { @apply sm:flex; }",
                ".btn{@media(width>=40rem){display:flex;}}",
            ),
            (
                ".btn { @apply dark:hover:flex; }",
                ".btn{@media(prefers-color-scheme:dark){&:hover{@media(hover:hover){display:flex;}}}}",
            ),
            (
                ".btn { @apply group-hover:flex; }",
                ".btn{&:is(:where(.group):hover*){@media(hover:hover){display:flex;}}}",
            ),
            (
                ".btn { @apply [&>svg]:flex; }",
                ".btn{&>svg{display:flex;}}",
            ),
            (
                ".btn { @apply [:hover]:flex; }",
                ".btn{&:is(:hover){display:flex;}}",
            ),
            (
                ".btn { @apply not-hover:flex; }",
                ".btn{&:not(:hover){display:flex;}@medianot(hover:hover){display:flex;}}",
            ),
            (
                ".btn { @apply before:content-['x']; }",
                ".btn{&::before{--tw-content:'x';content:var(--tw-content);}}",
            ),
        ] {
            assert_eq!(compact_expansion(body), expected, "{body}");
        }
    }

    #[test]
    fn applications_order_candidates_by_variants_before_properties() {
        assert_eq!(
            compact_expansion(".btn { @apply sm:hidden hover:p-2 p-4 hover:flex grid; }"),
            concat!(
                ".btn{display:grid;padding:calc(var(--spacing)*4);",
                "&:hover{@media(hover:hover){display:flex;}}",
                "&:hover{@media(hover:hover){padding:calc(var(--spacing)*2);}}",
                "@media(width>=40rem){display:none;}}",
            )
        );
    }

    #[test]
    fn applications_guard_declarations_under_ordinary_variants() {
        assert_eq!(
            compact_expansion(".btn { @apply hover:if(--a):flex; }"),
            ".btn{&:hover{@media(hover:hover){@-stillwind-rule-boundary{&{display:--if(--a,{flex},revert-rule);}}}}}"
        );
    }

    #[test]
    fn stacked_variant_blocks_nest_the_first_variant_outermost() {
        assert_eq!(
            compact_expansion(".card { @variant dark:hover { color: red; } }"),
            ".card{@media(prefers-color-scheme:dark){&:hover{@media(hover:hover){color:red;}}}}"
        );
    }

    #[test]
    fn variant_sequences_keep_quoted_and_nested_commas() {
        let prelude = " a ,[b,c],f(x,y),'z,w'";
        assert_eq!(
            variant_sequences(prelude)
                .into_iter()
                .map(|range| &prelude[range])
                .collect::<Vec<_>>(),
            ["a", "[b,c]", "f(x,y)", "'z,w'"]
        );
        assert_eq!(variant_sequences(""), vec![0..0]);
        assert_eq!(parse_variant_sequence("hover:"), None);
        assert_eq!(
            parse_variant_sequence("").map(|variants| variants.len()),
            None
        );
    }
}
