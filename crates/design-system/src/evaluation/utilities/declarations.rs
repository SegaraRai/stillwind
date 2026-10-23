//! Placeholder resolution for declarations and nested CSS blocks.
//!
//! Each declaration is a branch: declarations that match the candidate survive and the
//! others are dropped. Nested blocks resolve recursively and retain their rule headers.
//! Resolving a body as a whole instead would make a broad definition with one unmatched
//! typed value emit nothing at all, or would emit a static supporting declaration for a
//! candidate that never matched.
//!
//! Each declaration group containing placeholders must resolve at least one of them.
//! A group containing `--value(...)` or `--modifier(...)` must resolve the value and
//! modifier it depends on; a modifier-only sibling cannot make an unrelated branch
//! valid. Every declaration using the private shadow-color placeholder must resolve,
//! including declarations in descendant blocks.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{
    CssNodeKind, absolutize_nested_selector, inspect_css_value, parse_declaration_list,
    parse_structure,
};
use stillwind_syntax::Value;

use super::super::context::{ArbitraryValueCheck, EvaluationContext, ValueSelections};
use super::super::values::{PreparedValues, color_fallback, resolve_css_placeholders};
use super::formatting::resolve_declaration_value;
use super::resolve::{
    UtilityArbitraryUse, UtilityDeclaration, UtilityTokenUse, diagnostic_property,
};
use crate::ThemeIndex;

/// CSS and diagnostic records retained after evaluating one definition body.
#[derive(Clone, Debug, Default)]
pub(in crate::evaluation) struct ResolvedCssDeclarations {
    /// Declaration records for `base`, including its nested blocks, before candidate
    /// priority. Each record identifies whether its value is fixed implementation CSS.
    pub(in crate::evaluation) policy_declarations: Vec<UtilityDeclaration>,
    /// Source selections and checks retained before color compatibility fallbacks are
    /// added.
    pub(in crate::evaluation) diagnostic_data: DeclarationDiagnosticData,
    /// Declarations and nested blocks for the base branch, joined in authored order.
    pub(in crate::evaluation) base: LeanString,
    /// Modern-color variants of the base declarations that differ from the legacy text,
    /// to be emitted inside the capability query.
    pub(in crate::evaluation) compat_modern: Vec<ResolvedCssBranch>,
    /// Requirements satisfied by retained declarations, including descendant blocks.
    matches: DeclarationMatches,
}

/// Modern CSS and declaration records for one color compatibility branch.
#[derive(Clone, Debug)]
pub(in crate::evaluation) struct ResolvedCssBranch {
    /// Serialized CSS without the generated outer capability query. Authored nested
    /// blocks remain present.
    pub(in crate::evaluation) declaration: LeanStr,
    /// Declaration records belonging to this branch, before candidate priority.
    pub(in crate::evaluation) policy_declarations: Vec<UtilityDeclaration>,
}

/// Placeholder requirements matched by a declaration group and its nested blocks.
#[derive(Clone, Copy, Debug, Default)]
struct DeclarationMatches {
    /// At least one declaration with a recognized placeholder resolved.
    placeholder: bool,
    /// At least one declaration containing `--value()` resolved.
    value: bool,
    /// At least one declaration containing `--modifier()` resolved.
    modifier: bool,
    /// Number of resolved declarations containing `--stillwind-builtin-shadow-color()`.
    required: usize,
}

/// Direct source selections and alpha checks retained for one declaration group.
#[derive(Clone, Debug, Default)]
pub(in crate::evaluation) struct DeclarationDiagnosticData {
    /// Direct token names grouped by receiving declaration, in authored order.
    pub(in crate::evaluation) token_uses: Vec<UtilityTokenUse>,
    /// Accepted arbitrary captures before theme expansion and declaration formatting.
    pub(in crate::evaluation) arbitrary_uses: Vec<UtilityArbitraryUse>,
    /// The first declaration that mixes a color with an alpha percentage outside `0%`
    /// to `100%`, with that percentage as its value.
    pub(in crate::evaluation) out_of_range_alpha: Option<UtilityArbitraryUse>,
}

