//! Extracts ref declarations, finite receivers, and explicit provider and consumer
//! uses.
//!
//! `definitions` validates `style.ref()` and ref-condition syntax. `references`
//! resolves receiver expressions to declarations or checker facts. `projections`
//! lowers `asAnchor`, `positionAnchor`, `asContainer`, and anchor-scope lists to IR.
//! Lowering retains canonical identities and original source coordinates so compiler
//! composition analysis can check provider attachments after bases are resolved.

mod definitions;
mod projections;
mod references;
