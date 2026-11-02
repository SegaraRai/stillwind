//! Compilation contracts for emitted CSS, refs, motion, and source attribution.
//!
//! Modules:
//!
//! - [`emission`] checks stylesheet output.
//! - [`logical_stylesheets`] checks the logical axis utilities.
//! - [`motion`] checks animations and transitions.
//! - [`provenance`] checks source attribution.
//! - [`refs`] checks ref behavior.
//! - [`shared_assets`] checks shared-value selection, output modes, and the boundary
//!   with theme-token policies.
//! - [`style_analysis`] checks composition diagnostics.
//! - [`support`] compiles each case.
//! - [`utility_lint`] checks policies on direct utility uses.

mod emission;
mod logical_stylesheets;
mod motion;
mod provenance;
mod refs;
mod shared_assets;
mod style_analysis;
mod support;
mod utility_lint;

use support::compile;
