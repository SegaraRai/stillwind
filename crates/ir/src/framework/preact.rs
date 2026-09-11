use super::{AdapterKind, FrameworkSpec, StyleFormat};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Preact,
    macro_import: "stillwind/preact",
    runtime_import: "stillwind/preact/runtime",
    class_property: "class",
    style_format: StyleFormat::Object,
    projected_input: false,
    normalize: super::preserve,
};
