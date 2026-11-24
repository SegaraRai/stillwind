//! Macro policy for Qwik JSX.

use super::{AdapterKind, ForwardSupport, FrameworkSpec, OutputFormat, preserve};

pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Qwik,
    macro_import: "stillwind/qwik",
    output: OutputFormat::ClassObject,
    forward: ForwardSupport::Available,
    projected_input: false,
    normalize: preserve,
};
