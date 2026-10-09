//! Astro adapter policy.
//!
//! Astro templates are converted before analysis, but the converter preserves user
//! expressions, so the policy is the identity mapping.

use super::{AdapterKind, FrameworkSpec, StyleFormat, preserve};

/// Astro receives custom properties as a serialized style string.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Astro,
    macro_import: "stillwind/astro",
    runtime_import: "stillwind/runtime/astro",
    runtime_import_dev: "stillwind/runtime/astro-development",
    class_property: "class",
    style_property: "style",
    style_format: StyleFormat::String,
    projected_input: true,
    normalize: preserve,
};
