//! Property names, style representations, and runtime modules for compiled props.

use serde::{Deserialize, Serialize};

/// A supported representation of compiled class and style props.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// `class` with an object of inline CSS declarations.
    ClassObject,
    /// `class` with serialized inline CSS declarations.
    ClassString,
    /// `className` with an object of inline CSS declarations.
    ClassNameObject,
}

impl OutputFormat {
    /// Returns the property and runtime conventions shared by this representation.
    pub const fn spec(self) -> &'static OutputSpec {
        match self {
            Self::ClassObject => &CLASS_OBJECT,
            Self::ClassString => &CLASS_STRING,
            Self::ClassNameObject => &CLASS_NAME_OBJECT,
        }
    }
}

/// Property and runtime conventions independent of the source framework.
pub struct OutputSpec {
    /// Module providing production helpers and fragment keys for this representation.
    pub runtime_import: &'static str,
    /// Module providing checked helpers with the production runtime's fragment keys.
    pub runtime_import_dev: &'static str,
    /// Property receiving class text: `class` or `className`.
    pub class_property: &'static str,
    /// Property receiving inline CSS declarations.
    pub style_property: &'static str,
    /// Representation used to materialize inline CSS declarations.
    pub style_format: StyleFormat,
}

/// Representation of generated inline CSS declarations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleFormat {
    /// An object whose keys are CSS property names.
    Object,
    /// Semicolon-separated CSS declaration text.
    String,
}

const CLASS_OBJECT: OutputSpec = OutputSpec {
    runtime_import: "stillwind/runtime/class-object",
    runtime_import_dev: "stillwind/runtime/class-object-development",
    class_property: "class",
    style_property: "style",
    style_format: StyleFormat::Object,
};

const CLASS_STRING: OutputSpec = OutputSpec {
    runtime_import: "stillwind/runtime/class-string",
    runtime_import_dev: "stillwind/runtime/class-string-development",
    class_property: "class",
    style_property: "style",
    style_format: StyleFormat::String,
};

const CLASS_NAME_OBJECT: OutputSpec = OutputSpec {
    runtime_import: "stillwind/runtime/class-name-object",
    runtime_import_dev: "stillwind/runtime/class-name-object-development",
    class_property: "className",
    style_property: "style",
    style_format: StyleFormat::Object,
};
