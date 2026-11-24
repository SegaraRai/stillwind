//! Macro policy for direct DOM consumers.

use super::{AdapterKind, ForwardSupport, FrameworkSpec, OutputFormat, preserve};

pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Vanilla,
    macro_import: "stillwind/vanilla",
    output: OutputFormat::ClassObject,
    forward: ForwardSupport::Unavailable,
    projected_input: false,
    normalize: preserve,
};
