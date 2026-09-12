//! JavaScript-facing compiler options.
//!
//! Every field is optional and falls back to the Rust default, so a host can pass `{}`
//! and get the documented defaults. Values are validated here, which turns a bad prefix
//! or namespace into a JavaScript error before any source is read.

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::CompilerOptions;

/// Optional host overrides; omitted fields keep the compiler defaults.
#[napi(object)]
#[derive(Default)]
pub struct NativeCompilerOptions {
    /// Prefix for Stillwind-owned identifiers. Must start with `--` and end with `-`.
    pub prefix: Option<String>,
    /// Prefix for the retained Tailwind-compatibility namespace.
    pub tailwind_prefix: Option<String>,
    /// Whether theme relationships that only Tailwind needs are rejected.
    pub forbid_tailwind_relationships: Option<bool>,
    /// Identifier namespace for generated class names.
    pub class_namespace: Option<String>,
}

impl TryFrom<NativeCompilerOptions> for CompilerOptions {
    type Error = Error;

    fn try_from(input: NativeCompilerOptions) -> napi::Result<Self> {
        let defaults = Self::default();
        let options = Self {
            prefix: input.prefix.unwrap_or(defaults.prefix),
            tailwind_prefix: input.tailwind_prefix.unwrap_or(defaults.tailwind_prefix),
            forbid_tailwind_relationships: input
                .forbid_tailwind_relationships
                .unwrap_or(defaults.forbid_tailwind_relationships),
            class_namespace: input.class_namespace.unwrap_or(defaults.class_namespace),
        };
        options
            .validate()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(options)
    }
}

/// Resolves optional host overrides into validated compiler options.
pub(crate) fn compiler_options(
    options: Option<NativeCompilerOptions>,
) -> napi::Result<CompilerOptions> {
    options.unwrap_or_default().try_into()
}
