use super::{AdapterKind, FrameworkSpec};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Preact,
    macro_import: "stillwind/preact",
    runtime_import: "stillwind/preact/runtime",
    class_property: "class",
    projected_input: false,
    normalize: super::preserve,
};
