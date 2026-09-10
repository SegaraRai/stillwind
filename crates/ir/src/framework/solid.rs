use super::{AdapterKind, FrameworkSpec};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Solid,
    macro_import: "stillwind/solid",
    runtime_import: "stillwind/solid/runtime",
    class_property: "class",
    projected_input: false,
    normalize: super::preserve,
};
