//! Stateful Node host binding. Resolution and I/O remain in JavaScript.

use std::collections::BTreeMap;

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::{CompilerOptions, StylesheetPlacement, StylesheetRenderer};
use stillwind_design_system::{
    CssFileInput, DesignSystemLoadStep, DesignSystemLoader, DesignSystemSnapshot,
};
use stillwind_ir::{ModuleScan, SourceAnalyzer, TypeScriptFactsProvider};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode, StylesheetCoverage, StylesheetSelection};
use stillwind_semantic::{ModuleId, TypeFacts, TypeFactsUpdate};
use stillwind_source::{CompileError, FileId, Fingerprint, SourceFile};

use crate::api::binding_output;
use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::{ProjectBindingOutput, ProjectModuleOutput};
use crate::source_input::{NativeProjectSource, ProjectSource};
use crate::stylesheet::*;

#[derive(Debug, thiserror::Error)]
pub enum NativeSessionError {
    #[error("CSS load is incomplete")]
    IncompleteCssLoad,
    #[error("CSS reload compilation failed: {diagnostics:?}")]
    ReloadCompilation {
        diagnostics: BTreeMap<ModuleId, Vec<CompileError>>,
    },
}

impl From<NativeSessionError> for Error {
    fn from(error: NativeSessionError) -> Self {
        Self::from_reason(error.to_string())
    }
}

#[napi(object)]
pub struct NativeCssFileInput {
    pub id: u32,
    pub name: String,
    pub content: String,
}

#[napi]
pub struct NativeDesignSystemLoader {
    loader: DesignSystemLoader,
    options: CompilerOptions,
    snapshot: Option<DesignSystemSnapshot>,
}

#[napi]
impl NativeDesignSystemLoader {
    #[napi(constructor, strict)]
    pub fn new(options: Option<NativeCompilerOptions>) -> napi::Result<Self> {
        let options = compiler_options(options)?;
        Ok(Self {
            loader: DesignSystemLoader::new(options.design_system()),
            options,
            snapshot: None,
        })
    }

    #[napi]
    pub fn provide(
        &mut self,
        id: u32,
        name: String,
        content: String,
    ) -> napi::Result<NativeCssLoadStep> {
        self.provide_many(vec![NativeCssFileInput { id, name, content }])
    }

    #[napi(strict)]
    pub fn provide_many(
        &mut self,
        files: Vec<NativeCssFileInput>,
    ) -> napi::Result<NativeCssLoadStep> {
        match self
            .loader
            .provide_many(files.into_iter().map(|file| CssFileInput {
                id: file.id,
                name: file.name.as_str().into(),
                content: file.content.into(),
            })) {
            DesignSystemLoadStep::NeedFile(requests) => Ok(NativeCssLoadStep::NeedFile {
                requests: requests
                    .into_iter()
                    .map(|request| NativeCssRequest {
                        id: request.id,
                        importer: request.importer.0.to_string(),
                        specifier: request.specifier,
                    })
                    .collect(),
            }),
            DesignSystemLoadStep::Complete(Ok(snapshot)) => {
                let output = NativeCssLoadStep::Complete {
                    revision: snapshot.revision().0.to_string(),
                    resources: snapshot
                        .sources()
                        .resources
                        .iter()
                        .map(|resource| NativeCssResource {
                            placeholder: resource.placeholder.clone(),
                            owner: resource.owner.clone(),
                            specifier: resource.specifier.clone(),
                            start: resource.span.start.0,
                            end: resource.span.end.0,
                        })
                        .collect(),
                    sources: snapshot
                        .sources()
                        .sources
                        .iter()
                        .map(|source| source.name.0.to_string())
                        .collect(),
                };
                self.snapshot = Some(snapshot);
                Ok(output)
            }
            DesignSystemLoadStep::Complete(Err(error)) => {
                Err(Error::from_reason(error.to_string()))
            }
        }
    }

    #[napi]
    pub fn generate_react_types(&self) -> napi::Result<String> {
        Ok(StylesheetRenderer::new(&self.completed()?, &self.options).generate_react_types())
    }
}

