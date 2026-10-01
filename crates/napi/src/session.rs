//! Stateful Node host binding. Resolution and I/O remain in JavaScript.
//!
//! [`NativeDesignSystemLoader`] exposes the loader state machine directly: it returns
//! the outstanding CSS requests and accepts the host's answers, so the JavaScript side
//! keeps full control of resolution, package lookup, and reading order. The snapshot
//! only exists after a `Complete` step, and every method that needs it reports
//! [`NativeSessionError::IncompleteCssLoad`] instead of loading anything itself.
//!
//! [`NativeProjectSession`] wraps the project session. It receives TypeScript facts the
//! host already produced through its long-lived TS7 service rather than running a
//! checker of its own.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use napi::Error;
use napi::bindgen_prelude::Env;
use napi_derive::napi;

use stillwind_compiler::{
    CompileResult, CompilerOptions, IdentifierGenerator, IdentifierMode, Identifiers,
    MappedStylesheet, StylesheetPlacement, StylesheetRenderer,
};
use stillwind_design_system::{
    CssFileInput, DesignSystemError, DesignSystemLoadStep, DesignSystemLoader, DesignSystemSnapshot,
};
use stillwind_diagnostics::{CompileError, DiagnosticBatch, LintSettings, Reporter};
use stillwind_ir::{ModuleScan, SourceAnalyzer, TypeScriptFactsProvider};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode, StylesheetCoverage, StylesheetSelection};
use stillwind_semantic::{TypeFacts, TypeFactsUpdate};
use stillwind_source::{Fingerprint, ModuleId, SourceRegistry, SourceUri};

use crate::api::binding_output_with_batch;
use crate::naming::{NativeIdentifierNaming, NativeNaming};
use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::{
    BindingReport, NativeDiagnosticBatch, ProjectBindingOutput, ProjectModuleOutput,
};
use crate::source_input::{NativePreparedSource, PreparedSource};
use crate::stylesheet::{
    NativeCoverage, NativeCssLoadStep, NativeCssRequest, NativeCssResource, NativeReload,
    NativeStylesheetOutput, NativeStylesheetPart, NativeStylesheetPlacement, NativeStylesheetPlan,
    NativeStylesheetProducer, NativeStylesheetSelection, stylesheet_locations,
};

/// Failures specific to the stateful bindings.
#[derive(Debug, thiserror::Error)]
pub enum NativeSessionError {
    /// A snapshot was requested before the CSS load completed.
    #[error("CSS load is incomplete")]
    IncompleteCssLoad,
    /// A loader created outside this session cannot issue collision-free source IDs.
    #[error("CSS loader belongs to another source scope; create it with session.newLoader()")]
    ForeignCssLoader,
    /// Recompiling retained modules against the new stylesheet failed. Watch sessions
    /// keep the previous revision active while reporting this error.
    #[error("CSS reload compilation failed: {message}")]
    ReloadCompilation {
        /// Source-resolved failures for the attempted CSS revision.
        message: String,
    },
    /// Reserving a producer's own stylesheet gave a generated name an authored meaning.
    #[error("CSS a producer authors collides with generated names: {message}")]
    AuthoredCssCollision {
        /// Source-resolved name collisions.
        message: String,
    },
}

impl From<NativeSessionError> for Error {
    fn from(error: NativeSessionError) -> Self {
        Self::from_reason(error.to_string())
    }
}

/// Resolves each module's failures and renders text for a throwing Node call.
///
/// # Errors
///
/// Returns an error if any span cannot be resolved through the project registry.
fn resolve_error_map(
    diagnostics: &BTreeMap<ModuleId, Vec<CompileError>>,
    sources: &SourceRegistry,
) -> napi::Result<(BTreeMap<ModuleId, DiagnosticBatch>, String)> {
    let mut batches = BTreeMap::new();
    let mut lines = Vec::new();
    for (module, errors) in diagnostics {
        let batch = Reporter::collect(errors, &[], sources, LintSettings::default())
            .map_err(|error| Error::from_reason(error.to_string()))?;
        for diagnostic in &batch.diagnostics {
            lines.push(format!(
                "{}: {}",
                module.as_str(),
                diagnostic.display_text()
            ));
        }
        batches.insert(module.clone(), batch);
    }
    Ok((batches, lines.join("\n")))
}

/// One CSS source supplied by the host in one [`NativeDesignSystemLoader::provide`] call.
#[napi(object)]
pub struct NativeCssFileInput {
    /// Request id being answered; `0` is the root source.
    pub id: u32,
    /// Opaque host-chosen CSS resolution name, compared verbatim by the loader.
    pub name: String,
    /// Absolute document URI supplied by the host; absent means an internal buffer.
    pub uri: Option<String>,
    /// Exact source name to write into a generated source map.
    pub source_map_source: Option<String>,
    /// Full text of the source.
    pub content: String,
}