/// Parsed declarations, nested blocks, and branch requirements shared by snapshot
/// evaluation.
#[derive(Debug)]
pub(in crate::evaluation) struct PreparedDeclarations {
    /// Direct declarations and nested blocks in authored order.
    declarations: Vec<PreparedDeclaration>,
    /// This group or a descendant contains a recognized placeholder.
    uses_placeholder: bool,
    /// This group or a descendant contains `--value()`.
    uses_value: bool,
    /// This group or a descendant contains `--modifier()`.
    requires_modifier: bool,
    /// Number of declarations containing `--stillwind-builtin-shadow-color()` that
    /// must resolve across this group and its descendants.
    required_declarations: usize,
}

impl PreparedDeclarations {
    /// Prepares `body` for repeated evaluation, retaining declarations and nested
    /// blocks in authored order. `theme` resolves declarations without placeholders
    /// in advance for Stillwind and Tailwind-style definitions.
    pub(in crate::evaluation) fn parse(body: &str, theme: &ThemeIndex) -> Self {
        Self::parse_inner(body, theme, false)
    }

    /// Prepares one block and its descendants using `theme` for static values.
    ///
    /// `keyframes` identifies a keyframe definition's contents. Its direct style-rule
    /// headers retain their frame selectors; other style rules receive explicit
    /// nesting selectors. The returned group records requirements at every depth.
    fn parse_inner(body: &str, theme: &ThemeIndex, keyframes: bool) -> Self {
        let mut start = 0;
        let declarations: Vec<_> = parse_structure(body)
            .into_iter()
            .map(|node| {
                let declaration = LeanString::from(&body[start..node.range.end]);
                let nested = match &node.kind {
                    CssNodeKind::StyleRule { selector, block } => {
                        let prelude = if keyframes {
                            body[start..block.contents.start - 1].to_lean_str()
                        } else {
                            format_args!(
                                "{}{}",
                                &body[start..node.range.start],
                                absolutize_nested_selector(&body[selector.clone()])
                            )
                            .to_lean_str()
                        };
                        Some(PreparedNestedDeclarations {
                            prelude,
                            declarations: Self::parse_inner(
                                &body[block.contents.clone()],
                                theme,
                                false,
                            ),
                            supports_color_mix: false,
                        })
                    }
                    CssNodeKind::AtRule {
                        name,
                        prelude,
                        block: Some(block),
                    } => Some(PreparedNestedDeclarations {
                        prelude: body[start..block.contents.start - 1]
                            .trim_end()
                            .to_lean_str(),
                        declarations: Self::parse_inner(
                            &body[block.contents.clone()],
                            theme,
                            name.ends_with("keyframes"),
                        ),
                        supports_color_mix: name == "supports"
                            && body[prelude.clone()].contains("color-mix("),
                    }),
                    _ => None,
                };
                start = node.range.end;
                PreparedDeclaration::new(declaration, nested, theme)
            })
            .collect();
        let uses_placeholder = declarations.iter().any(|declaration| {
            let declaration = &declaration.text;
            declaration.contains("--value(")
                || declaration.contains("--value-map(")
                || declaration.contains("--modifier(")
                || declaration.contains("--modifier-map(")
                || declaration.contains("--modifier-none()")
                || declaration.contains("--alpha(")
                || declaration.contains("--compat-color(")
                || declaration.contains("--stillwind-builtin-shadow-color(")
        });
        let uses_value = declarations
            .iter()
            .any(|declaration| declaration.text.contains("--value("));
        let requires_modifier = declarations
            .iter()
            .any(|declaration| declaration.text.contains("--modifier("));
        let required_declarations = declarations
            .iter()
            .map(|declaration| {
                declaration.nested.as_ref().map_or_else(
                    || {
                        usize::from(
                            declaration
                                .text
                                .contains("--stillwind-builtin-shadow-color("),
                        )
                    },
                    |nested| nested.declarations.required_declarations,
                )
            })
            .sum();
        Self {
            declarations,
            uses_placeholder,
            uses_value,
            requires_modifier,
            required_declarations,
        }
    }
}

