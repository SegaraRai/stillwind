use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CompilerOptionsError {
    #[error("{option} must start with `--` and end with `-`")]
    InvalidPrefix { option: &'static str, value: String },
    #[error("prefix and tailwindPrefix must be different")]
    ConflictingPrefixes { prefix: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    pub prefix: String,
    pub tailwind_prefix: String,
    pub forbid_tailwind_relationships: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            prefix: "--sw-".to_owned(),
            tailwind_prefix: "--tw-".to_owned(),
            forbid_tailwind_relationships: false,
        }
    }
}

impl CompilerOptions {
    /// Validates that generated and Tailwind-compatible namespaces are distinct CSS prefixes.
    ///
    /// # Errors
    ///
    /// Returns an error when a prefix is malformed or both namespaces are identical.
    pub fn validate(&self) -> Result<(), CompilerOptionsError> {
        for (name, value) in [
            ("prefix", self.prefix.as_str()),
            ("tailwindPrefix", self.tailwind_prefix.as_str()),
        ] {
            if !value.starts_with("--") || !value.ends_with('-') || value.len() < 4 {
                return Err(CompilerOptionsError::InvalidPrefix {
                    option: name,
                    value: value.into(),
                });
            }
        }
        if self.prefix == self.tailwind_prefix {
            return Err(CompilerOptionsError::ConflictingPrefixes {
                prefix: self.prefix.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn rewrite_generated(&self, value: &str) -> String {
        value
            .replace("--sw-", "\u{e000}stillwind-prefix\u{e001}")
            .replace("--tw-", "\u{e000}tailwind-prefix\u{e001}")
            .replace("\u{e000}stillwind-prefix\u{e001}", &self.prefix)
            .replace("\u{e000}tailwind-prefix\u{e001}", &self.tailwind_prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::{CompilerOptions, CompilerOptionsError};

    #[test]
    fn invalid_options_identify_the_option_and_value() {
        let options = CompilerOptions {
            prefix: "bad".into(),
            ..CompilerOptions::default()
        };
        assert!(
            matches!(options.validate(), Err(CompilerOptionsError::InvalidPrefix { option: "prefix", value }) if value == "bad")
        );
        let options = CompilerOptions {
            tailwind_prefix: "bad".into(),
            ..CompilerOptions::default()
        };
        assert!(
            matches!(options.validate(), Err(CompilerOptionsError::InvalidPrefix { option: "tailwindPrefix", value }) if value == "bad")
        );
        let options = CompilerOptions {
            tailwind_prefix: "--sw-".into(),
            ..CompilerOptions::default()
        };
        assert!(
            matches!(options.validate(), Err(CompilerOptionsError::ConflictingPrefixes { prefix }) if prefix == "--sw-")
        );
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn swapped_prefixes_are_rewritten_simultaneously() {
        let options = CompilerOptions {
            prefix: "--tw-".into(),
            tailwind_prefix: "--sw-".into(),
            ..CompilerOptions::default()
        };
        assert!(options.validate().is_ok());
        assert_eq!(
            options.rewrite_generated("var(--sw-a) var(--tw-b) --other-c"),
            "var(--tw-a) var(--sw-b) --other-c"
        );
    }
}
