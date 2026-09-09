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
