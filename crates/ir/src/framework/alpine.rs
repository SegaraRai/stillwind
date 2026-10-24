//! Macro policy for Alpine directives.

use super::{AdapterKind, FrameworkSpec, OutputFormat, preserve};

pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Alpine,
    macro_import: "stillwind/alpine",
    output: OutputFormat::ClassObject,
    projected_input: false,
    normalize: preserve,
};
