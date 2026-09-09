use super::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StillwindConfig {
    pub prefix: Option<String>,
    pub tailwind_prefix: Option<String>,
    pub lint_rules: BTreeMap<String, String>,
}

impl StillwindConfig {
    #[must_use]
    pub fn lint_enabled(&self, rule: &str) -> bool {
        self.lint_rules
            .get(rule)
            .is_some_and(|level| matches!(level.as_str(), "warn" | "deny"))
    }
}

/// The Tailwind entry points selected by an authored `@import` directive.
/// Layer names are retained rather than inferred during rendering so a custom
/// `layer(...)` placement remains observable in the generated stylesheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkImports {
    pub theme_layer: Option<String>,
    pub base_layer: Option<String>,
    pub utilities_layer: Option<String>,
    pub builtins: bool,
    /// Whether the canonical single full-framework import was authored.
    /// Tailwind emits the global layer-order prelude only for that entrypoint.
    pub layer_order: bool,
    /// Tailwind's class/theme namespace from `prefix(name)` import modifiers.
    pub prefix: Option<String>,
}

impl Default for FrameworkImports {
    fn default() -> Self {
        Self {
            theme_layer: Some("theme".to_owned()),
            base_layer: Some("base".to_owned()),
            utilities_layer: Some("utilities".to_owned()),
            builtins: true,
            layer_order: true,
            prefix: None,
        }
    }
}

impl FrameworkImports {
    pub(super) fn explicit() -> Self {
        Self {
            theme_layer: None,
            base_layer: None,
            utilities_layer: None,
            builtins: false,
            layer_order: false,
            prefix: None,
        }
    }

    #[must_use]
    pub fn has_any(&self) -> bool {
        self.theme_layer.is_some() || self.base_layer.is_some() || self.utilities_layer.is_some()
    }
}
