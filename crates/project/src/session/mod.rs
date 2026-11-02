//! Publishes analyses, compiled modules, and project CSS as coherent revisions.
//!
//! Modules:
//!
//! - [`consumers`] indexes published composition edges by their base module.
//! - [`diagnostics`] keeps the latest reports and rejected sources.
//! - [`names`] checks generated symbols against document namespaces without changing
//!   publication state.
//! - [`state`] owns update and reload transitions through [`ProjectSession`].

mod consumers;
mod diagnostics;
mod names;
mod state;

pub use state::{ModuleUpdate, ProjectSession, SessionMode, SessionReload, SessionReloadError};

use consumers::CompositionConsumers;
use diagnostics::DiagnosticState;
use names::{DocumentNames, ModuleNames, SymbolIndex};
