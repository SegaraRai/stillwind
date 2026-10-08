//! Transfers compiled modules, generated identities, and project results.

use std::collections::BTreeMap;

use napi_derive::napi;
use serde::Serialize;

use stillwind_compiler::{
    GeneratedClass, GeneratedSymbol, GuardStep, ModuleResourceReference, RejectedIdentifier,
    SymbolOrigin,
};
use stillwind_ir::StyleDependency;
use stillwind_source::ModuleId;

use super::{
    BindingDiagnostic, BindingLintFinding, BindingReport, NativeSpan, NativeTransformMapping,
};
use crate::semantic::NativeIdentity;

/// Result of compiling one module.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingOutput {
    /// Composition edges this module declared or consumed.
    pub composition: Vec<NativeStyleDependency>,
    /// Assets and resources the module references.
    pub resources: Vec<NativeModuleResource>,
    /// Rendered CSS for this compilation; standalone entry points include document CSS.
    pub css: String,
    /// Completely transformed source, or `None` when compilation failed.
    pub code: Option<String>,
    /// Mappings from the transformed source back to the original.
    pub mappings: Vec<NativeTransformMapping>,
    /// Class names generated for this module's calls.
    pub classes: Vec<NativeGeneratedClass>,
    /// Generated identifiers and their identities.
    pub symbols: Vec<NativeGeneratedSymbol>,
    /// Errors. Non-empty means nothing was published for this module.
    pub diagnostics: Vec<BindingDiagnostic>,
    /// Source-resolved diagnostics, including related locations and fixes.
    pub reports: Vec<BindingReport>,
    /// Names a host-generated identifier was rejected for, which the host may warn about.
    ///
    /// These are not lint findings: a rejected name comes from the generator rather than
    /// from the source, so the entry carries the family, the request hash, and the reason
    /// instead of a source span.
    pub rejected_names: Vec<BindingRejectedIdentifier>,
    /// Non-fatal findings.
    pub lint_findings: Vec<BindingLintFinding>,
    /// Effective severity per lint rule.
    #[napi(ts_type = "Record<string, 'warn' | 'deny' | 'off'>")]
    pub lint_settings: BTreeMap<String, String>,
}

/// One composition edge: [`Self::derived`] includes the payload of [`Self::base`].
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeStyleDependency {
    /// The style call that consumes another payload.
    pub derived: NativeIdentity,
    /// The style call whose payload is consumed.
    pub base: NativeIdentity,
}

impl From<StyleDependency> for NativeStyleDependency {
    fn from(value: StyleDependency) -> Self {
        Self {
            derived: value.derived.into(),
            base: value.base.into(),
        }
    }
}

/// A `url()` reference the host must resolve.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeModuleResource {
    /// Placeholder left in the generated CSS.
    pub placeholder: String,
    /// Module that owns the reference.
    pub module_id: String,
    /// Authored specifier.
    pub specifier: String,
}

impl From<ModuleResourceReference> for NativeModuleResource {
    fn from(value: ModuleResourceReference) -> Self {
        Self {
            placeholder: value.placeholder,
            module_id: value.owner.into_string(),
            specifier: value.specifier,
        }
    }
}

/// One conditional test on the path that selects a generated class.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGuardStep {
    /// The guard expression, which the runtime evaluates as written.
    pub test: NativeSpan,
    /// Whether the class applies when the guard holds.
    pub when_true: bool,
}

impl From<GuardStep> for NativeGuardStep {
    fn from(value: GuardStep) -> Self {
        Self {
            test: value.test.into(),
            when_true: value.when_true,
        }
    }
}

/// A class name generated for one utility literal.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGeneratedClass {
    /// The generated class.
    pub class_name: String,
    /// Span of the call the class belongs to.
    pub call_span: NativeSpan,
    /// Outermost to innermost conditional tests that select this class.
    pub guard_path: Vec<NativeGuardStep>,
}

impl From<GeneratedClass> for NativeGeneratedClass {
    fn from(value: GeneratedClass) -> Self {
        Self {
            class_name: value.class_name,
            call_span: value.call_span.into(),
            guard_path: value.guard_path.into_iter().map(Into::into).collect(),
        }
    }
}

