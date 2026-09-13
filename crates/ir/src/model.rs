//! Extracted module model: what a frontend found in one module.
//!
//! Every [`Span`] here refers to the editable original source, never to a virtual or
//! converted view. The model keeps author intent, not generated CSS: a [`StyleInput`]
//! still says "these utilities come from this expression", it does not name classes or
//! declarations.
//!
//! Ordering is semantic and must survive any consumer. [`ModuleIr::style_calls`],
//! [`ModuleIr::handle_definitions`], and the input entries inside one call stay in
//! source order; the class expression tree mirrors the source expression so that each
//! user expression is evaluated exactly once, in source order, with short-circuiting
//! intact.

use serde::{Deserialize, Serialize};

use stillwind_semantic::IdentityId;
use stillwind_source::{Span, Spanned};

pub use crate::AdapterKind;

/// Everything a frontend extracted from one module.
///
/// A frontend builds this in one pass. Later phases may reorder [`Self::style_calls`]
/// for emission, but they must not drop entries, duplicate them, or change the identity
/// of a declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleIr {
    /// Framework the module belongs to. It selects the class property, the runtime
    /// import, and the materialized style format.
    pub adapter: AdapterKind,
    /// Bindings and imports needed by code generation.
    #[serde(default)]
    pub imports: ModuleImports,
    /// In source order. A call can emit a base unit plus conditional units, or no CSS.
    pub style_calls: Vec<StyleCall>,
    /// In source order. Each entry is a `style.ref()` declaration.
    pub handle_definitions: Vec<HandleDefinition>,
    /// In source order. Each entry is a `style.scope()` declaration.
    pub scope_definitions: Vec<ScopeDefinition>,
    /// In source order. Each entry is a `style.animation()` declaration.
    pub animation_declarations: Vec<AnimationDefinition>,
    /// In source order. Each entry is a `style.viewTransition()` declaration.
    pub view_transition_declarations: Vec<ViewTransitionDefinition>,
}

impl ModuleIr {
    /// Empty React IR. Frameworks without an explicit adapter start here and
    /// overwrite `adapter` through their own conversion.
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
    /// Macro imports that code generation may remove once the call is lowered.
    pub macros: Vec<MacroImport>,
    /// Source-literal spans of explicit module CSS value imports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub module_css: Vec<Span>,
}

/// One `import` statement that mentions the framework macro.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MacroImport {
    /// Span of the whole import statement in the editable source.
    pub span: Span,
    /// In source order, including specifiers that are not the macro itself.
    pub specifiers: Vec<MacroImportSpecifier>,
}

/// One binding of a macro import statement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MacroImportSpecifier {
    /// Span of the binding as written, local name included.
    pub span: Span,
    /// Name the module binds the import to.
    pub local: String,
    /// Only value imports of the framework macro may be removed after lowering.
    pub is_macro: bool,
    /// Incomplete reference correspondence forbids deleting this binding.
    pub has_unmapped_references: bool,
    /// Every read of `local`, in source order, excluding the import itself.
    pub references: Vec<Span>,
}

/// One `style(...)` call with its evaluated inputs in source order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StyleCall {
    /// Span of the whole call, including the callee expression.
    pub span: Span,
    /// Arguments in source order. The base unit of a rule is built from the
    /// unconditional entries; conditional entries follow in this order.
    pub inputs: Vec<StyleInput>,
}

/// A non-class entry of an object literal, such as `"aria-busy": loading`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttributeEntry {
    /// Attribute name as written. It is emitted as a prop, not as a class.
    pub name: Spanned<String>,
    /// The attribute value expression, emitted verbatim.
    pub value: Span,
}

/// A `style.ref()` declaration serving as an identity and a prop producer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HandleDefinition {
    /// Normalized module path of the declaring module.
    pub module_path: String,
    /// Lexical binding name of the ref.
    pub name: Spanned<String>,
    /// Optional user label that participates in the generated class name.
    pub label: Option<Spanned<String>>,
    /// Present when the ref was declared with `style.ref(scope)` or
    /// `style.ref().in(scope)`.
    pub scope: Option<ScopeDefinition>,
    /// Span of the producing call, used for replacement; identity uses `name.span`.
    pub call_span: Span,
}

