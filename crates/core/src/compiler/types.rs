use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use stillwind_source::{CompileError, LintFinding, SourceError, SourceFile, Span};

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

#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("invalid stillwind transform span: {0}")]
    InvalidSpan(#[source] SourceError),
    #[error("overlapping stillwind transform span {span:?}, previous start {previous_start}")]
    Overlap { span: Span, previous_start: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedClass {
    pub class_name: String,
    pub call_span: Span,
    pub guard: Option<Span>,
    pub guard_when_true: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PropsTransform {
    pub call_span: Span,
    pub replacement: String,
}

/// A generated CSS identifier with the canonical identity used to allocate it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedSymbol {
    pub name: String,
    pub kind: String,
    pub identity: String,
    pub source_path: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    pub css: String,
    /// Unwrapped utility rules used to assemble one stylesheet for a module graph.
    #[serde(skip)]
    pub module_css: String,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub transforms: Vec<PropsTransform>,
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub lint_policy: BTreeMap<String, String>,
}

impl CompileResult {
    /// Applies source-spanned macro replacements from right to left.
    ///
    /// This deliberately keeps the compiler's source mapping contract separate
    /// from any particular bundler. A Vite adapter can use the same plan and
    /// provide a high-resolution source map around these exact byte spans.
    ///
    /// # Errors
    ///
    /// Returns an error when a transform targets another file or overlaps a
    /// previously applied source span.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, TransformError> {
        let mut transforms = self.transforms.clone();
        transforms.sort_by_key(|transform| std::cmp::Reverse(transform.call_span.start.0));

        let mut output = source.text().to_owned();
        let mut previous_start = u32::MAX;
        for transform in transforms {
            source
                .slice(transform.call_span)
                .map_err(TransformError::InvalidSpan)?;
            if transform.call_span.end.0 > previous_start {
                return Err(TransformError::Overlap {
                    span: transform.call_span,
                    previous_start,
                });
            }
            let start = transform.call_span.start.0 as usize;
            let end = transform.call_span.end.0 as usize;
            output.replace_range(start..end, &transform.replacement);
            previous_start = transform.call_span.start.0;
        }
        Ok(output)
    }
}
