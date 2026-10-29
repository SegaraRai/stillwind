//! Built-in definitions bundled in package order by `vp run generate:builtin-css`.

/// The standard package's utility, variant, and property definitions without imports.
pub const PACKAGE_CSS: &str = include_str!("../assets/builtins.css");

/// The complete standard stylesheet, including theme, preflight, and output slots.
pub(crate) const DEFAULT_CSS: &str = include_str!("../assets/index.css");
