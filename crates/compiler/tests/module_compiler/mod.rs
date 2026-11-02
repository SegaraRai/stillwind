//! Module compilation through the public API.
//!
//! The cases pin composition ordering and the merging of selected bases, and artifact
//! identity: an options or frontend IR change gives a different key for the same source
//! revision. They also pin structured CSS and transform output against a pinned
//! snapshot, check that invalid IR and byte boundaries return diagnostics instead of
//! panicking, and check that a lint policy never changes generated artifacts.
//!
//! Modules:
//!
//! - [`artifacts`] checks pinned compilation output, invalid IR, and artifact identity.
//! - [`composition`] checks composition ordering and provider contributions.
//! - [`naming`] checks development mode and host name generators.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use stillwind_compiler::{
    ClassContext, CompileResult, Compiler, CompilerOptions, ConditionContext, ConditionHelperName,
    DeclarationContext, GeneratedClass, GeneratedName, GeneratedRule, GeneratedSymbol,
    GeneratedSymbolKind, IdentifierGenerator, IdentifierHash, IdentifierKind, IdentifierMode,
    Identifiers, ModuleCompiler, RefMarker, SourceTransform, StylesheetRenderer, UtilityClass,
};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{
    CompileDiagnostic, CompileError, LintFinding, LintOverrides, LintSeverity, SourceFailureContext,
};
use stillwind_ir::{
    ClassExpression, DirectSource, PreparedSource, ProjectedSource, RuleExpression, ScriptLanguage,
    ScriptModuleKind, SourceAnalyzer, SourceDocument, SourceInput, StyleInput,
};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::TypeFacts;
use stillwind_source::{CssSourceName, FileId, Fingerprint, ModuleId, SourceFile, Span, Spanned};
use stillwind_test_support::css_source;

mod artifacts;
mod composition;
mod naming;
