//! Final stylesheet minification and source-map composition through Lightning CSS.
//!
//! [`minify_css`] accepts expanded CSS and an optional map for that exact input.
//! Hosts own resource resolution, asset boundaries, and source-map publication.
//!
//! After Lightning CSS optimizes the stylesheet, [`optimizer`] applies rewrites that
//! Lightning CSS does not make. A rewrite of a common form may change the result in a
//! rare case when the change is small, such as a folded negative `calc()` result that
//! `width` would clamp. A rewrite of a rare form must keep the meaning of the
//! stylesheet for every browser target. Merging rules that are not adjacent is a
//! non-goal, because it can change the cascade of common stylesheets.
//!
//! Modules:
//!
//! - [`asset`] minifies one asset and composes its source map.
//! - [`optimizer`] visits the minified stylesheet and calls the rewrites of the other
//!   modules.
//! - [`options`] validates caller options and converts them to browser targets.
//! - [`queries`] shortens media features, merges ranges, and compacts `@supports`
//!   declarations.
//! - [`registrations`] finds custom properties that `@property` registers as numbers.
//! - [`rules`] removes repeated selectors and merges equal keyframes.
//! - [`tokens`] folds numeric `calc()` calls and drops whitespace in unparsed values.

mod asset;
mod optimizer;
mod options;
mod queries;
mod registrations;
mod rules;
mod tokens;

pub use asset::{CssMinifyError, MinifiedCss, minify_css};
pub use options::CssMinifyOptions;
