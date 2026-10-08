//! Projected-input tests: framework scaffolding removal and span identity.
//!
//! Each case builds a converter-shaped input (a shadow module plus a map) and
//! checks what the analyzer produces: which scaffolding was dropped, which spans
//! belong to the original file, and how converter configuration participates in
//! the revision.
//!
//! `correspondence` checks normalization and original correspondence for generated expressions.
//! `input_contracts` checks projection metadata, source revisions, and preparation contracts.
//! `facts` checks semantic identities and facts at original document coordinates.
//! `support` supplies the builders shared by these cases.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LabelScope, LintDiagnostic};
use stillwind_ir::{
    AdapterKind, CheckerRole, ClassExpression, ConverterIdentity, DirectSource, GroupEntry,
    ImportRealm, PreparedSource, ProjectedSource, RefDefinition, RuleExpression, ScriptLanguage,
    ScriptModuleKind, ScriptSyntax, SourceAnalyzer, SourceDocument, SourceFormat, SourceInput,
    SourceInputError, SourceProjection, StyleCall, StyleInput, StyleVariant,
};
use stillwind_js_oxc::{OxcSourceAnalyzer, normalize_projection, parse};
use stillwind_semantic::{
    AssetKind, ExportedValue, IntegrationDiagnostic, SymbolicValue, TypeFact, TypeFacts,
};
use stillwind_source::{
    FileId, Fingerprint, ModuleId, ProjectionMap, SourceFile, SourceProvenance, Span, VolarMapping,
};

mod correspondence;
mod facts;
mod input_contracts;
mod support;

use support::{projected, projected_script, rebuild_projection};
