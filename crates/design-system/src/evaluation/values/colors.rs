//! Color opacity expressions, compatibility branches, and theme shadow modifiers.

use std::collections::BTreeSet;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanString, ToLeanString};

use stillwind_css::{find_css_functions, shadow_with_color_fallback, shadow_with_color_override};

use super::super::EvaluationContext;
use super::super::builtins::COMPAT_COLOR_MODERN_SPACE;
use super::domains::{resolve_value_domains, split_domains};
use super::theme::theme_value_literal;
use crate::{ThemeIndex, ThemeNamespace};

/// Resolves a color and an alpha, optionally followed by another alpha, to oklab mixing.
/// A 100% first alpha keeps the original color. Compatibility emission follows resolution.
pub(in crate::evaluation) fn resolve_compat_color(arguments: &str) -> Option<LeanString> {
    let arguments = split_domains(arguments);
    let (color, alpha, modern_alpha) = match arguments.as_slice() {
        [color, alpha] => (*color, *alpha, None),
        [color, alpha, modern_alpha] => (*color, *alpha, Some(modern_alpha.trim())),
        _ => return None,
    };
    let color = color.trim();
    let alpha = alpha.trim();
    if color.is_empty() || alpha.is_empty() {
        return None;
    }
    if color.eq_ignore_ascii_case("inherit") && alpha == "100%" {
        return Some(color.to_lean_string());
    }
    let mixed = if alpha == "100%" {
        color.to_lean_string()
    } else {
        format_args!("color-mix(in {COMPAT_COLOR_MODERN_SPACE}, {color} {alpha}, transparent)")
            .to_lean_string()
    };
    if let Some(modern_alpha) = modern_alpha {
        return Some(
            format_args!(
                "color-mix(in {COMPAT_COLOR_MODERN_SPACE}, {mixed} {modern_alpha}, transparent)"
            )
            .to_lean_string(),
        );
    }
    Some(mixed)
}

/// Returns a fallback declaration when mixing depends on variables or currentcolor.
/// Known theme values are expanded only in the fallback. Unknown values keep the
/// first color without its opacity, matching Tailwind's compatibility transform.
/// Fully supported targets keep the original declaration without a capability query.
pub(crate) fn color_fallback(declaration: &str, theme: &ThemeIndex) -> Option<LeanString> {
    if !declaration.contains("color-mix(") {
        return None;
    }
    let (property, value) = declaration.split_once(':')?;
    let mut transform = ColorFallback {
        theme,
        required: false,
        runtime_color: false,
    };
    let fallback = transform.mixes(value, 0, false);
    (transform.required && !theme.target().supports_color_mix(transform.runtime_color))
        .then(|| format_args!("{property}:{fallback}").to_lean_string())
}

struct ColorFallback<'a> {
    theme: &'a ThemeIndex,
    required: bool,
    runtime_color: bool,
}

