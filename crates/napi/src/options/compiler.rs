//! Converts host compiler overrides and validates effective configuration.

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::CompilerOptions;
use stillwind_design_system::UtilityOrderMode;

use super::NativeLintOptions;

/// Optional host overrides for prefixes, class identity, and conditional CSS.
/// Conversion validates prefixes; omitted fields retain [`CompilerOptions`] defaults.
#[napi(object)]
#[derive(Default)]
pub struct NativeCompilerOptions {
    /// Project lint settings. Omitted fields retain compiler defaults.
    pub lint: Option<NativeLintOptions>,
    /// Prefix for Stillwind-owned identifiers. Must start with `--` and end with `-`.
    pub prefix: Option<String>,
    /// Prefix for the retained Tailwind-compatibility namespace.
    pub tailwind_prefix: Option<String>,
    /// Ordering strategy: authored or tailwind. Defaults to authored.
    pub utility_order: Option<String>,
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
            utility_order: match input.utility_order.as_deref() {
                None | Some("authored") => UtilityOrderMode::Authored,
                Some("tailwind") => UtilityOrderMode::Tailwind,
                Some(_) => {
                    return Err(Error::from_reason(
                        "utilityOrder must be authored or tailwind",
                    ));
                }
            },
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
