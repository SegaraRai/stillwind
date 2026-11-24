//! Preact adapter policy. It matches React except for the class property name.

use super::{AdapterKind, ForwardSupport, FrameworkSpec, OutputFormat, preserve};

/// Preact uses `class`, not `className`.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Preact,
    macro_import: "stillwind/preact",
    output: OutputFormat::ClassObject,
    forward: ForwardSupport::Available,
    projected_input: false,
    normalize: preserve,
};
