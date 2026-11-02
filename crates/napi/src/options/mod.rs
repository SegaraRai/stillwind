//! JavaScript-facing compiler options.
//!
//! Modules:
//!
//! - [`compiler`] validates compiler overrides and applies defaults.
//! - [`lint`] contains the aggregate options generated from the Rust lint catalog.
//! - [`lint_policy`] converts the options of individual lint rules.

mod compiler;
mod lint;
mod lint_policy;

pub(super) use compiler::{NativeCompilerOptions, compiler_options};
pub(super) use lint_policy::NativeCanonicalUtilitiesOptions;

use lint::NativeLintOptions;
use lint_policy::{
    NativeArbitrarySpacingOptions, NativeConflictingUtilitiesOptions, NativeImportantOptions,
    NativeLogicalPropertiesOptions, NativeMarkerUseOptions, NativeRestrictedDeclarationsOptions,
    NativeRestrictedLengthUnitsOptions, NativeRestrictedThemeTokensOptions,
    NativeRestrictedUtilitiesOptions, NativeVariantDirectiveOptions, lint_severity,
};
