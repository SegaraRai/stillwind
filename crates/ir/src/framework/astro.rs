use super::{AdapterKind, FrameworkSpec, StyleFormat};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Astro,
    macro_import: "stillwind/astro",
    runtime_import: "stillwind/astro/runtime",
    class_property: "class",
    style_format: StyleFormat::String,
    projected_input: true,
    normalize: super::preserve,
};
