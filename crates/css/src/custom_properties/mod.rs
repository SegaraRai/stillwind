//! Custom-property discovery, registration metadata, and usage classification.
//!
//! Three related questions live here. [`references`] finds `--name` occurrences
//! lexically, [`registration`] parses compiler-owned `@stillwind-property` blocks and
//! authored `@property` names, and [`usage`] classifies an emitted stylesheet into
//! declared properties, `var()` references and declaring properties. The split matters:
//! registration decides what the compiler may emit, while usage decides which
//! registrations an output actually needs.
//!
//! Authored `@property` rules are never rewritten or removed by these
//! functions; they only report names.

pub(super) mod references;
pub(super) mod registration;
pub(super) mod usage;

pub use references::custom_property_references;
pub use registration::{
    CustomPropertyActivation, CustomPropertyDefinition, extract_authored_property_names,
    extract_property_definitions, parse_property_definitions,
};
pub use usage::{
    CustomPropertyUsage, PropertyUsageError, custom_property_usage, referenced_custom_properties,
    used_custom_properties,
};
