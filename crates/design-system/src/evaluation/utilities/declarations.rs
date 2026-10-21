//! Per-declaration placeholder resolution.
//!
//! A utility body is a list of declarations, and each one is a branch: the declarations
//! that match the candidate survive and the others are dropped. Resolving the body as a
//! whole instead would make a broad definition with one unmatched typed value emit
//! nothing at all, or would emit a static supporting declaration for a candidate that
//! never matched.
//!
//! Two gates guard a body. A declaration list containing placeholders must resolve at
//! least one of them, and a list containing `--value(...)` or `--modifier(...)` must
//! resolve the value and modifier it depends on; a modifier-only sibling cannot make an
//! unrelated branch valid.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::inspect_css_value;
use stillwind_syntax::Value;

use super::super::context::{ArbitraryValueCheck, EvaluationContext, ValueSelections};
use super::super::values::{PreparedValues, color_fallback, resolve_css_placeholders};
use super::formatting::resolve_declaration_value;
use super::resolve::{UtilityArbitraryUse, UtilityTokenUse};
use crate::ThemeIndex;

/// The declarations one candidate produced from one definition body.
#[derive(Clone, Debug, Default)]
pub(in crate::evaluation) struct ResolvedCssDeclarations {
    /// Source selections and checks retained before color compatibility fallbacks are added.
    pub(in crate::evaluation) diagnostic_data: DeclarationDiagnosticData,
    /// Declarations for the base branch, joined in authored order.
    pub(in crate::evaluation) base: LeanString,
    /// Modern-color variants of the base declarations that differ from the legacy text,
    /// to be emitted inside the capability query.
    pub(in crate::evaluation) compat_modern: Vec<LeanStr>,
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

/// Parsed declaration boundaries and branch requirements shared by snapshot evaluation.
#[derive(Debug)]
pub(in crate::evaluation) struct PreparedDeclarations {
    declarations: Vec<PreparedDeclaration>,
    uses_placeholder: bool,
    uses_value: bool,
    requires_modifier: bool,
    required_declarations: usize,
}

impl PreparedDeclarations {
    /// Splits a declaration group into declarations and records which placeholder
    /// families they use. A declaration without placeholders also gets its value
    /// resolved for Stillwind and for Tailwind-style definitions.
    pub(in crate::evaluation) fn parse(body: &str, theme: &ThemeIndex) -> Self {
        let declarations: Vec<_> = split_css_declarations(body)
            .into_iter()
            .map(LeanString::from)
            .collect();
        let uses_placeholder = declarations.iter().any(|declaration| {
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
            .any(|declaration| declaration.contains("--value("));
        let requires_modifier = declarations
            .iter()
            .any(|declaration| declaration.contains("--modifier("));
        let required_declarations = declarations
            .iter()
            .filter(|declaration| declaration.contains("--stillwind-builtin-shadow-color("))
            .count();
        Self {
            declarations: declarations
                .into_iter()
                .map(|text| PreparedDeclaration::new(text, theme))
                .collect(),
            uses_placeholder,
            uses_value,
            requires_modifier,
            required_declarations,
        }
    }
}

/// One declaration of a prepared group.
#[derive(Debug)]
struct PreparedDeclaration {
    /// Authored declaration text.
    text: LeanStr,
    /// Placeholders of the value, or `None` when they cannot be prepared.
    values: Option<PreparedValues>,
    /// Resolved text of a declaration without placeholders, for a Stillwind definition.
    static_stillwind: Option<LeanStr>,
    /// Resolved text of a declaration without placeholders, for a Tailwind-style
    /// definition.
    static_project: Option<LeanStr>,
}

impl PreparedDeclaration {
    /// Prepares one declaration and resolves it in advance when it has no placeholders.
    fn new(text: LeanString, theme: &ThemeIndex) -> Self {
        let values = PreparedValues::parse(&text);
        let is_static = values.as_ref().is_some_and(PreparedValues::is_static);
        Self {
            static_stillwind: (is_static.then(|| resolve_declaration_value(&text, theme, true)))
                .map(Into::into),
            static_project: (is_static.then(|| resolve_declaration_value(&text, theme, false)))
                .map(Into::into),
            values,
            text: (text).into(),
        }
    }

    /// Returns the declaration resolved in advance for a Stillwind or a Tailwind-style
    /// definition, or `None` when it has placeholders.
    fn static_value(&self, stillwind: bool) -> Option<&LeanStr> {
        if stillwind {
            self.static_stillwind.as_ref()
        } else {
            self.static_project.as_ref()
        }
    }
}

/// Resolves each declaration independently. `body` contains a registered declaration
/// group or a group produced by expanding theme wildcards. `@utility` overloads are
/// often intentionally broad: a single body can expose both color and length forms. A
/// failed typed placeholder must remove only the incompatible declaration, never the
/// compatible siblings.
///
/// `arbitrary_check` selects how typed domains check arbitrary values without a type
/// hint. Returns [`None`] when no declaration resolved or when a dependency of the body
/// stayed unresolved, so the caller can try another overload.
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
            );
            resolved_value |= contains_value;
            resolved_modifier |= contains_modifier;
            resolved_required_declarations += usize::from(contains_required);
            let declaration = if color_compatibility
                && let Some(fallback) = color_fallback(&declaration, theme)
            {
                output
                    .compat_modern
                    .push(declaration.trim_start().to_lean_str());
                fallback.into()
            } else {
                declaration
            };
            if output.base.is_empty() {
                output.base.push_str(declaration.trim_start());
            } else {
                output.base.push_str(&declaration);
            }
        }
    }

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

