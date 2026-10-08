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

use stillwind_css::inspect_css_value;
use stillwind_syntax::Value;

use super::super::values::colors::CompatColorMode;
use super::super::values::placeholders::{PreparedValues, resolve_css_placeholders};
use super::formatting::resolve_declaration_value;
use super::resolve::{UtilityArbitraryUse, UtilityTokenUse};
use crate::ThemeIndex;

/// The declarations one candidate produced from one definition body.
#[derive(Clone, Debug, Default)]
pub(crate) struct ResolvedCssDeclarations {
    pub(crate) token_uses: Vec<UtilityTokenUse>,
    pub(crate) arbitrary_uses: Vec<UtilityArbitraryUse>,
    /// Declarations for the base branch, joined in authored order.
    pub(crate) base: String,
    /// Modern-color variants of the base declarations that differ from the legacy text,
    /// to be emitted inside the capability query.
    pub(crate) compat_modern: Vec<String>,
}

/// Parsed declaration boundaries and branch requirements shared by snapshot evaluation.
#[derive(Debug)]
pub(crate) struct PreparedDeclarations {
    declarations: Vec<PreparedDeclaration>,
    uses_placeholder: bool,
    uses_value: bool,
    requires_modifier: bool,
    required_declarations: usize,
}

impl PreparedDeclarations {
    pub(crate) fn parse(body: &str, theme: &ThemeIndex) -> Self {
        let declarations: Vec<_> = split_css_declarations(body)
            .into_iter()
            .map(str::to_owned)
            .collect();
        let uses_placeholder = declarations.iter().any(|declaration| {
            declaration.contains("--value(")
                || declaration.contains("--value-literal(")
                || declaration.contains("--value-map(")
                || declaration.contains("--modifier(")
                || declaration.contains("--modifier-map(")
                || declaration.contains("--modifier-none()")
                || declaration.contains("--alpha(")
                || declaration.contains("--compat-color(")
                || declaration.contains("--stillwind-builtin-shadow-color(")
        });
        let uses_value = declarations.iter().any(|declaration| {
            declaration.contains("--value(") || declaration.contains("--value-literal(")
        });
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

#[derive(Debug)]
struct PreparedDeclaration {
    text: String,
    values: Option<PreparedValues>,
    static_builtin: Option<String>,
    static_project: Option<String>,
}

impl PreparedDeclaration {
    fn new(text: String, theme: &ThemeIndex) -> Self {
        let values = PreparedValues::parse(&text);
        let is_static = values.as_ref().is_some_and(PreparedValues::is_static);
        Self {
            static_builtin: is_static.then(|| resolve_declaration_value(&text, theme, true)),
            static_project: is_static.then(|| resolve_declaration_value(&text, theme, false)),
            values,
            text,
        }
    }
}

/// Resolves each declaration independently. `body` contains a registered declaration
/// group or a group produced by expanding theme wildcards. `@utility` overloads are often
/// intentionally broad: a single body can expose both color and length forms. A failed
/// typed placeholder must remove only the incompatible declaration, never the
/// compatible siblings.
///
/// Returns [`None`] when no declaration resolved or when a dependency of the body stayed
/// unresolved, so the caller can try another overload.
pub(crate) fn resolve_css_declarations(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
    builtin: bool,
) -> Option<ResolvedCssDeclarations> {
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
        let tracked = theme.tracking_tokens();
        let theme = &tracked;
        let authored_declaration = &authored.text;
        let contains_placeholder = authored_declaration.contains("--value(")
            || authored_declaration.contains("--value-literal(")
            || authored_declaration.contains("--value-map(")
            || authored_declaration.contains("--modifier(")
            || authored_declaration.contains("--modifier-map(")
            || authored_declaration.contains("--modifier-none()")
            || authored_declaration.contains("--alpha(")
            || authored_declaration.contains("--compat-color(")
            || authored_declaration.contains("--stillwind-builtin-shadow-color(");
        let contains_modifier = authored_declaration.contains("--modifier(");
        let contains_required = authored_declaration.contains("--stillwind-builtin-shadow-color(");
        let contains_value = authored_declaration.contains("--value(")
            || authored_declaration.contains("--value-literal(");
        let static_value = if builtin {
            &authored.static_builtin
        } else {
            &authored.static_project
        };
        let resolved = if let Some(value) = static_value {
            Some(value.clone())
        } else if let Some(values) = &authored.values {
            values.resolve(utility_name, name, modifier, theme)
        } else {
            resolve_css_placeholders(
                authored_declaration,
                utility_name,
                name,
                modifier,
                theme,
                CompatColorMode::Legacy,
            )
        };
        if let Some(declaration) = resolved
            && !declaration.trim().is_empty()
        {
            let declaration = if static_value.is_some() {
                declaration
            } else {
                resolve_declaration_value(&declaration, theme, builtin)
            };
            resolved_placeholder |= contains_placeholder;
            record_tokens(&mut output, theme, authored_declaration, &declaration);
            resolved_value |= contains_value;
            resolved_modifier |= contains_modifier;
            resolved_required_declarations += usize::from(contains_required);
            if output.base.is_empty() {
                output.base.push_str(declaration.trim_start());
            } else {
                output.base.push_str(&declaration);
            }
            if authored_declaration.contains("--compat-color(")
                && let Some(modern) = resolve_css_placeholders(
                    authored_declaration,
                    utility_name,
                    name,
                    modifier,
                    theme,
                    CompatColorMode::Modern,
                )
                .map(|modern| resolve_declaration_value(&modern, theme, builtin))
                && modern != declaration
            {
                output.compat_modern.push(modern.trim_start().to_owned());
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

fn record_tokens(
    output: &mut ResolvedCssDeclarations,
    theme: &ThemeIndex,
    authored: &str,
    declaration: &str,
) {
    if let Some((property, _)) = declaration.split_once(':') {
        output
            .arbitrary_uses
            .extend(
                theme
                    .selected_values()
                    .into_iter()
                    .map(|selection| UtilityArbitraryUse {
                        base: true,
                        property: property.trim().to_ascii_lowercase(),
                        value: selection.value,
                        color: selection.color,
                    }),
            );
    }
    let mut tokens = theme.selected_tokens();
    tokens.extend(inspect_css_value(authored).1);
    if tokens.is_empty() {
        tokens.extend(inspect_css_value(declaration).1);
    }
    if let Some((property, _)) = declaration.split_once(':')
        && !tokens.is_empty()
    {
        output.token_uses.push(UtilityTokenUse {
            base: true,
            property: property.trim().to_ascii_lowercase(),
            tokens: tokens.into_iter().collect(),
        });
    }
}

/// Splits a declaration list without mistaking a semicolon in a string or a function
/// argument for a CSS declaration boundary.
pub(crate) fn split_css_declarations(body: &str) -> Vec<&str> {
    let mut declarations = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
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
            ';' if paren_depth == 0 && bracket_depth == 0 => {
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
                false
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
        )
        .unwrap();
        assert_eq!(resolved.base, "color:red;");
        assert_eq!(resolved.compat_modern, [] as [String; 0]);
    }
}
