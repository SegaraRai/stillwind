//! Validated correspondence between authored expressions and converter code.
//!
//! Framework policy selects candidates. Mapping validation produces one analysis view
//! and AST-specific checker targets. Generated declarations keep their scopes, and only
//! proven synthetic reads are excluded from macro import accounting.
//!
//! Modules:
//!
//! - [`build`] validates syntax and constructs the retained analysis buffer.
//! - [`view`] answers correspondence queries and maps the extracted IR back to original
//!   coordinates.
//! - [`visitor`] proves normalization candidates and checker targets.

mod build;
mod view;
mod visitor;

pub use build::normalize_projection;
pub use view::NormalizedProjection;

use visitor::{NormalizationPlan, collect_plan, merge_omissions};