/// One declaration or nested block of a prepared group.
#[derive(Debug)]
struct PreparedDeclaration {
    /// Complete authored text of this declaration or block, including preceding trivia.
    text: LeanStr,
    /// Prepared contents and header when this item is a CSS block.
    nested: Option<PreparedNestedDeclarations>,
    /// Prepared placeholders for a declaration, or `None` for a nested block or text
    /// whose placeholders cannot be prepared.
    values: Option<PreparedValues>,
    /// Resolved text of a declaration without placeholders, for a Stillwind definition.
    static_stillwind: Option<LeanStr>,
    /// Resolved text of a declaration without placeholders, for a Tailwind-style
    /// definition.
    static_project: Option<LeanStr>,
}

impl PreparedDeclaration {
    /// Prepares `text`, whose block structure the caller supplies in `nested`.
    ///
    /// `nested` is `None` for a declaration. Such declarations cache their resolved
    /// text for both definition formats when their values are static. `theme` supplies
    /// the values used by those caches.
    fn new(
        text: LeanString,
        nested: Option<PreparedNestedDeclarations>,
        theme: &ThemeIndex,
    ) -> Self {
        let values = nested
            .is_none()
            .then(|| PreparedValues::parse(&text))
            .flatten();
        let is_static = values.as_ref().is_some_and(PreparedValues::is_static);
        Self {
            static_stillwind: (is_static.then(|| resolve_declaration_value(&text, theme, true)))
                .map(Into::into),
            static_project: (is_static.then(|| resolve_declaration_value(&text, theme, false)))
                .map(Into::into),
            values,
            text: (text).into(),
            nested,
        }
    }

    /// Returns cached declaration text in the format selected by `stillwind`.
    /// Nested blocks and declarations without a prepared static value return `None`.
    fn static_value(&self, stillwind: bool) -> Option<&LeanStr> {
        if stillwind {
            self.static_stillwind.as_ref()
        } else {
            self.static_project.as_ref()
        }
    }
}

/// A nested CSS block whose declarations resolve independently at each depth.
#[derive(Debug)]
struct PreparedNestedDeclarations {
    /// Rule header without its braces, with explicit nesting for style selectors.
    /// Keyframe selectors retain their authored spelling.
    prelude: LeanStr,
    /// Declarations and child blocks inside this rule, in authored order.
    declarations: PreparedDeclarations,
    /// The authored support query already checks `color-mix()`.
    supports_color_mix: bool,
}

