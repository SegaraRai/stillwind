//! Validated correspondence between authored expressions and converter code.
//!
//! Framework policy selects candidates. Mapping validation produces one analysis
//! view and AST-specific checker targets. Generated declarations keep their scopes;
//! only proven synthetic reads are excluded from macro import accounting.
//!
//! `build` validates syntax and constructs the retained analysis buffer. `visitor`
//! proves normalization candidates and checker targets. `view` owns correspondence
//! queries and maps the extracted IR back to original coordinates.

mod build;
mod view;
mod visitor;

pub use build::normalize_projection;
pub use view::NormalizedProjection;

pub(crate) use build::normalize_with_language;

use visitor::{NormalizationPlan, collect_plan, merge_omissions};
