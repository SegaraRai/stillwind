from pathlib import Path
import subprocess
import shutil
import sys

root = Path(__file__).resolve().parents[2]
out = root / 'target/benchmark-research'
out.mkdir(parents=True, exist_ok=True)
files = [root / 'crates/project/src/session.rs', root / 'crates/project/src/stylesheet_plan.rs', root / 'crates/napi/src/session.rs']
original = {p: p.read_bytes() for p in files}
texts = {p: original[p].decode().replace('\r\n', '\n') for p in files}

def replace(p, old, new):
    assert texts[p].count(old) == 1, (p, old, texts[p].count(old))
    texts[p] = texts[p].replace(old, new)

project, plan, native = files
texts[project] += '''
std::thread_local! {
    static RESEARCH: std::cell::RefCell<std::collections::BTreeMap<String, f64>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}
pub fn research_record(name: &str, value: f64) {
    RESEARCH.with(|profile| *profile.borrow_mut().entry(name.into()).or_default() += value);
}
pub fn research_take() -> std::collections::BTreeMap<String, f64> {
    RESEARCH.with(|profile| std::mem::take(&mut *profile.borrow_mut()))
}
'''
replace(project, '    pub fn update_module(&mut self, analysis: ModuleAnalysis) -> ModuleUpdate {', '    pub fn update_module(&mut self, analysis: ModuleAnalysis) -> ModuleUpdate {\n        let research_start = std::time::Instant::now();')
replace(project, '            return ModuleUpdate::default();', '            research_record("cache_hits", 1.0);\n            research_record("update_key_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n            return ModuleUpdate::default();')
replace(project, '        let mut compiled = backend.compile(&analysis, &self.identifiers);', '        research_record("cache_misses", 1.0);\n        research_record("update_key_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n        let research_start = std::time::Instant::now();\n        let mut compiled = backend.compile(&analysis, &self.identifiers);\n        research_record("module_compile_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n        let research_start = std::time::Instant::now();')
replace(project, '        let mut symbols = self.document_names(Some(&module_id));', '        let mut symbols = self.document_names(Some(&module_id));\n        research_record("name_index_ms", research_start.elapsed().as_secs_f64() * 1000.0);')
replace(project, '    fn refresh_style_lints(&mut self, modules: impl IntoIterator<Item = ModuleId>) {', '    fn refresh_style_lints(&mut self, modules: impl IntoIterator<Item = ModuleId>) {\n        let research_start = std::time::Instant::now();')
replace(project, '                artifact.lint_findings.extend(findings);\n            }\n        }\n    }', '                artifact.lint_findings.extend(findings);\n            }\n        }\n        research_record("style_lints_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n    }')
replace(project, '    pub fn current_artifact(&self, module: &ModuleId) -> Option<&CompiledModule> {', '    pub fn current_artifact(&self, module: &ModuleId) -> Option<&CompiledModule> {\n        let research_start = std::time::Instant::now();')
replace(project, '        (artifact.key == **key).then_some(artifact.as_ref())', '        let result = (artifact.key == **key).then_some(artifact.as_ref());\n        research_record("current_key_calls", 1.0);\n        research_record("current_key_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n        result')

def plan_mark(old, name):
    replace(plan, old, f'        crate::research_record("{name}", research_start.elapsed().as_secs_f64() * 1000.0);\n        let research_start = std::time::Instant::now();\n' + old)

replace(plan, '        let order = self.stylesheet_order(&mut selection.modules)?;', '        let research_start = std::time::Instant::now();\n        let order = self.stylesheet_order(&mut selection.modules)?;')
plan_mark('        let stale = selection', 'plan_order_ms')
plan_mark('        let mut stylesheet = ProjectStylesheet::default();', 'plan_validity_ms')
plan_mark('        let snapshot = self.snapshot();', 'plan_assemble_ms')
plan_mark('        let renderer = StylesheetRenderer::new(&snapshot, self.options(), self.identifiers());', 'plan_origins_ms')
plan_mark('        let source_modules: BTreeMap<FileId, ModuleId> = self', 'plan_render_ms')
plan_mark('        self.check_document_names(&renderer.take_detached_symbols())?;', 'plan_parts_ms')
replace(plan, '        Ok(StylesheetPlan {', '        crate::research_record("plan_names_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n        Ok(StylesheetPlan {')

