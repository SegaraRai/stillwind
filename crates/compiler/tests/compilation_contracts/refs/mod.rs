//! Ref names, projections, scope lists, and provider diagnostics.
//!
//! `conditions` checks ref conditions, scope combinations, and selector targets.
//! `names` checks labels and the names allocated to declarations.
//! `projections` checks static and runtime projections and explicit css roles.
//! `providers` checks attachment requirements and conflicting provider properties.
//! `support` supplies the builders shared by these cases.

use std::collections::BTreeMap;

use stillwind_compiler::{CompileResult, Compiler, CompilerOptions, GeneratedSymbolKind};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, SymbolicValue, TypeFact};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};
use stillwind_test_support::css_source;

use super::compile;

mod conditions;
mod names;
mod projections;
mod providers;
mod support;

use support::marker;
