//! Compiles sources against a supplied theme for the compilation contract suites.
//! `CompilationFixture` shares a parsed theme across cases that keep the same options.

use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::ToLeanString;

use stillwind_compiler::{
    CompileResult, Compiler, CompilerOptions, Identifiers, ModuleCompiler, StylesheetRenderer,
};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::{TypeFact, TypeFacts};
use stillwind_source::{CssSourceName, FileId, Fingerprint, ModuleId, SourceFile};

pub(super) fn compile(code: &str, css: &str) -> (SourceFile, CompileResult) {
    compile_with_options(code, css, &CompilerOptions::default())
}

pub(super) fn compile_with_options(
    code: &str,
    css: &str,
    options: &CompilerOptions,
) -> (SourceFile, CompileResult) {
    let source = script_source(code);
    let theme = theme_source(css);
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        options,
    );
    (source, result)
}

/// Compiles `source` as `test.tsx` against `theme` with exported type `facts` and the
/// default options.
pub(super) fn compile_with_type_facts(
    source: &SourceFile,
    theme: &SourceFile,
    facts: &[TypeFact],
) -> CompileResult {
    let scan = OxcSourceAnalyzer.scan(
        Arc::new(source.clone()),
        ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of(format_args!("{facts:?}").to_lean_string()),
        facts: facts.to_vec(),
        assets: BTreeMap::new(),
    };
    Compiler::compile_analysis(
        &OxcSourceAnalyzer.finalize(scan, Some(&facts)),
        theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    )
}

/// Keeps one parsed theme and its compiler options for independent source cases.
pub(super) struct CompilationFixture {
    snapshot: DesignSystemSnapshot,
    options: CompilerOptions,
}

impl CompilationFixture {
    pub(super) fn new(css: &str, options: &CompilerOptions) -> Self {
        Self {
            snapshot: DesignSystemSnapshot::from_source(
                &theme_source(css),
                CssSourceName::from("stillwind.css"),
                options.design_system(),
            )
            .unwrap(),
            options: options.clone(),
        }
    }

    pub(super) fn compile(&self, code: &str) -> (SourceFile, CompileResult) {
        let source = script_source(code);
        let analysis = OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                Arc::new(source.clone()),
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        );
        let module = ModuleCompiler::new(self.snapshot.clone(), self.options.clone())
            .compile(&analysis, &Identifiers::default());
        let result =
            StylesheetRenderer::new(&self.snapshot, &self.options, &Identifiers::default())
                .render_compilation(&module, source.full_span());
        (source, result)
    }
}

fn script_source(code: &str) -> SourceFile {
    SourceFile::new(
        FileId(1),
        None,
        None,
        format!("import {{ style }} from 'stillwind/react'; {code}"),
    )
    .unwrap()
}

fn theme_source(css: &str) -> SourceFile {
    SourceFile::new(
        FileId(2),
        None,
        None,
        format!(
            "{css} @stillwind-slot utilities; {}",
            if css.contains("@stillwind-slot keyframes;") {
                ""
            } else {
                "@stillwind-slot keyframes;"
            }
        ),
    )
    .unwrap()
}
