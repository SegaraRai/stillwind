//! Loader and snapshot behaviour through the public protocol.
//!
//! Pins that response order and batch shape never change the resulting snapshot, that
//! opaque names identify sources, that cycles and invalid statements are skipped and
//! recorded, that conflicting content is reported, and that token spans keep pointing
//! into the buffer the host supplied. A package import stays a host request, and parse
//! failures keep a typed `#[source]` cause and a span in the supplied buffer.
//!
//! `protocol` checks host requests, batches, traversal order, and graph edges.
//! `diagnostics` checks source failures and authored resource provenance.
//! `snapshots` checks pinned revisions, resolved configuration, and registry contents.
//! `support` supplies the builders shared by these cases.

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
