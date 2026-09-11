use super::{AdapterKind, FrameworkSpec, StyleFormat};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::React,
    macro_import: "stillwind/react",
    runtime_import: "stillwind/react/runtime",
    class_property: "className",
    style_format: StyleFormat::Object,
    projected_input: false,
    normalize: super::preserve,
};
