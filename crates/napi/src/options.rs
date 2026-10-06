//! JavaScript-facing compiler options.
//!
//! Every field is optional and falls back to the Rust default, so a host can pass `{}`
//! and get the documented defaults. Values are validated here, which turns a bad prefix
//! or namespace into a JavaScript error before any source is read.

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::{
    CanonicalUtilitiesOverride, LintOverrides, LintSeverity, RootFontSize,
};

/// Canonical utility options. All omitted fields preserve the previous setting.
#[napi(object)]
pub struct NativeCanonicalUtilitiesOptions {
    /// Reporting severity: warn, deny, or off.
    pub severity: Option<String>,
    /// Allows pixel spacing to use the active rem scale. Defaults to true.
    pub px_to_rem: Option<bool>,
    /// Positive finite pixels per rem. Defaults to 16.
    pub root_font_size: Option<f64>,
}

/// Project lint fields applied after CSS configuration.
#[napi(object)]
pub struct NativeLintOptions {
    /// Field-wise canonical lint policy overrides.
    pub canonical_utilities: Option<NativeCanonicalUtilitiesOptions>,
    /// Optional sorting diagnostic severity: warn, deny, or off. Detection is off by default.
    pub sort_utilities: Option<String>,
}

impl TryFrom<NativeLintOptions> for LintOverrides {
    type Error = Error;

    fn try_from(input: NativeLintOptions) -> napi::Result<Self> {
        let severity = |value: String| {
            LintSeverity::from_css(&value)
                .ok_or_else(|| Error::from_reason("lint severity must be warn, deny, or off"))
        };
        Ok(Self {
            canonical_utilities: input
                .canonical_utilities
                .map(|config| {
                    Ok::<_, Error>(CanonicalUtilitiesOverride {
                        severity: config.severity.map(severity).transpose()?,
                        px_to_rem: config.px_to_rem,
                        root_font_size: config
                            .root_font_size
                            .map(|value| {
                                RootFontSize::new(value).ok_or_else(|| {
                                    Error::from_reason("rootFontSize must be positive and finite")
                                })
                            })
                            .transpose()?,
                    })
                })
                .transpose()?,
            sort_utilities: input.sort_utilities.map(severity).transpose()?,
        })
    }
}

/// Optional host overrides for prefixes, class identity, and conditional CSS.
/// Conversion validates prefixes; omitted fields retain [`CompilerOptions`] defaults.
#[napi(object)]
#[derive(Default)]
pub struct NativeCompilerOptions {
    /// Project lint overrides. Omitted fields retain stylesheet severity and canonical defaults.
    pub lint: Option<NativeLintOptions>,
    /// Prefix for Stillwind-owned identifiers. Must start with `--` and end with `-`.
    pub prefix: Option<String>,
    /// Prefix for the retained Tailwind-compatibility namespace.
    pub tailwind_prefix: Option<String>,
    /// Identifier namespace for generated class names.
    pub class_namespace: Option<String>,
    /// Emit native CSS `if()` for experimental conditional values and variants.
    /// Defaults to false; comparisons, negated variants, and CSS-wide helper branches
    /// require true.
    pub use_css_if: Option<bool>,
    /// Hash of the host's identifier generators, used to invalidate cached artifacts.
    pub naming_revision: Option<String>,
    /// Number of base-36 digits a built-in generated name keeps from its request hash.
    /// Defaults to 5. Values from 1 to 50 are accepted.
    pub hash_length: Option<u32>,
}

impl TryFrom<NativeCompilerOptions> for CompilerOptions {
    type Error = Error;

    fn try_from(input: NativeCompilerOptions) -> napi::Result<Self> {
        let defaults = Self::default();
        let options = Self {
            lint: input
                .lint
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or_default(),
            prefix: input.prefix.unwrap_or(defaults.prefix),
            tailwind_prefix: input.tailwind_prefix.unwrap_or(defaults.tailwind_prefix),
            class_namespace: input.class_namespace.unwrap_or(defaults.class_namespace),
            use_css_if: input.use_css_if.unwrap_or(defaults.use_css_if),
            naming_revision: input.naming_revision.unwrap_or(defaults.naming_revision),
            hash_length: input.hash_length.map_or(defaults.hash_length, |length| {
                usize::try_from(length).unwrap_or(usize::MAX)
            }),
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
