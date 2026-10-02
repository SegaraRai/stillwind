//! Composition-aware checks over properties and markers recorded during lowering.
//!
//! `StyleResolver` resolves local and imported bases without reading published CSS.
//! `check_style` applies the seven checks that depend on a call's composed inputs.
//! `composition` preserves conditions and origins; `properties` checks attachments
//! and property interference; `view_transition` checks participant names and classes.

mod composition;
mod properties;
mod view_transition;

use stillwind_diagnostics::{LintFinding, LintKind};
use stillwind_ir::ConditionalBranches;
use stillwind_semantic::IdentityId;
use stillwind_source::{ModuleId, Span};

pub use composition::{ResolvedStyle, StyleResolver};

/// The direct supplies and base references of one compiled style call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleEffects {
    /// Declaration identity of the style call.
    pub identity: IdentityId,
    /// Original source call span.
    pub span: Span,
    /// Contributions with their selection conditions.
    pub root: StyleEffect,
}

/// A contribution or a selection between contributions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StyleEffect {
    /// All children contribute; an empty list supplies nothing.
    All(Vec<Self>),
    /// A JavaScript condition selects one branch.
    Branch {
        /// Original test occurrence; conditions at different sites stay independent.
        condition: UseId,
        /// Contributions in the truthy and optional falsy arms.
        branches: ConditionalBranches<Box<Self>>,
    },
    /// One runtime use selects at most one candidate.
    Choice {
        /// Runtime receiver occurrence shared by all alternatives.
        site: UseId,
        /// Mutually exclusive alternatives in stable candidate order.
        alternatives: Vec<Self>,
    },
    /// One source use supplies properties or a marker.
    Supply(StyleSupply),
    /// Another style contributes its complete payload.
    Base(IdentityId),
    /// An input's supplies cannot be determined statically.
    Unknown,
}

/// A source occurrence qualified by its producer module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseId {
    /// Producer module of the original occurrence.
    pub module: ModuleId,
    /// Original source coordinates, qualified by the producer module.
    pub span: Span,
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

/// Properties, marker requirements, or transition participation from one use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSupply {
    /// Source occurrence that supplies this contribution.
    pub site: UseId,
    /// Resolved semantic contribution.
    pub kind: SupplyKind,
    /// Resolved utility position within this source use; CSS alternatives share it.
    pub ordinal: usize,
}

/// The semantic contribution of a source use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupplyKind {
    /// A resolved CSS or inline declaration.
    Declaration {
        /// Decoded property name. Ordinary names are ASCII lowercase.
        property: String,
        /// CSS value, excluding the declaration terminator.
        value: DeclarationValue,
        /// Whether the declaration carries `!important`.
        important: bool,
        /// Expanded selector and enclosing at-rules.
        context: RuleContext,
        /// Whether a ref provider or explicit anchor scope supplied the declaration.
        provider: bool,
        /// Required ref marker, if this declaration is a provider.
        gate: Option<IdentityId>,
    },
    /// Attaches the marker for this canonical ref identity.
    Marker(IdentityId),
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

/// A declaration's static CSS value or the source expression producing its runtime value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationValue {
    /// Serialized CSS value without the declaration terminator.
    Css(String),
    /// Original expression converted to a name at runtime.
    Runtime(Span),
}

/// Expanded CSS conditions with the generated class represented by `&`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RuleContext {
    /// Expanded selector with nesting tokens in place of the generated class.
    pub selector: String,
    /// Enclosing at-rule headers, outermost first.
    pub at_rules: Vec<String>,
    /// Element classification used for participant and provider checks.
    pub subject: RuleSubject,
}

/// Whether the selector applies declarations to the styled element.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RuleSubject {
    /// The element receiving the style.
    Target,
    /// Another element or a pseudo-element.
    Other,
    /// The selector does not prove which element receives the declarations.
    Unknown,
}

/// Runs the checks that need composed supplies. The result retains source origins.
/// Unknown bases prevent absence conclusions but do not hide known conflicts.
pub fn check_style(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    let mut findings = properties::check(style);
    findings.extend(view_transition::check(style));
    findings
}

/// Identifies findings that must be replaced after a composition dependency changes.
pub const fn is_style_analysis_lint(kind: LintKind) -> bool {
    matches!(
        kind,
        LintKind::RequireRefAttachment
            | LintKind::ConflictingProviderProperties
            | LintKind::RequireViewTransitionName
            | LintKind::SingleViewTransitionName
            | LintKind::SingleViewTransitionClass
            | LintKind::AmbiguousBaseOrder
            | LintKind::ImportedBaseInterference
    )
}
