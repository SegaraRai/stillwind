//! Checks whether a replacement produces the same CSS as the original candidate.

use lean_string::LeanString;

use stillwind_css::find_css_functions;
use stillwind_design_system::{ThemeValueMode, UtilityResolutionError, resolve_utility};
use stillwind_diagnostics::LintKind;
use stillwind_syntax::{Utility, parse_candidate, print_arbitrary_value};

use super::super::index::policy;
use super::super::preview::spacing_properties;
use super::spelling;
use crate::query::DesignSystemQuery;

impl DesignSystemQuery {
    /// Checks whether the candidate `right` can replace the candidate `left`.
    ///
    /// Both candidates must produce the same normalized CSS under the options of this
    /// query, and they must need the same registered properties. Each `var()` call that
    /// the author wrote in `left` must also appear in the CSS of `right`, unless the
    /// variable is an inline theme token that `right` uses. When the
    /// `prefer-typed-variables` rule is enabled, a replacement that adds a variable
    /// without a type hint is rejected. Returns false when either candidate does not
    /// resolve or cannot be normalized.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of either candidate.
    pub(super) fn equivalent(
        &self,
        left: &str,
        right: &str,
    ) -> Result<bool, UtilityResolutionError> {
        let variables = authored_variables(left);
        if self
            .lint_settings()
            .enabled_for_detection(LintKind::PreferTypedVariables)
        {
            let theme = self.context.design_system.theme();
            let before = resolve_utility(left, theme)?.ok();
            let after = resolve_utility(right, theme)?.ok();
            if before.is_some_and(|value| value.diagnostic_data.untyped_variable.is_none())
                && after.is_some_and(|value| value.diagnostic_data.untyped_variable.is_some())
            {
                return Ok(false);
            }
        }
        let Some(left) = self.context.canonical_preview(left)? else {
            return Ok(false);
        };
        let right_raw = right;
        let Some(right) = self.context.canonical_preview(right)? else {
            return Ok(false);
        };
        if left.required_properties != right.required_properties {
            return Ok(false);
        }
        if !variables.is_empty() {
            let references = variable_calls(&right.css);
            for variable in variables.iter().filter(|value| !references.contains(value)) {
                let name = variable
                    .strip_prefix("var(")
                    .and_then(|value| value.strip_suffix(')'))
                    .filter(|name| !name.contains(','));
                let Some(name) = name else { return Ok(false) };
                let theme = self.context.design_system.theme();
                let inline = theme
                    .token_property(name)
                    .is_some_and(|token| token.mode == ThemeValueMode::Inline);
                let selected = resolve_utility(right_raw, theme)?.is_ok_and(|utility| {
                    utility
                        .diagnostic_data
                        .token_uses
                        .iter()
                        .any(|usage| usage.tokens.iter().any(|token| token == name))
                });
                if !inline || !selected {
                    return Ok(false);
                }
            }
        }
        let scales = self.context.theme_scales();
        let mut policy = policy(&right.css, &self.canonical_options(), &scales);
        if self.canonical_options().px_to_rem {
            spacing_properties(&left.css, &scales, &mut policy.properties, 0);
        }
        let font = self.context.options.root_font_size;
        let signature = left.signature(&scales, &policy.properties, font);
        Ok(
            (signature.is_some()
                && signature == right.signature(&scales, &policy.properties, font))
                || (left.resolved_css.is_some()
                    && right.resolved_css.is_some()
                    && left.css == right.css),
        )
    }
}

/// Returns the `var()` calls that the author wrote in the arbitrary value of the
/// candidate `raw`, such as `var(--x)` in `w-[var(--x)]` and in `w-(--x)`. Returns an
/// empty list when `raw` has no arbitrary value.
pub(in crate::query::canonical) fn authored_variables(raw: &str) -> Vec<LeanString> {
    let written = parse_candidate(raw).and_then(|candidate| match candidate.utility {
        Utility::Named { name, .. } => spelling::arbitrary(&name).map(|(_, _, value)| value),
        Utility::ArbitraryProperty { value, .. } => Some(value.to_string()),
    });
    written.as_deref().map(variable_calls).unwrap_or_default()
}

/// Returns each complete `var()` call in `css`, written in the compact spelling of
/// candidates. Calls inside CSS strings and URLs are not included.
pub(in crate::query::canonical) fn variable_calls(css: &str) -> Vec<LeanString> {
    find_css_functions(css, &["var"], 0)
        .into_iter()
        .filter_map(|call| Some(print_arbitrary_value(&css[call.start..call.end?])))
        .collect()
}
