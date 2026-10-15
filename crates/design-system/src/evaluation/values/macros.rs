//! Argument evaluators for the placeholder forms before the main value family.
//!
//! Each evaluator returns replacement text for one invocation. The shared expansion
//! driver applies it to the body, detects recursive invocations, and bounds depth,
//! work and output size. An unclosed or unresolvable macro fails the whole body
//! rather than leaving the placeholder text in generated CSS.
//!
//! `--stillwind-theme-suffix(...)` is the one form that composes a name from two parts:
//! the candidate capture plus an authored suffix, which is how a wildcard definition
//! resolves a token whose name it cannot spell in advance.

use lean_string::{LeanStr, ToLeanStr};

use super::super::EvaluationContext;
use super::domains::{resolve_literal_value_domains, split_domains, theme_domain_pattern};
use super::theme::theme_value;

/// Resolves one `--value-literal(...)` to the authored token text. Non-theme domains
/// use the ordinary value resolver.
pub(in crate::evaluation) fn resolve_literal_value_placeholder(
    arguments: &str,
    capture: Option<&str>,
    theme: &EvaluationContext<'_>,
) -> Option<LeanStr> {
    resolve_literal_value_domains(arguments, capture, theme)
}

/// Resolves one `--stillwind-theme-suffix(...)` to a composed token value.
///
/// The first argument is a theme namespace pattern, the second a suffix that is
/// appended to the candidate capture; both the capture and a matching token are
/// required, so a missing token fails the branch.
pub(in crate::evaluation) fn resolve_theme_suffix_placeholder(
    arguments: &str,
    capture: Option<&str>,
    theme: &EvaluationContext<'_>,
) -> Option<LeanStr> {
    let arguments = split_domains(arguments);
    let [domain, suffix] = arguments.as_slice() else {
        return None;
    };
    let (namespace, domain_suffix) = theme_domain_pattern(domain.trim())?;
    let capture = capture?;
    let suffix = suffix.trim().strip_prefix("--")?;
    let value = theme_value(
        namespace,
        &format_args!("{capture}{domain_suffix}--{suffix}").to_lean_str(),
        theme,
    )?;
    Some(value)
}

/// Resolves one `--value-map(...)` to the entry matching the capture.
///
/// Keys are compared exactly as authored and the first matching entry wins; a candidate
/// without a capture, or with no matching key, fails the branch.
pub(in crate::evaluation) fn resolve_value_map_placeholder(
    arguments: &str,
    capture: Option<&str>,
) -> Option<LeanStr> {
    let capture = capture?;
    let replacement = split_domains(arguments)
        .into_iter()
        .find_map(|entry| {
            let (key, value) = entry.trim().split_once(':')?;
            (key.trim() == capture).then(|| value.trim())
        })?
        .to_lean_str();
    Some(replacement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThemeIndex;

    #[test]
    fn value_maps_match_exact_keys_and_report_missing_captures() {
        assert_eq!(
            resolve_value_map_placeholder("sm: 1px, lg: calc(2px + 3px)", Some("lg")),
            Some("calc(2px + 3px)".into())
        );
        assert_eq!(resolve_value_map_placeholder("sm:1", Some("s")), None);
        assert_eq!(resolve_value_map_placeholder("sm:1", None), None);
        let theme = ThemeIndex::default();
        let theme = EvaluationContext::untracked(&theme);
        assert_eq!(
            resolve_literal_value_placeholder("[number]", Some("[2]"), &theme,),
            Some("2".into())
        );
        assert_eq!(
            resolve_theme_suffix_placeholder("invalid", Some("x"), &theme),
            None
        );
    }
}
