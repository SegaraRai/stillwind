//! Framework-specific output that is not CSS.
//!
//! Only React needs generated output today: the ambient declaration file for the
//! utility and variant vocabulary. Keeping it in a per-framework module leaves room for
//! the other adapters without making the CSS pipeline framework aware.

pub(super) mod declarations;
