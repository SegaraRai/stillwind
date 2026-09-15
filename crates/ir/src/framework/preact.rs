//! Preact adapter policy. It matches React except for the class property name.

use super::{AdapterKind, FrameworkSpec, StyleFormat, preserve};

/// Preact uses `class`, not `className`.
pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Preact,
    macro_import: "stillwind/preact",
    runtime_import: "stillwind/preact/runtime",
    runtime_import_dev: "stillwind/runtime/dev",
    class_property: "class",
    style_format: StyleFormat::Object,
    projected_input: false,
    normalize: preserve,
};
