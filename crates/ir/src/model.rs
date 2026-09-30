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
use stillwind_source::{ModuleId, Span, Spanned};

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
}

impl ModuleIr {
    /// Empty React IR. Frameworks without an explicit adapter start here and
    /// overwrite `adapter` through their own conversion.
    pub const fn react() -> Self {
        Self {
            adapter: AdapterKind::React,
            imports: ModuleImports {
                consumed_uses: Vec::new(),
                reserved_names: Vec::new(),
                macros: Vec::new(),
                module_css: Vec::new(),
            },
            style_calls: Vec::new(),
            ref_definitions: Vec::new(),
            keyframes_declarations: Vec::new(),
            view_transition_declarations: Vec::new(),
        }
    }
}

/// Binding-aware import information. All spans belong to the editable source.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleImports {
    /// Ranges in the original source where the frontend accepted nested macro uses.
    /// Import removal uses a range only if it is inside a transformed style call and
    /// the reference to the macro binding starts at the same position.
    #[serde(default)]
    pub consumed_uses: Vec<Span>,
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
    /// Adapter selected by the imported macro entry point.
    pub adapter: AdapterKind,
    /// Span of the module specifier literal, including its quotes.
    pub source_span: Span,
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
/// One declaration covers both roles a ref plays: the element it is attached to, and
/// the boundary that other refs may be bounded by.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RefDefinition {
    /// Module identity of the declaring module.
    pub module_id: ModuleId,
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
    /// Module identity of the declaring module.
    pub module_id: ModuleId,
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
    /// Nested conditions conjoin with their ancestors and replace the complete
    /// definition.
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
    /// Original UTF-8 declaration identity in its own module.
    pub identity: IdentityId,
    /// Use-site UTF-8 span in the current module; mapped from analysis to original
    /// before compilation.
    pub span: Span,
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
    /// Supplies the receiver's anchor name and optional explicit scope.
    AsAnchor(
        /// `None` omits `anchor-scope`. An explicit list includes the receiver only
        /// when it occurs in that list.
        Option<AnchorScope>,
    ),
    /// Supplies the receiver's container name and optional container type.
    AsContainer(
        /// A supported literal in normalized form, with its source span. `None` omits
        /// `container-type`. The compiler does not provide a default.
        Option<Spanned<String>>,
    ),
    /// References an anchor from the styled element.
    PositionAnchor,
}

/// Explicit anchor visibility, resolved to the union of finite ref candidates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AnchorScope {
    /// The CSS keyword `all`.
    All,
    /// The CSS keyword `none`.
    None,
    /// Unique ref identities whose generated anchor names are scoped.
    Refs(
        /// All possible ref declarations, with each identity included once in
        /// first-seen order. An empty list emits no scope declaration.
        Vec<RefDefinition>,
    ),
}

/// Ordered variants shared by object wrappers and direct template interpolation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleVariant {
    /// A normal variant prefix. A nonempty prefix includes its final separator;
    /// an empty prefix contributes no variants. The compiler validates the CSS
    /// variant grammar when resolving the wrapper.
    Prefix(
        /// Authored prefix and its original source span.
        Spanned<String>,
    ),
    /// A named-container query over finite ref candidates, written as `ref.at(query)`.
    Container {
        /// Each candidate supplies a container identity, independently of the subject
        /// selector. An empty set is diagnosed as an unidentified ref by variant
        /// resolution.
        refs: Vec<RefDefinition>,
        /// Literal CSS query condition, without the generated container name.
        /// Empty text emits only the container name.
        query: Spanned<String>,
    },
    /// Correlated ref and boundary alternatives, written as `ref()`, `ref("variant:")`,
    /// or a ref tag, with a normalized variant prefix.
    Ref {
        /// The refs this variant may refer to at this use site. An empty set is
        /// diagnosed as an unidentified ref by variant resolution.
        refs: Vec<RefDefinition>,
        /// The variant prefix applied to the referenced element. A non-empty prefix
        /// ends in its separator; the surrounding variants apply to the target instead.
        /// The compiler validates this prefix when resolving the wrapper.
        prefix: Spanned<String>,
    },
}

