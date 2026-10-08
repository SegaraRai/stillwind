//! In-crate integration tests for theme parsing and snapshot construction.
//!
//! The crate's fixture parser expands package imports through
//! `stillwind_test_support::css_source` before parsing themes and builtin definitions.

use stillwind_source::{CssSourceName, FileId, SourceFile};

use crate::*;

mod theme;
