use super::{AdapterKind, FrameworkSpec};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Svelte,
    macro_import: "stillwind/svelte",
    runtime_import: "stillwind/svelte/runtime",
    class_property: "class",
    projected_input: true,
    normalize: super::preserve,
};
