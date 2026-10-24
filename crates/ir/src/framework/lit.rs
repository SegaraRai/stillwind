//! Macro policy for Lit templates.

use super::{AdapterKind, FrameworkSpec, OutputFormat, preserve};

pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Lit,
    macro_import: "stillwind/lit",
    output: OutputFormat::ClassObject,
    projected_input: false,
    normalize: preserve,
};
