//! Immutable source sites and CSS previews from finalized module analysis.
//!
//! `NativeProjectSession.editorModules` creates captures. Their ranges use original
//! UTF-8 byte offsets, and later preview queries retain the captured session inputs.

use std::rc::Rc;

use napi::Env;
use napi_derive::napi;

use stillwind_compiler::{EditorSiteKind, ModuleEditor};

use crate::editor::NativeCandidateContext;
use crate::naming::NativeNaming;
use crate::output::NativeText;
use crate::semantic::NativeIdentity;

/// The source object addressed by a preview.
#[napi(string_enum = "camelCase")]
pub enum NativeEditorSiteKind {
    /// Serialized as `style` for a complete style call.
    Style,
    /// Serialized as `definition` for a keyframes or view-transition declaration.
    Definition,
    /// Serialized as `use` for a rule-bearing receiver or whole scope call.
    Use,
    /// Serialized as `input` for a literal interpreted by a compiler resolver.
    Input,
    /// Serialized as `stylesheet` for a `style.css()` call.
    Stylesheet,
}

impl From<EditorSiteKind> for NativeEditorSiteKind {
    fn from(value: EditorSiteKind) -> Self {
        match value {
            EditorSiteKind::Style => Self::Style,
            EditorSiteKind::Definition => Self::Definition,
            EditorSiteKind::Use => Self::Use,
            EditorSiteKind::Input => Self::Input,
            EditorSiteKind::Stylesheet => Self::Stylesheet,
        }
    }
}

/// Generated output associated with an original module occurrence.
#[napi(object, object_from_js = false)]
pub struct NativeEditorSite {
    /// Inclusive UTF-8 byte boundary in the original source.
    pub start: u32,
    /// Exclusive UTF-8 byte boundary in the original source.
    pub end: u32,
    /// Meaning of the occurrence.
    pub kind: NativeEditorSiteKind,
    /// Declaration or complete style-call identity. Literal inputs and use sites omit
    /// it.
    pub identity: Option<NativeIdentity>,
    /// Resolver contract of a literal input. Whole expressions omit it.
    pub context: Option<NativeCandidateContext>,
    /// Whether emission succeeded. Whole calls and definitions also reject overlapping
    /// source diagnostics; diagnostics elsewhere do not invalidate this site.
    pub valid: bool,
    /// Generated CSS with `&` as the element subject and readable identifiers for other
    /// generated names. Empty on failure or when the occurrence contributes no CSS.
    #[napi(ts_type = "string")]
    pub css: NativeText,
    /// Required styles and global definitions, sorted and deduplicated by identity.
    pub dependencies: Vec<NativeIdentity>,
    /// Authored runtime expressions for view-transition keys, in source order.
    /// Distinct source occurrences can contain the same expression text.
    #[napi(ts_type = "Array<string>")]
    pub runtime_names: Vec<NativeText>,
}

/// Captured input plans and previews that remain valid after the session changes.
#[napi]
pub struct NativeModuleEditor {
    /// Finalized analysis, display output, and replacement plans retained by this
    /// capture.
    editor: ModuleEditor,
    /// Host callbacks kept alive and entered in the current Node-API environment for
    /// replacement queries. Their configuration must match the captured identifiers.
    naming: Option<Rc<NativeNaming>>,
}

impl NativeModuleEditor {
    /// Retains `editor` and its optional host naming callbacks for later JavaScript
    /// queries. `naming` must be the configuration used to create the editor's
    /// identifier context.
    pub(crate) fn new(editor: ModuleEditor, naming: Option<Rc<NativeNaming>>) -> Self {
        Self { editor, naming }
    }
}

#[napi]
impl NativeModuleEditor {
    /// Identifies the original module whose source ranges this capture addresses.
    #[napi(ts_return_type = "string")]
    pub fn module_id(&self) -> NativeText {
        self.editor.module_id().clone().into_string().into()
    }

    /// Returns a fingerprint of the source, supporting analyses, design system,
    /// settings, and captured output. The value remains pinned when the project session
    /// changes.
    #[napi(ts_return_type = "string")]
    pub fn revision(&self) -> NativeText {
        (self.editor.revision.0.clone()).into()
    }

    /// Returns copies of recognized occurrences, including invalid literals.
    /// Definitions precede style calls in capture traversal order; the result is not
    /// sorted by position. All ranges use the original source identified by `moduleId`.
    /// Mutating returned objects does not change this capture.
    #[napi]
    pub fn sites(&self) -> Vec<NativeEditorSite> {
        self.editor
            .sites
            .iter()
            .map(|site| NativeEditorSite {
                start: site.span.start.0,
                end: site.span.end.0,
                kind: site.kind.into(),
                identity: site.identity.clone().map(Into::into),
                context: site.context.map(Into::into),
                valid: site.valid,
                css: (site.css.clone()).into(),
                dependencies: site.dependencies.iter().cloned().map(Into::into).collect(),
                runtime_names: site.runtime_names.iter().cloned().map(Into::into).collect(),
            })
            .collect()
    }

    /// Renders decoded `candidate` text for the literal at the original inclusive UTF-8
    /// byte `start`. The text replaces the whole literal while retaining enclosing
    /// variants, JavaScript conditions, and slots from the complete call. Retained host
    /// callbacks run in the current Node-API environment during emission.
    /// Returns null for an unknown input, failed emission, or output with no CSS.
    /// The capture and its shared names remain unchanged by the query.
    #[napi(ts_return_type = "string | null")]
    #[allow(clippy::needless_pass_by_value)] // Node-API supplies owned strings.
    pub fn preview_input(&self, env: Env, start: u32, candidate: String) -> Option<NativeText> {
        let _naming = self.naming.as_ref().map(|naming| naming.enter(&env));
        (self.editor.preview_input(start, &candidate)).map(Into::into)
    }
}