/// Resolves each declaration independently. `body` contains a registered declaration
/// group or a group produced by expanding theme wildcards. `@utility` overloads are
/// often intentionally broad: a single body can expose both color and length forms. A
/// failed typed placeholder must remove only the incompatible declaration, never the
/// compatible siblings.
///
/// `utility_name` identifies the registered pattern, and `name` supplies the candidate
/// name matched against it. `modifier` supplies its optional slash value. `theme`
/// supplies theme values and prepared bodies. `stillwind` selects the definition's
/// formatting rules, and `color_compatibility` enables fallback branches for modern
/// colors. `arbitrary_check` selects how typed domains check values without a type
/// hint. `implementation` must reflect the supplying definition's header. It marks only
/// fixed declarations and their token uses. Declarations with executable placeholders
/// retain ordinary classifications.
///
/// Returns resolved CSS and diagnostic selections, or [`None`] when theme functions
/// cannot resolve or a required placeholder remains unresolved. An empty static body
/// can return an empty result. The inputs are not modified.
#[allow(clippy::too_many_arguments)] // One flag per evaluation policy of the caller.
pub(in crate::evaluation) fn resolve_css_declarations(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
    stillwind: bool,
    color_compatibility: bool,
    arbitrary_check: ArbitraryValueCheck,
    implementation: bool,
) -> Option<ResolvedCssDeclarations> {
    let lowered;
    let body = if body.contains("--alpha(")
        || theme.namespaces().scales().any(|namespace| {
            body.contains(format_args!("--{}(", namespace.name).to_lean_str().as_str())
        }) {
        lowered = theme.resolve_css_functions(body)?;
        lowered.as_str()
    } else {
        body
    };
    let expanded;
    let prepared = if let Some(prepared) = theme.programs().declarations(body) {
        prepared
    } else {
        // Theme wildcard expansion substitutes token names into nested declarations.
        // Those generated groups differ from the registered syntax.
        expanded = PreparedDeclarations::parse(body, theme);
        &expanded
    };
    resolve_prepared_declarations(
        prepared,
        utility_name,
        name,
        modifier,
        theme,
        stillwind,
        color_compatibility,
        arbitrary_check,
        implementation,
    )
}

