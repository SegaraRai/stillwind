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

use std::collections::BTreeMap;
use std::sync::Arc;

use napi::Error;
use napi::bindgen_prelude::Env;
use napi_derive::napi;

use stillwind_compiler::{
    CompilerOptions, IdentifierGenerator, IdentifierMode, Identifiers, StylesheetPlacement,
    StylesheetRenderer,
};
use stillwind_design_system::{
    CssFileInput, DesignSystemLoadStep, DesignSystemLoader, DesignSystemSnapshot,
};
use stillwind_diagnostics::CompileError;
use stillwind_ir::{ModuleScan, SourceAnalyzer, TypeScriptFactsProvider};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode, StylesheetCoverage, StylesheetSelection};
use stillwind_semantic::{ModuleId, TypeFacts, TypeFactsUpdate};
use stillwind_source::{FileId, Fingerprint, SourceFile};

use crate::api::binding_output;
use crate::naming::{NativeIdentifierNaming, NativeNaming};
use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::{ProjectBindingOutput, ProjectModuleOutput};
use crate::source_input::{NativeProjectSource, ProjectSource};
use crate::stylesheet::*;

/// Failures specific to the stateful bindings.
#[derive(Debug, thiserror::Error)]
pub enum NativeSessionError {
    /// A snapshot was requested before the CSS load completed.
    #[error("CSS load is incomplete")]
    IncompleteCssLoad,
    /// Recompiling retained modules against the new stylesheet failed. Watch sessions
    /// keep the previous revision active while reporting this error.
    #[error("CSS reload compilation failed: {}", format_reload_diagnostics(.diagnostics))]
    ReloadCompilation {
        /// Failures per module, keyed by module path.
        diagnostics: BTreeMap<ModuleId, Vec<CompileError>>,
    },
    /// Reserving a producer's own stylesheet gave a generated name an authored meaning.
    #[error("CSS a producer authors collides with generated names: {}", format_reload_diagnostics(.diagnostics))]
    AuthoredCssCollision {
        /// Failures per module, keyed by module path.
        diagnostics: BTreeMap<ModuleId, Vec<CompileError>>,
    },
}

impl From<NativeSessionError> for Error {
    fn from(error: NativeSessionError) -> Self {
        Self::from_reason(error.to_string())
    }
}

/// Joins failures in module-key order, preserving each module's diagnostic order.
///
/// Each entry contains the module ID, reporting code, and display message. Notes follow
/// on indented lines; labels are not serialized here. An empty map produces an empty string.
fn format_reload_diagnostics(diagnostics: &BTreeMap<ModuleId, Vec<CompileError>>) -> String {
    diagnostics
        .iter()
        .flat_map(|(module, errors)| {
            errors.iter().map(|error| {
                let mut message = format!("{}: {}: {error}", module.as_str(), error.code());
                for note in &error.notes {
                    message.push_str("\n  ");
                    message.push_str(note);
                }
                message
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One CSS source supplied by the host in one [`NativeDesignSystemLoader::provide`] call.
#[napi(object)]
pub struct NativeCssFileInput {
    /// Request id being answered; `0` is the root source.
    pub id: u32,
    /// Opaque host-chosen source name. The loader never interprets it.
    pub name: String,
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
    /// Creates a loader for the given compiler options.
    #[napi(constructor, strict)]
    pub fn new(options: Option<NativeCompilerOptions>) -> napi::Result<Self> {
        let options = compiler_options(options)?;
        Ok(Self {
            loader: DesignSystemLoader::new(options.design_system()),
            options,
            snapshot: None,
        })
    }

    /// Answers one outstanding request; equivalent to [`Self::provide_many`] with one file.
    #[napi]
    pub fn provide(
        &mut self,
        id: u32,
        name: String,
        content: String,
    ) -> napi::Result<NativeCssLoadStep> {
        self.provide_many(vec![NativeCssFileInput { id, name, content }])
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

    /// Generates the TypeScript augmentation for the completed snapshot.
    #[napi]
    pub fn generate_vocabulary_types(&self) -> napi::Result<String> {
        Ok(
            StylesheetRenderer::new(&self.completed()?, &self.options, &Identifiers::builtin())
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
        let identifiers = match &naming {
            Some(naming) => Identifiers::with(naming.clone() as Arc<dyn IdentifierGenerator>),
            None => Identifiers::builtin(),
        };
        Ok(Self {
            session: ProjectSession::new(
                loader.completed()?,
                if watch {
                    SessionMode::Watch
                } else {
                    SessionMode::Build
                },
                loader.options.clone(),
                identifiers.in_mode(identifier_mode),
            ),
            naming,
        })
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
                        producers: part.producers.into_iter().map(|id| id.0).collect(),
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

    /// Stable document CSS without a source map, independent of visited JS modules.
    #[napi]
    pub fn base_css(&self, env: Env) -> napi::Result<String> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
        StylesheetRenderer::new(
            &self.session.snapshot(),
            self.session.options(),
            self.session.identifiers(),
        )
        .render_base_stylesheet()
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
        .render_base_stylesheet_with_map()
        .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(NativeStylesheetOutput {
            css: output.css,
            source_map: stylesheet_locations(&output.mappings)?,
        })
    }

    /// Complete CSS for one producer, safe to transport across loader workers.
    #[napi]
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
    pub fn remove(&mut self, module: String) -> bool {
        self.session.remove_module(&module)
    }

    /// Reserves the names one producer's own stylesheet uses, or forgets it when `css` is absent.
    ///
    /// The design system's CSS is already reserved; a stylesheet a producer authors itself is not
    /// part of it, so the host supplies its text here. Returns the producers whose published
    /// artifacts now collide with an authored name, which the host recompiles or drops.
    #[napi]
    pub fn reserve_authored_css(
        &mut self,
        module: String,
        css: Option<String>,
    ) -> napi::Result<()> {
        let reported = match css {
            Some(css) => self.session.reserve_authored_css(&module, &css),
            None => self.session.forget_authored_css(&module),
        };
        if reported.is_empty() {
            return Ok(());
        }
        Err(NativeSessionError::AuthoredCssCollision {
            diagnostics: reported,
        }
        .into())
    }

    /// Scans, finalizes, and publishes the given modules.
    ///
    /// `producer` narrows the returned output to one module, but every supplied module
    /// is still compiled: cross-module values need the whole set.
    #[napi(strict)]
    pub fn compile(
        &mut self,
        env: Env,
        sources: Vec<NativeProjectSource>,
        producer: Option<String>,
    ) -> napi::Result<ProjectBindingOutput> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
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
        let mut transaction = self.session.clone();
        let mut attempted = Vec::new();
        for scan in scans {
            let mut analysis =
                OxcSourceAnalyzer.finalize(scan.clone(), facts.modules.get(&scan.module_id));
            analysis.attach_asset_support(&analyses);
            let update = transaction.update_module(analysis.clone());
            attempted.push(analysis);
            let mut result = transaction
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
        if modules.iter().all(|module| module.diagnostics.is_empty()) {
            self.session = transaction;
            if let Some(producer) = &producer {
                modules.retain(|module| &module.path == producer);
            }
        } else {
            self.session.reject_updates(attempted);
        }
        Ok(ProjectBindingOutput { modules })
    }
}
