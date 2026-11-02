//! Extracts ref declarations, finite receivers, and explicit provider and consumer
//! uses.
//!
//! Lowering keeps canonical identities and original source coordinates, so compiler
//! composition analysis can check provider attachments after bases are resolved.
//!
//! Modules:
//!
//! - [`definitions`] validates `style.ref()` and ref-condition syntax.
//! - [`projections`] lowers `asAnchor`, `positionAnchor`, `asContainer`, and
//!   anchor-scope lists to IR.
//! - [`references`] resolves receiver expressions to declarations or checker facts.

mod definitions;
mod projections;
mod references;
