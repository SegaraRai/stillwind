//! Custom-property discovery, registration metadata, and usage classification.
//!
//! Registration decides which properties the compiler may emit, and usage decides which
//! registrations an output actually needs. Authored `@property` rules are never
//! rewritten or removed by these functions; they only report names and syntax
//! descriptors.
//!
//! Modules:
//!
//! - [`references`] tokenizes CSS text to find `--name` identifiers.
//! - [`registration`] parses compiler-owned `@stillwind-property` blocks and the names
//!   and syntaxes of authored `@property` rules.
//! - [`usage`] classifies an emitted stylesheet into declared properties, `var()`
//!   references, and declaring properties.

mod references;
mod registration;
mod usage;

pub use references::custom_property_references;
pub use registration::{
    CustomPropertyActivation, CustomPropertyDefinition, extract_authored_property_names,
    extract_authored_property_syntaxes, parse_property_definitions,
};
pub use usage::{CustomPropertyUsage, PropertyUsageError, custom_property_usage};
