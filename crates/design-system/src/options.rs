//! Host configuration of the two CSS custom-property prefixes.
//!
//! The options belong to the snapshot: they are validated once while a snapshot is
//! built, so evaluation can rewrite identifiers without re-checking them. CSS-authored
//! settings override the host defaults during loading, which is why the loader
//! validates the resolved pair again.

use serde::{Deserialize, Serialize};

use stillwind_diagnostics::CompileDiagnostic;

/// The `--sw-` and `--tw-` namespaces used by generated custom properties.
///
/// Deserialization uses camelCase fields and fills omitted fields from [`Default`], so
/// a host may send only the prefix it changes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DesignSystemOptions {
    /// Prefix of Stillwind-owned generated properties.
    pub prefix: String,
    /// Prefix of the Tailwind-compatibility namespace, which stays distinct from
    /// `prefix` even when both are customized.
    pub tailwind_prefix: String,
}

impl Default for DesignSystemOptions {
    fn default() -> Self {
        Self {
            prefix: "--sw-".to_owned(),
            tailwind_prefix: "--tw-".to_owned(),
        }
    }
}

impl DesignSystemOptions {
    /// Validates that generated and Tailwind-compatible namespaces are distinct CSS prefixes.
    ///
    /// A prefix must start with `--`, end with `-`, and span at least four bytes. The
    /// error preserves the offending field name and supplied value, or the common prefix
    /// when the two namespaces collide. Validation does not modify either value.
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
        ] {
            if !value.starts_with("--") || !value.ends_with('-') || value.len() < 4 {
                return Err(CompileDiagnostic::InvalidPropertyPrefix {
                    option: name.into(),
                    value: value.into(),
                });
            }
        }
        if self.prefix == self.tailwind_prefix {
            return Err(CompileDiagnostic::ConflictingPropertyPrefixes {
                prefix: self.prefix.clone(),
            });
        }
        Ok(())
    }

    /// Rewrites literal `--sw-` and `--tw-` markers in generated CSS to the configured
    /// prefixes.
    ///
    /// Canonical markers are first replaced by private sentinels, then by the configured
    /// prefixes. This keeps swapped or overlapping prefixes from rewriting one another's
    /// output. The caller supplies generated text without these reserved sentinel strings;
    /// the operation is literal substitution rather than CSS-aware identifier rewriting.
    pub fn rewrite_generated(&self, value: &str) -> String {
        value
            .replace("--sw-", "\u{e000}stillwind-prefix\u{e001}")
            .replace("--tw-", "\u{e000}tailwind-prefix\u{e001}")
            .replace("\u{e000}stillwind-prefix\u{e001}", &self.prefix)
            .replace("\u{e000}tailwind-prefix\u{e001}", &self.tailwind_prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileDiagnostic, DesignSystemOptions};

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
        };
        assert!(options.validate().is_ok());
        assert_eq!(
            options.rewrite_generated("var(--sw-a) var(--tw-b) --other-c"),
            "var(--tw-a) var(--sw-b) --other-c"
        );
    }
}
