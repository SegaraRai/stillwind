//! Placeholder forms that are rewritten inside a body before the main value pass.
//!
//! These macros are resolved by text substitution in place, and each one consumes a
//! whole macro invocation. Replacement text may be longer or contain another macro;
//! these loops do not enforce a decreasing measure or detect cycles. An unclosed or
//! unresolvable macro fails the whole body rather than leaving the placeholder text in
//! generated CSS.
//!
//! `--stillwind-theme-suffix(...)` is the one form that composes a name from two parts:
//! the candidate capture plus an authored suffix, which is how a wildcard definition
//! resolves a token whose name it cannot spell in advance.

use super::super::syntax::closing_parenthesis;
use super::colors::CompatColorMode;
use super::domains::{
    resolve_literal_value_domains, resolve_value_domains, split_domains, theme_domain_pattern,
};
use super::theme::theme_value;
use crate::ThemeIndex;

/// Replaces every `--value-literal(...)` with the authored token text.
///
/// The compatibility mode decides the lookup: the fallback branch must read the literal
/// value, while the modern branch takes the same reference the ordinary `--value(...)`
/// form would produce.
pub(crate) fn resolve_literal_value_placeholders(
    body: &mut String,
    capture: Option<&str>,
    theme: &ThemeIndex,
    compat_color_mode: CompatColorMode,
) -> Option<()> {
    while let Some(start) = body.find("--value-literal(") {
        let open = start + "--value-literal".len();
        let end = closing_parenthesis(body, open)?;
        let replacement = match compat_color_mode {
            CompatColorMode::Legacy => {
                resolve_literal_value_domains(&body[open + 1..end], capture, theme)?
            }
            CompatColorMode::Modern => resolve_value_domains(&body[open + 1..end], capture, theme)?,
        };
        body.replace_range(start..=end, &replacement);
    }
    Some(())
}

/// Replaces every `--stillwind-theme-suffix(...)` with a composed token value.
///
/// The first argument is a theme namespace pattern, the second a suffix that is
/// appended to the candidate capture; both the capture and a matching token are
/// required, so a missing token fails the branch.
pub(crate) fn resolve_theme_suffix_placeholders(
    body: &mut String,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<()> {
    while let Some(start) = body.find("--stillwind-theme-suffix(") {
        let open = start + "--stillwind-theme-suffix".len();
        let end = closing_parenthesis(body, open)?;
        let arguments = split_domains(&body[open + 1..end]);
        let [domain, suffix] = arguments.as_slice() else {
            return None;
        };
        let (namespace, domain_suffix) = theme_domain_pattern(domain.trim())?;
        let capture = capture?;
        let suffix = suffix.trim().strip_prefix("--")?;
        let value = theme_value(
            namespace,
            &format!("{capture}{domain_suffix}--{suffix}"),
            theme,
        )?;
        body.replace_range(start..=end, &value);
    }
    Some(())
}

/// Replaces every `--value-map(...)` with the entry matching the capture.
///
/// Keys are compared exactly as authored and the first matching entry wins; a candidate
/// without a capture, or with no matching key, fails the branch.
pub(crate) fn resolve_value_map_placeholders(
    body: &mut String,
    capture: Option<&str>,
) -> Option<()> {
    while let Some(start) = body.find("--value-map(") {
        let open = start + "--value-map".len();
        let end = closing_parenthesis(body, open)?;
        let capture = capture?;
        let replacement = split_domains(&body[open + 1..end])
            .into_iter()
            .find_map(|entry| {
                let (key, value) = entry.trim().split_once(':')?;
                (key.trim() == capture).then(|| value.trim())
            })?
            .to_owned();
        body.replace_range(start..=end, &replacement);
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_maps_match_exact_keys_and_report_missing_captures() {
        let mut body = "x:--value-map(sm: 1px, lg: calc(2px + 3px));".to_owned();
        assert_eq!(
            resolve_value_map_placeholders(&mut body, Some("lg")),
            Some(())
        );
        assert_eq!(body, "x:calc(2px + 3px);");
        assert_eq!(
            resolve_value_map_placeholders(&mut "--value-map(sm:1)".into(), Some("s")),
            None
        );
        assert_eq!(
            resolve_value_map_placeholders(&mut "--value-map(sm:1)".into(), None),
            None
        );
        let theme = ThemeIndex::default();
        let mut literal = "x:--value-literal([number]);".to_owned();
        assert_eq!(
            resolve_literal_value_placeholders(
                &mut literal,
                Some("[2]"),
                &theme,
                CompatColorMode::Legacy
            ),
            Some(())
        );
        assert_eq!(literal, "x:2;");
        assert_eq!(
            resolve_theme_suffix_placeholders(
                &mut "--stillwind-theme-suffix(invalid)".into(),
                Some("x"),
                &theme
            ),
            None
        );
    }
}