/// One evaluated argument of a `style(...)` call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleInput {
    /// Source array whose entries can contribute to multiple props channels.
    Array(
        /// Source elements in evaluation order, possibly empty.
        Vec<Self>,
    ),
    /// Selects complete style contributions, including classes, names, refs, and
    /// composition.
    Conditional {
        /// Original condition expression.
        test: Span,
        /// Contributions selected by the source operator, in source order.
        branches: ConditionalBranches<Vec<Self>>,
    },
    /// `style(base)`: this call derives from another style call's payload.
    /// `candidates` lists declaration identities reachable from the base value so
    /// dependency planning does not re-parse the referenced expression.
    Composition {
        /// The referenced base expression, emitted verbatim.
        value: Span,
        /// Candidate identities of the base call, deduplicated in identity order. An
        /// empty finite set has no static dependency; the base expression is still
        /// passed to the runtime composition path.
        candidates: Vec<IdentityId>,
    },
    /// Class contributions that keep the source expression structure.
    Class(
        /// Root class expression, which may contain props-bearing object entries.
        ClassExpression,
    ),
    /// `style(ref)`: attach a ref's props and marker class to the element.
    ///
    /// The same attachment marks a boundary element: the class token is what bounded
    /// selectors read as the boundary of their `:has()` test.
    RefAttachment {
        /// Ref whose marker and attachment props apply to this element.
        definition: RefDefinition,
        /// Original expression selecting the ref at this use site.
        value: Span,
    },
    /// A ref value that is only known at runtime, with every statically reachable
    /// alternative listed for CSS planning.
    DynamicRefAttachment {
        /// The runtime ref value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning. An empty
        /// finite set contributes no planned ref assets; the runtime class read remains
        /// in the emitted expression.
        refs: Vec<RefDefinition>,
    },
    /// `style(transition)` with a statically known transition.
    ViewTransitionUse {
        /// The declared transition being used.
        transition: AssetReference,
        /// How the use names its participants.
        name: ViewTransitionName,
    },
    /// A transition value that is only known at runtime.
    DynamicViewTransitionUse {
        /// The runtime transition value.
        value: Span,
        /// Every statically reachable alternative. The JavaScript frontend omits this
        /// use when the finite set is empty.
        transitions: Vec<AssetReference>,
        /// How the use names its participants.
        name: ViewTransitionName,
    },
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
}

/// One key a view-transition use pairs its participants under.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ViewTransitionKey {
    /// A literal key: the name becomes a static declaration.
    Literal(String),
    /// An expression only the runtime knows: the name becomes an inline style value.
    Dynamic(Span),
}

/// Branches selected by a source conditional operator. The operator determines whether
/// a falsy branch exists throughout parsing, lowering, and rendering. Deserialization
/// rejects an `alternate` on `And` and requires it on `Ternary`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ConditionalBranches<T> {
    /// `test && consequent`.
    And {
        /// Values selected when the test is truthy.
        consequent: T,
    },
    /// `test ? consequent : alternate`.
    Ternary {
        /// Values selected when the test is truthy.
        consequent: T,
        /// Values selected when the test is falsy.
        alternate: T,
    },
}

impl<T> ConditionalBranches<T> {
    /// Returns the branch evaluated when the test is truthy for either operator.
    pub fn consequent(&self) -> &T {
        match self {
            Self::And { consequent } | Self::Ternary { consequent, .. } => consequent,
        }
    }

    /// Returns the falsy branch for a ternary, or `None` for logical conjunction.
    pub fn alternate(&self) -> Option<&T> {
        match self {
            Self::And { .. } => None,
            Self::Ternary { alternate, .. } => Some(alternate),
        }
    }

    /// Returns branches of `U` by mapping the truthy branch, then any falsy branch,
    /// without changing the source operator.
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> ConditionalBranches<U> {
        match self {
            Self::And { consequent } => ConditionalBranches::And {
                consequent: f(consequent),
            },
            Self::Ternary {
                consequent,
                alternate,
            } => ConditionalBranches::Ternary {
                consequent: f(consequent),
                alternate: f(alternate),
            },
        }
    }

    /// Returns branches of `U` by mapping the truthy branch, then any falsy branch,
    /// without changing the source operator.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by `f`, in branch order.
    pub fn try_map<U, E>(
        self,
        mut f: impl FnMut(T) -> Result<U, E>,
    ) -> Result<ConditionalBranches<U>, E> {
        Ok(match self {
            Self::And { consequent } => ConditionalBranches::And {
                consequent: f(consequent)?,
            },
            Self::Ternary {
                consequent,
                alternate,
            } => ConditionalBranches::Ternary {
                consequent: f(consequent)?,
                alternate: f(alternate)?,
            },
        })
    }