impl ColorFallback<'_> {
    fn mixes(&mut self, value: &str, depth: usize, inside_mix: bool) -> LeanString {
        if depth >= 32 {
            return value.to_lean_string();
        }

        let mut output = LeanString::new();
        let mut cursor = 0;
        let functions: &[&str] = if inside_mix {
            &["color-mix", "var"]
        } else {
            &["color-mix"]
        };
        for function in find_css_functions(value, functions, 0) {
            if function.start < cursor {
                continue;
            }
            let Some(end) = function.end else {
                continue;
            };
            output.push_str(&value[cursor..function.start]);
            if function.name_index == 1 {
                let original = &value[function.start..end];
                output.push_str(self.variable(original).unwrap_or(original));
                cursor = end;
                continue;
            }
            let arguments = &value[function.arguments];
            let runtime_color = split_domains(arguments)
                .iter()
                .skip(1)
                .any(|part| !find_css_functions(first_component(part), &["var"], 0).is_empty());
            let (variables, unresolved) = self.variables(arguments);
            let current_color = contains_current_color(arguments);
            self.required |= variables || current_color;
            self.runtime_color |= runtime_color || current_color;
            let parts = split_domains(arguments);
            if parts.len() == 3 {
                if unresolved || current_color {
                    output.push_str(&self.mixes(first_component(parts[1]), depth + 1, true));
                } else {
                    let space = if self.required
                        && matches!(
                            parts[0].trim(),
                            "in oklab" | "in oklch" | "in lab" | "in lch"
                        ) {
                        "in srgb"
                    } else {
                        parts[0].trim()
                    };
                    let first = self.mixes(parts[1].trim(), depth + 1, true);
                    let second = self.mixes(parts[2].trim(), depth + 1, true);
                    output.push_str(
                        &format_args!("color-mix({space}, {first}, {second})").to_lean_string(),
                    );
                }
            } else {
                output.push_str(&value[function.start..end]);
            }
            cursor = end;
        }
        output.push_str(&value[cursor..]);
        output
    }

    fn variables(&self, value: &str) -> (bool, bool) {
        let mut cursor = 0;
        let mut found = false;
        let mut unresolved = false;
        for function in find_css_functions(value, &["var"], 0) {
            if function.start < cursor {
                continue;
            }
            let Some(end) = function.end else {
                continue;
            };
            found = true;
            unresolved |= self.variable(&value[function.start..end]).is_none();
            cursor = end;
        }
        (found, unresolved)
    }

    fn variable<'a>(&'a self, value: &str) -> Option<&'a str> {
        let mut variable = value;
        let mut seen = BTreeSet::new();
        loop {
            let function = find_css_functions(variable, &["var"], 0)
                .into_iter()
                .next()?;
            let name = split_domains(&variable[function.arguments]).first()?.trim();
            if !seen.insert(name.to_owned()) {
                return None;
            }
            let token = self.theme.custom_property(name)?;
            let value = token.value.value.as_str();
            if value.eq_ignore_ascii_case("currentcolor") {
                return None;
            }
            if !value.starts_with("var(") {
                return Some(value);
            }
            variable = value;
        }
    }
}

fn first_component(value: &str) -> &str {
    let value = value.trim_start();
    let mut parser = Parser::new(value);
    if let Ok(Token::Function(_)) = parser.next() {
        let _ = parser.parse_nested_block(|nested| {
            while nested.next().is_ok() {}
            Ok::<_, ParseError<()>>(())
        });
    }
    &value[..parser.position().byte_index()]
}

fn contains_current_color(value: &str) -> bool {
    fn scan(parser: &mut Parser<'_>) -> bool {
        while let Ok(token) = parser.next() {
            let found = match token {
                Token::Ident(name) => name.eq_ignore_ascii_case("currentcolor"),
                Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock => parser
                    .parse_nested_block(|nested| Ok::<_, ParseError<()>>(scan(nested)))
                    .unwrap_or(false),
                _ => false,
            };
            if found {
                return true;
            }
        }
        false
    }
    scan(&mut Parser::new(value))
}

/// CSS-first extension for theme shadow opacity modifiers. `--value()` emits the normal
/// `var(--shadow-*)` reference; this intentionally reads the authored token to
/// construct a CSS Color 5 relative color function.
///
/// The first argument names the theme namespace whose token value is read, the second
/// names the color property that will hold the result, and the optional third is an
/// alpha override. The property name must be a custom property, because the generated
/// function replaces that property inside the shadow.
pub(in crate::evaluation) fn resolve_shadow_color(
    arguments: &str,
    capture: Option<&str>,
    theme: &EvaluationContext<'_>,
) -> Option<LeanString> {
    let arguments = split_domains(arguments);
    let (domain, color_property, alpha) = match arguments.as_slice() {
        [domain, color_property] => (*domain, *color_property, None),
        [domain, color_property, alpha] => (*domain, *color_property, Some(alpha.trim())),
        _ => return None,
    };
    let value = if domain.trim() == "[shadow]" {
        let value = resolve_value_domains(domain, capture, theme)?;
        // A variable hides the shadow's color, so it has no color to override.
        if is_single_var_reference(&value) {
            return Some(value.into());
        }
        value
    } else {
        let namespace = domain
            .trim()
            .strip_prefix("--")
            .and_then(|domain| domain.strip_suffix("-*"))
            .and_then(ThemeNamespace::parse)?;
        theme_value_literal(namespace, capture?, theme)?
    };
    let color_property = color_property.trim();
    color_property.strip_prefix("--").filter(|name| {
        !name.is_empty()
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
    })?;
    match alpha {
        Some(alpha) => shadow_with_color_fallback(&value, color_property, alpha),
        None => shadow_with_color_override(&value, color_property, None),
    }
}

