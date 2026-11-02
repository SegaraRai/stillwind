//! Captures source occurrences and renders isolated CSS previews without publishing
//! artifacts.
//!
//! Create a batch through [`crate::ModuleCompiler::editors`], inspect
//! [`ModuleEditor::sites`], and use [`ModuleEditor::preview_input`] to render decoded
//! replacement text at the original UTF-8 byte start of a captured literal.
//!
//! Modules:
//!
//! - [`capture`] reuses compiler emission with display names.
//! - [`input`] selects or replaces one input and keeps its enclosing variants and
//!   JavaScript conditions.
//! - [`names`] reserves the names of reusable declarations across the captured modules.
//! - [`resources`] restores authored URLs for display.
//! - [`runtime`] finds the view-transition keys that require runtime values.

mod capture;
mod input;
mod names;
mod resources;
mod runtime;

pub use capture::{EditorSite, EditorSiteKind, ModuleEditor};

pub(crate) use names::reserve_preview_names;
pub(crate) use resources::preview_resource_urls;

use capture::EditorPlan;
