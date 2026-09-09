//! Stateful Node host binding. Resolution and I/O remain in JavaScript.
use std::{collections::BTreeMap, sync::Arc};

use napi::{Error, Result};
use napi_derive::napi;
use serde_json::json;
use stillwind_core::{
    DesignSystemLoadStep, DesignSystemLoader, DesignSystemSnapshot, Fingerprint, ModuleScan,
    OxcSourceAnalyzer, ProjectSession, SessionMode, SourceAnalyzer, TypeFacts, TypeFactsUpdate,
    TypeScriptFactsProvider,
};
use stillwind_source::{FileId, SourceFile};

use crate::{
    ProjectBindingOutput, ProjectModuleOutput, ProjectSource, binding_output, parse_options,
};

#[napi(object)]
pub struct NativeCssFileInput {
    pub id: u32,
    pub name: String,
    pub content: String,
}

#[napi]
pub struct NativeDesignSystemLoader {
    loader: DesignSystemLoader,
    snapshot: Option<DesignSystemSnapshot>,
}

#[napi]
impl NativeDesignSystemLoader {
    #[napi(constructor)]
    pub fn new(options_json: Option<String>) -> Result<Self> {
        Ok(Self {
            loader: DesignSystemLoader::new(parse_options(options_json.as_deref())?),
            snapshot: None,
        })
    }

    #[napi]
    pub fn provide(&mut self, id: u32, name: String, content: String) -> Result<String> {
        self.provide_many(vec![NativeCssFileInput { id, name, content }])
    }

    #[napi]
    pub fn provide_many(&mut self, files: Vec<NativeCssFileInput>) -> Result<String> {
        match self.loader.provide_many(files.into_iter().map(|file| stillwind_core::CssFileInput {
            id: file.id, name: file.name.as_str().into(), content: file.content.into()
        })) {
            DesignSystemLoadStep::NeedFile(requests) => Ok(json!({"kind":"needFile", "requests": requests.iter().map(|request| json!({"id":request.id,"importer":request.importer.0.as_ref(),"specifier":request.specifier})).collect::<Vec<_>>()}).to_string()),
            DesignSystemLoadStep::Complete(Ok(snapshot)) => {
                let output = json!({"kind":"complete", "revision":snapshot.revision().0.to_string(), "sources":snapshot.sources().sources.iter().map(|source| source.name.0.as_ref()).collect::<Vec<_>>()}).to_string();
                self.snapshot = Some(snapshot);
                Ok(output)
            }
            DesignSystemLoadStep::Complete(Err(error)) => Err(Error::from_reason(error.to_string())),
        }
    }

    #[napi]
    pub fn generate_react_types(&self) -> Result<String> {
        Ok(self.completed()?.generate_react_types())
    }
}

impl NativeDesignSystemLoader {
    fn completed(&self) -> Result<DesignSystemSnapshot> {
        self.snapshot
            .clone()
            .ok_or_else(|| Error::from_reason("CSS load is incomplete"))
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
    pub fn new(loader: &NativeDesignSystemLoader, watch: bool) -> Result<Self> {
        Ok(Self {
            session: ProjectSession::new(
                loader.completed()?,
                if watch {
                    SessionMode::Watch
                } else {
                    SessionMode::Build
                },
            ),
        })
    }

    #[napi]
    pub fn reload(&mut self, loader: &NativeDesignSystemLoader) -> Result<String> {
        let update = self
            .session
            .reload(Ok(loader.completed()?))
            .map_err(|error| Error::from_reason(error.to_string()))?;
        if !update.diagnostics.is_empty() {
            return Err(Error::from_reason(format!(
                "CSS reload compilation failed: {:?}",
                update.diagnostics
            )));
        }
        Ok(json!({"cssChanged":update.css_changed,"transformedModules":update.transformed_modules}).to_string())
    }

    #[napi]
    pub fn css(&self) -> String {
        self.session.css()
    }

    #[napi]
    pub fn remove(&mut self, module: String) -> bool {
        self.session.remove_module(&module).css_changed
    }

    #[napi]
    pub fn compile(&mut self, sources_json: String) -> Result<String> {
        let sources: Vec<ProjectSource> = serde_json::from_str(&sources_json)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let scans: Vec<_> = sources
            .iter()
            .map(|source| {
                OxcSourceAnalyzer.scan(Arc::new(SourceFile::new(
                    FileId(1),
                    source.path.as_str(),
                    source.source.as_str(),
                )))
            })
            .collect();
        let facts = ProvidedTypeScriptFacts { sources: &sources }.update(&scans);
        let snapshot = self.session.snapshot();
        let theme = SourceFile::new(FileId(2), "stillwind.css", "");
        let mut modules = Vec::new();
        for scan in scans {
            let analysis =
                OxcSourceAnalyzer.finalize(scan.clone(), facts.modules.get(&scan.module_id));
            let update = self.session.update_module(analysis);
            let mut result = self
                .session
                .artifacts()
                .get(&scan.module_id)
                .map(|artifact| artifact.to_legacy(&snapshot))
                .unwrap_or_default();
            if let Some(diagnostics) = update.diagnostics.get(&scan.module_id) {
                result.diagnostics.clone_from(diagnostics);
            }
            result.css.clone_from(&result.module_css);
            modules.push(ProjectModuleOutput {
                path: scan.module_id,
                output: binding_output(result, &scan.source, &theme),
            });
        }
        serde_json::to_string(&ProjectBindingOutput { modules })
            .map_err(|error| Error::from_reason(error.to_string()))
    }
}
