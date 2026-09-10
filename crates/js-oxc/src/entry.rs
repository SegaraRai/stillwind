use std::collections::BTreeMap;
use std::path::Path;

use oxc::allocator::Allocator;
use oxc::ast_visit::Visit;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;

use stillwind_ir::{AdapterKind, ModuleIr};
use stillwind_semantic::{AssetMetadata, IdentityId, ModuleSummary, TypeFact};
use stillwind_source::{CompileError, LintFinding, SourceFile, Span};

use super::state::MacroExtractor;

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
    let mut summary = extractor.module_summary();
    super::demand::collect(
        &result.program,
        semantic.scoping(),
        &extractor.type_probes.borrow(),
        &extractor.export_symbols,
        &mut summary,
    );
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
            adapter: AdapterKind::React,
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

#[cfg(test)]
mod tests {
    use super::*;
    use stillwind_source::FileId;

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
