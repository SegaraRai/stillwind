from pathlib import Path
import zipfile
import shutil

import argparse
import subprocess

parser = argparse.ArgumentParser(description="Instrument an isolated Rust source tree for canonical and diagnostic profiling.")
parser.add_argument("--revision", default="HEAD", help="Git revision to archive.")
parser.add_argument("--working-tree", action="store_true", help="Overlay tracked Cargo and Rust files from the workspace.")
parser.add_argument("--directory", default="target/canonical-profile", help="Fresh output directory inside the repository.")
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
tree = (root / args.directory).resolve()
if not tree.is_relative_to(root) or tree == root or tree.exists():
    parser.error("Choose a new directory inside the repository.")
tree.mkdir(parents=True)
archive = tree / "sources.zip"
subprocess.run(["git", "archive", "--output", str(archive), args.revision, "Cargo.toml", "Cargo.lock", "crates"], cwd=root, check=True)
with zipfile.ZipFile(archive) as sources:
    sources.extractall(tree)
if args.working_tree:
    paths = subprocess.check_output(["git", "ls-files", "Cargo.toml", "Cargo.lock", "crates"], cwd=root, text=True).splitlines()
    for path in paths:
        shutil.copy2(root / path, tree / path)
after = "fn location(&mut self," in (tree / "crates/diagnostics/src/report.rs").read_text(encoding="utf-8")

def edit(path, old, new, count=1):
    target = tree / path
    text = target.read_text(encoding='utf-8')
    assert text.count(old) == count, (path, old, text.count(old))
    target.write_text(text.replace(old, new), encoding='utf-8', newline='\n')

source = tree / 'crates/source/src/lib.rs'
source.write_text(source.read_text(encoding='utf-8') + '\n#[allow(missing_docs)]\npub mod performance_probe;\n', encoding='utf-8', newline='\n')
(source.parent / 'performance_probe.rs').write_text('''
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::Instant;
thread_local! { static COUNTERS: RefCell<BTreeMap<&'static str, (f64, usize)>> = RefCell::new(BTreeMap::new()); }
pub struct Scope(&'static str, Instant);
impl Scope { pub fn new(name: &'static str) -> Self { Self(name, Instant::now()) } }
impl Drop for Scope {
    fn drop(&mut self) {
        COUNTERS.with(|values| {
            let mut values = values.borrow_mut();
            let entry = values.entry(self.0).or_default();
            entry.0 += self.1.elapsed().as_secs_f64() * 1000.0;
            entry.1 += 1;
        });
    }
}
pub struct Root(&'static str, Instant);
impl Root {
    pub fn new(name: &'static str) -> Self {
        COUNTERS.with(|values| values.borrow_mut().clear());
        Self(name, Instant::now())
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let total = self.1.elapsed().as_secs_f64() * 1000.0;
        COUNTERS.with(|values| {
            let body = values.borrow().iter().map(|(name, (ms, count))| format!("\\"{name}\\":{{\\"ms\\":{ms},\\"calls\\":{count}}}")).collect::<Vec<_>>().join(",");
            eprintln!("PERF {{\\"operation\\":\\"{}\\",\\"rustTotalMs\\":{total},\\"stages\\":{{{body}}}}}", self.0);
        });
    }
}
''', encoding='utf-8', newline='\n')

def scope(path, anchor, label):
    owner = 'crate' if path.startswith('crates/source/') else 'stillwind_source'
    edit(path, anchor, anchor + f'\n        let _profile = {owner}::performance_probe::Scope::new("{label}");')

def expression(path, old, label, count=1):
    edit(path, old, '{ let _profile = stillwind_source::performance_probe::Scope::new("' + label + '"); ' + old + ' }', count)

