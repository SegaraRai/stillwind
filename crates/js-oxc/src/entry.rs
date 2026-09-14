//! One parse of a module into IR, diagnostics and a module summary.
//!
//! [`parse_with_language`] is the single entry point: it runs Oxc, builds the semantic
//! scoping table, seeds direct producer bindings, visits the program with
//! [`MacroExtractor`], then adds demand-driven import slices and macro argument validation. Checker facts are
//! optional inputs that let expressions imported from other modules resolve without a
//! type checker in this crate.
//!
//! Extraction records fatal errors and advisory lint findings separately. Only
//! [`LintFinding`]s participate in `swlint-*` suppression; reporting codes do not
//! determine whether compilation may proceed.

use std::collections::BTreeMap;
use std::path::Path;

use oxc::allocator::Allocator;
use oxc::ast_visit::Visit;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;

use stillwind_diagnostics::{CompileError, CompileErrorKind, LintFinding};
use stillwind_ir::{AdapterKind, ModuleIr, ScriptLanguage};
use stillwind_semantic::{AssetMetadata, IdentityId, ModuleSummary, TypeFact};
use stillwind_source::{SourceFile, Span};

use super::contract::validate;
use crate::imports::collect;
use crate::state::MacroExtractor;

#[cfg(doc)]
use {crate::OxcSourceAnalyzer, stillwind_ir::SourceAnalyzer};

/// The result of parsing one module.
///
/// The fields are independent: [`Self::module`] is the IR, [`Self::summary`] the
/// cross-module facts, and the two diagnostic vectors the findings. Every span they
/// carry belongs to `source`.
#[derive(Clone, Debug)]
pub struct ParsedJavaScript {
    /// Adapter selected from the macro import, when one was imported.
    pub framework: Option<AdapterKind>,
    /// Expression spans the host checker should resolve, deduplicated.
    pub type_probes: Vec<Span>,
    /// Fatal errors produced by parsing and extraction.
    pub diagnostics: Vec<CompileError>,
    /// Advisory findings produced by extraction.
    pub lint_findings: Vec<LintFinding>,
    /// Module IR lowered from the parsed program.
    pub module: ModuleIr,
    /// Cross-module facts published for this module.
    pub summary: ModuleSummary,
}

/// Parses a module with optional checker facts and extension-based language detection.
pub fn parse(source: &SourceFile) -> ParsedJavaScript {
    parse_with_type_facts(source, &[], &BTreeMap::new())
}

/// Parses a module using expression facts produced by the TypeScript checker.
///
/// Facts are matched by expression span and are only as trustworthy as their enclosing
/// `TypeFacts` revisions; [`OxcSourceAnalyzer::finalize`] validates those before
/// calling here.
pub fn parse_with_type_facts(
    source: &SourceFile,
    type_facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
) -> ParsedJavaScript {
    parse_with_language(
        source,
        type_facts,
        assets,
        ScriptLanguage::from_path(source.path()),
    )
}

/// Parses a module with an explicit language and checker facts.
///
/// The language decides the Oxc source type, so a shadow module for a `.vue` or
/// `.svelte` file must be given the language of its script rather than one
/// guessed from the path. Returned spans address `source`; projection back to an
/// editable framework file is a separate step. Macro contracts are checked
/// for extracted expression spans, and parse/extraction failures are returned as
/// diagnostics rather than as a `Result` error.
pub fn parse_with_language(
    source: &SourceFile,
    type_facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
    language: ScriptLanguage,
) -> ParsedJavaScript {
    let allocator = Allocator::default();
    let source_type = source_type(source.path(), language);
    let result = Parser::new(&allocator, source.text(), source_type).parse();

    let mut diagnostics: Vec<_> = result
        .diagnostics
        .into_iter()
        .map(|diagnostic| {
            let mut error =
                CompileError::new(CompileErrorKind::JavaScriptParse, source.full_span());
            error.notes.push(diagnostic.to_string());
            error
        })
        .collect();

    let type_facts = if type_facts.iter().all(|fact| fact.value.validate().is_ok()) {
        type_facts
    } else {
        diagnostics.push(CompileError::new(
            CompileErrorKind::InvalidSymbolicFacts,
            source.full_span(),
        ));
        &[]
    };
    let semantic = SemanticBuilder::new().build(&result.program).semantic;
    let mut extractor = MacroExtractor::new(source, semantic.scoping(), type_facts, assets);
    extractor.seed_style_bindings(&result.program);
    extractor.visit_program(&result.program);
    extractor.validate_handles();
    let mut summary = extractor.module_summary();
    super::demand::collect(
        &result.program,
        semantic.scoping(),
        &extractor.type_probes.borrow(),
        &extractor.export_symbols,
        &mut summary,
    );
    diagnostics.extend(extractor.diagnostics);
    let mut lint_findings = extractor.lint_findings;

    let module = ModuleIr {
        adapter: extractor.adapter.unwrap_or(AdapterKind::React),
        imports: collect(&result.program, semantic.scoping(), source.id()),
        style_calls: extractor.style_calls,
        handle_definitions: extractor.handle_definitions,
        scope_definitions: extractor.scope_definitions,
        animation_declarations: extractor.animation_declarations,
        view_transition_declarations: extractor.view_transition_declarations,
    };
    lint_findings.extend(validate(&result.program, &module, source.id()));
    ParsedJavaScript {
        framework: extractor.adapter,
        type_probes: extractor
            .type_probes
            .into_inner()
            .into_iter()
            .map(|(start, end)| Span::new(source.id(), start, end))
            .collect(),
        diagnostics,
        lint_findings,
        summary,
        module,
    }
}

/// Chooses the Oxc source type for a path and an explicit language.
///
/// The language wins over the extension, because a projection or an opaque host name
/// may not carry a usable suffix; an unknown extension falls back to the Oxc default
/// rather than failing.
pub(crate) fn source_type(path: &str, language: ScriptLanguage) -> SourceType {
    let typescript = matches!(language, ScriptLanguage::Ts | ScriptLanguage::Tsx);
    let jsx = matches!(language, ScriptLanguage::Jsx | ScriptLanguage::Tsx);
    SourceType::from_path(Path::new(path))
        .unwrap_or_default()
        .with_javascript(!typescript)
        .with_typescript(typescript)
        .with_jsx(jsx)
        .with_standard(!jsx)
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn accepts_typescript_jsx() {
        let source = SourceFile::new(
            FileId(1),
            "Button.tsx",
            "const Button = ({ loading }: { loading: boolean }) => <button>{loading}</button>;",
        )
        .unwrap();

        assert!(parse(&source).diagnostics.is_empty());
    }
}
