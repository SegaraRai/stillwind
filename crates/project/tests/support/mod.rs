//! Inputs shared by the project session integration tests.
//!
//! Each integration-test binary uses a subset of these constructors.
//!
//! Modules:
//!
//! - [`analysis`] parses source fixtures.
//! - [`snapshot`] builds design-system fixtures.

#![allow(dead_code, unused_imports)]

mod analysis;
mod snapshot;

pub(super) use analysis::{analysis, module};
pub(super) use snapshot::{brand_snapshot, snapshot};
