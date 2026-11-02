//! Mutable artifact ownership, reloads, and explicit stylesheet coverage and placement.
//!
//! A host (Vite, CLI watch, or an LSP workspace) embeds this crate to orchestrate a
//! project. The crate owns the current snapshot, the analyses and compiled artifacts of
//! each module, the reverse dependency index, and the assembled project stylesheet. It
//! does not parse, compile, or render by itself. It delegates those operations to the
//! immutable services in [`stillwind_compiler`] and [`stillwind_design_system`], which
//! are cheap to clone and safe to use concurrently.
//!
//! These state rules apply to the whole crate:
//!
//! - A revision is published as one coherent set. A failed watch reload keeps the
//!   previous snapshot, artifacts, and CSS instead of mixing new registries with stale
//!   output.
//! - A failed design-system load in build mode clears artifacts, even if a previous
//!   revision exists. A build reload with module errors publishes the new revision with
//!   the successful modules only.
//! - Observing a transform never proves that the module graph is complete, so a
//!   stylesheet selection must carry an explicit [`StylesheetCoverage`].
//!
//! The session passes current style records to the compiler's composition checker and
//! invalidates findings when a base producer changes.
//!
//! Modules:
//!
//! - [`dependencies`] indexes the consumers of each module.
//! - [`project_stylesheet`] tracks the ownership of shared rules.
//! - [`reserved`] tracks authored names.
//! - [`session`] publishes revisions.
//! - [`stylesheet_plan`] validates producer coverage.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod dependencies;
mod project_stylesheet;
mod reserved;
mod session;
mod stylesheet_plan;

pub use dependencies::DependencyIndex;
pub use project_stylesheet::ProjectStylesheet;
pub use reserved::ReservedNames;
pub use session::{ModuleUpdate, ProjectSession, SessionMode, SessionReload, SessionReloadError};
pub use stylesheet_plan::{
    StylesheetCoverage, StylesheetPart, StylesheetPlan, StylesheetPlanError, StylesheetSelection,
};
