//! Rewrites legacy theme paths, such as `theme(colors.red.500)`, and writes scale
//! calculations in their short form, such as `--spacing(4)`.

use std::fmt::Write;

use cssparser::Token;
use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{find_css_functions, split_once_top_level};
use stillwind_design_system::{ThemeIndex, UtilityResolutionError};
use stillwind_syntax::{
    CandidatePrefix, Utility, decode_arbitrary_value, parse_candidate, print_arbitrary_value,
};

use super::spelling::{arbitrary, value_candidate};
use crate::query::DesignSystemQuery;

impl DesignSystemQuery {
    /// Returns `raw` with legacy syntax replaced by its current form.
    ///
    /// The method rewrites `theme()` calls in variants and arbitrary values with
    /// `theme_value`. It also renames a few built-in utilities whose name changed, such
    /// as `bg-gradient-to-r` to `bg-linear-to-r`. A renamed utility is used only when
    /// the old name no longer resolves, or when both names produce the same CSS, so
    /// custom definitions keep their behavior. Returns `raw` unchanged when the
    /// candidate cannot be parsed.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a utility definition that the method checks.
    pub(super) fn migrate_candidate(&self, raw: &str) -> Result<LeanStr, UtilityResolutionError> {
        let Some(parsed) = parse_candidate(raw) else {
            return Ok(raw.to_lean_str());
        };
        let theme = self.context.design_system.theme();
        let prefix = parsed
            .variants
            .iter()
            .map(|variant| {
                let decoded = decode_arbitrary_value(&variant.raw);
                let converted = theme_value(&decoded, theme, true, false, 0).0;
                if decoded == converted {
                    variant.raw.to_string()
                } else {
                    print_arbitrary_value(&converted).to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(":");
        let prefix = if prefix.is_empty() {
            prefix
        } else {
            format!("{prefix}:")
        };
        let important = if parsed.important { "!" } else { "" };
        let sign = if parsed.negative { "-" } else { "" };
        let split = CandidatePrefix::variant_end(raw);
        let base = raw[split..].trim_matches('!');
        let modifier = CandidatePrefix::parse(base)
            .and_then(|prefix| prefix.last_separator('/'))
            .map_or("", |index| &base[index..]);
        let (utility, extracted) = match &parsed.utility {
            Utility::ArbitraryProperty {
                property, value, ..
            } => {
                let (value, extracted) = theme_value(value, theme, false, modifier.is_empty(), 0);
                (
                    format!("[{property}:{}]", print_arbitrary_value(&value)),
                    extracted,
                )
            }
            Utility::Named { name, .. } => {
                if let Some((root, hint, value)) = arbitrary(name) {
                    let (converted, extracted) =
                        theme_value(&value, theme, false, modifier.is_empty(), 0);
                    if value == converted {
                        (format!("{sign}{name}"), extracted)
                    } else {
                        (
                            format!(
                                "{sign}{}",
                                value_candidate(
                                    root,
                                    &hint.map_or(String::new(), |hint| format!("{hint}:")),
                                    &converted
                                )
                            ),
                            extracted,
                        )
                    }
                } else {
                    (format!("{sign}{name}"), None)
                }
            }
        };
        let modifier = extracted.as_deref().unwrap_or(modifier);
        let migrated = format!("{prefix}{utility}{modifier}{important}").to_lean_str();
        let importance_only = raw[split..]
            .strip_prefix('!')
            .is_some_and(|base| migrated == format!("{}{base}!", &raw[..split]));
        if importance_only && self.context.canonical_preview(&migrated)?.is_none() {
            return Ok(raw.to_lean_str());
        }
        let parsed = parse_candidate(&migrated);
        if let Some(Utility::Named { name, .. }) = parsed.as_ref().map(|parsed| &parsed.utility) {
            for (legacy, current) in theme.registry().legacy_utilities() {
                let modern = match legacy.strip_suffix('*') {
                    Some(prefix) => name
                        .strip_prefix(prefix)
                        .filter(|rest| !rest.is_empty())
                        .map(|rest| format!("{}{rest}", current.trim_end_matches('*'))),
                    None => (name == legacy).then(|| current.to_string()),
                };
                let Some(modern) = modern else {
                    continue;
                };
                let replacement = migrated.replacen(name.as_str(), &modern, 1);
                if self.context.canonical_preview(&replacement)?.is_some()
                    && (self.context.canonical_preview(&migrated)?.is_none()
                        || self.equivalent(&migrated, &replacement)?)
                {
                    return Ok(replacement.into());
                }
            }
        }
        Ok(migrated)
    }
}

/// Rewrites each legacy `theme(path)` call in `value`, and then shortens scale
/// calculations with `scale_functions`.
///
/// - A path that names an active token becomes `var(--token)`. It becomes
///   `--theme(--token/alpha)` when the path has an alpha value, or when `variant` is
///   true.
/// - A `spacing` path whose number is a nonnegative multiple of 0.25 becomes
///   `--spacing(n)`.
/// - A `colors.` path without an active token is replaced by its fallback.
/// - Other calls keep their path.
///
/// The fallback argument of a call is rewritten in the same way. `depth` is the nesting
/// level of `value`, and the method stops after 64 levels.
///
/// When `modifier` is true and `variant` is false, and `value` is exactly one `theme()`
/// call with an alpha value, the method removes the alpha from the call. The second
/// element of the result is then that alpha written as a candidate modifier, such as
/// `/50`. Otherwise the second element is `None`.
fn theme_value(
    value: &str,
    theme: &ThemeIndex,
    variant: bool,
    modifier: bool,
    depth: usize,
) -> (String, Option<String>) {
    if depth > 64 {
        return (value.into(), None);
    }
    let calls = find_css_functions(value, &["theme"], 0);
    let modifier_count = calls
        .iter()
        .filter(|call| {
            let (path, _) = split_once_top_level(&value[call.arguments.clone()], |token| {
                matches!(token, Token::Comma)
            });
            path.contains('/')
        })
        .count();
    let mut output = String::new();
    let mut cursor = 0;
    let mut extracted = None;
    for call in &calls {
        if call.start < cursor {
            continue;
        }
        let Some(end) = call.end else { break };
        output.push_str(&value[cursor..call.start]);
        let (path, fallback) = split_once_top_level(&value[call.arguments.clone()], |token| {
            matches!(token, Token::Comma)
        });
        let path = path.trim().trim_matches(['\'', '"']);
        let (path, alpha) = split_once_top_level(path, |token| matches!(token, Token::Delim('/')));
        let fallback = fallback
            .map(|fallback| theme_value(fallback.trim(), theme, variant, false, depth + 1).0);
        let alpha = alpha.map(str::trim);
        let has_multiple_alpha = alpha.is_some_and(|alpha| alpha.contains('/'));
        let variable = variable(path.trim(), theme);
        let whole = call.start == 0 && end == value.len();
        if let Some(variable) = variable.filter(|_| !has_multiple_alpha) {
            let mut alpha = alpha;
            if modifier
                && !variant
                && whole
                && modifier_count == 1
                && let Some(value) = alpha
            {
                extracted = Some(opacity_modifier(value));
                alpha = None;
            }
            let fallback = fallback
                .as_deref()
                .map_or(String::new(), |fallback| format!(",{fallback}"));
            if variant || alpha.is_some() {
                let _ = write!(
                    output,
                    "--theme({variable}{}{fallback})",
                    alpha.map_or(String::new(), |alpha| format!("/{alpha}"))
                );
            } else {
                let _ = write!(output, "var({variable}{fallback})");
            }
        } else if let Some(scale) = (!variant && !has_multiple_alpha && alpha.is_none())
            .then(|| scale_function(path.trim(), theme))
            .flatten()
        {
            output.push_str(&scale);
        } else {
            if !variant && !has_multiple_alpha && alpha.is_none() && fallback.is_some() {
                output.push_str(fallback.as_deref().unwrap_or(""));
                cursor = end;
                continue;
            }
            let (original, _) = split_once_top_level(&value[call.arguments.clone()], |token| {
                matches!(token, Token::Comma)
            });
            let _ = write!(
                output,
                "theme({original}{})",
                fallback.map_or(String::new(), |fallback| format!(",{fallback}"))
            );
        }
        cursor = end;
    }
    output.push_str(&value[cursor..]);
    (scale_functions(&output, theme), extracted)
}

/// Returns the opacity `value` written as a candidate modifier, including its leading
/// `/`.
///
/// A whole percentage such as `50%` becomes `/50`. A fraction such as `.5` becomes
/// `/50`, or an arbitrary modifier when the percentage is not whole. `var(--x)` becomes
/// `/(--x)`. Any other value becomes an arbitrary modifier such as `/[calc(...)]`.
fn opacity_modifier(value: &str) -> String {
    if let Some(number) = value
        .strip_suffix('%')
        .filter(|number| number.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return format!("/{number}");
    }
    if (value.starts_with('.') || value.starts_with("0."))
        && let Ok(number) = value.parse::<f64>()
    {
        let number = number * 100.0;
        if number.fract() == 0.0 {
            return format!("/{number}");
        }
        return format!("/[{number}]");
    }
    if let Some(value) = value
        .strip_prefix("var(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return format!("/({value})");
    }
    format!("/[{}]", print_arbitrary_value(value))
}

/// Returns the custom property of the active token that the legacy theme `path` names,
/// such as `--color-red-500` for `colors.red.500`. Returns `None` when no active token
/// matches.
///
/// A `path` that starts with `--` is returned when it names an active token. Otherwise
/// the first key selects the namespace with `legacy_namespace`, and the other keys
/// follow it, joined with `-`. A `1` that is not the last key marks the second item of
/// a tuple: `fontSize.xs.1.lineHeight` becomes `--text-xs--line-height`. A final `0`
/// after a tuple key, such as in `fontSize.xs.0`, names the token without a suffix.
fn variable(path: &str, theme: &ThemeIndex) -> Option<String> {
    if path.starts_with("--") {
        return theme.token_property(path).map(|_| path.into());
    }
    let path = path
        .replace("[1].", ".1.")
        .replace('[', ".")
        .replace([']', '\'', '"'], "");
    let mut parts: Vec<_> = path.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    if parts.len() > 2 && parts.last() == Some(&"0") {
        parts.pop();
    }
    let mut name = format!("--{}", legacy_namespace(parts[0], theme));
    let mut suffix = false;
    let last = parts.len() - 1;
    for (index, part) in parts.iter().enumerate().skip(1) {
        name.push('-');
        if *part == "1" && index != last {
            suffix = true;
        } else if suffix {
            name.push_str(&kebab_case(part));
        } else {
            name.push_str(part);
        }
    }
    theme.token_property(&name).map(|_| name)
}

/// Returns the scale function for the legacy theme `path`, such as `--spacing(4)` for
/// `spacing.4`. Returns `None` when the namespace of the first key has no scale, when
/// its scale token is not active, or when the rest of the path is not a nonnegative
/// multiple of 0.25.
fn scale_function(path: &str, theme: &ThemeIndex) -> Option<String> {
    let split = path.find(['.', '[']).unwrap_or(path.len());
    let namespace = theme
        .namespaces()
        .get(&legacy_namespace(&path[..split], theme))
        .filter(|namespace| namespace.scale.is_some())?;
    theme.token_property(&namespace.scale_token())?;
    let multiplier = path[split..]
        .trim_start_matches('.')
        .trim_matches(['[', ']', '\'', '"']);
    multiplier
        .parse::<f64>()
        .is_ok_and(|value| value.is_finite() && value >= 0.0 && (value * 4.0).fract() == 0.0)
        .then(|| format!("{}({multiplier})", namespace.scale_token()))
}

/// Returns the name of the theme namespace that the Tailwind v3 config key `key` reads.
/// A `@stillwind-legacy-theme-key` statement names it, such as `text` for `fontSize`.
/// Without a statement, the namespace has the kebab-case name of `key`, such as
/// `font-weight` for `fontWeight`.
fn legacy_namespace(key: &str, theme: &ThemeIndex) -> String {
    theme
        .registry()
        .legacy_theme_keys()
        .get(key)
        .map_or_else(|| kebab_case(key), |legacy| legacy.namespace.to_string())
}

/// Returns `value` in kebab case, such as `line-height` for `lineHeight`.
fn kebab_case(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 4);
    for character in value.chars() {
        if character.is_ascii_uppercase() {
            output.push('-');
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}

/// Returns `value` with scale calculations written as scale functions. For example,
/// `calc(var(--spacing) * 4)` becomes `--spacing(4)`.
///
/// The method replaces only `calc()` calls without nested `calc()` calls, and only for
/// scales whose token is active. It repeats until no call changes, at most 32 times, so
/// nested calculations are also replaced from the inside out.
fn scale_functions(value: &str, theme: &ThemeIndex) -> String {
    let mut value = value.to_string();
    for _ in 0..32 {
        let calls = find_css_functions(&value, &["calc"], 0);
        let leaves: Vec<_> = calls
            .iter()
            .enumerate()
            .filter(|(index, call)| {
                calls
                    .get(index + 1)
                    .is_none_or(|next| next.start >= call.end.unwrap_or(value.len()))
            })
            .map(|(_, call)| call.clone())
            .collect();
        let mut changed = false;
        for call in leaves.into_iter().rev() {
            let Some(end) = call.end else { continue };
            let (left, right) = split_once_top_level(&value[call.arguments.clone()], |token| {
                matches!(token, Token::Delim('*'))
            });
            let Some(right) = right else { continue };
            for namespace in theme.namespaces().scales() {
                let name = namespace.scale_token();
                if theme.token_property(&name).is_none() {
                    continue;
                }
                let reference = format!("var({name})");
                let multiplier = if left.trim() == reference {
                    Some(right.trim())
                } else if right.trim() == reference {
                    Some(left.trim())
                } else {
                    None
                };
                if let Some(multiplier) = multiplier {
                    let replacement = format!("{name}({multiplier})");
                    value.replace_range(call.start..end, &replacement);
                    changed = true;
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
    value
}