/// Evaluates `prepared` for the registered `utility_name`, candidate `name`, and
/// optional slash `modifier`, preserving the order of retained declarations and blocks.
///
/// `prepared` must have been parsed with `theme`, which supplies placeholder values.
/// `stillwind` selects the definition's formatting rules, and `arbitrary_check`
/// selects how unhinted arbitrary values are checked.
/// `color_compatibility` adds fallback branches except inside authored `color-mix()`
/// support queries. `implementation` marks fixed declarations and their token uses;
/// declarations with executable placeholders remain ordinary. The result contains CSS,
/// declaration classifications, diagnostic selections, and matched requirements from
/// retained blocks. The inputs are unchanged.
///
/// Returns `None` when this group fails a required placeholder gate. A nested group
/// that fails its gates is omitted, and its requirements remain unsatisfied in its
/// parent. An empty static group can return an empty result.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn resolve_prepared_declarations(
    prepared: &PreparedDeclarations,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
    stillwind: bool,
    color_compatibility: bool,
    arbitrary_check: ArbitraryValueCheck,
    implementation: bool,
) -> Option<ResolvedCssDeclarations> {
    let PreparedDeclarations {
        declarations,
        uses_placeholder,
        uses_value,
        requires_modifier,
        required_declarations,
    } = prepared;
    let mut resolved_placeholder = false;
    let mut resolved_value = false;
    let mut resolved_modifier = false;
    let mut resolved_required_declarations = 0;
    let mut output = ResolvedCssDeclarations::default();

    for authored in declarations {
        if let Some(nested) = &authored.nested {
            let Some(mut resolved) = resolve_prepared_declarations(
                &nested.declarations,
                utility_name,
                name,
                modifier,
                theme,
                stillwind,
                color_compatibility && !nested.supports_color_mix,
                arbitrary_check,
                implementation,
            ) else {
                continue;
            };
            resolved_placeholder |= resolved.matches.placeholder;
            resolved_value |= resolved.matches.value;
            resolved_modifier |= resolved.matches.modifier;
            resolved_required_declarations += resolved.matches.required;
            output
                .policy_declarations
                .extend(
                    resolved
                        .policy_declarations
                        .into_iter()
                        .map(|mut declaration| {
                            declaration.base = false;
                            declaration
                        }),
                );
            for usage in &mut resolved.diagnostic_data.token_uses {
                usage.base = false;
            }
            for usage in &mut resolved.diagnostic_data.arbitrary_uses {
                usage.base = false;
            }
            if let Some(usage) = &mut resolved.diagnostic_data.out_of_range_alpha {
                usage.base = false;
            }
            output
                .diagnostic_data
                .token_uses
                .extend(resolved.diagnostic_data.token_uses);
            output
                .diagnostic_data
                .arbitrary_uses
                .extend(resolved.diagnostic_data.arbitrary_uses);
            output.diagnostic_data.out_of_range_alpha = output
                .diagnostic_data
                .out_of_range_alpha
                .or(resolved.diagnostic_data.out_of_range_alpha);
            if !resolved.base.is_empty() {
                output.base.push_str(
                    &format_args!("{}{{{}}}", nested.prelude, resolved.base).to_lean_string(),
                );
            }
            output
                .compat_modern
                .extend(resolved.compat_modern.into_iter().map(|mut modern| {
                    modern.declaration =
                        format_args!("{}{{{}}}", nested.prelude, modern.declaration).to_lean_str();
                    for declaration in &mut modern.policy_declarations {
                        declaration.base = false;
                    }
                    modern
                }));
            continue;
        }
        let tracked = EvaluationContext::tracked(theme, arbitrary_check);
        let theme = &tracked;
        let authored_declaration = &authored.text;
        let contains_placeholder = authored_declaration.contains("--value(")
            || authored_declaration.contains("--value-map(")
            || authored_declaration.contains("--modifier(")
            || authored_declaration.contains("--modifier-map(")
            || authored_declaration.contains("--modifier-none()")
            || authored_declaration.contains("--alpha(")
            || authored_declaration.contains("--compat-color(")
            || authored_declaration.contains("--stillwind-builtin-shadow-color(");
        let contains_modifier = authored_declaration.contains("--modifier(");
        let contains_required = authored_declaration.contains("--stillwind-builtin-shadow-color(");
        let contains_value = authored_declaration.contains("--value(");
        let static_value = authored.static_value(stillwind);
        let resolved = if let Some(value) = static_value {
            Some(value.clone())
        } else if let Some(values) = &authored.values {
            values
                .resolve(utility_name, name, modifier, theme)
                .map(Into::into)
        } else {
            resolve_css_placeholders(authored_declaration, utility_name, name, modifier, theme)
                .map(Into::into)
        };
        if let Some(declaration) = resolved
            && !declaration.trim().is_empty()
        {
            let declaration = if static_value.is_some() {
                declaration
            } else {
                (resolve_declaration_value(&declaration, theme, stillwind)).into()
            };
            resolved_placeholder |= contains_placeholder;
            record_tokens(
                &mut output.diagnostic_data,
                theme.take_selections(),
                authored_declaration,
                &declaration,
                implementation && static_value.is_some(),
            );
            resolved_value |= contains_value;
            resolved_modifier |= contains_modifier;
            resolved_required_declarations += usize::from(contains_required);
            let declaration = if color_compatibility
                && let Some(fallback) = color_fallback(&declaration, theme)
            {
                output.compat_modern.push(ResolvedCssBranch {
                    policy_declarations: policy_declarations(
                        &declaration,
                        implementation && static_value.is_some(),
                    ),
                    declaration: declaration.trim_start().to_lean_str(),
                });
                fallback.into()
            } else {
                declaration
            };
            output.policy_declarations.extend(policy_declarations(
                &declaration,
                implementation && static_value.is_some(),
            ));
            if output.base.is_empty() {
                output.base.push_str(declaration.trim_start());
            } else {
                output.base.push_str(&declaration);
            }
        }
    }

    output.matches = DeclarationMatches {
        placeholder: resolved_placeholder,
        value: resolved_value,
        modifier: resolved_modifier,
        required: resolved_required_declarations,
    };

    // Static declarations in a functional body are supporting declarations, not an
    // independent branch. For example the generic `translate-*` body must not emit
    // `translate: …` when none of its typed values match.
    (!*uses_placeholder || resolved_placeholder)
        .then_some(())
        // A modifier-only sibling (such as `text-*/leading`) cannot make an unresolved
        // value branch valid. This is what prevents a color token from being
        // misinterpreted as a spacing value merely because both spellings use a slash.
        .filter(|()| !*uses_value || resolved_value)
        .filter(|()| !*requires_modifier || resolved_modifier)
        .filter(|()| resolved_required_declarations == *required_declarations)
        .map(|()| output)
}

