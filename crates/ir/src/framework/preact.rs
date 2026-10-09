//! Preact adapter policy. It matches React except for the class property name.

use super::{AdapterKind, FrameworkSpec, StyleFormat, preserve};

/// Preact uses `class`, not `className`.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Preact,
    macro_import: "stillwind/preact",
    runtime_import: "stillwind/runtime/preact",
    runtime_import_dev: "stillwind/runtime/preact/development",
    class_property: "class",
    style_property: "style",
    style_format: StyleFormat::Object,
    projected_input: false,
    normalize: preserve,
};
