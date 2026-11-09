//! Registers direct and projected host inputs in their required source order.

use std::sync::Arc;

use napi::Error;
use napi_derive::napi;
use serde::Deserialize;

use stillwind_ir::{
    AdapterKind, DirectSource, PreparedSource, ProjectedSource, ScriptSyntax, SourceDocument,
    SourceInput,
};
use stillwind_semantic::TypeFact;
use stillwind_source::{ModuleId, SourceFile, SourceRegistry};

use super::{
    HostSourceDocument, HostSourceProjection, NativeFramework, NativeScriptSyntax,
    NativeSourceDocument, NativeSourceProjection,
};
use crate::semantic::NativeTypeFact;

/// Host input with an editable document and explicit parser preparation.
#[napi(
    discriminant = "kind",
    discriminant_case = "lowercase",
    object_to_js = false
)]
pub enum NativePreparedSource {
    /// Parses the editable script or JSX document directly.
    Direct {
        /// Editable source document.
        document: NativeSourceDocument,
        /// Parser settings for the document's code.
        syntax: NativeScriptSyntax,
        /// Host-chosen identity of this module.
        module_id: String,
        /// Facts the host produced for this module, empty when absent.
        type_facts: Option<Vec<NativeTypeFact>>,
        /// Source framework constraint for component projections. Macro outputs remain
        /// independent.
        framework: Option<NativeFramework>,
        /// Host-allocated stylesheet modules to insert at the macro import.
        stylesheet_imports: Option<Vec<String>>,
        /// Host-allocated module that exports this module's CSS value.
        module_css_import: Option<String>,
    },
    /// Parses code produced from an editable component document.
    Projected {
        /// Editable source document.
        document: NativeSourceDocument,
        /// Generated code and its correspondence to the document.
        projection: NativeSourceProjection,
        /// Host-chosen identity of this module.
        module_id: String,
        /// Facts the host produced for this module, empty when absent.
        type_facts: Option<Vec<NativeTypeFact>>,
        /// Source framework constraint for component projections. Macro outputs remain
        /// independent.
        framework: Option<NativeFramework>,
        /// Host-allocated stylesheet modules to insert at the macro import.
        stylesheet_imports: Option<Vec<String>>,
        /// Host-allocated module that exports this module's CSS value.
        module_css_import: Option<String>,
    },
}

/// Host transport whose parser buffer is direct or projected.
#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum HostPreparedSource {
    /// Parses an editable script or JSX document.
    Direct {
        /// Original editable document.
        document: HostSourceDocument,
        /// Parser settings for the document's code.
        syntax: ScriptSyntax,
        /// Host-chosen identity of this module.
        module_id: ModuleId,
        /// Facts the host produced for this module, empty when absent.
        #[serde(default)]
        type_facts: Vec<TypeFact>,
        /// Source framework constraint for component projections. Macro outputs remain
        /// independent.
        framework: Option<AdapterKind>,
        /// Host-allocated stylesheet modules.
        #[serde(default)]
        stylesheet_imports: Vec<String>,
        /// Host-allocated module that exports this module's CSS value.
        module_css_import: Option<String>,
    },
    /// Parses generated code while reporting and editing the component document.
    Projected {
        /// Original editable document.
        document: HostSourceDocument,
        /// Generated parser buffer and its correspondence to the document.
        projection: HostSourceProjection,
        /// Host-chosen identity of this module.
        module_id: ModuleId,
        /// Facts the host produced for this module, empty when absent.
        #[serde(default)]
        type_facts: Vec<TypeFact>,
        /// Source framework constraint for component projections. Macro outputs remain
        /// independent.
        framework: Option<AdapterKind>,
        /// Host-allocated stylesheet modules.
        #[serde(default)]
        stylesheet_imports: Vec<String>,
        /// Host-allocated module that exports this module's CSS value.
        module_css_import: Option<String>,
    },
}

impl HostPreparedSource {
    /// Returns the host identity used for graph edges and duplicate checks.
    pub const fn module_id(&self) -> &ModuleId {
        match self {
            Self::Direct { module_id, .. } | Self::Projected { module_id, .. } => module_id,
        }
    }

    /// Returns the host facts used to finalize this module's scan.
    pub fn type_facts(&self) -> &[TypeFact] {
        match self {
            Self::Direct { type_facts, .. } | Self::Projected { type_facts, .. } => type_facts,
        }
    }

    /// Registers the parser and editable buffers and returns the analyzer input.
    ///
    /// Projection regions and realms must use UTF-16 offsets in the document.
    /// Published spans refer to that document. A projected parser buffer has its own
    /// file ID, and its analysis buffer receives a reserved ID in `sources`.
    /// Projected inputs register the parser buffer first, the document second, and
    /// reserve the analysis ID third. Registrations remain in `sources` on failure.
    ///
    /// # Errors
    ///
    /// Returns an error for a component document without a projection, a projected
    /// script or JSX document, invalid UTF-16 coordinates, reversed ranges, exhausted
    /// file IDs, or invalid projection metadata rejected by core constructors.
    pub fn input(&self, sources: &mut SourceRegistry) -> napi::Result<SourceInput> {
        self.input_after(sources, None)
    }

