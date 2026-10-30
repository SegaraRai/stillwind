//! Host configuration of property prefixes, utility ordering, and browser targets.
//!
//! Snapshot construction validates the supplied namespaces and captures the ordering
//! strategy before utility evaluation. Generated identifiers use the captured prefixes.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};
use serde::{Deserialize, Serialize};

use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::ReplaceText;

use crate::target::BrowserTargets;

/// Property namespaces and utility ordering captured by a design-system snapshot.
///
/// Deserialization uses camelCase fields and fills omitted fields from [`Default`], so
/// a host may send only the prefix it changes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DesignSystemOptions {
    /// Prefix of Stillwind-owned generated properties.
    pub prefix: LeanStr,
    /// Prefix of the Tailwind-compatibility namespace, which stays distinct from
    /// `prefix` even when both are customized.
    pub tailwind_prefix: LeanStr,
    /// Prefix that replaces the leading `--` of theme variables in generated CSS, or an
    /// empty string to keep their authored names.
    pub theme_prefix: LeanStr,
    /// Strategy for ordering resolved utilities.
    pub utility_order: UtilityOrderMode,
    /// Minimum browser versions used to omit unnecessary color fallbacks.
    pub target: BrowserTargets,
}

impl Default for DesignSystemOptions {
    fn default() -> Self {
        Self {
            prefix: "--sw-".to_lean_str(),
            tailwind_prefix: "--tw-".to_lean_str(),
            theme_prefix: LeanStr::new(),
            utility_order: UtilityOrderMode::default(),
            target: BrowserTargets::default(),
        }
    }
}

impl DesignSystemOptions {
    /// Validates that the configured namespaces are CSS prefixes and that Stillwind's
    /// namespace is distinct from the others.
    ///
    /// A prefix must start with `--`, end with `-`, span at least four bytes, and
    /// contain no whitespace. The error preserves the offending field name and supplied
    /// value, or the common prefix when two namespaces collide. An empty `theme_prefix`
    /// is valid, and `theme_prefix` may equal `tailwind_prefix`. Validation does not
    /// modify any value.
    ///
    /// # Errors
    ///
    /// Returns [`CompileDiagnostic::InvalidPropertyPrefix`] for malformed prefixes, or
    /// [`CompileDiagnostic::ConflictingPropertyPrefixes`] for identical namespaces.
    /// Shape checks do not constitute full CSS identifier validation.
    pub fn validate(&self) -> Result<(), CompileDiagnostic> {
        for (name, value) in [
            ("prefix", self.prefix.as_str()),
            ("tailwindPrefix", self.tailwind_prefix.as_str()),
            ("themePrefix", self.theme_prefix.as_str()),
        ] {
            if name == "themePrefix" && value.is_empty() {
                continue;
            }
            if !value.starts_with("--")
                || !value.ends_with('-')
                || value.len() < 4
                || value.chars().any(char::is_whitespace)
            {
                return Err(CompileDiagnostic::InvalidPropertyPrefix {
                    option: name.into(),
                    value: value.into(),
                });
            }
        }
        if self.prefix == self.tailwind_prefix || self.prefix == self.theme_prefix {
            return Err(CompileDiagnostic::ConflictingPropertyPrefixes {
                prefix: self.prefix.clone(),
            });
        }
        Ok(())
    }

    /// Rewrites literal `--sw-` and `--tw-` markers in generated text to the configured
    /// prefixes.
    ///
    /// Canonical markers are first replaced by private sentinels, then by the
    /// configured prefixes. This keeps swapped or overlapping prefixes from rewriting
    /// one another's output. The caller supplies generated text without these reserved
    /// sentinel strings. The operation is literal substitution, so it suits JavaScript
    /// and single property names. CSS that may contain authored strings needs a
    /// tokenizing caller.
    pub fn rewrite_generated(&self, value: &str) -> LeanString {
        rewrite_generated_prefixes(&self.prefix, &self.tailwind_prefix, value)
    }
}

/// Performs [`DesignSystemOptions::rewrite_generated`] for the two configured prefixes.
///
/// Returns `value` unchanged when it contains no sentinel and neither marker needs a
/// different spelling.
#[must_use]
pub fn rewrite_generated_prefixes(prefix: &str, tailwind_prefix: &str, value: &str) -> LeanString {
    if !value.contains('\u{e000}')
        && (prefix == "--sw-" || !value.contains("--sw-"))
        && (tailwind_prefix == "--tw-" || !value.contains("--tw-"))
    {
        return value.to_lean_string();
    }
    value
        .replace_text("--sw-", "\u{e000}stillwind-prefix\u{e001}")
        .replace_text("--tw-", "\u{e000}tailwind-prefix\u{e001}")
        .replace_text("\u{e000}stillwind-prefix\u{e001}", prefix)
        .replace_text("\u{e000}tailwind-prefix\u{e001}", tailwind_prefix)
}

#[cfg(test)]
mod tests {
    use super::{CompileDiagnostic, DesignSystemOptions};

    #[test]
    fn theme_prefix_validation_allows_the_compatibility_namespace() {
        assert!(
            DesignSystemOptions {
                theme_prefix: "--tw-".into(),
                ..DesignSystemOptions::default()
            }
            .validate()
            .is_ok()
        );
        assert!(matches!(
            DesignSystemOptions {
                theme_prefix: "tw".into(),
                ..DesignSystemOptions::default()
            }
            .validate(),
            Err(CompileDiagnostic::InvalidPropertyPrefix { option, value })
                if option == "themePrefix" && value == "tw"
        ));
        assert!(matches!(
            DesignSystemOptions {
                theme_prefix: "--sw-".into(),
                ..DesignSystemOptions::default()
            }
            .validate(),
            Err(CompileDiagnostic::ConflictingPropertyPrefixes { .. })
        ));
    }

    #[test]
    fn invalid_options_identify_the_option_and_value() {
        let options = DesignSystemOptions {
            prefix: "bad".into(),
            ..DesignSystemOptions::default()
        };
        assert!(
            matches!(options.validate(), Err(CompileDiagnostic::InvalidPropertyPrefix {
                option,
                value,
            }) if option == "prefix" && value == "bad")
        );
        let options = DesignSystemOptions {
            tailwind_prefix: "bad".into(),
            ..DesignSystemOptions::default()
        };
        assert!(
            matches!(options.validate(), Err(CompileDiagnostic::InvalidPropertyPrefix {
                option,
                value,
            }) if option == "tailwindPrefix" && value == "bad")
        );
        let options = DesignSystemOptions {
            tailwind_prefix: "--sw-".into(),
            ..DesignSystemOptions::default()
        };
        assert!(
            matches!(options.validate(), Err(CompileDiagnostic::ConflictingPropertyPrefixes {
                prefix,
            }) if prefix == "--sw-")
        );
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn swapped_prefixes_are_rewritten_simultaneously() {
        let options = DesignSystemOptions {
            prefix: "--tw-".into(),
            tailwind_prefix: "--sw-".into(),
            ..DesignSystemOptions::default()
        };
        assert!(options.validate().is_ok());
        assert_eq!(
            options.rewrite_generated("var(--sw-a) var(--tw-b) --other-c"),
            "var(--tw-a) var(--sw-b) --other-c"
        );
    }
}

/// Strategy for ordering resolved utilities.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UtilityOrderMode {
    /// Uses definition positions after CSS imports are expanded.
    #[default]
    Authored,
    /// Uses Tailwind property metadata and candidate comparison.
    Tailwind,
}
