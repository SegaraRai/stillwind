//! Messages about relations between the variants of one candidate.

use std::fmt;

/// Message of [`super::LintDiagnostic::RedundantVariant`].
pub(super) struct RedundantVariantMessage<'a> {
    pub(super) variant: &'a str,
    pub(super) kept: &'a str,
    pub(super) implied: bool,
}

impl fmt::Display for RedundantVariantMessage<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            variant,
            kept,
            implied,
        } = self;
        if *implied {
            write!(
                formatter,
                "variant `{variant}` has no effect because `{kept}` implies its condition"
            )
        } else {
            write!(
                formatter,
                "variant `{variant}` repeats the earlier `{kept}` and has no effect"
            )
        }
    }
}

/// Message of [`super::LintDiagnostic::UnsatisfiableVariants`], which explains every
/// branch when the combined condition has several.
pub(super) struct UnsatisfiableMessage<'a> {
    pub(super) variants: &'a str,
    pub(super) first: &'a str,
    pub(super) second: &'a str,
    pub(super) branches: u32,
}

impl fmt::Display for UnsatisfiableMessage<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            variants,
            first,
            second,
            branches,
        } = self;
        if *branches > 1 {
            write!(
                formatter,
                "variants `{variants}` never match: each of the {branches} alternatives of their condition contains conditions that exclude each other, such as `{first}` and `{second}`"
            )
        } else {
            write!(
                formatter,
                "variants `{variants}` never match because `{first}` and `{second}` exclude each other"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use super::super::LintDiagnostic;

    #[test]
    fn variant_relation_messages_name_both_variants_or_conditions() {
        let duplicate = LintDiagnostic::RedundantVariant {
            variant: "hover".into(),
            kept: "hover".into(),
            implied: false,
        };
        assert_eq!(duplicate.code(), "redundant-variant");
        assert_eq!(
            duplicate.to_lean_string(),
            "variant `hover` repeats the earlier `hover` and has no effect"
        );
        let implied = LintDiagnostic::RedundantVariant {
            variant: "sm".into(),
            kept: "md".into(),
            implied: true,
        };
        assert_eq!(
            implied.to_lean_string(),
            "variant `sm` has no effect because `md` implies its condition"
        );
        let single = LintDiagnostic::UnsatisfiableVariants {
            variants: "max-sm:sm".into(),
            first: "@media (width < 40rem)".into(),
            second: "@media (width >= 40rem)".into(),
            branches: 1,
        };
        assert_eq!(single.code(), "unsatisfiable-variants");
        assert_eq!(
            single.to_lean_string(),
            "variants `max-sm:sm` never match because `@media (width < 40rem)` and `@media (width >= 40rem)` exclude each other"
        );
        let several = LintDiagnostic::UnsatisfiableVariants {
            variants: "not-hover:hover".into(),
            first: "&:not(:hover)".into(),
            second: "&:hover".into(),
            branches: 2,
        };
        assert_eq!(
            several.to_lean_string(),
            "variants `not-hover:hover` never match: each of the 2 alternatives of their condition contains conditions that exclude each other, such as `&:not(:hover)` and `&:hover`"
        );
    }
}