/// Caller-driven design-system loader.
///
/// Feed the root source first, then answer each requested id. The loader reports parse
/// and link failures; a host that cannot read a requested file reports its own error
/// and drops the loader.
#[napi]
pub struct NativeDesignSystemLoader {
    loader: DesignSystemLoader,
    options: CompilerOptions,
    snapshot: Option<DesignSystemSnapshot>,
}

#[napi]
impl NativeDesignSystemLoader {
    fn with_sources(
        options: Option<NativeCompilerOptions>,
        sources: SourceRegistry,
    ) -> napi::Result<Self> {
        let options = compiler_options(options)?;
        Ok(Self {
            loader: DesignSystemLoader::new(options.design_system(), sources),
            options,
            snapshot: None,
        })
    }

    /// Creates a loader for the given compiler options.
    #[napi(constructor, strict)]
    pub fn new(options: Option<NativeCompilerOptions>) -> napi::Result<Self> {
        Self::with_sources(options, SourceRegistry::default())
    }

    /// Answers one outstanding request; equivalent to [`Self::provide_many`] with one file.
    /// An absent URI marks the CSS as an internal buffer without a reportable location.
    #[napi]
    pub fn provide(
        &mut self,
        id: u32,
        name: String,
        content: String,
        uri: Option<String>,
        source_map_source: Option<String>,
    ) -> napi::Result<NativeCssLoadStep> {
        self.provide_many(vec![NativeCssFileInput {
            id,
            name,
            uri,
            source_map_source,
            content,
        }])
    }

    /// Answers any subset of the outstanding requests, in any order.
    ///
    /// A partially supplied batch does not block parsing: each supplied source is
    /// parsed immediately and its imports appear in the next step. Unknown or duplicate
    /// ids are protocol errors.
    #[napi(strict)]
    pub fn provide_many(
        &mut self,
        files: Vec<NativeCssFileInput>,
    ) -> napi::Result<NativeCssLoadStep> {
        let files = files
            .into_iter()
            .map(|file| {
                let uri = file
                    .uri
                    .map(SourceUri::parse)
                    .transpose()
                    .map_err(|error| Error::from_reason(error.to_string()))?;
                Ok(CssFileInput {
                    id: file.id,
                    name: file.name.as_str().into(),
                    uri,
                    source_map_source: file.source_map_source.map(Into::into),
                    content: file.content.into(),
                })
            })
            .collect::<napi::Result<Vec<_>>>()?;
        match self.loader.provide_many(files) {
            DesignSystemLoadStep::NeedFile(requests) => Ok(NativeCssLoadStep::NeedFile {
                requests: requests
                    .into_iter()
                    .map(|request| NativeCssRequest {
                        id: request.id,
                        importer: request.importer.as_str().to_owned(),
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
                            source_name: resource.source_name.as_str().to_owned(),
                            specifier: resource.specifier.clone(),
                            start: resource.span.start.0,
                            end: resource.span.end.0,
                        })
                        .collect(),
                    sources: snapshot
                        .sources()
                        .sources
                        .iter()
                        .map(|source| source.name.as_str().to_owned())
                        .collect(),
                };
                self.snapshot = Some(snapshot);
                Ok(output)
            }
            DesignSystemLoadStep::Complete(Err(error)) => {
                if let Some(root) = self
                    .loader
                    .root_source()
                    .filter(|source| source.uri().is_some())
                {
                    let compiled =
                        DesignSystemError::Load(error.clone()).into_compile_error(root.full_span());
                    let text = Reporter::collect(
                        std::slice::from_ref(&compiled),
                        &[],
                        self.loader.sources(),
                        LintSettings::default(),
                    )
                    .map_err(|failure| Error::from_reason(failure.to_string()))?
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.display_text())
                    .collect::<Vec<_>>()
                    .join("\n");
                    Err(Error::from_reason(text))
                } else {
                    Err(Error::from_reason(error.to_string()))
                }
            }
        }
    }

    /// Generates the TypeScript augmentation for the completed snapshot.
    #[napi]
    pub fn generate_vocabulary_types(&self) -> napi::Result<String> {
        Ok(
            StylesheetRenderer::new(&self.completed()?, &self.options, &Identifiers::default())
                .generate_vocabulary_types(),
        )
    }
}

