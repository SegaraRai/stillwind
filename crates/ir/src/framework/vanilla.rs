//! Macro policy for direct DOM consumers.

use super::{AdapterKind, FrameworkSpec, OutputFormat, preserve};

pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Vanilla,
    macro_import: "stillwind/vanilla",
    output: OutputFormat::ClassObject,
    projected_input: false,
    normalize: preserve,
};
