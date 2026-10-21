//! Message of a `var()` value whose known type the receiving property rejects.

use std::fmt;

use lean_string::LeanStr;

/// Message of [`super::LintDiagnostic::VariableTypeMismatch`], which names where the
/// variable's type comes from and lists hinted spellings when there are any.
pub(super) struct VariableTypeMessage<'a> {
    pub(super) utility: &'a str,
    pub(super) property: &'a str,
    pub(super) variable: &'a str,
    pub(super) value_type: &'a str,
    pub(super) namespace: &'a Option<LeanStr>,
    pub(super) alternatives: &'a [LeanStr],
}

impl fmt::Display for VariableTypeMessage<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            utility,
            property,
            variable,
            value_type,
            namespace,
            alternatives,
        } = self;
        match namespace {
            Some(namespace) => write!(
                formatter,
                "`{variable}` is a `{value_type}` value of the `--{namespace}-*` theme namespace"
            )?,
            None => write!(
                formatter,
                "`{variable}` is registered with the syntax `{value_type}`"
            )?,
        }
        write!(
            formatter,
            ", but `{utility}` sets `{property}`, which does not accept it"
        )?;
        let Some((last, rest)) = alternatives.split_last() else {
            return Ok(());
        };
        formatter.write_str("; add a type hint, such as ")?;
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

    #[test]
    fn variable_type_messages_name_the_source_of_the_type() {
        let theme = LintDiagnostic::VariableTypeMismatch {
            utility: "text-(--text-sm)".into(),
            property: "color".into(),
            variable: "--text-sm".into(),
            value_type: "font-size".into(),
            namespace: Some("text".into()),
            alternatives: vec!["text-(length:--text-sm)".into()],
        };
        assert_eq!(theme.code(), "incompatible-property-value");
        assert_eq!(
            theme.to_lean_string(),
            "`--text-sm` is a `font-size` value of the `--text-*` theme namespace, but `text-(--text-sm)` sets `color`, which does not accept it; add a type hint, such as `text-(length:--text-sm)`"
        );
        let registered = LintDiagnostic::VariableTypeMismatch {
            utility: "bg-(--my-len)".into(),
            property: "background-color".into(),
            variable: "--my-len".into(),
            value_type: "<length>".into(),
            namespace: None,
            alternatives: Vec::new(),
        };
        assert_eq!(
            registered.to_lean_string(),
            "`--my-len` is registered with the syntax `<length>`, but `bg-(--my-len)` sets `background-color`, which does not accept it"
        );
    }
}
