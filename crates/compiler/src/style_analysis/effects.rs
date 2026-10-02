//! Direct contributions retained while lowering still knows their source occurrences.
//!
//! The compiler stores these records in each successful module artifact. Project hosts
//! pass the opaque `StyleEffects` records to `StyleResolver`; only lowering constructs
//! or changes the contribution tree. CSS contexts describe emitted rules and do not
//! encode implications between CSS predicates or correlations between runtime tests.

use stillwind_ir::ConditionalBranches;
use stillwind_semantic::IdentityId;
use stillwind_source::{ModuleId, Span};

/// The direct supplies and base references of one compiled style call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleEffects {
    /// Declaration identity of the style call.
    pub(crate) identity: IdentityId,
    /// Original source call span.
    pub(crate) span: Span,
    /// Direct contribution tree before base resolution; an empty `All` supplies
    /// nothing.
    pub(crate) root: StyleEffect,
}

impl StyleEffects {
    /// Returns the declaration identity used to register and resolve this style call.
    pub fn identity(&self) -> &IdentityId {
        &self.identity
    }
}

/// A direct contribution or a source selection between contributions.
/// Lowering records every source occurrence before class-unit merging removes the
/// boundaries needed to compare independent uses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StyleEffect {
    /// All children contribute; an empty list supplies nothing.
    All(
        /// Contributions in encounter order.
        Vec<Self>,
    ),
    /// A JavaScript condition selects one branch.
    Branch {
        /// Original test occurrence; conditions at different sites stay independent.
        condition: UseId,
        /// Contributions in the truthy and optional falsy arms.
        branches: ConditionalBranches<Box<Self>>,
    },
    /// One runtime use selects at most one candidate; an empty list supplies nothing.
    Choice {
        /// Runtime receiver occurrence shared by all alternatives. Records for the same
        /// occurrence must keep the same candidate order and count.
        site: UseId,
        /// Mutually exclusive alternatives in stable candidate order.
        alternatives: Vec<Self>,
    },
    /// One source use supplies properties or a marker.
    Supply(
        /// Original contribution and its source occurrence.
        StyleSupply,
    ),
    /// Another style contributes its complete payload.
    Base(
        /// Canonical identity of the consumed style declaration.
        IdentityId,
    ),
}

/// A source occurrence qualified by its producer module.
/// Ordering compares the module, source file, and byte boundaries. Different modules
/// can reuse file IDs and offsets without sharing a runtime decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UseId {
    /// Producer module of the original occurrence.
    pub(crate) module: ModuleId,
    /// Original file ID and half-open UTF-8 byte boundaries in the producer source.
    pub(crate) span: Span,
}

impl Ord for UseId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            &self.module,
            self.span.file.0,
            self.span.start.0,
            self.span.end.0,
        )
            .cmp(&(
                &other.module,
                other.span.file.0,
                other.span.start.0,
                other.span.end.0,
            ))
    }
}

impl PartialOrd for UseId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// One supply from an original use before inheritance adds source decisions.
/// The site and utility ordinal distinguish authored name supplies; expanded CSS
/// alternatives for one utility retain the same ordinal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StyleSupply {
    /// Source occurrence that supplies this contribution.
    pub(crate) site: UseId,
    /// Resolved semantic contribution.
    pub(crate) kind: SupplyKind,
    /// Resolved utility position within this source use; CSS alternatives share it.
    pub(crate) ordinal: usize,
}

/// The semantic contribution of a source use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SupplyKind {
    /// A resolved CSS or inline declaration.
    Declaration {
        /// Decoded property name; ordinary names are ASCII lowercase and custom names
        /// retain case.
        property: String,
        /// CSS value or runtime expression; serialized CSS retains any priority suffix.
        value: DeclarationValue,
        /// Whether the declaration carries `!important`.
        important: bool,
        /// Expanded selector and enclosing at-rules.
        context: RuleContext,
        /// Whether a ref provider or explicit anchor scope supplied the declaration.
        provider: bool,
        /// Required marker for ref providers; explicit anchor scopes have no marker
        /// gate.
        gate: Option<IdentityId>,
    },
    /// Attaches the marker for this canonical ref identity.
    Marker(
        /// Canonical identity of the attached ref declaration.
        IdentityId,
    ),
    /// Requires the corresponding ref marker on the styled element.
    Provider {
        /// Canonical ref declaration identity.
        reference: IdentityId,
        /// Ref binding name used in diagnostics.
        name: String,
    },
    /// Declares participation in a view transition.
    Transition {
        /// Canonical transition declaration identity.
        identity: IdentityId,
        /// Expanded selector and enclosing at-rules.
        context: RuleContext,
    },
}

/// A declaration's static CSS value or the source expression producing its runtime
/// value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DeclarationValue {
    /// Serialized CSS value without the declaration terminator; priority text may
    /// remain.
    Css(
        /// Serialized value; a CSS priority suffix is retained when present.
        String,
    ),
    /// Original expression converted to a name at runtime.
    Runtime(
        /// Span in the original producer module.
        Span,
    ),
}

/// Expanded CSS conditions with the generated class represented by `&`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct RuleContext {
    /// Expanded selector with nesting tokens in place of the generated class.
    pub(crate) selector: String,
    /// Enclosing at-rule headers, outermost first.
    pub(crate) at_rules: Vec<String>,
    /// Element classification used for participant and provider checks.
    pub(crate) subject: RuleSubject,
}

/// Whether the selector applies declarations to the styled element.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum RuleSubject {
    /// The element receiving the style.
    Target,
    /// Another element or a pseudo-element.
    Other,
    /// The selector does not prove which element receives the declarations.
    Unknown,
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn occurrence_order_preserves_module_file_and_byte_identity() {
        let ids = [
            UseId {
                module: ModuleId::from("a"),
                span: Span::new(FileId(1), 2, 3),
            },
            UseId {
                module: ModuleId::from("a"),
                span: Span::new(FileId(1), 2, 4),
            },
            UseId {
                module: ModuleId::from("a"),
                span: Span::new(FileId(1), 3, 4),
            },
            UseId {
                module: ModuleId::from("a"),
                span: Span::new(FileId(2), 0, 1),
            },
            UseId {
                module: ModuleId::from("b"),
                span: Span::new(FileId(1), 0, 1),
            },
        ];
        for pair in ids.windows(2) {
            assert_eq!(pair[0].cmp(&pair[1]), Ordering::Less);
            assert_eq!(pair[1].cmp(&pair[0]), Ordering::Greater);
            assert_ne!(pair[0], pair[1]);
        }
        assert_eq!(ids[0].partial_cmp(&ids[0]), Some(Ordering::Equal));
    }
}
