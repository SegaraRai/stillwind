use super::{AdapterKind, FrameworkSpec};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::React,
    macro_import: "stillwind/react",
    runtime_import: "stillwind/react/runtime",
    class_property: "className",
    projected_input: false,
    normalize: super::preserve,
};
