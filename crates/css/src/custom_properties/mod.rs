//! Custom-property discovery, registration metadata, and usage classification.
//!
//! Three related questions live here. [`references`] tokenizes CSS text to find
//! `--name` idents, [`registration`] parses compiler-owned `@stillwind-property` blocks
//! and authored `@property` names and syntaxes, and [`usage`] classifies an emitted
//! stylesheet into declared properties, `var()` references and declaring properties.
//! The split matters: registration decides what the compiler may emit, while usage
//! decides which registrations an output actually needs.
//!
//! Authored `@property` rules are never rewritten or removed by these
//! functions; they only report names and syntax descriptors.

mod references;
mod registration;
mod usage;

pub use references::custom_property_references;
pub use registration::{
    CustomPropertyActivation, CustomPropertyDefinition, extract_authored_property_names,
    extract_authored_property_syntaxes, parse_property_definitions,
};
pub use usage::{CustomPropertyUsage, PropertyUsageError, custom_property_usage};
