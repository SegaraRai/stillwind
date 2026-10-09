//! Captures source occurrences and renders isolated CSS previews without publishing
//! artifacts.
//!
//! Create a batch through [`crate::ModuleCompiler::editors`], inspect
//! [`ModuleEditor::sites`], and use [`ModuleEditor::preview_input`] to render decoded
//! replacement text at a captured literal's original UTF-8 byte start.
//!
//! `capture` reuses compiler emission with display names. `input` retains enclosing
//! variants and JavaScript conditions while selecting or replacing one input.
//! `names` reserves names for reusable declarations across the captured modules.
//! `resources` restores authored URLs for display.
//! `runtime` identifies view-transition keys that require runtime values.

mod capture;
mod input;
mod names;
mod resources;
mod runtime;

pub use capture::{EditorSite, EditorSiteKind, ModuleEditor};

pub(crate) use names::reserve_preview_names;
pub(crate) use resources::preview_resource_urls;

use capture::EditorPlan;
