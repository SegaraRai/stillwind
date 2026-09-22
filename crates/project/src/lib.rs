//! Mutable artifact ownership, reloads, and explicit stylesheet coverage and placement.
//!
//! This crate is the orchestration layer a host (Vite, CLI watch, LSP workspace)
//! embeds. It owns the current snapshot, the analyses and compiled artifacts keyed by
//! module, the reverse dependency index, and the assembled project stylesheet. It does
//! not parse, compile, or render by itself: every such operation is delegated to the
//! immutable services in [`stillwind_compiler`] and [`stillwind_design_system`], which
//! stay cheap to clone and safe to use concurrently.
//!
//! Two state rules define the behaviour of the whole layer. A revision is published as
//! one coherent set, so a failed watch reload retains the previous snapshot, artifacts,
//! and CSS rather than mixing new registries with stale output. A failed design-system
//! load in build mode clears artifacts even if a previous revision existed. A build
//! reload with module errors publishes the new revision with successful modules only.
//! And coverage is only as good as the host says: observing a transform never proves
//! that the module graph is complete, so a selection must carry an explicit
//! [`StylesheetCoverage`].
//!
//! `session` publishes revisions; `project_stylesheet` tracks shared rule ownership;
//! `dependencies` indexes consumers; `stylesheet_plan` validates producer coverage;
//! `composition_lint` reports composition overlaps whose order the contract leaves open.

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

mod composition_lint;
mod dependencies;
mod project_stylesheet;
mod reserved;
mod session;
mod stylesheet_plan;

pub use dependencies::DependencyIndex;
pub use project_stylesheet::ProjectStylesheet;
pub use reserved::ReservedNames;
pub use session::*;
pub use stylesheet_plan::*;