    /// Returns the same operator and branches as shared references.
    pub fn as_ref(&self) -> ConditionalBranches<&T> {
        match self {
            Self::And { consequent } => ConditionalBranches::And { consequent },
            Self::Ternary {
                consequent,
                alternate,
            } => ConditionalBranches::Ternary {
                consequent,
                alternate,
            },
        }
    }
}

/// A root expression that preserves mixed class and props contributions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ClassExpression {
    /// `[a, b]`: elements are flattened at runtime in evaluation order.
    Array(
        /// Source elements in evaluation order, possibly empty.
        Vec<Self>,
    ),
    /// `test ? a : b` or `test && a`; an empty branch contributes nothing.
    Conditional {
        /// The guard expression, emitted verbatim.
        test: Span,
        /// Contributions selected by the source operator, in source order.
        branches: ConditionalBranches<Vec<Self>>,
    },
    /// CSS rules, including their variant wrappers and rule-only nested branches.
    Rule(
        /// Rule subtree permitted beneath a CSS variant.
        Box<RuleExpression>,
    ),
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
}

/// A CSS rule expression whose every descendant may occur under a variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RuleExpression {
    /// A utility string or utility template literal.
    Utilities(
        /// Decoded utility text and its original source span.
        Spanned<String>,
    ),
    /// A source array of rule expressions.
    Array(
        /// Rule elements in source order, possibly empty.
        Vec<Self>,
    ),
    /// A source condition selecting rule expressions.
    Conditional {
        /// Original condition expression.
        test: Span,
        /// Rules selected by the source operator.
        branches: ConditionalBranches<Vec<Self>>,
    },
    /// A CSS variant applied to rule expressions.
    Variant {
        /// Variant selector or at-rule.
        variant: StyleVariant,
        /// Rules under the variant.
        inputs: Vec<Self>,
    },
    /// An object expression containing only rule entries.
    Group {
        /// Whole object span.
        span: Span,
        /// Rule entries in source order.
        inputs: Vec<Self>,
    },
    /// A scope-only use with no receiver attachment.
    AnchorScope {
        /// Scope supplied independently of a receiver or marker attachment.
        scope: AnchorScope,
        /// Whole use expression for diagnostics and source mapping.
        span: Span,
    },
    /// A use with one known receiver identity.
    RefProjection {
        /// The ref this projection applies to.
        definition: RefDefinition,
        /// The receiver expression the use site wrote, in the original source.
        ///
        /// Retained for source locations; static uses need no runtime marker read.
        value: Span,
        /// The CSS role the referenced element plays for this call.
        projection: RefProjection,
    },
    /// A ref projection whose ref is only known at runtime.
    DynamicRefProjection {
        /// The runtime ref value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning. An empty
        /// finite set produces no CSS and selects from an empty lookup table.
        refs: Vec<RefDefinition>,
        /// The CSS role the projected element plays in each alternative.
        projection: RefProjection,
    },
    /// A view-transition use resolved to one declared transition.
    ViewTransitionUse {
        /// The declared transition selected by the use.
        transition: AssetReference,
        /// Participant naming supplied by this use.
        name: ViewTransitionName,
    },
    /// A view-transition use selected from finite runtime candidates.
    DynamicViewTransitionUse {
        /// The runtime transition receiver.
        value: Span,
        /// Every statically reachable transition declaration. The JavaScript frontend
        /// omits this use when the finite set is empty.
        transitions: Vec<AssetReference>,
        /// Participant naming supplied by this use.
        name: ViewTransitionName,
    },
    /// `style(keyframes("options"))` with a statically known keyframes.
    KeyframesUse {
        /// The declared keyframes being used.
        keyframes: AssetReference,
        /// Literal animation options and their source span, resolved into keyframes
        /// declarations by the compiler.
        options: Spanned<String>,
    },
    /// A keyframes use whose declaration is only known at runtime.
    DynamicKeyframesUse {
        /// The receiver expression the use site wrote, whose keyframes name selects the
        /// prebuilt class.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning. The
        /// JavaScript frontend omits this use when the finite set is empty.
        keyframes: Vec<AssetReference>,
        /// Literal animation options and their source span, resolved into keyframes
        /// declarations by the compiler.
        options: Spanned<String>,
    },
}

