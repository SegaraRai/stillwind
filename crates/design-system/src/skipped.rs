//! CSS statements that loading skips instead of failing.
//!
//! An invalid import or Stillwind directive does not stop loading. The loader leaves
//! the statement out, or ignores its condition, and records a [`SkippedCss`] with the
//! span of the statement in its authored source. Stylesheet lint reports the records of
//! the sources that a host marks for linting. Malformed CSS that the loader cannot
//! divide into statements, such as an unterminated block, still fails the load.

use std::ops::Range;

use lean_string::LeanStr;

use stillwind_diagnostics::LintKind;
use stillwind_source::{FileId, Span};

/// Why loading skipped a statement.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SkipReason {
    /// An `@import` or `@plugin` statement has no valid specifier or options.
    #[error("the import has an invalid specifier: {detail}")]
    ImportSpecifier {
        /// Parser explanation.
        detail: LeanStr,
    },
    /// The modifiers after an `@import` specifier cannot be read, or they are not in
    /// the order `layer`, `supports`, media query.
    #[error("the import has invalid modifiers: {detail}")]
    ImportModifiers {
        /// Parser explanation.
        detail: LeanStr,
    },
    /// The host could not resolve or read the imported file.
    #[error("`{specifier}` cannot be resolved or read")]
    UnresolvedImport {
        /// Specifier as written.
        specifier: LeanStr,
    },
    /// The import closes a cycle of imports.
    #[error("importing `{specifier}` here closes an import cycle")]
    ImportCycle {
        /// Specifier as written.
        specifier: LeanStr,
    },
    /// A definition is inside a conditional at-rule. The definition applies without the
    /// condition.
    #[error(
        "`@{directive}` defines at compile time, so the enclosing condition is ignored; put conditions around `@stillwind-slot utilities;` instead"
    )]
    ConditionalDefinition {
        /// At-rule name without `@`.
        directive: LeanStr,
    },
    /// A directive that requires a block is a statement, or the reverse.
    #[error("`@{directive}` has an unsupported block or statement form")]
    DirectiveForm {
        /// At-rule name without `@`.
        directive: LeanStr,
    },
    /// An output slot names no supported slot.
    #[error("`@stillwind-slot {slot}` names an unsupported slot")]
    UnsupportedSlot {
        /// Slot text as written.
        slot: LeanStr,
    },
    /// An output slot is in a CSS context that cannot contain its output.
    #[error("the `{slot}` slot cannot appear in this CSS context")]
    SlotContext {
        /// Slot name.
        slot: LeanStr,
    },
    /// A style rule with output slots also contains rules or other slots. Its slots are
    /// ignored, and the rule stays authored CSS.
    #[error(
        "a style rule with `@stillwind-slot` can contain only authored CSS and the theme or property-initials slots"
    )]
    SlotStyleRule,
    /// A `@stillwind-css-template` rule does not have one name and a block.
    #[error("`@stillwind-css-template` requires one name and a block")]
    CssTemplate,
    /// A `@stillwind-css-template` body contains a definition.
    #[error("`@{directive}` cannot appear inside `@stillwind-css-template`")]
    DefinitionInCssTemplate {
        /// At-rule name without `@`.
        directive: LeanStr,
    },
    /// A `@stillwind-css-template` body has no utilities slot.
    #[error("`@stillwind-css-template {name}` requires `@stillwind-slot utilities;`")]
    CssTemplateWithoutUtilities {
        /// Template name.
        name: LeanStr,
    },
    /// A `@stillwind-registry` header is empty or names an unknown registry.
    #[error("`@stillwind-registry` names an unknown registry `{context}`")]
    UnknownRegistryContext {
        /// Registry name as written; empty when the header names none.
        context: LeanStr,
    },
    /// A `@stillwind-registry` block contains a statement that no registry holds.
    #[error("`@stillwind-registry` cannot contain `{statement}`")]
    RegistryStatement {
        /// `@` and the at-rule name, or the trimmed text of another statement.
        statement: LeanStr,
    },
}

impl SkipReason {
    /// Returns the lint rule that reports this reason: `invalid-css-import` for an
    /// `@import` or `@plugin` statement, `ignored-definition-condition` for a
    /// definition that applies without its condition, and `invalid-css-directive`
    /// otherwise.
    #[must_use]
    pub const fn lint_kind(&self) -> LintKind {
        match self {
            Self::ImportSpecifier { .. }
            | Self::ImportModifiers { .. }
            | Self::UnresolvedImport { .. }
            | Self::ImportCycle { .. } => LintKind::InvalidCssImport,
            Self::ConditionalDefinition { .. } => LintKind::IgnoredDefinitionCondition,
            _ => LintKind::InvalidCssDirective,
        }
    }
}

/// One statement that loading skipped, located in its authored source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedCss {
    /// Span of the statement, or of the part of it that the reason names.
    pub span: Span,
    /// Why the statement was skipped.
    pub reason: SkipReason,
}

/// A skipped statement located by its byte range in the buffer being parsed.
pub(crate) type SkippedRange = (Range<usize>, SkipReason);

/// Converts `skipped`, whose ranges address the buffer `file`, into located records.
/// Ranges beyond the 32-bit source limit are clamped.
pub(crate) fn locate(file: FileId, skipped: Vec<SkippedRange>) -> Vec<SkippedCss> {
    let offset = |position: usize| u32::try_from(position).unwrap_or(u32::MAX);
    skipped
        .into_iter()
        .map(|(range, reason)| SkippedCss {
            span: Span::new(file, offset(range.start), offset(range.end)),
            reason,
        })
        .collect()
}
