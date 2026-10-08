//! Shared source builders and result helpers for these contracts.

use super::*;

pub(super) fn marker(result: &CompileResult, label: &str) -> String {
    result
        .symbols
        .iter()
        .find(|symbol| {
            symbol.kind == GeneratedSymbolKind::RefMarker
                && symbol.name.starts_with(&format!("sw_{label}_"))
        })
        .unwrap_or_else(|| panic!("missing marker for {label}: {:?}", result.symbols))
        .name
        .to_string()
}
