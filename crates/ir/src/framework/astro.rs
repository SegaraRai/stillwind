//! Astro adapter policy.
//!
//! Astro templates are converted before analysis, but the converter preserves user
//! expressions, so the policy is the identity mapping.

use super::{AdapterKind, FrameworkSpec, OutputFormat, preserve};

/// Astro receives custom properties as a serialized style string.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Astro,
    macro_import: "stillwind/astro",
    output: OutputFormat::ClassString,
    projected_input: true,
    normalize: preserve,
};
