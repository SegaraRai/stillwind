use crate::blocks::extract_order_statements;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinArbitraryPropertyOrder {
    pub pattern: String,
    pub builtin_order: usize,
}

/// Parses explicit cascade positions for arbitrary-property candidates.
/// Patterns use an exact property name or a trailing `*` prefix wildcard.
#[must_use]
pub fn parse_builtin_arbitrary_property_order(css: &str) -> Vec<BuiltinArbitraryPropertyOrder> {
    extract_order_statements(css, "@stillwind-arbitrary-property")
        .into_iter()
        .map(|(builtin_order, pattern)| BuiltinArbitraryPropertyOrder {
            pattern,
            builtin_order,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_whitespace_patterns_and_keeps_source_offsets() {
        let css = "@stillwind-arbitrary-property margin; @stillwind-arbitrary-property --x-*; @stillwind-arbitrary-property two words;";
        let orders = parse_builtin_arbitrary_property_order(css);
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].builtin_order, 0);
        assert_eq!(
            orders[1].builtin_order,
            css.find("@stillwind-arbitrary-property --x-*").unwrap()
        );
        assert_eq!(orders[1].pattern, "--x-*");
    }
}
