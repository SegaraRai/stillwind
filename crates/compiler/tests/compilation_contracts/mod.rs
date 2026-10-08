//! Compilation contracts for emitted CSS, refs, motion, and source attribution.
//!
//! `support` compiles each case. `emission` checks stylesheet output, `motion` checks
//! animations and transitions, `provenance` checks source attribution, `refs` checks
//! ref behavior, `style_analysis` checks composition diagnostics, and `utility_lint`
//! checks policies on direct utility uses.

mod emission;
mod motion;
mod provenance;
mod refs;
mod style_analysis;
mod support;
mod utility_lint;

use support::compile;
