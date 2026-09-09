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

#[derive(Clone, Debug)]
pub struct ParsedJavaScript {
    pub type_probes: Vec<Span>,
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub module: ModuleIr,
    pub summary: ModuleSummary,
}

pub fn parse(source: &SourceFile) -> ParsedJavaScript {
    parse_with_type_facts(source, &[], &BTreeMap::new())
}

/// Parses a module using expression facts produced by the TypeScript checker.
pub fn parse_with_type_facts(
    source: &SourceFile,
    type_facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
) -> ParsedJavaScript {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(Path::new(source.path())).unwrap_or_default();
    let result = Parser::new(&allocator, source.text(), source_type).parse();

    let mut diagnostics: Vec<_> = result
        .diagnostics
        .into_iter()
        .map(|diagnostic| CompileError::new("SHJS001", diagnostic.to_string(), source.full_span()))
        .collect();

    let semantic = SemanticBuilder::new().build(&result.program).semantic;
    let mut extractor = MacroExtractor::new(source, semantic.scoping(), type_facts, assets);
    extractor.visit_program(&result.program);
    extractor.validate_handles();
    let summary = extractor.module_summary();
    let mut lint_findings = Vec::new();
    for diagnostic in extractor.diagnostics {
        if let Some(rule) = lint_rule_for_code(diagnostic.code) {
            let mut finding = LintFinding::new(
                rule,
                diagnostic.message,
                diagnostic
                    .labels
                    .first()
                    .map_or(source.full_span(), |label| label.span),
            );
            finding.labels = diagnostic.labels;
            finding.notes = diagnostic.notes;
            lint_findings.push(finding);
        } else {
            diagnostics.push(diagnostic);
        }
    }

    ParsedJavaScript {
        type_probes: extractor
            .type_probes
            .into_inner()
            .into_iter()
            .map(|(start, end)| Span::new(source.id(), start, end))
            .collect(),
        diagnostics,
        lint_findings,
        summary,
        module: ModuleIr {
            adapter: stillwind_ir::AdapterKind::React,
            style_calls: extractor.style_calls,
            handle_definitions: extractor.handle_definitions,
            scope_definitions: extractor.scope_definitions,
            animation_declarations: extractor.animation_declarations,
            view_transition_declarations: extractor.view_transition_declarations,
        },
    }
}

fn lint_rule_for_code(code: &str) -> Option<&'static str> {
    Some(match code {
        "SHREF003" => "require-anchor-provider",
        "SHREF006" => "require-container-provider",
        "SHREF007" => "valid-ref-ancestry",
        "SHSCOPE002" => "require-scope-provider",
        "SHSCOPE003" => "valid-scope-ancestry",
        "SHSCOPE004" => "scoped-selector-in-scope",
        _ => return None,
    })
}

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

fn is_strict_ancestor(ancestor: &[usize], descendant: &[usize]) -> bool {
    ancestor.len() < descendant.len() && descendant.starts_with(ancestor)
}

fn is_marker_label(label: &Spanned<String>) -> bool {
    !label.value.is_empty()
        && label
            .value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn is_animation_frame(value: &str) -> bool {
    value == "from"
        || value == "to"
        || value.split(',').all(|part| {
            part.trim()
                .strip_suffix('%')
                .is_some_and(|number| number.trim().parse::<u8>().is_ok())
        })
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
