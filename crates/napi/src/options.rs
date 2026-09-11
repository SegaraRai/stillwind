use napi::Error;
use napi_derive::napi;

use stillwind_compiler::CompilerOptions;

/// Optional host overrides; omitted fields keep the compiler defaults.
#[napi(object)]
#[derive(Default)]
pub struct NativeCompilerOptions {
    pub prefix: Option<String>,
    pub tailwind_prefix: Option<String>,
    pub forbid_tailwind_relationships: Option<bool>,
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

pub(crate) fn compiler_options(
    options: Option<NativeCompilerOptions>,
) -> napi::Result<CompilerOptions> {
    options.unwrap_or_default().try_into()
}
