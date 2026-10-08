//! Ordered expansion of every placeholder family in one resolved declaration.
//!
//! Placeholders are resolved in a fixed order because later ones may depend on earlier
//! text: nested macros first, then `--value(...)`, then the modifier forms, then
//! `--alpha(...)`, and finally the color forms. Every remaining placeholder at the end
//! from the recognized marker list fails the declaration. Unknown function names
//! are not rejected by this check and can remain in generated CSS.
//!
//! The capture a placeholder sees is the candidate text after the matched definition's
//! prefix, with a slash modifier kept separately; a fraction domain is the one case
//! where the two are reassembled, and only when both sides are unsigned integers.

use crate::evaluation::EvaluationContext;

use stillwind_css::{CssFunctionMatch, declaration_ranges};
use stillwind_syntax::Value;

use super::arbitrary::alpha;
use super::colors::{CompatColorMode, resolve_compat_color, resolve_shadow_color};
use super::domains::{resolve_value_domains, split_domains};
use super::expansion::{expand_placeholders, matching_functions};
use super::macros::{
    resolve_literal_value_placeholder, resolve_theme_suffix_placeholder,
    resolve_value_map_placeholder,
};

// Order is semantic: replacements may introduce functions from later families.
const PLACEHOLDERS: &[&str] = &[
    "--value-literal",
    "--stillwind-theme-suffix",
    "--value-map",
    "--value",
    "--modifier",
    "--modifier-map",
    "--modifier-none",
    "--alpha",
    "--compat-color",
    "--stillwind-builtin-shadow-color",
];

/// A declaration whose executable placeholders all use the ordinary value family.
#[derive(Debug)]
pub(in super::super) struct PreparedValues {
    body: String,
    calls: Vec<CssFunctionMatch>,
}

impl PreparedValues {
    pub(in super::super) fn parse(body: &str) -> Option<Self> {
        let calls = matching_functions(body, PLACEHOLDERS);
        let mut end = 0;
        for call in &calls {
            if PLACEHOLDERS[call.name_index] != "--value" || call.start < end {
                return None;
            }
            end = call.end?;
        }
        Some(Self {
            body: body.to_owned(),
            calls,
        })
    }

    pub(in super::super) fn is_static(&self) -> bool {
        self.calls.is_empty()
    }

    pub(in super::super) fn resolve(
        &self,
        utility_name: &str,
        name: &str,
        modifier: Option<&Value>,
        theme: &EvaluationContext<'_>,
    ) -> Option<String> {
        if !self.calls.is_empty() && (self.body.len() > 1_048_576 || self.calls.len() > 4096) {
            return None;
        }
        let capture = utility_name
            .strip_suffix('*')
            .and_then(|prefix| name.strip_prefix(prefix));
        let mut output = String::with_capacity(self.body.len());
        let mut cursor = 0;
        for call in &self.calls {
            let value =
                resolve_value(&self.body[call.arguments.clone()], capture, modifier, theme)?;
            // An authored inline token can introduce further compiler functions.
            if PLACEHOLDERS.iter().any(|marker| value.contains(marker)) {
                return resolve_css_placeholders(
                    &self.body,
                    utility_name,
                    name,
                    modifier,
                    theme,
                    CompatColorMode::Legacy,
                );
            }
            output.push_str(&self.body[cursor..call.start]);
            output.push_str(&value);
            if output.len() > 1_048_576 {
                return None;
            }
            cursor = call.end?;
        }
        output.push_str(&self.body[cursor..]);
        if !self.calls.is_empty() && output.len() > 1_048_576 {
            return None;
        }
        Some(output)
    }
}

fn resolve_value(
    domains: &str,
    capture: Option<&str>,
    modifier: Option<&Value>,
    theme: &EvaluationContext<'_>,
) -> Option<String> {
    let fraction = domains
        .contains("--fraction()")
        .then(|| {
            capture.zip(modifier).and_then(|(capture, modifier)| {
                let Value::Named(modifier) = modifier else {
                    return None;
                };
                (capture.parse::<u16>().is_ok() && modifier.parse::<u16>().is_ok())
                    .then(|| format!("{capture}/{modifier}"))
            })
        })
        .flatten();
    resolve_value_domains(domains, fraction.as_deref().or(capture), theme)
}