impl NativeDesignSystemLoader {
    /// Snapshot of a completed load, or an error naming the missing step.
    fn completed(&self) -> Result<DesignSystemSnapshot, NativeSessionError> {
        self.snapshot
            .clone()
            .ok_or(NativeSessionError::IncompleteCssLoad)
    }
}

/// Adapter for facts already produced by the host's long-lived TS7 service.
///
/// The host pairs its facts with the scans it sent; this adapter only wraps them with
/// the module and asset metadata the domain type requires.
struct ProvidedTypeScriptFacts<'a> {
    /// Host sources in the same order as the scans passed to `update`.
    sources: &'a [PreparedSource],
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
                    Fingerprint::of(format!("{:?}{assets:?}", source.type_facts()));
                (
                    scan.module_id.clone(),
                    TypeFacts {
                        schema: TypeFacts::SCHEMA,
                        module_id: scan.module_id.clone(),
                        source_revision: scan.source_revision.clone(),
                        semantic_revision,
                        facts: source.type_facts().to_vec(),
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

/// Mutable project state for one Vite or CLI session.
#[napi]
pub struct NativeProjectSession {
    /// Project session.
    session: ProjectSession,
    /// Host name generators, which are callable only during a host call.
    naming: Option<Arc<NativeNaming>>,
}

#[napi]
impl NativeProjectSession {
    /// Returns the latest attempted diagnostics for every retained module.
    #[napi]
    pub fn diagnostic_batches(&self) -> Vec<NativeDiagnosticBatch> {
        self.session
            .diagnostic_batches()
            .iter()
            .map(|(module, batch)| NativeDiagnosticBatch {
                module: module.as_str().to_owned(),
                reports: batch
                    .diagnostics
                    .iter()
                    .cloned()
                    .map(BindingReport::from)
                    .collect(),
            })
            .collect()
    }

    /// Creates a session from a completed loader. `watch` selects whether a failed
    /// reload retains the last known good revision.
    ///
    /// `mode` is `"development"` for a dev server and anything else for a release
    /// build; it decides whether a rejected host-supplied identifier name is reported
    /// as a lint finding or fails the compilation. A development session is offered to
    /// a dev server, where usable output matters more than a fatal diagnostic.
    ///
    /// `naming` supplies the host's own name generators, one optional function per
    /// family. A family whose function returns `undefined` keeps the compiler's name.
    /// Functions must be pure: the same request has to produce the same name. Pair them
    /// with a matching `naming_revision` so cached artifacts are not reused across a
    /// change to them.
    #[napi(constructor)]
    #[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
    pub fn new(
        loader: &NativeDesignSystemLoader,
        watch: bool,
        mode: Option<String>,
        naming: Option<NativeIdentifierNaming>,
    ) -> napi::Result<Self> {
        let identifier_mode = match mode.as_deref() {
            Some("development") => IdentifierMode::Development,
            _ => IdentifierMode::Release,
        };
        let naming = NativeNaming::from_js(naming)?;
        let identifiers = Identifiers::new(
            naming
                .as_ref()
                .map(|naming| naming.clone() as Arc<dyn IdentifierGenerator>),
            identifier_mode,
        );
        let snapshot = loader.completed()?;
        let session = ProjectSession::new(
            snapshot.clone(),
            loader.loader.sources(),
            if watch {
                SessionMode::Watch
            } else {
                SessionMode::Build
            },
            loader.options.clone(),
            identifiers,
        );
        Ok(Self { session, naming })
    }

    /// Creates a CSS loader in this session's source ID scope for a reload.
    #[napi]
    pub fn new_loader(
        &self,
        options: Option<NativeCompilerOptions>,
    ) -> napi::Result<NativeDesignSystemLoader> {
        NativeDesignSystemLoader::with_sources(options, self.session.sources().fork_empty())
    }

    /// Publishes a reloaded design system and reports what changed.
    ///
    /// Recompiling retained modules can fail; the error then lists the failing modules.
    /// Watch mode retains the previous revision. Build mode has already published the
    /// new revision with failed modules omitted when this error is returned.
    #[napi]
    pub fn reload(
        &mut self,
        env: Env,
        loader: &NativeDesignSystemLoader,
    ) -> napi::Result<NativeReload> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
        if !self
            .session
            .sources()
            .shares_ids_with(loader.loader.sources())
        {
            return Err(NativeSessionError::ForeignCssLoader.into());
        }
        let snapshot = loader.completed()?;
        let update = self
            .session
            .reload(Ok(snapshot.clone()), loader.options.clone())
            .map_err(|error| Error::from_reason(error.to_string()))?;
        if !update.diagnostics.is_empty() {
            let sources = self
                .session
                .diagnostic_sources_for(&snapshot)
                .map_err(|error| Error::from_reason(error.to_string()))?;
            let (mut batches, message) = resolve_error_map(&update.diagnostics, &sources)?;
            let modules = self.session.analyses().keys().cloned().collect::<Vec<_>>();
            for module in modules {
                let batch = batches.remove(&module).unwrap_or_default();
                self.session.publish_diagnostic_batch(module, batch);
            }
            self.session
                .prune_sources()
                .map_err(|error| Error::from_reason(error.to_string()))?;
            return Err(NativeSessionError::ReloadCompilation { message }.into());
        }
        Self::refresh_batches(&mut self.session)?;
        Ok(NativeReload {
            css_changed: update.css_changed,
            transformed_modules: update
                .transformed_modules
                .into_iter()
                .map(ModuleId::into_string)
                .collect(),
        })
    }

    /// Renders the complete stylesheet for the current revision.
    #[napi]
    pub fn css(&self, env: Env) -> napi::Result<String> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
        self.session
            .css()
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Plans an explicit host selection and returns CSS with coverage and producer revisions.
    #[napi(strict)]
    pub fn plan_stylesheet(
        &self,
        env: Env,
        input: NativeStylesheetSelection,
    ) -> napi::Result<NativeStylesheetPlan> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
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
            parts: plan
                .parts
                .into_iter()
                .map(|part| {
                    Ok(NativeStylesheetPart {
                        source_map: stylesheet_locations(&part.mappings)?,
                        slot: u32::try_from(part.slot)
                            .map_err(|_| Error::from_reason("Stylesheet slot exceeds 32 bits"))?,
                        sequence: u32::try_from(part.sequence).map_err(|_| {
                            Error::from_reason("Stylesheet sequence exceeds 32 bits")
                        })?,
                        key: part.key.0,
                        css: part.css,
                        producers: part
                            .producers
                            .into_iter()
                            .map(ModuleId::into_string)
                            .collect(),
                    })
                })
                .collect::<napi::Result<_>>()?,
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
                    module: module.into_string(),
                    source_revision: key.source_revision.0,
                    semantic_revision: key.semantic_revision.map(|revision| revision.0),
                    compiler_version: key.compiler_version,
                    compiler_options: key.compiler_options.0,
                    analysis_revision: key.analysis_revision.0,
                })
                .collect(),
        })
    }

    /// Stable document CSS without a source map, independent of visited JS modules.
    #[napi]
    pub fn base_css(&self, env: Env) -> napi::Result<String> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
        StylesheetRenderer::new(
            &self.session.snapshot(),
            self.session.options(),
            self.session.identifiers(),
        )
        .render_stylesheet("")
        .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Base document CSS with a source map, for review tooling.
    #[napi]
    pub fn base_css_with_map(&self, env: Env) -> napi::Result<NativeStylesheetOutput> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
        let output = StylesheetRenderer::new(
            &self.session.snapshot(),
            self.session.options(),
            self.session.identifiers(),
        )
        .render_mapped_stylesheet(&MappedStylesheet::default(), StylesheetPlacement::Document)
        .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(NativeStylesheetOutput {
            css: output.css,
            source_map: stylesheet_locations(&output.mappings)?,
        })
    }

    /// Complete CSS for one producer, safe to transport across loader workers.
    #[napi]
    #[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
    pub fn module_css(&self, env: Env, module: String) -> napi::Result<String> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
        self.session
            .plan_stylesheet(
                StylesheetSelection {
                    modules: [module.into()].into(),
                    coverage: StylesheetCoverage::Observed,
                },
                StylesheetPlacement::Carrier,
            )
            .map(|output| output.css)
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    /// Removes a producer, returning whether it was registered.
    #[napi]
    #[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
    pub fn remove(&mut self, module: String) -> napi::Result<bool> {
        let removed = self.session.remove_module(&ModuleId::from(module));
        Self::refresh_batches(&mut self.session)?;
        Ok(removed)
    }

    /// Reserves the names one producer's own stylesheet uses, or forgets it when `css` is absent.
    ///
    /// The design system's CSS is already reserved; a stylesheet a producer authors itself is not
    /// part of it, so the host supplies its text here. Returns the producers whose published
    /// artifacts now collide with an authored name, which the host recompiles or drops.
    #[napi]
    #[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
    pub fn reserve_authored_css(
        &mut self,
        module: String,
        css: Option<String>,
    ) -> napi::Result<()> {
        let reported = match css {
            Some(css) => self
                .session
                .reserve_authored_css(&ModuleId::from(module), &css),
            None => self.session.forget_authored_css(&ModuleId::from(module)),
        };
        if reported.is_empty() {
            Self::refresh_batches(&mut self.session)?;
            return Ok(());
        }
        let sources = self
            .session
            .diagnostic_sources()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let (batches, message) = resolve_error_map(&reported, &sources)?;
        for (module, batch) in batches {
            self.session.publish_diagnostic_batch(module, batch);
        }
        self.session
            .prune_sources()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Err(NativeSessionError::AuthoredCssCollision { message }.into())
    }

    /// Scans, finalizes, and publishes the given modules.
    ///
    /// `producer` narrows the returned output to one module, but every supplied module
    /// is still compiled: cross-module values need the whole set.
    #[napi(strict)]
    #[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
    pub fn compile(
        &mut self,
        env: Env,
        sources: Vec<NativePreparedSource>,
        producer: Option<String>,
    ) -> napi::Result<ProjectBindingOutput> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
        let producer = producer.map(ModuleId::from);
        let sources = sources
            .into_iter()
            .map(TryInto::try_into)
            .collect::<napi::Result<Vec<PreparedSource>>>()?;
        let mut module_ids = BTreeSet::new();
        for source in &sources {
            if !module_ids.insert(source.module_id()) {
                return Err(Error::from_reason(format!(
                    "Duplicate Stillwind module identity: {}",
                    source.module_id()
                )));
            }
        }
        if let Some(producer) = &producer
            && !sources.iter().any(|source| source.module_id() == producer)
        {
            return Err(Error::from_reason(format!(
                "Unknown Stillwind producer: {producer}"
            )));
        }
        let mut transaction = self.session.clone();
        let scans: Vec<_> = sources
            .iter()
            .map(|source| {
                Ok::<_, Error>(
                    OxcSourceAnalyzer.scan_input(source.input(transaction.sources_mut())?),
                )
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
        let mut modules = Vec::new();
        let mut attempted = Vec::new();
        let diagnostic_sources = transaction
            .diagnostic_sources()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        for scan in scans {
            let mut module_analysis =
                OxcSourceAnalyzer.finalize(scan.clone(), facts.modules.get(&scan.module_id));
            module_analysis.attach_asset_support(&analyses);
            let update = transaction.update_module(module_analysis.clone());
            attempted.push(module_analysis);
            let result = if update.diagnostics.is_empty() {
                transaction
                    .artifacts()
                    .get(&scan.module_id)
                    .map(|artifact| artifact.to_module_output(&snapshot))
                    .unwrap_or_default()
            } else {
                CompileResult {
                    diagnostics: update.diagnostics,
                    lint_settings: snapshot.theme().config.lint_settings.clone(),
                    ..CompileResult::default()
                }
            };
            let (output, batch) =
                binding_output_with_batch(result, &scan.source, &diagnostic_sources)?;
            transaction.publish_diagnostic_batch(scan.module_id.clone(), batch);
            modules.push(ProjectModuleOutput::new(scan.module_id, output));
        }
        if modules.iter().all(|module| module.diagnostics.is_empty()) {
            transaction
                .prune_sources()
                .map_err(|error| Error::from_reason(error.to_string()))?;
            self.session = transaction;
            if let Some(producer) = &producer {
                modules.retain(|module| module.module_id == producer.as_str());
            }
        } else {
            self.session.reject_updates(attempted);
            self.session.retain_attempted_sources(&transaction);
            self.session.retain_attempted_diagnostics(&transaction);
            self.session
                .prune_sources()
                .map_err(|error| Error::from_reason(error.to_string()))?;
        }
        Ok(ProjectBindingOutput { modules })
    }
}

impl NativeProjectSession {
    /// Rebuilds reports for published artifacts after a graph or CSS state change.
    ///
    /// # Errors
    ///
    /// Returns an error when a retained artifact has an unreportable source span.
    fn refresh_batches(session: &mut ProjectSession) -> napi::Result<()> {
        let snapshot = session.snapshot();
        let refreshed = session
            .analyses()
            .iter()
            .filter_map(|(module, analysis)| {
                session.current_artifact(module).map(|artifact| {
                    (
                        module.clone(),
                        analysis.source.clone(),
                        artifact.to_module_output(&snapshot),
                    )
                })
            })
            .collect::<Vec<_>>();
        let diagnostic_sources = session
            .diagnostic_sources()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        for (module, source, result) in refreshed {
            let (_, batch) = binding_output_with_batch(result, &source, &diagnostic_sources)?;
            session.publish_diagnostic_batch(module, batch);
        }
        session
            .prune_sources()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(())
    }
}
