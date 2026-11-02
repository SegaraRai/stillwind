//! Validated host inputs for direct scripts and converted component documents.
//!
//! Build a [`DirectSource`] or [`ProjectedSource`] before creating a [`SourceInput`].
//! Their constructors validate source identities, formats, and import metadata.
//! Original framework markup stays opaque to the core. Map loading and expression
//! restoration belong to the analyzer and can still produce diagnostics.
//!
//! [`SourceInput::parser_source`] supplies the parser bytes, and
//! [`SourceInput::document`] identifies the editable source that published spans must
//! refer to.
//!
//! Modules:
//!
//! - [`document`] defines editable documents and direct parser settings.
//! - [`prepared`] combines documents and projections into module inputs.
//! - [`projection`] validates converter output and import metadata against component
//!   documents.

mod document;
mod prepared;
mod projection;

pub use document::{
    DirectSource, ScriptLanguage, ScriptModuleKind, ScriptSyntax, SourceDocument, SourceFormat,
};
pub use prepared::{PreparedSource, SourceInput, SourceInputError};
pub use projection::{
    AttributeQuote, ConverterIdentity, ImportRealm, ProjectedSource, ProjectionRegion,
    SourceProjection,
};

#[cfg(test)]
mod tests;