/// A generated identifier that host tooling must treat as reserved.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGeneratedSymbol {
    /// The generated name.
    pub name: String,
    /// Kind of declaration that produced it.
    #[napi(
        ts_type = "'style' | 'ref' | 'ref-marker' | 'keyframes' | 'view-transition' | 'class' | 'anchor' | 'container' | 'view-transition-class' | 'condition-helper' | 'rule-boundary' | 'view-transition-slot' | 'layer'"
    )]
    pub kind: String,
    /// Owner identity used to distinguish name reuse from a collision.
    pub identity: String,
    /// Domain of the source that owns this name: `module` or `stylesheet`.
    #[napi(ts_type = "'module' | 'stylesheet'")]
    pub source_kind: String,
    /// Identity within `source_kind`.
    pub source_id: String,
    /// Byte start of the declaration, absent for a name that declares nothing itself.
    pub start: Option<u32>,
    /// Byte end of the declaration, absent for a name that declares nothing itself.
    pub end: Option<u32>,
}

impl From<GeneratedSymbol> for NativeGeneratedSymbol {
    fn from(value: GeneratedSymbol) -> Self {
        Self {
            name: value.name.to_string(),
            kind: value.kind.as_str().to_owned(),
            identity: value.identity.to_string(),
            source_kind: match &value.origin {
                SymbolOrigin::Module(_) => "module",
                SymbolOrigin::Stylesheet(_) => "stylesheet",
            }
            .to_owned(),
            source_id: match value.origin {
                SymbolOrigin::Module(id) => id.into_string(),
                SymbolOrigin::Stylesheet(id) => id.as_str().to_owned(),
            },
            start: value.start,
            end: value.end,
        }
    }
}

/// One rejected generated name, reported for a host that supplies its own generators.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingRejectedIdentifier {
    /// Identifier family the generator was asked for.
    pub kind: String,
    /// Canonical hash of the request, which identifies the rejected input.
    pub hash: String,
    /// Why the name was rejected.
    pub reason: String,
    /// Whether the built-in name was substituted for it.
    pub substituted: bool,
}

impl From<RejectedIdentifier> for BindingRejectedIdentifier {
    fn from(value: RejectedIdentifier) -> Self {
        Self {
            kind: value.kind.as_str().to_owned(),
            hash: value.hash,
            reason: value.reason,
            substituted: value.substituted,
        }
    }
}

/// Output of a whole-graph compile, one entry per requested module.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBindingOutput {
    /// Per-module results. Rules shared between modules appear once.
    pub modules: Vec<ProjectModuleOutput>,
}

/// One module's result inside a whole-graph compile.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModuleOutput {
    /// Composition edges of this module.
    pub composition: Vec<NativeStyleDependency>,
    /// Host-chosen identity of this module.
    pub module_id: String,
    /// Assets and resources this module references.
    pub resources: Vec<NativeModuleResource>,
    /// Generated CSS. Rules shared with an earlier module are already removed.
    pub css: String,
    /// Completely transformed source, or `None` when compilation failed.
    pub code: Option<String>,
    /// Mappings from the transformed source back to the original.
    pub mappings: Vec<NativeTransformMapping>,
    /// Class names generated for this module's calls.
    pub classes: Vec<NativeGeneratedClass>,
    /// Generated identifiers and their identities.
    pub symbols: Vec<NativeGeneratedSymbol>,
    /// Errors. Non-empty means nothing was published for this module.
    pub diagnostics: Vec<BindingDiagnostic>,
    /// Source-resolved diagnostics for this module.
    pub reports: Vec<BindingReport>,
    /// Names a host-generated identifier was rejected for, which the host may warn about.
    pub rejected_names: Vec<BindingRejectedIdentifier>,
    /// Non-fatal findings.
    pub lint_findings: Vec<BindingLintFinding>,
    /// Effective severity per lint rule.
    #[napi(ts_type = "Record<string, 'warn' | 'deny' | 'off'>")]
    pub lint_settings: BTreeMap<String, String>,
}

impl ProjectModuleOutput {
    pub(crate) fn new(module_id: ModuleId, output: BindingOutput) -> Self {
        let BindingOutput {
            composition,
            resources,
            css,
            code,
            mappings,
            classes,
            symbols,
            diagnostics,
            reports,
            rejected_names,
            lint_findings,
            lint_settings,
        } = output;
        Self {
            composition,
            module_id: module_id.into_string(),
            resources,
            css,
            code,
            mappings,
            classes,
            symbols,
            diagnostics,
            reports,
            rejected_names,
            lint_findings,
            lint_settings,
        }
    }
}
