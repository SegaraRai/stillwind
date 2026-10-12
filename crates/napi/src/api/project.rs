//! Compiles a prepared module graph with shared CSS and style lint analysis.

use std::collections::{BTreeMap, BTreeSet};

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::{
    CompilerOptions, Identifiers, ModuleCompiler, StyleResolver, check_style,
    is_style_analysis_lint,
};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::filter_lint_findings;
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFacts};
use stillwind_source::{Fingerprint, SourceRegistry};

use super::{BindingApiError, CompileCssInput, NativeCompileCssInput, binding_output};
use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::{ProjectBindingOutput, ProjectModuleOutput};
use crate::source_input::{HostPreparedSource, NativePreparedSource};

/// Compiles a complete source graph and returns one output per input module.
///
/// The source graph uses the same domain records as the typed Node adapter.
///
/// A direct input registers its editable document as the parser buffer. A projected
/// input registers separate generated and original buffers and reserves an analysis
/// file ID. Published spans refer to the editable document even when the generated
/// buffer contains the same text. Module identities are chosen by the host.
///
/// Composition findings are recomputed against all successful input artifacts before
/// outputs are built, so a base may appear after its consumer in `input`. Source
/// suppression runs in each consuming source; missing base records remain unknown.
///
/// Rules shared by two modules are emitted once: the first module in input order keeps
/// the rule and the later module's duplicate is dropped, because this output is one
/// document rather than one fragment per module.
///
/// # Errors
///
/// Returns an error for invalid options or sources, exhausted file IDs, or an
/// unresolved module edge, or a failure while resolving diagnostic sources or rendering
/// source transforms. Module compilation errors are carried in the returned outputs.
pub fn compile_project_input(
    input: &[HostPreparedSource],
    css: &CompileCssInput,
    options: &CompilerOptions,
) -> Result<ProjectBindingOutput, BindingApiError> {
    let analyzer = OxcSourceAnalyzer::with_lint(options.lint_settings().has_enabled_rules());
    options.validate()?;
    let mut registry = SourceRegistry::default();
    let scans = input
        .iter()
        .map(|source| {
            let input = source
                .input(&mut registry)
                .map_err(|error| BindingApiError::Input(error.reason))?;
            Ok::<_, BindingApiError>(analyzer.scan_input(input))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let theme = registry.register(
        Some(css.uri.clone()),
        css.source_map_source.as_deref().map(Into::into),
        css.content.as_str(),
    )?;
    let assets = scans
        .iter()
        .flat_map(|scan| scan.summary.assets.iter().cloned())
        .map(|asset| (asset.identity, asset.metadata))
        .collect::<BTreeMap<IdentityId, AssetMetadata>>();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, css.name.clone(), options.design_system())
            .map_err(BindingApiError::DesignSystem)?;
    let compiler = ModuleCompiler::new(snapshot.clone(), options.clone());
    let mut results = scans
        .iter()
        .zip(input)
        .map(|(scan, source)| {
            let type_facts = source.type_facts();
            let facts = TypeFacts {
                schema: TypeFacts::SCHEMA,
                module_id: scan.module_id.clone(),
                source_revision: scan.source_revision.clone(),
                semantic_revision: Fingerprint::of(format!("{type_facts:?}{assets:?}")),
                facts: type_facts.to_vec(),
                assets: assets.clone(),
            };
            compiler.compile(
                &analyzer.finalize(scan.clone(), Some(&facts)),
                &Identifiers::default(),
            )
        })
        .collect::<Vec<_>>();
    if options.lint_settings().has_enabled_rules() {
        let mut resolver =
            StyleResolver::new(results.iter().flat_map(|artifact| &artifact.style_effects));
        let findings = results
            .iter()
            .zip(&scans)
            .map(|(artifact, scan)| {
                let mut findings = artifact
                    .style_effects
                    .iter()
                    .filter_map(|effects| resolver.resolve(effects.identity()))
                    .flat_map(|style| check_style(&style))
                    .collect::<Vec<_>>();
                filter_lint_findings(&scan.source, &mut findings);
                findings
            })
            .collect::<Vec<_>>();
        for (artifact, findings) in results.iter_mut().zip(findings) {
            artifact
                .lint_findings
                .retain(|finding| !is_style_analysis_lint(finding.kind()));
            artifact.lint_findings.extend(findings);
        }
    }
    let mut emitted_css_rules = BTreeSet::new();
    for result in &mut results {
        result
            .css
            .rules
            .retain(|rule| emitted_css_rules.insert(rule.content_key()));
    }
    let modules = scans
        .iter()
        .zip(results)
        .map(|(scan, result)| {
            let result = result.to_module_output();
            Ok(ProjectModuleOutput::new(
                scan.module_id.clone(),
                binding_output(result, &scan.source, &registry)?,
            ))
        })
        .collect::<Result<Vec<_>, BindingApiError>>()?;
    Ok(ProjectBindingOutput { modules })
}

/// Compiles a resolved module graph in one native invocation.
///
/// # Errors
///
/// Returns a JavaScript error for invalid input or an unresolved graph edge.
#[napi(strict)]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn compile_project(
    sources: Vec<NativePreparedSource>,
    css: NativeCompileCssInput,
    options: Option<NativeCompilerOptions>,
) -> napi::Result<ProjectBindingOutput> {
    let options = compiler_options(options)?;
    let sources = sources
        .into_iter()
        .map(TryInto::try_into)
        .collect::<napi::Result<Vec<HostPreparedSource>>>()?;
    compile_project_input(&sources, &css.try_into()?, &options)
        .map_err(|error| Error::from_reason(error.to_string()))
}