impl NativeDesignSystemLoader {
    fn completed(&self) -> Result<DesignSystemSnapshot, NativeSessionError> {
        self.snapshot
            .clone()
            .ok_or(NativeSessionError::IncompleteCssLoad)
    }
}

/// Adapter for facts already produced by the host's long-lived TS7 service.
struct ProvidedTypeScriptFacts<'a> {
    sources: &'a [ProjectSource],
}

impl TypeScriptFactsProvider for ProvidedTypeScriptFacts<'_> {
    fn update(&mut self, scans: &[ModuleScan]) -> TypeFactsUpdate {
        let assets: BTreeMap<_, _> = scans
            .iter()
            .flat_map(|scan| scan.summary.assets.iter())
            .map(|asset| (asset.identity.clone(), asset.metadata.clone()))
            .collect();
        let modules = scans
            .iter()
            .zip(self.sources)
            .map(|(scan, source)| {
                let semantic_revision =
                    Fingerprint::of(format!("{:?}{assets:?}", source.type_facts));
                (
                    scan.module_id.clone(),
                    TypeFacts {
                        schema: TypeFacts::SCHEMA,
                        module_id: scan.module_id.clone(),
                        source_revision: scan.source_revision.clone(),
                        semantic_revision,
                        facts: source.type_facts.clone(),
                        assets: assets.clone(),
                    },
                )
            })
            .collect();
        TypeFactsUpdate {
            modules,
            diagnostics: Vec::new(),
        }
    }
}

#[napi]
pub struct NativeProjectSession {
    session: ProjectSession,
}

#[napi]
impl NativeProjectSession {
    #[napi(constructor)]
    pub fn new(loader: &NativeDesignSystemLoader, watch: bool) -> napi::Result<Self> {
        Ok(Self {
            session: ProjectSession::new(
                loader.completed()?,
                if watch {
                    SessionMode::Watch
                } else {
                    SessionMode::Build
                },
                loader.options.clone(),
            ),
        })
    }

    #[napi]
    pub fn reload(&mut self, loader: &NativeDesignSystemLoader) -> napi::Result<NativeReload> {
        let update = self
            .session
            .reload(Ok(loader.completed()?), loader.options.clone())
            .map_err(|error| Error::from_reason(error.to_string()))?;
        if !update.diagnostics.is_empty() {
            return Err(NativeSessionError::ReloadCompilation {
                diagnostics: update.diagnostics,
            }
            .into());
        }
        Ok(NativeReload {
            css_changed: update.css_changed,
            transformed_modules: update
                .transformed_modules
                .into_iter()
                .map(|module| module.0)
                .collect(),
        })
    }

