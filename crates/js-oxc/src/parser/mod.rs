//! Oxc-backed JS/TS/JSX parsing and the first import-bound `style()` extractor.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
};

use oxc::{
    allocator::Allocator,
    ast::ast::{
        Argument, ArrayExpressionElement, BindingPattern, CallExpression, Declaration,
        ExportAllDeclaration, ExportDeclaration, ExportFromDeclaration, ExportNamedDeclaration,
        Expression, ImportDeclaration, ImportDeclarationSpecifier, JSXElement, ModuleExportName,
        ObjectExpression, ObjectPropertyKind, PropertyKey, PropertyKind, StaticMemberExpression,
        StringLiteral, VariableDeclarator,
    },
    ast_visit::{Visit, walk},
    parser::Parser,
    semantic::{Scoping, SemanticBuilder, SymbolId},
    span::{GetSpan, SourceType},
    syntax::operator::LogicalOperator,
};
use stillwind_ir::{
    AnimationDefinition, AnimationFrame, AssetReference, AttributeEntry, HandleDefinition,
    HandleProjection, ModuleIr, ScopeDefinition, StyleCall, StyleInput, ViewTransitionDefinition,
};
use stillwind_semantic::{
    AssetKind, AssetMetadata, AssetRecord, ExportedValue, IdentityId, ImportBinding, ModuleSummary,
    ReExport, RefAlternative, SymbolicValue, TypeFact,
};
use stillwind_source::{CompileError, LintFinding, SourceFile, Span, Spanned};

mod entry;
pub use entry::*;
mod labels;
use labels::{is_animation_frame, is_marker_label, is_strict_ancestor};
struct MacroExtractor<'a> {
    source: &'a SourceFile,
    scoping: &'a Scoping,
    style_symbols: HashSet<SymbolId>,
    handle_symbols: HashMap<SymbolId, HandleDefinition>,
    scope_symbols: HashMap<SymbolId, ScopeDefinition>,
    handle_uses: Vec<HandleUse>,
    scope_uses: Vec<ScopeUse>,
    handle_definitions: Vec<HandleDefinition>,
    scope_definitions: Vec<ScopeDefinition>,
    animation_symbols: HashMap<SymbolId, Spanned<String>>,
    animation_declarations: Vec<AnimationDefinition>,
    view_transition_symbols: HashMap<SymbolId, Spanned<String>>,
    view_transition_declarations: Vec<ViewTransitionDefinition>,
    local_values: HashMap<SymbolId, ExportedValue>,
    style_calls: Vec<StyleCall>,
    diagnostics: Vec<CompileError>,
    jsx_path: Vec<usize>,
    next_jsx_element: usize,
    imports: Vec<ImportBinding>,
    re_exports: Vec<ReExport>,
    export_symbols: Vec<(SymbolId, String)>,
    type_facts: HashMap<(u32, u32), ExportedValue>,
    type_probes: RefCell<BTreeSet<(u32, u32)>>,
    imported_assets: &'a BTreeMap<IdentityId, AssetMetadata>,
}

#[derive(Clone, Debug)]
struct HandleUse {
    name: String,
    projection: &'static str,
    span: Span,
    jsx_path: Option<Vec<usize>>,
}

#[derive(Clone, Debug)]
struct ScopeUse {
    name: String,
    jsx_path: Option<Vec<usize>>,
}

mod definitions;
mod expressions;
mod motion;
mod projections;
mod references;
mod state;
#[cfg(test)]
mod tests;
mod validation;
mod visit;