/// A `style.scope()` declaration. Scopes are pure identities: they produce CSS
/// selectors and class-bearing runtime fragments rather than arbitrary attribute props.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScopeDefinition {
    /// Normalized module path of the declaring module.
    pub module_path: String,
    /// Lexical binding name of the scope.
    pub name: Spanned<String>,
    /// Optional user label that participates in the generated class name.
    pub label: Option<Spanned<String>>,
    /// Span of the producing call, used for replacement; identity uses `name.span`.
    pub call_span: Span,
}

/// A `style.animation()` declaration: named keyframes plus their identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnimationDefinition {
    /// Local binding name and its declaration span, used for identity.
    pub name: Spanned<String>,
    /// Span of the producing call, used for replacement; identity uses `name.span`.
    pub call_span: Span,
    /// In source order. The declaration order is the keyframe order.
    pub frames: Vec<AnimationFrame>,
}

/// One `"from"` / `"to"` / percentage frame of a named animation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnimationFrame {
    /// Frame selector text: `from`, `to`, or a percentage list.
    pub selector: Spanned<String>,
    /// Utility list applied inside the frame, in source order.
    pub utilities: Spanned<String>,
}

/// A `style.viewTransition()` declaration. Uses of one declaration share a single
/// generated class; the optional key stays a user expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTransitionDefinition {
    /// Local binding name and its declaration span. Uses share the declaration identity.
    pub name: Spanned<String>,
    /// Span of the producing call, used for replacement; identity uses `name.span`.
    pub call_span: Span,
    /// Utilities for the `::view-transition-group` pseudo-element.
    pub group: Option<Spanned<String>>,
    /// Utilities for the `::view-transition-old` pseudo-element.
    pub old: Option<Spanned<String>>,
    /// Utilities for the `::view-transition-new` pseudo-element.
    pub new: Option<Spanned<String>>,
}

/// A declaration identity paired with source provenance at its use site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetReference {
    /// Durable declaration identity; external offsets are never interpreted in this file.
    pub identity: stillwind_semantic::IdentityId,
    /// Expression referring to the asset in the current module.
    pub span: Span,
    /// A local declaration's range, which must follow source projection alongside its
    /// identity. External declarations have no span in the current source buffer.
    pub local_declaration: Option<Span>,
}

/// The CSS role a ref plays at a use site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HandleProjection {
    /// `ref.anchor`: the element is the anchor for positioning.
    Anchor,
    /// `ref.position`: the element is positioned against an anchor.
    Position,
    /// `ref.anchorScope`: limits anchor-name visibility with `anchor-scope`.
    Scope,
    /// `container()`: the element is a query container of the named type.
    Container(Spanned<String>),
}

/// Ordered variants shared by object wrappers and direct template interpolation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleVariant {
    /// A normal variant prefix, including its final separator.
    Prefix(Spanned<String>),
    /// Correlated ref/scope alternatives, with a normalized variant prefix.
    Ref {
        /// The refs this variant may refer to at this use site.
        handles: Vec<HandleDefinition>,
        /// The variant prefix applied to the referenced element, ending in its
        /// separator; the surrounding variants apply to the target instead.
        prefix: Spanned<String>,
    },
}

