//! Caller-driven CSS loading. Source identities are deliberately opaque.
use std::{collections::BTreeMap, sync::Arc};

use cssparser::{Parser, Token};
use stillwind_source::{FileId, SourceFile, Span};

use crate::{CompilerOptions, DesignSystemSnapshot};

mod protocol;
pub use protocol::*;
mod state;
pub use state::{DesignSystemLoadError, DesignSystemLoader};
mod expansion;
use expansion::ExpandedSource;
mod imports;
pub use imports::CssImportError;
use imports::Import;
use imports::imports;

#[cfg(test)]
mod tests;
