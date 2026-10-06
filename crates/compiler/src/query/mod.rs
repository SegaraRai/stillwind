//! Editor queries resolve candidates against one immutable design-system snapshot.
//!
//! [`DesignSystemQuery`] pins each answer to a revision so hosts can discard stale
//! results. [`resolution`] generates candidate previews and diagnostics; [`ordering`]
//! compares candidates using compiler order. [`completion`] offers utility, variant,
//! and theme labels, and [`canonical`] finds equivalent spellings under the active lint
//! policy. [`theme`] exposes theme values and resolves colors for editor decorations.

use std::cell::RefCell;
use std::collections::BTreeMap;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::LintSettings;

use crate::CompilerOptions;

mod canonical;
mod completion;
mod ordering;
mod resolution;
mod theme;

pub use completion::{CompletionInput, CompletionItem};
pub use resolution::{CandidateHover, CandidateResolution};

/// Answers candidate questions against one immutable snapshot.
///
/// Holding the snapshot rather than a project keeps editor answers independent of
/// module state and deterministic for as long as the revision does not change.
#[derive(Clone, Debug)]
pub struct DesignSystemQuery {
    canonical_cache: RefCell<BTreeMap<String, Option<String>>>,
    /// Immutable revision against which every answer from this query object is
    /// computed.
    design_system: DesignSystemSnapshot,
    /// Compiler configuration used for preview generation and name allocation.
    options: CompilerOptions,
}

impl DesignSystemQuery {
    /// Creates a query pinned to `design_system`.
    ///
    /// The options' prefixes are resolved against the snapshot so rendered previews
    /// match what a compilation would emit.
    pub fn new(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let options = options.resolved(&design_system);
        Self {
            canonical_cache: RefCell::new(BTreeMap::new()),
            design_system,
            options,
        }
    }

    /// Returns effective lint settings after stylesheet severity and project overrides.
    pub fn lint_settings(&self) -> LintSettings {
        self.options.lint_settings(&self.design_system)
    }
}

/// Finds a delimiter outside arbitrary values, selectors, functions, and escaped text.
fn last_top_level(value: &str, delimiter: char) -> Option<usize> {
    let mut depth = 0_usize;
    let mut escaped = false;
    let mut result = None;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            character if character == delimiter && depth == 0 => result = Some(index),
            _ => {}
        }
    }
    result
}
