//! Loader and snapshot behavior through the public protocol.
//!
//! The cases pin that response order and batch shape never change the resulting
//! snapshot, that opaque names identify sources, that cycles and invalid statements are
//! skipped and recorded, that conflicting content is reported, and that token spans
//! point into the buffer that the host supplied. A package import stays a host request,
//! and a parse failure keeps a typed `#[source]` cause and a span in the supplied
//! buffer.
//!
//! Modules:
//!
//! - [`diagnostics`] checks source failures and authored resource provenance.
//! - [`protocol`] checks host requests, batches, traversal order, and graph edges.
//! - [`snapshots`] checks pinned revisions, resolved configuration, and registry
//!   contents.
//! - [`support`] supplies the builders that these cases share.

use std::sync::Arc;

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_source::{CssSourceName, FileId, SourceFile, SourceRegistry, SourceUri};
use stillwind_syntax::parse_candidate;

use super::*;
use crate::{
    CustomUtility, DefinitionOrigin, DesignSystemError, DesignSystemOptions, DesignSystemSnapshot,
    resolve_variant,
};
use crate::{SkipReason, SkippedCss};

mod diagnostics;
mod protocol;
mod snapshots;
mod support;

use support::{input, requests};