native = 'crates/napi/src/session.rs'
scope('crates/design-system/src/theme/parser.rs', ') -> Result<ThemeBuilder, CompileError> {', 'css.parse_definitions')
scope('crates/design-system/src/theme/index.rs', 'pub(crate) fn new(builder: ThemeBuilder) -> Self {', 'css.prepare_theme')
scope('crates/compiler/src/query/canonical/index.rs', 'pub(in crate::query) fn prepare(query: &QueryContext) -> Self {', 'canonical.index_total')
expression('crates/compiler/src/query/canonical/index.rs', 'VariantIndex::prepare(query)', 'canonical.variants')
expression('crates/compiler/src/query/canonical/index.rs', 'ThemeValueIndex::prepare(query, &labels)', 'canonical.values')
scope('crates/compiler/src/query/canonical/values/direct.rs', 'pub(super) fn prepare(query: &QueryContext) -> Self {', 'canonical.direct_values')
scope('crates/compiler/src/query/canonical/values/index.rs', "fn new(query: &'a QueryContext) -> Self {", 'canonical.value_inputs')
scope('crates/compiler/src/query/canonical/values/index.rs', '        let query = self.query;', 'canonical.namespace')
scope('crates/compiler/src/query/canonical/preview.rs', ') -> Result<Option<Arc<CanonicalPreview>>, UtilityResolutionError> {', 'canonical.preview')
scope('crates/compiler/src/query/canonical/preview.rs', '    options: &CanonicalUtilitiesConfig,\n) -> Option<LeanStr> {', 'canonical.normalize')
edit(native, '        let files = files\n', '        let _profile_root = stillwind_source::performance_probe::Root::new("load");\n        let files = files\n')
expression(native, 'self.loader.provide_many(files)', 'css.loader_total')
edit(native, '                prepare_canonical_index(&snapshot, &self.options);', '                { let _profile = stillwind_source::performance_probe::Scope::new("canonical.prepare_total"); prepare_canonical_index(&snapshot, &self.options); }')
edit(native, '    ) -> napi::Result<Vec<NativeDiagnosticBatch>> {', '    ) -> napi::Result<Vec<NativeDiagnosticBatch>> {\n        let _profile_root = stillwind_source::performance_probe::Root::new("diagnose");')
expression(native, 'OxcSourceAnalyzer.scan_input(source.input(&mut registry)?)', 'diagnose.scan')
expression(native, 'OxcSourceAnalyzer.finalize(scan.clone(), facts.modules.get(&scan.module_id))', 'diagnose.finalize', 2)
expression(native, 'compiler.diagnose(&analysis, self.session.identifiers())', 'diagnose.compiler_total')
expression(native, 'Reporter::collect(\n                    &result.diagnostics,\n                    &result.lint_findings,\n                    &registry,\n                    result.lint_settings,\n                )', 'diagnose.reporter')
binding = (tree / native).read_text(encoding='utf-8')
expression(native, 'NativeDiagnosticBatch::from_owned(&scan.module_id, batch)' if 'NativeDiagnosticBatch::from_owned(&scan.module_id, batch)' in binding else 'NativeDiagnosticBatch::new(&scan.module_id, &batch)', 'diagnose.binding_structs')
expression('crates/compiler/src/compiler/pipeline.rs', 'detect(source, parsed, &query)', 'diagnose.detect')
scope('crates/compiler/src/style_analysis/utilities.rs', "pub(super) fn check(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {", 'diagnose.pairwise')
scope('crates/compiler/src/style_analysis/utilities.rs', ') -> BTreeSet<String> {', 'diagnose.overlap_properties')
scope('crates/design-system/src/evaluation/utilities/resolve.rs', ') -> Result<Option<ResolvedUtility>, UtilityResolutionError> {', 'utility.resolve')
scope('crates/diagnostics/src/lint/suppression.rs', 'pub fn filter_lint_findings(source: &SourceFile, findings: &mut Vec<LintFinding>) {', 'diagnose.suppression')
scope('crates/diagnostics/src/lint/suppression.rs', 'pub fn collect_lint_directives(source: &str) -> Vec<LintDirective> {', 'diagnose.parse_directives')
scope('crates/diagnostics/src/report.rs', 'pub fn report(&mut self, diagnostic: &LocatedDiagnostic) -> Result<(), ReportError> {', 'diagnose.report_one')
scope('crates/diagnostics/src/report.rs', 'fn location(&mut self, range: &SourceRange) -> Result<ResolvedLocation, ReportError> {' if after else 'fn resolve_location(range: &SourceRange) -> Result<ResolvedLocation, ReportError> {', 'diagnose.resolve_location')
scope('crates/source/src/text.rs', 'pub fn utf16_offset(&self, byte_offset: BytePos) -> Result<u32, SourceError> {', 'source.utf16_offset')
scope('crates/source/src/text.rs', 'pub fn position(&self, byte_offset: BytePos) -> Result<Position, SourceError> {', 'source.position')
print('Instrumented isolated source tree:', tree)
print('Build with CARGO_INCREMENTAL=0 and the same release target as the production addon.')
