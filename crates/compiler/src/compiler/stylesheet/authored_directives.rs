//! Expansion of `@apply` and `@variant` in authored CSS.
//!
//! [`expand_authored_directives`] replaces utility applications with declarations and
//! variant blocks with selector rules, using the design system's resolution and
//! declaration-ordering rules. It appends registration requirements to a separate
//! accumulator; it does not select registrations or place output slots.
//!
//! Expansion uses textual directive discovery. Unresolved application candidates
//! contribute nothing. An unterminated application or an unresolvable variant ends
//! its expansion loop, leaving that directive and later occurrences unchanged.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{ThemeIndex, VariantApplication, resolve_variant};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_syntax::parse_candidate;

use super::super::conditionals::guard;
use super::super::rule_boundaries::protect;
use super::super::utilities::resolve_utility;
use super::super::utilities::{order_utilities, prepend_missing_declarations};
use super::super::variants::{VariantStep, variant_paths};
use super::blocks::find_at_rule_block;
use crate::identifiers::IdentifierContext;

/// Rewrites every `@apply` and `@variant` in `css` that can be expanded.
///
/// Returns a new string; `css` and `theme` are not modified. Resolved applications
/// append registration names to `required_properties` in utility emission order,
/// retaining existing entries and duplicates. Applications are expanded before
/// variants. Within an application, unconditional declarations precede runtime-guarded
/// rules; builtin ordering is retained within each group. Runtime guards remain
/// unresolved `--if` expressions for the later options-aware lowering pass. A directive
/// that cannot be expanded remains in the returned text, so this function is not a CSS
/// validator.
pub(in crate::compiler) fn expand_authored_directives(
    css: &str,
    theme: &ThemeIndex,
    required_properties: &mut Vec<LeanStr>,
    identifiers: &IdentifierContext,
) -> Result<LeanString, CompileDiagnostic> {
    let mut output = css.to_lean_string();
    while let Some(start) = output.find("@apply ") {
        let Some(end_rel) = output[start..].find(';') else {
            break;
        };
        let end = start + end_rel;
        let candidates = output[start + 7..end].split_whitespace();
        let mut utilities = Vec::new();
        for candidate in candidates {
            match resolve_utility(candidate, theme) {
                Ok(Ok(utility)) => utilities.push(utility),
                Ok(Err(_)) => {}
                Err(error) => {
                    return Err(CompileDiagnostic::MalformedUtilityRequirement {
                        utility: error.name,
                        offset: error.source.offset,
                    });
                }
            }
        }
        let mut ordered = order_utilities(utilities.iter().collect());
        ordered.sort_by_key(|utility| {
            utility.variants.iter().any(|variant| {
                matches!(
                    resolve_variant(variant, theme).ok(),
                    Some(VariantApplication::Conditional(_))
                )
            })
        });
        let mut declarations = LeanString::new();
        for utility in ordered {
            required_properties.extend(utility.required_properties.iter().cloned());
            if utility.variants.iter().any(|variant| {
                matches!(
                    resolve_variant(variant, theme).ok(),
                    Some(VariantApplication::Conditional(_))
                )
            }) {
                let applications = utility
                    .variants
                    .iter()
                    .filter_map(|variant| resolve_variant(variant, theme).ok())
                    .collect::<Vec<_>>();
                declarations.push_str(&render_authored_variant_path(
                    &utility.declaration,
                    &applications,
                    identifiers,
                )?);
            } else {
                declarations.push_str(&utility.declaration);
            }
        }
        output.replace_range(start..=end, &declarations);
    }
    while let Some((start, prelude, open, close)) = find_at_rule_block(&output, "variant") {
        let replacement =
            expand_authored_variant(&prelude, &output[open + 1..close], theme, identifiers)?;
        let Some(replacement) = replacement else {
            break;
        };
        output.replace_range(start..=close, &replacement);
    }
    Ok(output)
}

/// Expands one `@variant <prelude> { body }` block.
///
/// The prelude is a comma-separated list of variant sequences, and each sequence
/// expands to its own copy of the body. Returns `None` when a sequence is empty or an
/// unknown variant appears, so the caller can leave the block unexpanded instead of
/// emitting partial rules.
///
/// # Errors
///
/// Returns the host generator's failure for a conditional helper's name.
fn expand_authored_variant(
    prelude: &str,
    body: &str,
    theme: &ThemeIndex,
    identifiers: &IdentifierContext,
) -> Result<Option<LeanString>, CompileDiagnostic> {
    let mut output = LeanString::new();
    for variant_sequence in split_css_top_level(prelude, ',') {
        let Some(candidate) =
            parse_candidate(&format_args!("{}:flex", variant_sequence.trim()).to_lean_string())
        else {
            return Ok(None);
        };
        if candidate.variants.is_empty() {
            return Ok(None);
        }
        let mut paths = vec![Vec::new()];
        for variant in &candidate.variants {
            let Ok(application) = resolve_variant(variant, theme) else {
                return Ok(None);
            };
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
    Ok(Some(output))
}

/// Splits `value` on `separator` at the top level only.
///
/// Separators inside quotes, parentheses, brackets, or braces stay part of the item
/// they belong to, which is what allows a variant prelude to contain `:is(a, b)`.
fn split_css_top_level(value: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' | '[' | '{' => stack.push(character),
            ')' | ']' | '}' => {
                stack.pop();
            }
            _ if character == separator && stack.is_empty() => {
                parts.push(&value[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

/// Wraps `body` in every application of one variant path.
///
/// The innermost step is applied first, so the outermost at-rule of the path ends up
/// containing the whole rule. Declarations a variant contributes are prepended only
/// when the body does not already set that property, which keeps an explicit authored
/// value authoritative. Runtime conditions guard declarations recursively. Callers must
/// finalize the retained rule boundaries after all directives are expanded and lower
/// `--if` expressions using the selected compiler strategy.
///
/// One path's conditional steps share one list of gate names, so a later step leaves
/// the gate declarations an earlier one wrote alone. Those declarations are already
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
                        VariantStep::SelectorTemplate(selector) => {
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
    use crate::Compiler;
    use crate::identifiers::{
        DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierContext, IdentifierMode,
    };

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
        let theme = Compiler::read_theme(&source, &CssSourceName::from("stillwind.css")).unwrap();
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
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn top_level_list_splitting_keeps_quoted_and_nested_separators() {
        assert_eq!(
            split_css_top_level("a,[b,c],f(x,y),'z,w'", ','),
            vec!["a", "[b,c]", "f(x,y)", "'z,w'"]
        );
        assert_eq!(split_css_top_level("", ','), vec![""]);
        assert_eq!(split_css_top_level("a,", ','), vec!["a", ""]);
    }
}
