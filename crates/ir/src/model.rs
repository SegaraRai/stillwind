//! Language-neutral intermediate representation shared by all source frontends.

use serde::{Deserialize, Serialize};

use stillwind_source::{Span, Spanned};

pub use crate::framework::AdapterKind;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleIr {
    pub adapter: AdapterKind,
    #[serde(default)]
    pub imports: ModuleImports,
    pub style_calls: Vec<StyleCall>,
    pub handle_definitions: Vec<HandleDefinition>,
    pub scope_definitions: Vec<ScopeDefinition>,
    pub animation_declarations: Vec<AnimationDefinition>,
    pub view_transition_declarations: Vec<ViewTransitionDefinition>,
}

impl ModuleIr {
    pub const fn react() -> Self {
        Self {
            adapter: AdapterKind::React,
            imports: ModuleImports {
                reserved_names: Vec::new(),
                macros: Vec::new(),
                module_css: Vec::new(),
            },
            style_calls: Vec::new(),
            handle_definitions: Vec::new(),
            scope_definitions: Vec::new(),
            animation_declarations: Vec::new(),
            view_transition_declarations: Vec::new(),
        }
    }
}

/// Binding-aware import information. All spans belong to the editable source.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleImports {
    /// Includes nested bindings and unresolved reads to avoid capturing user code.
    pub reserved_names: Vec<String>,
    pub macros: Vec<MacroImport>,
    /// Source-literal spans of explicit module CSS value imports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub module_css: Vec<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MacroImport {
    pub span: Span,
    pub specifiers: Vec<MacroImportSpecifier>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MacroImportSpecifier {
    pub span: Span,
    pub local: String,
    /// Only value imports of the framework macro may be removed after lowering.
    pub is_macro: bool,
    /// Incomplete reference correspondence forbids deleting this binding.
    pub has_unmapped_references: bool,
    pub references: Vec<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StyleCall {
    pub span: Span,
    pub inputs: Vec<StyleInput>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttributeEntry {
    pub name: Spanned<String>,
    pub value: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HandleDefinition {
    pub module_path: String,
    pub name: Spanned<String>,
    pub label: Option<Spanned<String>>,
    pub scope: Option<ScopeDefinition>,
    pub call_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScopeDefinition {
    pub module_path: String,
    pub name: Spanned<String>,
    pub label: Option<Spanned<String>>,
    pub call_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnimationDefinition {
    pub name: Spanned<String>,
    pub call_span: Span,
    pub frames: Vec<AnimationFrame>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnimationFrame {
    pub selector: Spanned<String>,
    pub utilities: Spanned<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTransitionDefinition {
    pub name: Spanned<String>,
    pub call_span: Span,
    pub group: Option<Spanned<String>>,
    pub old: Option<Spanned<String>>,
    pub new: Option<Spanned<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetReference {
    pub module_path: String,
    pub name: Spanned<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HandleProjection {
    Anchor,
    Position,
    Scope,
    Container(Spanned<String>),
}

/// Ordered variants shared by object wrappers and direct template interpolation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleVariant {
    /// A normal variant prefix, including its final separator.
    Prefix(Spanned<String>),
    /// Correlated ref/scope alternatives, with a normalized variant prefix.
    Ref {
        handles: Vec<HandleDefinition>,
        prefix: Spanned<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleInput {
    UtilityString(Spanned<String>),
    ConditionalUtilityString {
        guard: Span,
        when_true: bool,
        value: Spanned<String>,
    },
    Attributes(Vec<AttributeEntry>),
    Variant {
        variant: StyleVariant,
        inputs: Vec<StyleInput>,
    },
    HandleProjection {
        handle: HandleDefinition,
        projection: HandleProjection,
    },
    DynamicHandleProjection {
        value: Span,
        handles: Vec<HandleDefinition>,
        projection: HandleProjection,
    },
    ContainerQuery {
        handle: HandleDefinition,
        query: Spanned<String>,
        value: Spanned<String>,
    },
    ScopeAttachment(ScopeDefinition),
    HandleAttachment(HandleDefinition),
    DynamicScopeAttachment {
        value: Span,
        scopes: Vec<ScopeDefinition>,
    },
    DynamicHandleAttachment {
        value: Span,
        handles: Vec<HandleDefinition>,
    },
    /// A user-owned class value evaluated by the runtime `stillwindClsx` helper.
    ///
    /// This is intentionally available only through `{ class: … }`, so the
    /// surrounding object syntax remains unambiguous for attributes/selectors.
    ExternalClass(Span),
    AnimationUse {
        animation: AssetReference,
        options: Spanned<String>,
    },
    DynamicAnimationUse {
        value: Span,
        animations: Vec<AssetReference>,
        options: Spanned<String>,
    },
    ViewTransitionUse {
        transition: AssetReference,
        key: Option<Span>,
    },
    DynamicViewTransitionUse {
        value: Span,
        transitions: Vec<AssetReference>,
        key: Option<Span>,
    },
}
