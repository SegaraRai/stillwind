//! React adapter policy.

use super::{AdapterKind, FrameworkSpec, StyleFormat, preserve};

/// React macros produce a props object with a `className` entry.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::React,
    macro_import: "stillwind/react",
    runtime_import: "stillwind/react/runtime",
    runtime_import_dev: "stillwind/runtime/dev",
    class_property: "className",
    style_property: "style",
    style_format: StyleFormat::Object,
    projected_input: false,
    normalize: preserve,
};