start = texts[native].index('    pub fn compile(\n')
end = texts[native].index('\n}\n\nimpl NativeProjectSession', start)
body = texts[native][start:end]
def b_replace(old, new):
    global body
    assert body.count(old) == 1, (old, body.count(old))
    body = body.replace(old, new)
def mark(old, name):
    b_replace(old, f'        stillwind_project::research_record("{name}", research_start.elapsed().as_secs_f64() * 1000.0);\n        let research_start = std::time::Instant::now();\n' + old)
b_replace('        let _naming', '        let _ = stillwind_project::research_take();\n        let research_total = std::time::Instant::now();\n        let research_start = std::time::Instant::now();\n        let _naming')
mark('        let mut transaction = self.session.clone();', 'convert_ms')
mark('        let scans: Vec<_> = sources', 'clone_ms')
mark('        let facts = ProvidedTypeScriptFacts', 'scan_ms')
mark('        let snapshot = self.session.snapshot();', 'facts_finalize_ms')
mark('        for module_analysis in ModuleAnalysis::attach_asset_support_batch(&analyses) {', 'initial_registry_ms')
b_replace('        for module_analysis in ModuleAnalysis::attach_asset_support_batch(&analyses) {', '        for module_analysis in ModuleAnalysis::attach_asset_support_batch(&analyses) {\n            let research_start = std::time::Instant::now();')
b_replace('            let update = transaction.update_module(module_analysis.clone());', '            stillwind_project::research_record("finalize_attach_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n            let research_start = std::time::Instant::now();\n            let update = transaction.update_module(module_analysis.clone());\n            stillwind_project::research_record("update_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n            let research_start = std::time::Instant::now();')
b_replace('            modules.push(ProjectModuleOutput::new(module_id, output));', '            modules.push(ProjectModuleOutput::new(module_id, output));\n            stillwind_project::research_record("first_output_ms", research_start.elapsed().as_secs_f64() * 1000.0);')
b_replace('            Self::refresh_batches(&mut transaction)?;', '            let research_start = std::time::Instant::now();\n            Self::refresh_batches(&mut transaction)?;\n            stillwind_project::research_record("refresh_reports_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n            let research_start = std::time::Instant::now();')
b_replace('            self.session = transaction;', '            stillwind_project::research_record("final_output_ms", research_start.elapsed().as_secs_f64() * 1000.0);\n            let research_start = std::time::Instant::now();\n            self.session = transaction;\n            stillwind_project::research_record("commit_ms", research_start.elapsed().as_secs_f64() * 1000.0);')
b_replace('        Ok(ProjectBindingOutput { modules })', '        stillwind_project::research_record("native_total_ms", research_total.elapsed().as_secs_f64() * 1000.0);\n        eprintln!("RESEARCH_PROFILE {}", serde_json::to_string(&stillwind_project::research_take()).unwrap_or_default());\n        Ok(ProjectBindingOutput { modules })')
texts[native] = texts[native][:start] + body + texts[native][end:]
replace(native, '        Ok(NativeStylesheetPlan {', '        eprintln!("RESEARCH_PLAN {}", serde_json::to_string(&stillwind_project::research_take()).unwrap_or_default());\n        Ok(NativeStylesheetPlan {')
texts[native] = texts[native].replace('eprintln!("RESEARCH_PROFILE {}", serde_json::to_string(&stillwind_project::research_take()).unwrap_or_default());', 'eprintln!("RESEARCH_PROFILE {:?}", stillwind_project::research_take());').replace('eprintln!("RESEARCH_PLAN {}", serde_json::to_string(&stillwind_project::research_take()).unwrap_or_default());', 'eprintln!("RESEARCH_PLAN {:?}", stillwind_project::research_take());')

if '--check' in sys.argv:
    print('All profiling anchors match the current sources.')
    sys.exit(0)

try:
    for p in files:
        p.write_text(texts[p], encoding='utf-8', newline='\n')
    subprocess.run(['cargo', 'build', '-p', 'stillwind-napi', '--release', '--target', 'x86_64-pc-windows-msvc'], cwd=root, check=True)
    shutil.copy2(root / 'target/x86_64-pc-windows-msvc/release/stillwind_napi.dll', out / 'profile.node')
finally:
    for p, data in original.items():
        p.write_bytes(data)
