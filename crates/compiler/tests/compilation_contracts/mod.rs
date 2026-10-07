//! Compilation contracts for emitted CSS, refs, motion, and source attribution.
//!
//! `support` compiles each case. `emission` checks stylesheet output, `motion` checks
//! animations and transitions, `provenance` checks source attribution, `refs` checks
//! ref behavior, and `style_analysis` checks composition diagnostics.

mod emission;
mod motion;
mod provenance;
mod refs;
mod style_analysis;
mod support;

use support::compile;
