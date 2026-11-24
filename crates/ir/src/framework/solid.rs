//! Solid adapter policy.

use super::{AdapterKind, ForwardSupport, FrameworkSpec, OutputFormat, preserve};

/// Solid passes `class` and materializes custom properties as an object.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Solid,
    macro_import: "stillwind/solid",
    output: OutputFormat::ClassObject,
    forward: ForwardSupport::Available,
    projected_input: false,
    normalize: preserve,
};
