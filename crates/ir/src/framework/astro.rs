use super::{AdapterKind, FrameworkSpec};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Astro,
    macro_import: "stillwind/astro",
    runtime_import: "stillwind/astro/runtime",
    class_property: "class",
    projected_input: true,
    normalize: super::preserve,
};
