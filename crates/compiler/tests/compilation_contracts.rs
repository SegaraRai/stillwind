//! Compilation contracts for emitted CSS, refs, motion, and source attribution.

use stillwind_compiler::{CompileResult, Compiler, CompilerOptions};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

#[path = "compilation_contracts/emission.rs"]
mod emission;

#[path = "compilation_contracts/motion.rs"]
mod motion;

#[path = "compilation_contracts/provenance.rs"]
mod provenance;

#[path = "compilation_contracts/refs.rs"]
mod refs;

#[path = "compilation_contracts/style_analysis.rs"]
mod style_analysis;

fn compile(code: &str, css: &str) -> (SourceFile, CompileResult) {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        format!("import {{ style }} from 'stillwind/react'; {code}"),
    )
    .unwrap();
    let theme = SourceFile::new(
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
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    (source, result)
}
