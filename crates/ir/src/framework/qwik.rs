//! Macro policy for Qwik JSX.

use super::{AdapterKind, FrameworkSpec, OutputFormat, preserve};

pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Qwik,
    macro_import: "stillwind/qwik",
    output: OutputFormat::ClassObject,
    projected_input: false,
    normalize: preserve,
};
