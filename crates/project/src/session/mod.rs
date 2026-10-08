//! Publishes analyses, compiled modules, and project CSS as coherent revisions.
//!
//! `state` owns update and reload transitions through [`ProjectSession`].
//! `diagnostics` retains latest reports and rejected sources. `names` checks generated
//! symbols against document namespaces without changing publication state.

mod diagnostics;
mod names;
mod state;

pub use state::{ModuleUpdate, ProjectSession, SessionMode, SessionReload, SessionReloadError};

use diagnostics::DiagnosticState;
use names::DocumentNames;