    #[napi]
    pub fn css(&self) -> napi::Result<String> {
        self.session
            .css()
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Plans an explicit host selection and returns CSS with coverage and producer revisions.
    #[napi(strict)]
    pub fn plan_stylesheet(
        &self,
        input: NativeStylesheetSelection,
    ) -> napi::Result<NativeStylesheetPlan> {
        let coverage = match input.coverage {
            NativeCoverage::Observed => StylesheetCoverage::Observed,
            NativeCoverage::Complete { target, revision } => {
                StylesheetCoverage::Complete { target, revision }
            }
        };
        let placement = match input.placement {
            NativeStylesheetPlacement::Document => StylesheetPlacement::Document,
            NativeStylesheetPlacement::Carrier => StylesheetPlacement::Carrier,
        };
        let plan = self
            .session
            .plan_stylesheet(
                StylesheetSelection {
                    modules: input.modules.into_iter().map(Into::into).collect(),
                    coverage,
                },
                placement,
            )
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let coverage = match plan.coverage {
            StylesheetCoverage::Observed => NativeCoverage::Observed,
            StylesheetCoverage::Complete { target, revision } => {
                NativeCoverage::Complete { target, revision }
            }
        };
        Ok(NativeStylesheetPlan {
            css: plan.css,
            source_map: stylesheet_locations(&plan.mappings)?,
            resources: plan.resources.into_iter().map(Into::into).collect(),
            coverage,
            placement: input.placement,
            design_system: plan.design_system.0,
            producers: plan
                .producers
                .into_iter()
                .map(|(module, key)| NativeStylesheetProducer {
                    module: module.0,
                    source_revision: key.source_revision.0,
                    semantic_revision: key.semantic_revision.map(|revision| revision.0),
                    compiler_version: key.compiler_version,
                    compiler_options: key.compiler_options.0,
                    analysis_revision: key.analysis_revision.0,
                })
                .collect(),
        })
    }

    /// Stable document CSS independent of which JS modules have been visited.
    #[napi]
    pub fn base_css(&self) -> napi::Result<String> {
        StylesheetRenderer::new(&self.session.snapshot(), self.session.options())
            .render_base_stylesheet()
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    #[napi]
    pub fn base_css_with_map(&self) -> napi::Result<NativeStylesheetOutput> {
        let output = StylesheetRenderer::new(&self.session.snapshot(), self.session.options())
            .render_base_stylesheet_with_map()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(NativeStylesheetOutput {
            css: output.css,
            source_map: stylesheet_locations(&output.mappings)?,
        })
    }

    /// Complete CSS for one producer, safe to transport across loader workers.
    #[napi]
    pub fn module_css(&self, module: String) -> napi::Result<String> {
        let artifact = self
            .session
            .artifacts()
            .get(module.as_str())
            .ok_or_else(|| Error::from_reason(format!("Unknown Stillwind module: {module}")))?;
        StylesheetRenderer::new(&self.session.snapshot(), self.session.options())
            .render_module_stylesheet(&artifact.css.to_css())
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    #[napi]
    /// Removes a producer, returning whether it was registered.
    pub fn remove(&mut self, module: String) -> bool {
        self.session.remove_module(&module)
    }

    #[napi(strict)]
    pub fn compile(
        &mut self,
        sources: Vec<NativeProjectSource>,
        producer: Option<String>,
    ) -> napi::Result<ProjectBindingOutput> {
        let sources = sources
            .into_iter()
            .map(TryInto::try_into)
            .collect::<napi::Result<Vec<ProjectSource>>>()?;
        if let Some(producer) = &producer
            && !sources.iter().any(|source| &source.path == producer)
        {
            return Err(Error::from_reason(format!(
                "Unknown Stillwind producer: {producer}"
            )));
        }
        let scans: Vec<_> = sources
            .iter()
            .map(|source| {
                Ok::<_, Error>(OxcSourceAnalyzer.scan_input(source.input(
                    FileId(1),
                    FileId(3),
                    FileId(4),
                )?))
            })
            .collect::<Result<_, _>>()?;
        let facts = ProvidedTypeScriptFacts { sources: &sources }.update(&scans);
        let analyses: Vec<_> = scans
            .iter()
            .map(|scan| {
                OxcSourceAnalyzer.finalize(scan.clone(), facts.modules.get(&scan.module_id))
            })
            .collect();
        let snapshot = self.session.snapshot();
        let theme = SourceFile::new(FileId(2), "stillwind.css", "")
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let mut modules = Vec::new();
        for scan in scans {
            if producer
                .as_ref()
                .is_some_and(|producer| producer != scan.module_id.as_str())
            {
                continue;
            }
            let mut analysis =
                OxcSourceAnalyzer.finalize(scan.clone(), facts.modules.get(&scan.module_id));
            analysis.attach_asset_support(&analyses);
            let update = self.session.update_module(analysis);
            let mut result = self
                .session
                .artifacts()
                .get(&scan.module_id)
                .map(|artifact| artifact.to_module_output(&snapshot))
                .unwrap_or_default();
            if !update.diagnostics.is_empty() {
                result.diagnostics = update.diagnostics;
            }
            modules.push(ProjectModuleOutput::new(
                scan.module_id.0,
                binding_output(result, &scan.source, &theme),
            ));
        }
        Ok(ProjectBindingOutput { modules })
    }
}
