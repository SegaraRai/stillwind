//! React adapter policy.

use super::{AdapterKind, FrameworkSpec, StyleFormat, preserve};

/// React macros produce a props object with a `className` entry.
pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::React,
    macro_import: "stillwind/react",
    runtime_import: "stillwind/react/runtime",
    class_property: "className",
    style_format: StyleFormat::Object,
    projected_input: false,
    normalize: preserve,
};
