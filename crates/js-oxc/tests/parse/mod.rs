//! Integration of import binding, declarations, reference resolution, and module
//! summaries.
//!
//! Every test goes through the public [`parse`] entry point with real source text,
//! because most contracts are about what a module looks like from the outside: which
//! framework is selected, which spans are recorded, which exports denote assets, and
//! which shapes are rejected.
//!
//! Modules:
//!
//! - [`assets`] checks asset declarations, uses, and exported identities.
//! - [`diagnostics`] checks parser failures and transactional IR span mapping.
//! - [`expressions`] checks the lowering of class, object, array, and variant
//!   expressions.
//! - [`imports`] checks macro binding, import editing, and semantic reads.

use std::collections::BTreeMap;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintFinding};
use stillwind_ir::{
    AdapterKind, ClassExpression, FRAMEWORKS, GroupEntry, RuleExpression, ScriptLanguage,
    ScriptModuleKind, StyleCall, StyleInput, StyleVariant,
};
use stillwind_js_oxc::parse;
use stillwind_semantic::{AssetKind, ExportedValue, ImportBinding};
use stillwind_source::{FileId, ModuleId, SourceFile, Span};

mod assets;
mod diagnostics;
mod expressions;
mod imports;
