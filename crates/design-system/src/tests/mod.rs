//! In-crate integration tests for theme parsing and snapshot construction.
//!
//! The fixture parser of the crate expands package imports through
//! [`stillwind_test_support::css_source`] before it parses themes and the package's
//! definitions.
//!
//! Modules:
//!
//! - [`theme`] checks theme parsing against the real `stillwind` package.

use stillwind_source::{CssSourceName, FileId, SourceFile};

use crate::*;

mod theme;
