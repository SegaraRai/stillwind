//! Svelte adapter policy.

use super::{AdapterKind, FrameworkSpec, StyleFormat};

/// Svelte receives custom properties as a serialized style string.
pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Svelte,
    macro_import: "stillwind/svelte",
    runtime_import: "stillwind/svelte/runtime",
    class_property: "class",
    style_format: StyleFormat::String,
    projected_input: true,
    normalize: super::preserve,
};
