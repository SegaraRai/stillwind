//! Extracted module model: what a frontend found in one module.
//!
//! Every [`Span`] here refers to the editable original source, never to a virtual or
//! converted view. The model keeps author intent, not generated CSS: a [`StyleInput`]
//! still says "these utilities come from this expression", it does not name classes or
//! declarations.
//!
//! Ordering is semantic and must survive any consumer. [`ModuleIr::style_calls`],
//! [`ModuleIr::ref_definitions`], and the input entries inside one call stay in source
//! order to preserve CSS and contribution precedence. Value expressions must satisfy
//! the macro purity contract. Folding may reorder their evaluation and omit unused
//! values while retaining guards for selected contributions.

use serde::{Deserialize, Serialize};

use stillwind_semantic::IdentityId;
use stillwind_source::{Span, Spanned};

use crate::AdapterKind;

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
    pub ref_definitions: Vec<RefDefinition>,
    /// In source order. Each entry is a `style.keyframes()` declaration.
    pub keyframes_declarations: Vec<KeyframesDefinition>,
    /// In source order. Each entry is a `style.viewTransition()` declaration.
    pub view_transition_declarations: Vec<ViewTransitionDefinition>,
    /// In source order. Each entry is a `transition.for(key)` value built outside a call.
    pub view_transition_key_applications: Vec<ViewTransitionKeyApplication>,
}

/// One `transition.for(key)` application written outside a `style()` call.
///
/// The application builds a value rather than a style input, so it lowers to a record
/// that carries the key, or to a runtime call when only the value knows its class.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTransitionKeyApplication {
    /// The whole `.for(...)` call, used for the replacement.
    pub call_span: Span,
    /// The receiver expression, which denotes the transitions.
    pub value: Span,
    /// The key the application states.
    pub key: ViewTransitionKey,
    /// Every statically reachable transition of the receiver.
    pub transitions: Vec<AssetReference>,
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
            ref_definitions: Vec::new(),
            keyframes_declarations: Vec::new(),
            view_transition_declarations: Vec::new(),
            view_transition_key_applications: Vec::new(),
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
///
/// One declaration covers both roles a ref plays: the element it is attached to, and the
/// boundary that other refs may be bounded by.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RefDefinition {
    /// Normalized module path of the declaring module.
    pub module_path: String,
    /// Lexical binding of the ref; its span seeds the declaration identity.
    pub binding: Spanned<String>,
    /// Optional user label that participates in the declaration's generated names.
    pub label: Option<Spanned<String>>,
    /// The single boundary passed to `style.ref()`, resolved statically; `None` for
    /// `style.ref()` and `style.ref(label)`.
    pub boundary: Option<BoundaryRef>,
    /// Span of the producing call, used for replacement.
    pub call_span: Span,
}

/// The bounding ref a bounded ref resolved to.
///
/// A boundary is a ref declaration elsewhere, and only its declaration fields are kept:
/// holding a whole [`RefDefinition`] here would let a bounded ref recurse through the
/// boundary's own boundary, which nothing reads. Selector and marker generation need no
/// more than these fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoundaryRef {
    /// Normalized module path of the declaring module.
    pub module_path: String,
    /// Lexical binding of the boundary ref; its span seeds the declaration identity.
    pub binding: Spanned<String>,
    /// Optional user label that participates in the declaration's generated names.
    pub label: Option<Spanned<String>>,
    /// Span of the producing call, used for replacement.
    pub call_span: Span,
}

/// A `style.keyframes()` declaration: named keyframes plus their identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyframesDefinition {
    /// Local binding; its span seeds the declaration identity.
    pub binding: Spanned<String>,
    /// Span of the producing call, used for replacement.
    pub call_span: Span,
    /// Authored label, which the built-in generator spells into the keyframes name.
    #[serde(default)]
    pub label: Option<Spanned<String>>,
    /// Default frames and complete conditional replacements.
    pub body: KeyframesBody,
}

/// A complete keyframe definition with optional conditional replacements.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyframesBody {
    /// Frame declarations in authored order; omitted frames are not inherited.
    pub frames: Vec<KeyframesFrame>,
    /// Nested conditions conjoin with their ancestors and replace the complete definition.
    pub branches: Vec<KeyframesBranch>,
}

/// A condition selecting a complete keyframes definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyframesBranch {
    /// Variant conditions resolved at the keyframes rule boundary.
    pub variant: StyleVariant,
    /// Complete replacement frames and further conditional replacements.
    pub body: KeyframesBody,
}

/// One `"from"` / `"to"` / percentage frame of a named keyframes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyframesFrame {
    /// Frame selector text: `from`, `to`, or a percentage list.
    pub selector: Spanned<String>,
    /// Utility list applied inside the frame, in source order.
    pub utilities: Spanned<String>,
}

/// A `style.viewTransition()` declaration. Uses of one declaration share a single
/// generated class; the optional key stays a user expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTransitionDefinition {
    /// Local binding; its span seeds the declaration identity. Every use shares it.
    pub binding: Spanned<String>,
    /// Span of the producing call, used for replacement.
    pub call_span: Span,
    /// Authored label, which the built-in generator spells into the class token.
    #[serde(default)]
    pub label: Option<Spanned<String>>,
    /// Option list for the `::view-transition-group` pseudo-element.
    pub group: Option<Spanned<String>>,
    /// Utility list for the `::view-transition-old` pseudo-element.
    pub old: Option<Spanned<String>>,
    /// Utility list for the `::view-transition-new` pseudo-element.
    pub new: Option<Spanned<String>>,
    /// Utility list for the `::view-transition-image-pair` pseudo-element.
    pub image_pair: Option<Spanned<String>>,
}

