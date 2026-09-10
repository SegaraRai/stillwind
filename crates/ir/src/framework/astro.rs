use super::{AdapterKind, FrameworkSpec};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Astro,
    macro_import: "stillwind/astro",
    class_property: "class",
    projected_input: false,
    normalize: super::preserve,
};