/// Parses the declaration list `css` into records with `base: true`.
///
/// The caller must classify the list before passing `implementation`; the flag is
/// applied to every parsed declaration. Property names are decoded, ordinary property
/// names are lowercased, and values retain authored priority. Invalid statements are
/// omitted. Invalid CSS syntax can reject the entire list.
fn policy_declarations(css: &str, implementation: bool) -> Vec<UtilityDeclaration> {
    parse_declaration_list(css)
        .into_iter()
        .flatten()
        .map(|declaration| UtilityDeclaration {
            property: diagnostic_property(&declaration.name),
            value: declaration.value,
            implementation,
            base: true,
        })
        .collect()
}

/// Adds the selections of one resolved declaration to `diagnostic_data`.
///
/// `authored` contains the declaration before placeholder substitution, and
/// `declaration` contains its resolved CSS before color compatibility fallbacks are
/// added. `selections` must come from evaluating that declaration. `implementation`
/// classifies its token uses and must be false for executable placeholders.
///
/// Appends accepted captures and direct token names to `diagnostic_data`. An
/// out-of-range alpha value is stored only if none was recorded. Token references
/// introduced by expansion are used only when no selected or authored token names
/// remain.
fn record_tokens(
    diagnostic_data: &mut DeclarationDiagnosticData,
    selections: ValueSelections,
    authored: &str,
    declaration: &str,
    implementation: bool,
) {
    if let Some((property, _)) = declaration.split_once(':') {
        if diagnostic_data.out_of_range_alpha.is_none()
            && let Some(value) = selections.out_of_range_alpha
        {
            diagnostic_data.out_of_range_alpha = Some(UtilityArbitraryUse {
                base: true,
                property: diagnostic_property(property),
                value,
                color: true,
            });
        }
        diagnostic_data
            .arbitrary_uses
            .extend(
                selections
                    .values
                    .into_iter()
                    .map(|selection| UtilityArbitraryUse {
                        base: true,
                        property: diagnostic_property(property),
                        value: selection.value,
                        color: selection.color,
                    }),
            );
    }
    let mut tokens = selections.tokens;
    tokens.extend(inspect_css_value(authored).1);
    if tokens.is_empty() {
        tokens.extend(inspect_css_value(declaration).1);
    }
    if let Some((property, _)) = declaration.split_once(':')
        && !tokens.is_empty()
    {
        diagnostic_data.token_uses.push(UtilityTokenUse {
            implementation,
            base: true,
            property: diagnostic_property(property),
            tokens: tokens.into_iter().collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::{CssSourceName, FileId, SourceFile};

    use super::*;
    use crate::parse_theme;

    #[test]
    fn unmatched_dynamic_branch_cannot_emit_static_supporting_declarations() {
        let source = SourceFile::new(FileId(1), None, None,
            "@utility w-* { display:block;width:--value([length]); } @utility x-* { width:--value([length]);color:--value([color]); }").unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
        assert!(
            resolve_css_declarations(
                "display:block;width:--value([length]);",
                "w-*",
                "w-[red]",
                None,
                &theme,
                false,
                true,
                ArbitraryValueCheck::Grammar,
                false,
            )
            .is_none()
        );
        let resolved = resolve_css_declarations(
            "width:--value([length]);color:--value([color]);",
            "x-*",
            "x-[red]",
            None,
            &theme,
            false,
            true,
            ArbitraryValueCheck::Grammar,
            false,
        )
        .unwrap();
        assert_eq!(resolved.base, "color:red;");
        assert!(resolved.compat_modern.is_empty());
    }
}
