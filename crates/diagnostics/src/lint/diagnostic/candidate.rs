//! Optional parts of messages about utility candidates: a spelling suggestion, the
//! branch of a selector list that keeps it document-wide, and the type hints for a
//! value whose property was chosen as a fallback.

use std::fmt;

use lean_string::LeanStr;

/// Formats ``; did you mean `x`?`` after an unknown name, or nothing without a
/// suggestion.
pub(super) struct DidYouMean<'a>(pub(super) &'a Option<LeanStr>);

impl fmt::Display for DidYouMean<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(suggestion) => write!(formatter, "; did you mean `{suggestion}`?"),
            None => Ok(()),
        }
    }
}

/// Message of [`super::LintDiagnostic::DocumentScopedSelector`], which names the branch
/// that keeps a selector list document-wide when one is known.
pub(super) struct DocumentScopedMessage<'a> {
    pub(super) selector: &'a str,
    pub(super) branch: Option<&'a str>,
}

impl fmt::Display for DocumentScopedMessage<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let selector = self.selector;
        match self.branch {
            Some(branch) => write!(
                formatter,
                "`{selector}` does not depend on the styled element because of the branch `{branch}`; move that branch to its own variant and declare document-wide CSS in a global stylesheet"
            ),
            None => write!(
                formatter,
                "`{selector}` does not depend on the styled element; declare document-wide CSS in a global stylesheet"
            ),
        }
    }
}

/// Message of [`super::LintDiagnostic::IncompatiblePropertyValue`], which explains a
/// property chosen as a fallback and lists hinted spellings when there are any.
pub(super) struct PropertyValueMessage<'a> {
    pub(super) utility: &'a str,
    pub(super) property: &'a str,
    pub(super) value: &'a str,
    pub(super) alternatives: &'a [LeanStr],
}

impl fmt::Display for PropertyValueMessage<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            utility,
            property,
            value,
            alternatives,
        } = self;
        let Some((last, rest)) = alternatives.split_last() else {
            return write!(formatter, "`{property}` does not accept `{value}`");
        };
        write!(
            formatter,
            "`{utility}` sets `{property}` because `{value}` has no type hint and its type cannot be inferred, but `{property}` does not accept it; add a type hint, such as "
        )?;
        for alternative in rest {
            write!(formatter, "`{alternative}`")?;
            formatter.write_str(if rest.len() > 1 { ", " } else { " " })?;
        }
        if !rest.is_empty() {
            formatter.write_str("or ")?;
        }
        write!(formatter, "`{last}`")
    }
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use super::super::LintDiagnostic;

    /// Unknown names carry a suggestion in their message, and the message of the
    /// document-scoped selector names the branch to split, while each cause selects its
    /// own rule.
    #[test]
    fn unresolved_input_messages_name_their_cause() {
        let unknown = LintDiagnostic::UnknownUtility {
            utility: "bg-red-999".into(),
            detail: "`bg` has no value `red-999`".into(),
            suggestion: Some("bg-red-900".into()),
        };
        assert_eq!(unknown.code(), "unknown-utility");
        assert_eq!(
            unknown.to_lean_string(),
            "`bg` has no value `red-999`; did you mean `bg-red-900`?"
        );
        let invalid = LintDiagnostic::IncompatiblePropertyValue {
            utility: "w-[1/2]".into(),
            property: "width".into(),
            value: "1/2".into(),
            alternatives: Vec::new(),
        };
        assert_eq!(invalid.code(), "incompatible-property-value");
        assert_eq!(invalid.to_lean_string(), "`width` does not accept `1/2`");
        let fallback = LintDiagnostic::IncompatiblePropertyValue {
            utility: "text-[12px_foo]".into(),
            property: "color".into(),
            value: "12px foo".into(),
            alternatives: vec![
                "text-[color:12px_foo]".into(),
                "text-[length:12px_foo]".into(),
            ],
        };
        assert_eq!(
            fallback.to_lean_string(),
            "`text-[12px_foo]` sets `color` because `12px foo` has no type hint and its type cannot be inferred, but `color` does not accept it; add a type hint, such as `text-[color:12px_foo]` or `text-[length:12px_foo]`"
        );
        let variant = LintDiagnostic::UnknownVariant {
            variant: "foo".into(),
            detail: "unknown variant `foo`".into(),
            suggestion: None,
        };
        assert_eq!(variant.code(), "unknown-variant");
        assert_eq!(variant.to_lean_string(), "unknown variant `foo`");
        let scoped = LintDiagnostic::DocumentScopedSelector {
            selector: ":hover,:root".into(),
            branch: Some(":root".into()),
        };
        assert!(
            scoped
                .to_lean_string()
                .contains("because of the branch `:root`; move that branch to its own variant")
        );
    }
}
