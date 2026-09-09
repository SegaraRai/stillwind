use stillwind_css::ThemeIndex;
use stillwind_grammar::Value;

use crate::values::colors::CompatColorMode;
use crate::values::placeholders::resolve_css_placeholders;

#[derive(Clone, Debug, Default)]
pub(crate) struct ResolvedCssDeclarations {
    pub(crate) base: String,
    pub(crate) compat_modern: Vec<String>,
}

/// Resolves each declaration independently. `@utility` overloads are often
/// intentionally broad: a single body can expose both colour and length
/// forms. A failed typed placeholder must remove only the incompatible
/// declaration, never the compatible siblings.
pub(crate) fn resolve_css_declarations(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<ResolvedCssDeclarations> {
    let declarations = split_css_declarations(body);
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
    let mut resolved_placeholder = false;
    let mut resolved_value = false;
    let mut resolved_modifier = false;
    let mut resolved_required_declarations = 0;
    let mut output = ResolvedCssDeclarations::default();

    for authored_declaration in declarations {
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
        if let Some(declaration) = resolve_css_placeholders(
            authored_declaration,
            utility_name,
            name,
            modifier,
            theme,
            CompatColorMode::Legacy,
        ) && !declaration.trim().is_empty()
        {
            resolved_placeholder |= contains_placeholder;
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
                && modern != declaration
            {
                output.compat_modern.push(modern.trim_start().to_owned());
            }
        }
    }

    // Static declarations in a functional body are supporting declarations,
    // not an independent branch. For example the generic `translate-*` body
    // must not emit `translate: …` when none of its typed values match.
    (!uses_placeholder || resolved_placeholder)
        .then_some(())
        // A modifier-only sibling (such as `text-*/leading`) cannot make an
        // unresolved value branch valid. This is what prevents a colour token
        // from being misinterpreted as a spacing value merely because both
        // spellings use a slash.
        .filter(|()| !uses_value || resolved_value)
        .filter(|()| !requires_modifier || resolved_modifier)
        .filter(|()| resolved_required_declarations == required_declarations)
        .map(|()| output)
}

/// Splits a declaration list without mistaking a semicolon in a string or a
/// function argument for a CSS declaration boundary.
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
    use super::*;

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
        let theme = ThemeIndex::default();
        assert!(
            resolve_css_declarations(
                "display:block;width:--value([length]);",
                "w-*",
                "w-[red]",
                None,
                &theme
            )
            .is_none()
        );
        let resolved = resolve_css_declarations(
            "width:--value([length]);color:--value([color]);",
            "x-*",
            "x-[red]",
            None,
            &theme,
        )
        .unwrap();
        assert_eq!(resolved.base, "color:red;");
        assert!(resolved.compat_modern.is_empty());
    }
}
