//! One parse of a module into IR, diagnostics and a module summary.
//!
//! [`parse`] runs Oxc, builds the semantic scoping table, seeds direct producer
//! bindings, visits the program with [`MacroExtractor`], then adds demand-driven import
//! slices and macro argument validation. Checker facts are optional inputs that let
//! expressions imported from other modules resolve without a type checker in this
//! crate.
//!
//! Extraction records fatal errors and advisory lint findings separately. Only
//! [`LintFinding`]s participate in `swlint-*` suppression; reporting codes do not
//! determine whether compilation may proceed.

use std::collections::BTreeMap;

use oxc::allocator::Allocator;
use oxc::ast_visit::Visit;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintFinding};
use stillwind_ir::{AdapterKind, ModuleIr, ScriptLanguage, ScriptModuleKind};
use stillwind_semantic::{AssetMetadata, IdentityId, ModuleSummary, TypeFact};
use stillwind_source::{ModuleId, SourceFile, Span};

use crate::contract::validate;
use crate::demand::collect as collect_demand;
use crate::imports::collect as collect_imports;
use crate::state::MacroExtractor;

#[cfg(doc)]
use {crate::OxcSourceAnalyzer, stillwind_ir::SourceAnalyzer};

/// The result of parsing one module.
///
/// The fields are independent: [`Self::module`] is the IR, [`Self::summary`] the
/// cross-module facts, and the two diagnostic vectors the findings. Extraction spans
/// use analysis UTF-8 coordinates until projection; declaration identities and checker
/// facts always use original UTF-8 coordinates. Projected scans return original spans.
#[derive(Clone, Debug)]
pub struct ParsedJavaScript {
    /// Adapter selected from the macro import, when one was imported.
    pub framework: Option<AdapterKind>,
    /// Expression spans the host checker should resolve, deduplicated.
    pub type_probes: Vec<Span>,
    /// Receiver ranges queried to check whether unconsumed syntax is a macro use.
    /// Missing facts are allowed. Ranges already in `type_probes` are excluded.
    pub use_probes: Vec<Span>,
    /// Fatal errors produced by parsing and extraction.
    pub diagnostics: Vec<CompileError>,
    /// Advisory findings produced by extraction.
    pub lint_findings: Vec<LintFinding>,
    /// Module IR lowered from the parsed program.
    pub module: ModuleIr,
    /// Cross-module facts published for this module.
    pub summary: ModuleSummary,
}

/// Parses a module with an explicit language and checker facts.
///
/// The language decides the Oxc source type, so a shadow module for a `.vue` or
/// `.svelte` file must be given the language of its script rather than one
/// guessed from the path. Returned spans address `source`; projection back to an
/// editable framework file is a separate step. Macro contracts are checked
/// for extracted expression spans, and parse/extraction failures are returned as
/// diagnostics rather than as a `Result` error.
pub fn parse(
    source: &SourceFile,
    module_id: &ModuleId,
    type_facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
    language: ScriptLanguage,
    module_kind: ScriptModuleKind,
) -> ParsedJavaScript {
    parse_with_lint(
        source,
        module_id,
        type_facts,
        assets,
        language,
        module_kind,
        true,
    )
}

pub(crate) fn parse_with_lint(
    source: &SourceFile,
    module_id: &ModuleId,
    type_facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
    language: ScriptLanguage,
    module_kind: ScriptModuleKind,
    lint: bool,
) -> ParsedJavaScript {
    parse_with_coordinates(
        source,
        module_id,
        type_facts,
        assets,
        language,
        module_kind,
        source,
        &Some,
        lint,
    )
}

