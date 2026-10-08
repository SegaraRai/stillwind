//! Inputs shared by the project session integration tests.
//!
//! `snapshot` builds design-system fixtures; `analysis` parses source fixtures.
//! Each integration-test binary uses a subset of these constructors.

#![allow(dead_code, unused_imports)]

mod analysis;
mod snapshot;

pub(super) use analysis::{analysis, module};
pub(super) use snapshot::{brand_snapshot, snapshot};
