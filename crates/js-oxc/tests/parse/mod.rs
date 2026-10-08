//! Integration of import binding, declarations, reference resolution and module summaries.
//!
//! Every test here goes through the public [`parse`] entry point with real source
//! text, because most of the contracts are about what a module looks like from
//! the outside: which framework was selected, which spans are recorded, which
//! exports denote assets, and which shapes are rejected.
//!
//! `diagnostics` checks parser failures and transactional ir span mapping.
//! `imports` checks macro binding, import editing, and semantic reads.
//! `expressions` checks class, object, array, and variant expression lowering.
//! `assets` checks asset declarations, uses, and exported identities.

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