/// Extracts an analysis buffer with original UTF-8 identity and checker coordinates.
#[allow(clippy::too_many_arguments)]
pub(crate) fn parse_with_coordinates(
    source: &SourceFile,
    module_id: &ModuleId,
    type_facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
    language: ScriptLanguage,
    module_kind: ScriptModuleKind,
    original: &SourceFile,
    coordinates: &dyn Fn(Span) -> Option<Span>,
    lint: bool,
) -> ParsedJavaScript {
    let allocator = Allocator::default();
    let source_type = source_type(language, module_kind);
    let result = Parser::new(&allocator, source.text(), source_type).parse();

    let mut diagnostics: Vec<_> = result
        .diagnostics
        .into_iter()
        .map(|diagnostic| {
            let mut error =
                CompileError::new(CompileDiagnostic::JavaScriptParse, source.full_span());
            error.notes.push(diagnostic.to_string());
            error
        })
        .collect();

    let type_facts = if type_facts.iter().all(|fact| fact.value.validate().is_ok()) {
        type_facts
    } else {
        diagnostics.push(CompileError::new(
            CompileDiagnostic::InvalidSymbolicFacts,
            source.full_span(),
        ));
        &[]
    };
    let semantic = SemanticBuilder::new().build(&result.program).semantic;
    let mut extractor = MacroExtractor::new(
        source,
        module_id,
        semantic.scoping(),
        type_facts,
        assets,
        original,
        coordinates,
    );
    extractor.lint = lint;
    extractor.seed_style_bindings(&result.program);
    extractor.visit_program(&result.program);
    let mut summary = extractor.module_summary();
    collect_demand(
        &result.program,
        semantic.scoping(),
        &extractor
            .type_probes
            .borrow()
            .union(&extractor.use_probes.borrow())
            .copied()
            .collect(),
        &extractor.export_symbols,
        &mut summary,
    );
    diagnostics.extend(extractor.diagnostics);
    let mut lint_findings = extractor.lint_findings;

    let mut imports = collect_imports(&result.program, semantic.scoping(), source.id());
    imports.consumed_uses = extractor.consumed_macro_uses;
    let module = ModuleIr {
        adapter: extractor.adapter.unwrap_or(AdapterKind::React),
        imports,
        style_calls: extractor.style_calls,
        cva_calls: extractor.cva_calls,
        ref_definitions: extractor.ref_definitions,
        keyframes_declarations: extractor.keyframes_declarations,
        view_transition_declarations: extractor.view_transition_declarations,
    };
    if lint {
        lint_findings.extend(validate(&result.program, &module, source.id()));
    }
    let use_probes = extractor
        .use_probes
        .borrow()
        .difference(&extractor.type_probes.borrow())
        .map(|span| Span::new(source.id(), span.start, span.end))
        .collect();
    ParsedJavaScript {
        framework: extractor.adapter,
        use_probes,
        type_probes: extractor
            .type_probes
            .into_inner()
            .into_iter()
            .map(|span| Span::new(source.id(), span.start, span.end))
            .collect(),
        diagnostics,
        lint_findings,
        summary,
        module,
    }
}

/// Chooses the Oxc source type from host-supplied language and module metadata.
pub(crate) fn source_type(language: ScriptLanguage, module_kind: ScriptModuleKind) -> SourceType {
    let typescript = matches!(
        language,
        ScriptLanguage::Ts | ScriptLanguage::Tsx | ScriptLanguage::Dts
    );
    let jsx = matches!(language, ScriptLanguage::Jsx | ScriptLanguage::Tsx);
    let source_type = SourceType::default()
        .with_javascript(!typescript)
        .with_typescript(typescript)
        .with_jsx(jsx)
        .with_standard(!jsx)
        .with_typescript_definition(matches!(language, ScriptLanguage::Dts));
    match module_kind {
        ScriptModuleKind::Module => source_type.with_module(true),
        ScriptModuleKind::CommonJs => source_type.with_commonjs(true),
        ScriptModuleKind::Script => source_type.with_script(true),
        ScriptModuleKind::Unambiguous => source_type.with_unambiguous(true),
    }
}

#[cfg(test)]
mod tests {
    use stillwind_ir::StyleCall;
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn accepts_typescript_jsx() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "const Button = ({ loading }: { loading: boolean }) => <button>{loading}</button>;",
        )
        .unwrap();

        assert_eq!(
            parse(
                &source,
                &ModuleId::from("test.tsx"),
                &[],
                &BTreeMap::default(),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous
            )
            .diagnostics,
            [] as [CompileError; 0]
        );
    }

    /// The coordinate entry produces the same analysis as the plain entry when there
    /// are no type facts, and an empty buffer is a valid module.
    #[test]
    fn coordinate_entry_matches_the_plain_entry_without_facts() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; export const x = style('flex');",
        )
        .unwrap();
        let plain = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert_eq!(plain.diagnostics, [] as [CompileError; 0]);
        assert_eq!(plain.module.adapter, AdapterKind::React);
        assert_eq!(plain.module.style_calls.len(), 1);
        assert_eq!(plain.framework, Some(AdapterKind::React));
        let located = parse_with_coordinates(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::new(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &source,
            &|_| None,
            true,
        );
        assert_eq!(located.diagnostics, [] as [CompileError; 0]);
        assert_eq!(located.module.style_calls.len(), 1);
        assert_eq!(located.module.adapter, AdapterKind::React);
        assert_eq!(plain.summary.module_id, located.summary.module_id);
        let empty = SourceFile::new(FileId(2), None, None, "").unwrap();
        let parsed = parse(
            &empty,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
        assert_eq!(parsed.module.style_calls, [] as [StyleCall; 0]);
        assert_eq!(parsed.framework, None);
    }

    /// A syntax error becomes one parse diagnostic that keeps the parser's own detail
    /// as a note and points at the whole buffer.
    #[test]
    fn syntax_errors_keep_the_parser_detail_as_a_note() {
        let source = SourceFile::new(FileId(1), None, None, "const = ;").unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert_eq!(parsed.diagnostics.len(), 1);
        let diagnostic = &parsed.diagnostics[0];
        assert_eq!(diagnostic.diagnostic, CompileDiagnostic::JavaScriptParse);
        assert_eq!(diagnostic.labels[0].span, source.full_span());
        // The parser's own wording is retained as a note rather than rewritten.
        assert!(
            diagnostic.notes[0].contains("Unexpected"),
            "{}",
            diagnostic.notes[0]
        );
    }
}
