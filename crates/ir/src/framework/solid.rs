//! Solid adapter policy.

use super::{AdapterKind, FrameworkSpec, StyleFormat, preserve};

/// Solid passes `class` and materializes custom properties as an object.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Solid,
    macro_import: "stillwind/solid",
    runtime_import: "stillwind/runtime/solid",
    runtime_import_dev: "stillwind/runtime/solid-development",
    class_property: "class",
    style_property: "style",
    style_format: StyleFormat::Object,
    projected_input: false,
    normalize: preserve,
};