    /// Returns the analyzer input like [`Self::input`], reusing `previous` when it fits.
    ///
    /// `previous` must be a buffer that `sources` retains, such as the source of this
    /// module's latest analysis. A direct input whose URI, source-map name, and text
    /// equal `previous` keeps its file ID instead of registering a new buffer. The
    /// equal ID keeps the artifact key equal, so the session reuses the compiled
    /// module. Projected inputs always register new buffers.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Self::input`].
    pub fn input_after(
        &self,
        sources: &mut SourceRegistry,
        previous: Option<&Arc<SourceFile>>,
    ) -> napi::Result<SourceInput> {
        let (
            document,
            syntax,
            projection,
            module_id,
            framework,
            stylesheet_imports,
            module_css_import,
        ) = match self {
            Self::Direct {
                document,
                syntax,
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
                ..
            } => (
                document,
                syntax,
                None,
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
            ),
            Self::Projected {
                document,
                projection,
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
                ..
            } => (
                document,
                &projection.syntax,
                Some(projection),
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
            ),
        };
        let code = parser_buffer(sources, document, projection, previous)?;
        let prepared = if let Some(projection) = projection {
            let original = sources
                .register(
                    Some(document.uri.clone()),
                    document.source_map_source.as_deref().map(Into::into),
                    document.code.as_str(),
                )
                .map_err(|error| Error::from_reason(error.to_string()))?;
            let projection = projection.input(code, &original, sources)?;
            let document = SourceDocument::new(
                original,
                document.format,
                (document.upstream_map.clone()).map(Into::into),
            );
            PreparedSource::Projected(
                ProjectedSource::try_new(document, projection)
                    .map_err(|error| Error::from_reason(error.to_string()))?,
            )
        } else {
            let document = SourceDocument::new(
                code,
                document.format,
                (document.upstream_map.clone()).map(Into::into),
            );
            PreparedSource::Direct(
                DirectSource::try_new(document, *syntax)
                    .map_err(|error| Error::from_reason(error.to_string()))?,
            )
        };
        let mut input = SourceInput::new(module_id.clone(), prepared);
        input.framework = *framework;
        input.stylesheet_imports = stylesheet_imports
            .iter()
            .map(|value| value.as_str().into())
            .collect();
        input.module_css_import = module_css_import.as_deref().map(Into::into);
        Ok(input)
    }
}

/// Returns the parser buffer of `document`, reusing `previous` when it fits.
///
/// A direct document whose URI, source-map name, and text equal `previous` gets
/// `previous`, which `sources` then retains. Otherwise the buffer is registered: a
/// projection's generated code without a URI, or the document itself.
///
/// # Errors
///
/// Returns an error when `previous` conflicts with a retained buffer, or registration
/// fails because the text is too large or file IDs are exhausted.
fn parser_buffer(
    sources: &mut SourceRegistry,
    document: &HostSourceDocument,
    projection: Option<&HostSourceProjection>,
    previous: Option<&Arc<SourceFile>>,
) -> napi::Result<Arc<SourceFile>> {
    let reusable = previous.filter(|previous| {
        projection.is_none()
            && previous.uri() == Some(&document.uri)
            && previous.source_map_source() == document.source_map_source.as_deref()
            && previous.text() == document.code
    });
    if let Some(previous) = reusable {
        sources
            .retain(Arc::clone(previous))
            .map_err(|error| Error::from_reason(error.to_string()))?;
        return Ok(Arc::clone(previous));
    }
    let registered = match projection {
        Some(projection) => sources.register(None, None, projection.code.as_str()),
        None => sources.register(
            Some(document.uri.clone()),
            document.source_map_source.as_deref().map(Into::into),
            document.code.as_str(),
        ),
    };
    registered.map_err(|error| Error::from_reason(error.to_string()))
}

impl TryFrom<NativePreparedSource> for HostPreparedSource {
    type Error = Error;

    fn try_from(value: NativePreparedSource) -> napi::Result<Self> {
        match value {
            NativePreparedSource::Direct {
                document,
                syntax,
                module_id,
                type_facts,
                framework,
                stylesheet_imports,
                module_css_import,
            } => Ok(Self::Direct {
                document: document.try_into()?,
                syntax: syntax.into(),
                module_id: module_id.into(),
                type_facts: type_facts
                    .unwrap_or_default()
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<napi::Result<_>>()?,
                framework: framework.map(Into::into),
                stylesheet_imports: stylesheet_imports.unwrap_or_default(),
                module_css_import,
            }),
            NativePreparedSource::Projected {
                document,
                projection,
                module_id,
                type_facts,
                framework,
                stylesheet_imports,
                module_css_import,
            } => Ok(Self::Projected {
                document: document.try_into()?,
                projection: projection.into(),
                module_id: module_id.into(),
                type_facts: type_facts
                    .unwrap_or_default()
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<napi::Result<_>>()?,
                framework: framework.map(Into::into),
                stylesheet_imports: stylesheet_imports.unwrap_or_default(),
                module_css_import,
            }),
        }
    }
}
