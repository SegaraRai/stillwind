//! The standard package's builtin CSS, concatenated for tests.
//!
//! The data is `packages/core/css/builtins/*.css` plus `builtins.css`, read at compile
//! time with `include_str!`. The list is hand-maintained: nothing generates it, so
//! adding or renaming a builtin file means updating it here. Order matters and mirrors
//! the package's own layering (registry metadata, then utility families, then variant
//! families, then the package entry).

/// The full builtin CSS of the standard package, in package order.
pub const BUILTIN_REGISTRY_CSS: &str = concat!(
    include_str!("../../packages/core/css/builtins/registry-metadata.css"),
    include_str!("../../packages/core/css/builtins/relationship-metadata.css"),
    include_str!("../../packages/core/css/builtins/layout-utilities.css"),
    include_str!("../../packages/core/css/builtins/spacing-sizing-utilities.css"),
    include_str!("../../packages/core/css/builtins/flex-grid-utilities.css"),
    include_str!("../../packages/core/css/builtins/typography-utilities.css"),
    include_str!("../../packages/core/css/builtins/backgrounds-utilities.css"),
    include_str!("../../packages/core/css/builtins/borders-utilities.css"),
    include_str!("../../packages/core/css/builtins/effects-utilities.css"),
    include_str!("../../packages/core/css/builtins/masks-utilities.css"),
    include_str!("../../packages/core/css/builtins/transforms-utilities.css"),
    include_str!("../../packages/core/css/builtins/motion-utilities.css"),
    include_str!("../../packages/core/css/builtins/interactivity-utilities.css"),
    include_str!("../../packages/core/css/builtins/svg-utilities.css"),
    include_str!("../../packages/core/css/builtins/relationship-variants.css"),
    include_str!("../../packages/core/css/builtins/structural-variants.css"),
    include_str!("../../packages/core/css/builtins/state-variants.css"),
    include_str!("../../packages/core/css/builtins/dynamic-selector-variants.css"),
    include_str!("../../packages/core/css/builtins/conditional-variants.css"),
    include_str!("../../packages/core/css/builtins/properties.css"),
    include_str!("../../packages/core/css/builtins.css"),
);
