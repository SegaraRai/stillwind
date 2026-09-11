//! Language-neutral intermediate representation shared by all source frontends.

use serde::{Deserialize, Serialize};

use stillwind_semantic::IdentityId;
use stillwind_source::{Span, Spanned};

pub use crate::AdapterKind;

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
    Composition {
        value: Span,
        candidates: Vec<IdentityId>,
    },
    /// Class contributions that keep the source expression structure.
    Class(ClassExpression),
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

/// Which operator produced a conditional branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConditionalKind {
    /// `test ? consequent : alternate`
    Ternary,
    /// `test && consequent`
    LogicalAnd,
}

/// A class-valued expression that mirrors the source expression.
///
/// Lowering replaces each leaf with the class name generated for it and emits
/// every other part of the expression verbatim, so each user expression is
/// evaluated exactly once, in source order, with short-circuiting intact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ClassExpression {
    /// A utility string or utility template literal.
    Utilities(Spanned<String>),
    /// `[a, b]` — elements are flattened at runtime in evaluation order.
    Array(Vec<ClassExpression>),
    /// `test ? a : b` or `test && a`; an empty branch contributes nothing.
    Conditional {
        test: Span,
        kind: ConditionalKind,
        consequent: Vec<ClassExpression>,
        alternate: Vec<ClassExpression>,
    },
    /// `{ "hover:": … }` — the wrapper applies to every leaf below it.
    Variant {
        variant: StyleVariant,
        inputs: Vec<ClassExpression>,
    },
    /// An object literal: attributes become props, other entries are classes.
    Group {
        span: Span,
        entries: Vec<GroupEntry>,
    },
    /// A user-owned class value from `{ class: … }`.
    External(Span),
    HandleProjection {
        handle: HandleDefinition,
        projection: HandleProjection,
    },
    DynamicHandleProjection {
        value: Span,
        handles: Vec<HandleDefinition>,
        projection: HandleProjection,
    },
    AnimationUse {
        animation: AssetReference,
        options: Spanned<String>,
    },
    DynamicAnimationUse {
        value: Span,
        animations: Vec<AssetReference>,
        options: Spanned<String>,
    },
}

/// One source-ordered entry of an object literal class expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GroupEntry {
    Attribute(AttributeEntry),
    Class(Box<ClassExpression>),
}