/// True when `value` consists of exactly one `var()` function.
fn is_single_var_reference(value: &str) -> bool {
    find_css_functions(value, &["var"], 0)
        .first()
        .is_some_and(|function| function.start == 0 && function.end == Some(value.len()))
}

#[cfg(test)]
mod tests {
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::{assert_css_eq, css_source, fixture_css};

    use super::*;
    use crate::{BrowserTargets, DesignSystemOptions, DesignSystemSnapshot, resolve_utility};

    fn snapshot(target: &[&str]) -> DesignSystemSnapshot {
        let source = css_source(
            FileId(1),
            fixture_css("@import \"tailwindcss\"; @theme {--color-linked:var(--external);}"),
        )
        .unwrap();
        DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("test.css"),
            DesignSystemOptions {
                target: BrowserTargets::parse(target).unwrap(),
                ..DesignSystemOptions::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn fallback_values_match_tailwind_for_theme_arbitrary_and_runtime_values() {
        let snapshot = snapshot(&[]);
        let theme = snapshot.theme();
        for (candidate, expected, conditional) in [
            (
                "bg-[red]/50",
                "background-color:color-mix(in oklab, red 50%, transparent);",
                false,
            ),
            (
                "bg-red-500/100",
                "background-color:var(--color-red-500);",
                false,
            ),
            ("bg-(--brand)/50", "background-color:var(--brand);", true),
            (
                "bg-linked/50",
                "background-color:var(--color-linked);",
                true,
            ),
            ("bg-[red]/(--opacity)", "background-color:red;", true),
            ("bg-current/0", "background-color:currentcolor;", true),
            (
                "shadow-[color:red]/50",
                "--tw-shadow-color:color-mix(in srgb, red 50%, transparent);",
                true,
            ),
        ] {
            let output = resolve_utility(candidate, theme).unwrap().unwrap();
            assert_css_eq!(output.declaration, expected);
            assert_eq!(
                output.nested.iter().any(|rule| rule.compatibility),
                conditional,
                "{candidate}"
            );
        }
    }

    #[test]
    fn only_fully_supported_targets_remove_generated_compatibility() {
        for (target, fallback) in [
            (vec![], true),
            (vec!["chrome110"], true),
            (vec!["chrome111", "firefox112"], true),
            (vec!["safari16.2"], true),
            (vec!["safari16.6"], true),
            (vec!["chrome111", "firefox113", "safari17", "ios17"], false),
        ] {
            let snapshot = snapshot(&target);
            for candidate in [
                "bg-red-500/50",
                "bg-current/0",
                "bg-(--brand)/50",
                "shadow-red-500/50",
            ] {
                let output = resolve_utility(candidate, snapshot.theme())
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    output.nested.iter().any(|rule| rule.compatibility),
                    fallback,
                    "{target:?}: {candidate}"
                );
                if !fallback {
                    assert!(output.declaration.contains("color-mix(in oklab"));
                    assert!(!output.declaration.contains("color-mix(in srgb"));
                }
            }
        }
    }

    #[test]
    fn arbitrary_color_mix_uses_the_same_fallback_transform() {
        let snapshot = snapshot(&[]);
        let output = resolve_utility(
            "[color:color-mix(in_oklab,var(--external)_50%,transparent)]",
            snapshot.theme(),
        )
        .unwrap()
        .unwrap();
        assert_css_eq!(output.declaration, "color:var(--external);");
        assert_eq!(output.nested.len(), 1);
    }

    #[test]
    fn single_var_references_cover_the_whole_value() {
        assert!(is_single_var_reference("var(--x)"));
        assert!(is_single_var_reference("var(--x, 0 1px red)"));
        assert!(!is_single_var_reference("0 1px var(--x)"));
        assert!(!is_single_var_reference("var(--x) var(--y)"));
    }

    #[test]
    fn opacity_expressions_handle_keywords_and_explicit_alpha() {
        assert_eq!(
            resolve_compat_color("red, 50%"),
            Some("color-mix(in oklab, red 50%, transparent)".into())
        );
        assert_eq!(
            resolve_compat_color("inherit, 100%"),
            Some("inherit".into())
        );
        assert_eq!(resolve_compat_color("red"), None);
        assert_eq!(resolve_compat_color(", 50%"), None);
    }
}
