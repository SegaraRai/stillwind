//! JavaScript-facing compiler options.
//!
//! `compiler` validates compiler overrides and applies defaults. `lint_policy`
//! converts options for individual lint rules. `lint` contains the aggregate
//! generated from the Rust lint catalog.

mod compiler;
mod lint;
mod lint_policy;

pub(super) use compiler::{NativeCompilerOptions, compiler_options};
pub(super) use lint_policy::NativeCanonicalUtilitiesOptions;

use lint::NativeLintOptions;
use lint_policy::{
    NativeArbitrarySpacingOptions, NativeConflictingUtilitiesOptions,
    NativeRestrictedDeclarationsOptions, NativeRestrictedLengthUnitsOptions,
    NativeRestrictedThemeTokensOptions, NativeRestrictedUtilitiesOptions,
    NativeVariantDirectiveOptions, lint_severity,
};
