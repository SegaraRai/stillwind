//! Macro policy for Lit templates.

use super::{AdapterKind, ForwardSupport, FrameworkSpec, OutputFormat, preserve};

pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Lit,
    macro_import: "stillwind/lit",
    output: OutputFormat::ClassObject,
    forward: ForwardSupport::Unavailable,
    projected_input: false,
    normalize: preserve,
};
