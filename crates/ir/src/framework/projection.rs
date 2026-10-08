//! Parser-neutral expressions and decisions for framework converter normalization.

use stillwind_source::Span;

/// Parser-neutral view of a possible converter-generated expression or reference.
///
/// The frontend reports what it saw; the framework's [`super::FrameworkSpec::normalize`]
/// decides which source expression to retain and how to handle generated references.
#[derive(Clone, Copy, Debug)]
pub enum ProjectionExpression<'a> {
    /// An identifier used as a runtime expression.
    Reference {
        /// Name of the referenced identifier.
        name: &'a str,
    },
    /// A type alias injected by a language tool. `definition` is its text.
    SyntheticTypeAlias {
        /// Name of the injected alias, for example `__VLS_SetupExposed`.
        name: &'a str,
        /// Text of the alias definition.
        definition: &'a str,
    },
    /// A reference list used by the converter for completions.
    SyntheticReferenceList {
        /// Comment line the converter placed before the list.
        preceding_line: &'a str,
    },
    /// `receiver.property`, possibly optional (`receiver?.property`).
    Member {
        /// Text of the receiver expression.
        receiver: &'a str,
        /// Span of the receiver expression.
        object: Span,
        /// Span of the whole member expression in the analyzed buffer.
        whole: Span,
        /// Span of the property name, which is the part the user wrote.
        property: Span,
        /// Property name, including names introduced by the converter.
        property_name: &'a str,
        /// Whether the access used optional chaining.
        optional: bool,
        /// False for a qualified type name.
        runtime: bool,
    },
    /// A shorthand property that a converter expanded into `{ a: a }`; `key` and
    /// `value` are the two occurrences.
    ExpandedShorthand {
        /// Span of the whole expanded property.
        whole: Span,
        /// Span of the key occurrence, which is the part the user wrote.
        key: Span,
        /// Span of the value expression, after its children have been normalized.
        value: Span,
        /// Key text as written.
        key_name: &'a str,
        /// Value text as written; equal to `key_name` for a true shorthand.
        value_name: &'a str,
    },
    /// A call with a value argument and a converter-supplied type witness.
    Call {
        /// Name of the callee identifier.
        callee: &'a str,
        /// Span of the complete call.
        whole: Span,
        /// Span of the first argument.
        value: Span,
        /// Text between the first argument and the end of the call.
        witness: &'a str,
    },
    /// Parentheses around the operand of `new`.
    ConstructorOperand {
        /// Span including the parentheses.
        whole: Span,
        /// Span inside the parentheses.
        operand: Span,
    },
    /// A return object in a method of a converter-created binding.
    SyntheticReturn {
        /// Name of the enclosing variable binding.
        binding: &'a str,
        /// Name of the enclosing method.
        method: &'a str,
        /// True for an object whose values assert each key's identically named binding.
        mirrored_bindings: bool,
    },
}

/// What the frontend should do with a candidate expression or reference.
#[derive(Clone, Copy, Debug)]
pub enum ProjectionDecision {
    /// Keep the expression as user-authored code.
    Preserve,
    /// Bind a reference to its global helper after validating its generated syntax.
    BindGlobalHelper,
    /// Retain authored fragments and query the generated runtime expression.
    Keep {
        /// Span of the complete converter wrapper.
        whole: Span,
        /// Span whose fragments must match the original source.
        retained: Span,
        /// Runtime expression to query, absent for type-only names.
        checker: Option<Span>,
    },
    /// Ignore import reads only after the frontend proves their generated syntax.
    IgnoreReferences,
}

/// The policy for a framework whose converter never rewrites user code.
pub(super) fn preserve(_: ProjectionExpression<'_>) -> ProjectionDecision {
    ProjectionDecision::Preserve
}
