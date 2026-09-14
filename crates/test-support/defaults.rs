//! The standard package's builtin CSS, concatenated for tests.
//!
//! The data is `packages/core/css/builtin-{utilities,variants,metadata}/*.css` plus `builtins.css`,
//! read at compile time with `include_str!`. The list is hand-maintained, so
//! adding or renaming a builtin file means updating it here. Order matters and mirrors
//! the package's own layering (registry metadata, then utility families, then variant
//! families, then the package entry).

/// The full builtin CSS of the standard package, in package order.
pub const BUILTIN_REGISTRY_CSS: &str = concat!(
    include_str!("../../packages/core/css/builtin-metadata/tw-registry-metadata.css"),
    include_str!("../../packages/core/css/builtin-metadata/tw-relationship-metadata.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-layout.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-spacing-sizing.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-flex-grid.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-typography.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-backgrounds.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-borders.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-effects.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-masks.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-transforms.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-motion.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-interactivity.css"),
    include_str!("../../packages/core/css/builtin-utilities/tw-svg.css"),
    include_str!("../../packages/core/css/builtin-variants/tw-relationship.css"),
    include_str!("../../packages/core/css/builtin-variants/tw-structural.css"),
    include_str!("../../packages/core/css/builtin-variants/tw-state.css"),
    include_str!("../../packages/core/css/builtin-variants/tw-dynamic-selector.css"),
    include_str!("../../packages/core/css/builtin-variants/tw-conditional.css"),
    include_str!("../../packages/core/css/builtin-variants/sw-conditional.css"),
    include_str!("../../packages/core/css/builtin-metadata/tw-properties.css"),
    include_str!("../../packages/core/css/builtins.css"),
);