/// The reason a class expression cannot become a CSS rule expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuleExpressionErrorKind {
    /// An object entry supplies an element attribute.
    #[error("an attribute cannot be placed under a CSS variant")]
    Attribute,
    /// An object entry supplies an inline custom property.
    #[error("a custom property cannot be placed under a CSS variant")]
    CustomProperty,
    /// A class value has no CSS rule for the variant to transform.
    #[error("an external class cannot be placed under a CSS variant")]
    ExternalClass,
}

/// A failed class-to-rule conversion with the first incompatible descendant's kind and span.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{reason}")]
pub struct RuleExpressionError {
    /// Why this descendant cannot be used as a CSS rule.
    pub reason: RuleExpressionErrorKind,
    /// Original source range of the incompatible descendant.
    pub span: Span,
}

impl TryFrom<ClassExpression> for RuleExpression {
    type Error = RuleExpressionError;

    /// Returns the rule subtree of a root expression when every descendant can
    /// contribute CSS under a variant.
    ///
    /// # Errors
    ///
    /// Returns the reason and source range of the first incompatible descendant.
    fn try_from(expression: ClassExpression) -> Result<Self, Self::Error> {
        match expression {
            ClassExpression::Rule(rule) => Ok(*rule),
            ClassExpression::Array(elements) => Ok(Self::Array(
                elements
                    .into_iter()
                    .map(Self::try_from)
                    .collect::<Result<_, _>>()?,
            )),
            ClassExpression::Conditional { test, branches } => Ok(Self::Conditional {
                test,
                branches: branches
                    .try_map(|values| values.into_iter().map(Self::try_from).collect())?,
            }),
            ClassExpression::Group { span, entries } => Ok(Self::Group {
                span,
                inputs: entries
                    .into_iter()
                    .map(|entry| match entry {
                        GroupEntry::Class(expression) => Self::try_from(*expression),
                        GroupEntry::Attribute(attribute) => Err(RuleExpressionError {
                            reason: RuleExpressionErrorKind::Attribute,
                            span: attribute.name.span,
                        }),
                        GroupEntry::CustomProperty(property) => Err(RuleExpressionError {
                            reason: RuleExpressionErrorKind::CustomProperty,
                            span: property.name.span,
                        }),
                    })
                    .collect::<Result<_, _>>()?,
            }),
            ClassExpression::ClassValue { span, .. } => Err(RuleExpressionError {
                reason: RuleExpressionErrorKind::ExternalClass,
                span,
            }),
        }
    }
}

impl From<RuleExpression> for ClassExpression {
    /// Wraps a rule subtree as one root class contribution.
    fn from(rule: RuleExpression) -> Self {
        Self::Rule(Box::new(rule))
    }
}

/// One source-ordered entry of an object literal class expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GroupEntry {
    /// An entry contributing classes. Boxed to keep the enum small.
    Class(
        /// Class expression supplied by this object entry.
        Box<ClassExpression>,
    ),
    /// An entry emitted as a prop rather than a class.
    Attribute(
        /// Authored attribute key and source value.
        AttributeEntry,
    ),
    /// A custom CSS property evaluated at runtime and emitted into inline style.
    CustomProperty(
        /// Authored custom-property key and source value.
        CustomPropertyEntry,
    ),
}

/// An inline CSS custom property with an expression evaluated at the call site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CustomPropertyEntry {
    /// Decoded CSS property name, beginning with `--`. The JavaScript frontend
    /// selects this entry only after checking the prefix.
    pub name: Spanned<String>,
    /// Original expression whose value is converted to CSS text.
    pub value: Span,
    /// Decoded CSS text when a literal can be converted without evaluating user
    /// code. When present, rendering uses this value instead of evaluating `value`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialization_requires_the_branches_of_the_source_operator() {
        assert!(
            serde_json::from_str::<ConditionalBranches<Vec<u8>>>(
                r#"{"And":{"consequent":[1],"alternate":[2]}}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<ConditionalBranches<Vec<u8>>>(
                r#"{"Ternary":{"consequent":[1]}}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<ConditionalBranches<Vec<u8>>>(r#"{"And":{"consequent":[1]}}"#)
                .is_ok()
        );
    }
}