/// Adds the selections of one resolved declaration to `diagnostic_data`.
///
/// `authored` contains the declaration before placeholder substitution, and
/// `declaration` contains its resolved CSS before color compatibility fallbacks are
/// added. Direct token names take precedence over references introduced by expansion.
fn record_tokens(
    diagnostic_data: &mut DeclarationDiagnosticData,
    selections: ValueSelections,
    authored: &str,
    declaration: &str,
) {
    if let Some((property, _)) = declaration.split_once(':') {
        if diagnostic_data.out_of_range_alpha.is_none()
            && let Some(value) = selections.out_of_range_alpha
        {
            diagnostic_data.out_of_range_alpha = Some(UtilityArbitraryUse {
                base: true,
                property: (property.trim().to_lean_string().to_ascii_lowercase()).into(),
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
                        property: (property.trim().to_lean_string().to_ascii_lowercase()).into(),
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
            base: true,
            property: (property.trim().to_lean_string().to_ascii_lowercase()).into(),
            tokens: tokens.into_iter().collect(),
        });
    }
}

/// Splits a declaration list without mistaking a semicolon in a string, a function
/// argument, or a block for a CSS declaration boundary.
///
/// An at-rule block such as `@layer x { color: red; }` is one item that ends at its
/// closing brace. A custom property value that contains braces still ends at the next
/// top-level semicolon.
fn split_css_declarations(body: &str) -> Vec<&str> {
    let mut declarations = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut brace_depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in body.char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                if brace_depth == 0
                    && paren_depth == 0
                    && bracket_depth == 0
                    && body[start..index].trim_start().starts_with('@')
                {
                    declarations.push(&body[start..=index]);
                    start = index + 1;
                }
            }
            ';' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                declarations.push(&body[start..=index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if !body[start..].trim().is_empty() {
        declarations.push(&body[start..]);
    }
    declarations
}

#[cfg(test)]
mod tests {
    use stillwind_source::{CssSourceName, FileId, SourceFile};

    use super::*;
    use crate::parse_theme;

    #[test]
    fn declarations_split_outside_strings_and_functions_only() {
        assert_eq!(
            split_css_declarations("content:'a;b';x:f(a;b);last:1"),
            vec!["content:'a;b';", "x:f(a;b);", "last:1"]
        );
        assert_eq!(split_css_declarations(" "), Vec::<&str>::new());
        assert_eq!(
            split_css_declarations("@layer x{color:red;gap:0;}top:0;--x:{a;b};last:1"),
            vec![
                "@layer x{color:red;gap:0;}",
                "top:0;",
                "--x:{a;b};",
                "last:1"
            ]
        );
    }

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
        )
        .unwrap();
        assert_eq!(resolved.base, "color:red;");
        assert_eq!(resolved.compat_modern, [] as [LeanString; 0]);
    }
}
