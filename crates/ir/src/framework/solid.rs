use super::{AdapterKind, FrameworkSpec, StyleFormat};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Solid,
    macro_import: "stillwind/solid",
    runtime_import: "stillwind/solid/runtime",
    class_property: "class",
    style_format: StyleFormat::Object,
    projected_input: false,
    normalize: super::preserve,
};
