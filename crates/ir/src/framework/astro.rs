//! Astro adapter policy.
//!
//! Astro templates are converted before analysis, but the converter preserves user
//! expressions, so the policy is the identity mapping.

use super::{AdapterKind, FrameworkSpec, StyleFormat, preserve};

/// Astro receives custom properties as a serialized style string.
pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Astro,
    macro_import: "stillwind/astro",
    runtime_import: "stillwind/astro/runtime",
    class_property: "class",
    style_format: StyleFormat::String,
    projected_input: true,
    normalize: preserve,
};
