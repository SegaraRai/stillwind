//! Language-neutral intermediate representation shared by all source frontends.
//!
//! This crate is the seam between a frontend (`js-oxc` today, other parsers later) and
//! the backend. It defines three things: the host input contract ([`SourceInput`]), the
//! extracted module model ([`ModuleIr`]), and the two-phase analysis contract
//! ([`SourceAnalyzer`]: `ModuleScan` before TypeScript facts, [`ModuleAnalysis`] after
//! them).
//!
//! The IR stores original source spans rather than re-printed JavaScript. A frontend
//! that parses a virtual or shadow source must map every span back to the editable
//! original before handing the IR on; [`ModuleIr::try_map_spans`] performs that rewrite
//! transactionally, and the frontend keeps ownership of correspondence validation and
//! of its own provenance records.
//!
//! [`validate_composition`] and [`ordered_style_calls`] are pure checks over the IR:
//! they report dependency cycles and produce a deterministic producer order, but they
//! never emit CSS or resolve candidates.

#![warn(missing_docs)]
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

mod analysis;
mod composition;
mod framework;
mod input;
mod model;
mod spans;

pub use analysis::*;
pub use composition::*;
pub use framework::*;
pub use input::*;
pub use model::*;