/// One evaluated argument of a `style(...)` call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleInput {
    /// `style(base)`: this call derives from another style call's payload. `candidates`
    /// lists the base call's candidate spans so the identity can be resolved without
    /// re-parsing the referenced expression.
    Composition {
        /// The referenced base expression, emitted verbatim.
        value: Span,
        /// Candidate identities of the base call, deduplicated in identity order.
        candidates: Vec<IdentityId>,
    },
    /// Class contributions that keep the source expression structure.
    Class(ClassExpression),
    /// `ref.at("query", "utilities")`: applies utilities inside a container query.
    ContainerQuery {
        /// The container ref this query applies to.
        handle: HandleDefinition,
        /// Query condition text, such as `sm` or a size feature list.
        query: Spanned<String>,
        /// Utility string to emit inside the query.
        value: Spanned<String>,
    },
    /// `style(scope)`: mark the element as a member of a scope.
    ScopeAttachment(ScopeDefinition),
    /// `style(ref)`: attach a ref's props and marker class to the element.
    HandleAttachment(HandleDefinition),
    /// A scope value that is only known at runtime, with every statically reachable
    /// alternative listed for CSS planning.
    DynamicScopeAttachment {
        /// The runtime scope value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning.
        scopes: Vec<ScopeDefinition>,
    },
    /// A ref value that is only known at runtime, with every statically reachable
    /// alternative listed for CSS planning.
    DynamicHandleAttachment {
        /// The runtime ref value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning.
        handles: Vec<HandleDefinition>,
    },
    /// `style(transition)` with a statically known transition.
    ViewTransitionUse {
        /// The declared transition being used.
        transition: AssetReference,
        /// Optional user key expression, evaluated by the runtime.
        key: Option<Span>,
    },
    /// A transition value that is only known at runtime.
    DynamicViewTransitionUse {
        /// The runtime transition value.
        value: Span,
        /// Every statically reachable alternative.
        transitions: Vec<AssetReference>,
        /// Optional user key expression, evaluated by the runtime.
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
    /// `[a, b]`: elements are flattened at runtime in evaluation order.
    Array(Vec<ClassExpression>),
    /// `test ? a : b` or `test && a`; an empty branch contributes nothing.
    Conditional {
        /// The guard expression, emitted verbatim.
        test: Span,
        /// Which operator produced the branch, preserved for reporting.
        kind: ConditionalKind,
        /// Contribution when the guard holds, in source order.
        consequent: Vec<ClassExpression>,
        /// Contribution otherwise; empty for `&&`.
        alternate: Vec<ClassExpression>,
    },
    /// `{ "hover:": … }`: the wrapper applies to every leaf below it.
    Variant {
        /// The wrapper every leaf below this node is resolved under.
        variant: StyleVariant,
        /// The wrapped leaves, in source order.
        inputs: Vec<ClassExpression>,
    },
    /// An object literal: attributes become props, other entries are classes.
    Group {
        /// Span of the whole object literal, used for diagnostics.
        span: Span,
        /// Source order is preserved; attributes and classes may interleave.
        entries: Vec<GroupEntry>,
    },
    /// A user-owned class value from `{ class: … }`.
    ///
    /// The expression is passed to the runtime merge unchanged, so it may contain
    /// anything the framework runtime accepts.
    External(Span),
    /// `ref.anchor` and the other ref projections.
    HandleProjection {
        /// The ref this projection applies to.
        handle: HandleDefinition,
        /// The CSS role the referenced element plays for this call.
        projection: HandleProjection,
    },
    /// A ref projection whose ref is only known at runtime.
    DynamicHandleProjection {
        /// The runtime ref value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning.
        handles: Vec<HandleDefinition>,
        /// The CSS role the projected element plays in each alternative.
        projection: HandleProjection,
    },
    /// `style(animation("options"))` with a statically known animation.
    AnimationUse {
        /// The declared animation being used.
        animation: AssetReference,
        /// Literal option candidates resolved into animation declarations by the compiler.
        options: Spanned<String>,
    },
    /// An animation use whose animation is only known at runtime.
    DynamicAnimationUse {
        /// The runtime animation value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning.
        animations: Vec<AssetReference>,
        /// Literal option candidates resolved into animation declarations by the compiler.
        options: Spanned<String>,
    },
}

/// One source-ordered entry of an object literal class expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GroupEntry {
    /// An entry contributing classes. Boxed to keep the enum small.
    Class(Box<ClassExpression>),
    /// An entry emitted as a prop rather than a class.
    Attribute(AttributeEntry),
    /// A custom CSS property evaluated at runtime and emitted into inline style.
    CustomProperty(CustomPropertyEntry),
}

/// An inline CSS custom property with an expression evaluated at the call site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CustomPropertyEntry {
    /// Decoded CSS property name, beginning with `--`.
    pub name: Spanned<String>,
    /// Original expression evaluated once and converted to CSS text at runtime.
    pub value: Span,
}
