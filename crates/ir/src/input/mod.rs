//! Validated host inputs for direct scripts and converted component documents.
//!
//! Build a [`DirectSource`] or [`ProjectedSource`] before creating a [`SourceInput`].
//! Their constructors validate source identities, formats, and import metadata.
//! Original framework markup remains opaque to the core. Map loading and expression
//! restoration belong to the analyzer and can still produce diagnostics.
//!
//! [`SourceInput::parser_source`] supplies parser bytes; [`SourceInput::document`]
//! identifies the editable source to which published spans must refer.
//!
//! `prepared` combines documents and projections into module inputs. `document`
//! defines editable documents and direct parser settings. `projection` validates
//! converter output and import metadata against component documents.

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