/// Resolves all placeholders in `body` for one candidate.
///
/// `utility_name` supplies the wildcard prefix that yields the capture; `modifier` is
/// the authored slash modifier, which is optional and may be an arbitrary value or a
/// variable. [`None`] means the declaration does not apply to this candidate, which
/// lets the caller try the next overload.
#[allow(clippy::too_many_lines)]
pub(in crate::evaluation) fn resolve_css_placeholders(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &EvaluationContext<'_>,
    compat_color_mode: CompatColorMode,
) -> Option<String> {
    let capture = utility_name
        .strip_suffix('*')
        .and_then(|prefix| name.strip_prefix(prefix));
    let mut body = body.to_owned();
    expand_placeholders(
        &mut body,
        &PLACEHOLDERS[..6],
        |function, arguments| match function {
            "--value-literal" => {
                resolve_literal_value_placeholder(arguments, capture, theme, compat_color_mode)
            }
            "--stillwind-theme-suffix" => {
                resolve_theme_suffix_placeholder(arguments, capture, theme)
            }
            "--value-map" => resolve_value_map_placeholder(arguments, capture),
            "--value" => resolve_value(arguments, capture, modifier, theme),
            "--modifier" => {
                let modifier = modifier?;
                let domains = arguments;
                let replacement = if domains.trim().is_empty() {
                    match modifier {
                        Value::Named(value) | Value::Arbitrary(value) => value.to_string(),
                        Value::CssVariable(value) => format!("var({value})"),
                    }
                } else {
                    let capture = match modifier {
                        Value::Named(value) => value.to_string(),
                        // `Value::Arbitrary` is decoded by the grammar. Re-wrap it so the same
                        // typed-domain resolver used by `--value(...)` can check an optional
                        // arbitrary-value hint.
                        Value::Arbitrary(value) => format!("[{value}]"),
                        Value::CssVariable(value) => format!("({value})"),
                    };
                    resolve_value_domains(domains, Some(&capture), theme)?
                };
                Some(replacement)
            }
            "--modifier-map" => {
                let modifier = modifier?;
                let modifier = match modifier {
                    Value::Named(value) | Value::Arbitrary(value) => value,
                    Value::CssVariable(_) => return None,
                };
                let replacement = split_domains(arguments)
                    .into_iter()
                    .find_map(|entry| {
                        let (key, value) = entry.trim().split_once(':')?;
                        (key.trim() == modifier).then(|| value.trim())
                    })?
                    .to_owned();
                Some(replacement)
            }
            _ => None,
        },
    )?;
    // This family removes the enclosing declaration, so it forms a phase boundary.
    remove_modifier_none(&mut body, modifier)?;
    expand_placeholders(
        &mut body,
        &PLACEHOLDERS[7..],
        |function, arguments| match function {
            "--alpha" => alpha(arguments),
            "--compat-color" => resolve_compat_color(arguments, compat_color_mode),
            "--stillwind-builtin-shadow-color" => resolve_shadow_color(arguments, capture, theme),
            _ => None,
        },
    )?;
    matching_functions(&body, PLACEHOLDERS)
        .is_empty()
        .then_some(body)
}

/// Removes each declaration selected by an empty modifier guard once.
fn remove_modifier_none(body: &mut String, modifier: Option<&Value>) -> Option<()> {
    let matches = matching_functions(body, &["--modifier-none"]);
    if matches.is_empty() {
        return Some(());
    }
    if modifier.is_some() {
        return None;
    }
    let declarations = declaration_ranges(body);
    let mut removed = Vec::new();
    let mut cursor = 0;
    for found in matches {
        // A removed declaration also consumes subsequent guards in that declaration.
        if found.start < cursor {
            continue;
        }
        if !body[found.start..].starts_with("--modifier-none()") {
            return None;
        }
        let range = declarations
            .iter()
            .find(|range| range.contains(&found.start))?;
        cursor = range.end + usize::from(body.as_bytes().get(range.end) == Some(&b';'));
        removed.push(range.start..cursor);
    }
    for range in removed.into_iter().rev() {
        body.replace_range(range, "");
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThemeIndex;

    #[test]
    fn prepared_values_keep_expansion_limits_and_recursive_token_behavior() {
        let theme = ThemeIndex::default();
        let theme = EvaluationContext::untracked(&theme);
        let body = format!("width:{};", "--value([number]) ".repeat(4097));
        let prepared = PreparedValues::parse(&body).unwrap();
        assert!(prepared.resolve("x-*", "x-[1]", None, &theme).is_none());
        assert!(
            resolve_css_placeholders(&body, "x-*", "x-[1]", None, &theme, CompatColorMode::Legacy)
                .is_none()
        );
        let body = "content:'--value([number])';width:--value([number]);";
        let prepared = PreparedValues::parse(body).unwrap();
        assert_eq!(
            prepared.resolve("x-*", "x-[1]", None, &theme),
            resolve_css_placeholders(body, "x-*", "x-[1]", None, &theme, CompatColorMode::Legacy)
        );
    }

    #[test]
    fn batch_resolution_preserves_phase_barrier_and_rejects_reintroduced_families() {
        let theme = ThemeIndex::default();
        let theme = EvaluationContext::untracked(&theme);
        let resolve = |body| {
            resolve_css_placeholders(body, "x-*", "x-a", None, &theme, CompatColorMode::Legacy)
        };
        assert_eq!(
            resolve("opacity:--value-map(a:--alpha(.5));"),
            Some("opacity:50%;".into())
        );
        assert_eq!(resolve("x:--value-map(a:--value-literal([number]));"), None);
        assert_eq!(
            resolve(
                "x:--modifier-none() --modifier-none(invalid);y:--alpha(.5);z:--modifier-none();"
            ),
            Some("y:50%;".into())
        );
        assert_eq!(resolve("x:--value([number]) --modifier-none();"), None);
        assert_eq!(
            resolve("x:--modifier-none() --alpha(invalid);"),
            Some(String::new())
        );
        assert_eq!(
            resolve("content:'--value([number])';x:custom(1);"),
            Some("content:'--value([number])';x:custom(1);".into())
        );
    }

    #[test]
    fn placeholders_reject_incompatible_values_and_missing_modifiers() {
        let theme = ThemeIndex::default();
        let theme = EvaluationContext::untracked(&theme);
        assert_eq!(
            resolve_css_placeholders(
                "width:--value([length]);",
                "w-*",
                "w-[2px]",
                None,
                &theme,
                CompatColorMode::Legacy
            ),
            Some("width:2px;".into())
        );
        assert_eq!(
            resolve_css_placeholders(
                "width:--value([length]);",
                "w-*",
                "w-[red]",
                None,
                &theme,
                CompatColorMode::Legacy
            ),
            None
        );
        assert_eq!(
            resolve_css_placeholders(
                "opacity:--modifier([number]);",
                "x-*",
                "x-a",
                None,
                &theme,
                CompatColorMode::Legacy
            ),
            None
        );
        assert_eq!(
            resolve_css_placeholders(
                "opacity:--alpha(.5);",
                "x",
                "x",
                None,
                &theme,
                CompatColorMode::Legacy
            ),
            Some("opacity:50%;".into())
        );
    }
}
