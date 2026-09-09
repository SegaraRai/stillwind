//! CSS-first theme parsing. Lightning CSS validates the stylesheet before stillwind
//! extracts its small, source-spanned `@theme` surface.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use lightningcss::{
    printer::PrinterOptions,
    properties::{
        Property, PropertyId, background::BackgroundSize, border::BorderSideWidth,
        box_shadow::BoxShadow,
    },
    stylesheet::{ParserOptions, StyleSheet},
    traits::{Parse, ToCss},
    values::{
        angle::{Angle, AnglePercentage},
        color::CssColor,
        easing::EasingFunction,
        ident::Ident,
        image::Image,
        length::{Length, LengthPercentage},
        number::CSSNumber,
        percentage::{NumberOrPercentage, Percentage},
        position::Position,
        ratio::Ratio,
        time::Time,
        url::Url,
    },
    visit_types,
    visitor::{Visit, VisitTypes, Visitor},
};
use stillwind_source::{FileId, SourceFile, Span, Spanned};
use thiserror::Error;

mod catalog;
pub use catalog::{TAILWIND_V4_PREFLIGHT, csswg_formal_syntax};
use catalog::{TAILWIND_V4_THEME, webref_property};

mod values;
pub use values::*;
mod value_syntax;
use value_syntax::{
    contains_custom_property_reference, content_value, gradient_arguments, property_value,
    transform_function_arguments,
};
mod colors;
pub use colors::*;
mod properties;
pub use properties::*;
mod domains;
pub use domains::*;
mod builtins;
pub use builtins::*;
mod theme_types;
pub use theme_types::*;
mod definitions;
pub use definitions::*;
mod registry;
pub use registry::*;
mod math_expression;
use math_expression::{MathExpression, MathValue};
mod math_parser;
use math_parser::MathParser;
mod math_folding;
pub use math_folding::*;
mod format_registry;
pub use format_registry::*;
mod theme_index;

mod theme_references;
use theme_references::{custom_property_references, resolve_theme_references};
mod property_usage;
pub use property_usage::*;
mod theme_parser;
pub use theme_parser::*;
mod imports;
use imports::{framework_imports, parse_import_path};
mod theme_declarations;
use theme_declarations::{extract_theme_declarations, matching_brace};
mod directives;
use directives::{
    extract_authored_property_names, extract_property_definitions, extract_stillwind_config,
    strip_blocks, strip_framework_directives,
};
mod utility_definitions;
use utility_definitions::{
    extract_custom_utilities, extract_declaration_formats, extract_marker_utilities,
    extract_relationship_patterns,
};
mod blocks;
use blocks::{
    extract_named_blocks, extract_named_blocks_with_offsets, extract_order_statements,
    strip_css_comments,
};
mod variants;
use variants::extract_custom_variants;

#[cfg(test)]
mod tests;
