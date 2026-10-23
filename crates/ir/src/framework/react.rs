//! React adapter policy.

use super::{AdapterKind, FrameworkSpec, OutputFormat, preserve};

/// React macros produce a props object with a `className` entry.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::React,
    macro_import: "stillwind/react",
    output: OutputFormat::ClassNameObject,
    projected_input: false,
    normalize: preserve,
};