/// A declaration identity paired with source provenance at its use site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetReference {
    /// Durable declaration identity; external offsets are never interpreted in this file.
    pub identity: IdentityId,
    /// Expression referring to the asset in the current module.
    pub span: Span,
    /// A local declaration's range, which must follow source projection alongside its
    /// identity. External declarations have no span in the current source buffer.
    pub local_declaration: Option<Span>,
    /// Authored label of the referenced declaration, when the frontend could read it.
    ///
    /// A label participates in the declaration's generated name, so a use site in
    /// another module needs it to spell the same name the declaring module writes.
    #[serde(default)]
    pub label: Option<Spanned<String>>,
}

/// The CSS role a ref plays at a use site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RefProjection {
    /// `ref.anchor`: the element is the anchor for positioning.
    Anchor,
    /// `ref.positionAnchor`: the element is positioned against an anchor, writing `position-anchor`.
    PositionAnchor,
    /// `ref.anchorScope`: limits anchor-name visibility with `anchor-scope`.
    AnchorScope,
    /// `container()`: the element is a query container of the named type.
    Container(Spanned<String>),
}

/// Ordered variants shared by object wrappers and direct template interpolation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleVariant {
    /// A normal variant prefix, including its final separator.
    Prefix(Spanned<String>),
    /// A named-container query over finite ref candidates, written as `ref.at(query)`.
    Container {
        /// Each candidate supplies a container identity, independently of the subject selector.
        refs: Vec<RefDefinition>,
        /// Literal CSS query condition, without the generated container name.
        query: Spanned<String>,
    },
    /// Correlated ref and boundary alternatives, written as `ref()`, `ref("variant:")`,
    /// or a ref tag, with a normalized variant prefix.
    Ref {
        /// The refs this variant may refer to at this use site.
        refs: Vec<RefDefinition>,
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
    /// `style(ref)`: attach a ref's props and marker class to the element.
    ///
    /// The same attachment marks a boundary element: the class token is what bounded
    /// selectors read as the boundary of their `:has()` test.
    RefAttachment(RefDefinition),
    /// A ref value that is only known at runtime, with every statically reachable
    /// alternative listed for CSS planning.
    DynamicRefAttachment {
        /// The runtime ref value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning.
        refs: Vec<RefDefinition>,
    },
    /// `style(transition)` with a statically known transition.
    ViewTransitionUse {
        /// The declared transition being used.
        transition: AssetReference,
        /// How the use names its participants.
        name: ViewTransitionName,
        /// Conditional tests that select this use, outermost first.
        guard: Vec<StyleGuard>,
    },
    /// A transition value that is only known at runtime.
    DynamicViewTransitionUse {
        /// The runtime transition value.
        value: Span,
        /// Every statically reachable alternative.
        transitions: Vec<AssetReference>,
        /// How the use names its participants.
        name: ViewTransitionName,
        /// Conditional tests that select this use, outermost first.
        guard: Vec<StyleGuard>,
    },
}

/// One conditional test that selects a style input rather than a class literal.
///
/// A view-transition use is a top-level input, so the test that guards it cannot live in
/// the class expression tree the way a guarded utility does.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StyleGuard {
    /// The guard expression, emitted verbatim.
    pub test: Span,
    /// Whether the input applies when the test is truthy.
    pub when_true: bool,
}

/// How one view-transition use names the participants it marks.
///
/// A view-transition-name is a pairing key rather than an allocated name: different
/// transitions may compose the same value on purpose, so the compiler derives it from the
/// class token and the key instead of checking it against a namespace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ViewTransitionName {
    /// No name: the use marks its participants with a class only.
    None,
    /// A key the use site states.
    Key(ViewTransitionKey),
    /// The value carries the key that was applied where the transition was declared.
    Value,
}

/// One key a view-transition use pairs its participants under.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ViewTransitionKey {
    /// A literal key: the name becomes a static declaration.
    Literal(String),
    /// An expression only the runtime knows: the name becomes an inline style value.
    Dynamic(Span),
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
/// Utility leaves become generated classes; external class leaves retain their
/// authored values. The tree preserves contribution precedence and guards while
/// allowing value-expression folding under the macro purity contract.
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
    /// The `class` key's value, normalized into clsx arguments under the macro contract.
    ClassValue {
        /// Whole original value, retained as a contribution-order and helper-import anchor.
        span: Span,
        /// Original expression occurrences in class order. Array literals without
        /// spread are expanded and holes omitted; spread arrays and guards remain
        /// complete expressions. An empty list contributes no class value.
        arguments: Vec<Span>,
    },
    /// `ref.anchor` and the other ref projections.
    RefProjection {
        /// The ref this projection applies to.
        definition: RefDefinition,
        /// The receiver expression the use site wrote, in the original source.
        ///
        /// Generated code reads the declaration token off this expression, so an alias or a
        /// member receiver names the same declaration.
        value: Span,
        /// The CSS role the referenced element plays for this call.
        projection: RefProjection,
    },
    /// A ref projection whose ref is only known at runtime.
    DynamicRefProjection {
        /// The runtime ref value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning.
        refs: Vec<RefDefinition>,
        /// The CSS role the projected element plays in each alternative.
        projection: RefProjection,
    },
    /// `style(keyframes("options"))` with a statically known keyframes.
    KeyframesUse {
        /// The declared keyframes being used.
        keyframes: AssetReference,
        /// Literal option candidates resolved into keyframes declarations by the compiler.
        options: Spanned<String>,
    },
    /// An keyframes use whose keyframes is only known at runtime.
    DynamicKeyframesUse {
        /// The runtime keyframes value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning.
        keyframes: Vec<AssetReference>,
        /// Literal option candidates resolved into keyframes declarations by the compiler.
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
