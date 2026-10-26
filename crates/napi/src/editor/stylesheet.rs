//! Editor requests for stylesheets that the user writes.
//!
//! Every offset and range counts UTF-8 bytes in the original source. The requests
//! accept incomplete blocks and incomplete candidates.

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::StylesheetCompletionOptions;
use stillwind_source::BytePos;

use super::query::reason;
use super::{NativeEditorLintEdit, NativeEditorQuery};
use crate::output::NativeText;

/// The location of a CSS definition, as zero-based lines and UTF-16 columns that an
/// editor can show.
#[napi(object)]
pub struct NativeStylesheetDefinition {
    /// The absolute URI of the document that contains the definition.
    pub uri: String,
    /// The zero-based line of the inclusive start.
    pub start_line: u32,
    /// The inclusive start, as a UTF-16 column on `startLine`.
    pub start_column: u32,
    /// The zero-based line of the exclusive end.
    pub end_line: u32,
    /// The exclusive end, as a UTF-16 column on `endLine`.
    pub end_column: u32,
}

/// One completion in a stylesheet. Its range counts UTF-8 bytes in the original source.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetCompletion {
    /// The inclusive start of the replaced text, as a UTF-8 byte offset.
    pub start: u32,
    /// The exclusive end of the replaced text, as a UTF-8 byte offset.
    pub end: u32,
    /// The text that replaces the range, exactly as written.
    pub label: String,
    /// Whether the text is the name of an at-rule.
    pub keyword: bool,
    /// Whether the text is the name of a variant.
    pub variant: bool,
    /// For a directive, a description of who uses it and which syntax it belongs to.
    /// The field is absent for other completions.
    pub detail: Option<String>,
    /// For a directive, a text that an editor sorts lexically. It orders directives by
    /// where they are valid, who uses them, and which syntax they belong to. The field
    /// is absent for other completions.
    pub sort_text: Option<String>,
}

/// Editor settings that choose which at-rules stylesheet completion offers. They do not
/// change the compiler output.
#[napi(object)]
#[derive(Default)]
pub struct NativeStylesheetCompletionOptions {
    /// Whether to also offer directives that describe registry data, in places where
    /// they are useful. The default is false.
    pub registry_directives: Option<bool>,
    /// Whether to offer arbitrary variants with an empty `[]`. The default is false.
    pub snippets: Option<bool>,
    /// The complete list of candidate labels that host plugins provide. A variant label
    /// ends with `:`.
    pub catalog: Option<Vec<String>>,
}

impl From<NativeStylesheetCompletionOptions> for StylesheetCompletionOptions {
    fn from(value: NativeStylesheetCompletionOptions) -> Self {
        Self {
            registry_directives: value.registry_directives.unwrap_or(false),
            snippets: value.snippets.unwrap_or(false),
            catalog: value
                .catalog
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

/// The CSS or the documentation that an editor shows for a range of a stylesheet. The
/// range counts UTF-8 bytes in the original source.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetHover {
    /// The inclusive start of the described text, as a UTF-8 byte offset.
    pub start: u32,
    /// The exclusive end of the described text, as a UTF-8 byte offset.
    pub end: u32,
    /// The generated CSS. It is empty when the hover shows only documentation.
    pub css: String,
    /// A description in Markdown. It is empty when the hover shows the CSS of a
    /// candidate.
    pub documentation: String,
}

/// A range of UTF-8 bytes in the original stylesheet. The start is inclusive, and the
/// end is exclusive.
#[napi(object)]
pub struct NativeStylesheetRange {
    /// The inclusive start, as a UTF-8 byte offset.
    pub start: u32,
    /// The exclusive end, as a UTF-8 byte offset.
    pub end: u32,
}

/// A known sRGB color at a utility or a reference in the original CSS.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetColor {
    /// The inclusive start, as a UTF-8 byte offset.
    pub start: u32,
    /// The exclusive end, as a UTF-8 byte offset.
    pub end: u32,
    /// The sRGB red component, from 0 to 1.
    pub red: f64,
    /// The sRGB green component, from 0 to 1.
    pub green: f64,
    /// The sRGB blue component, from 0 to 1.
    pub blue: f64,
    /// The opacity that applies, from 0 to 1.
    pub alpha: f64,
    /// The utility as written in the source. The field is absent when an editor would
    /// replace a reference with a CSS color value.
    pub candidate: Option<String>,
}

/// The path of a `@plugin` statement. Its range does not include the CSS quotes.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetPluginPath {
    /// The UTF-8 byte offset after the opening quote.
    pub start: u32,
    /// The UTF-8 byte offset of the closing quote, or the end of the source when the
    /// quote is missing.
    pub end: u32,
    /// The decoded CSS string that module resolution uses.
    pub specifier: String,
    /// The opening quote, either `'` or `"`.
    pub quote: String,
    /// Whether the string has a closing quote in the original source.
    pub closed: bool,
    /// The decoded text before the requested cursor, when the cursor lies inside this
    /// path. The field is absent otherwise.
    pub prefix: Option<String>,
}

/// Returns the definitions and imports of the incomplete CSS `source`, so that an
/// editor can load them while the user types. Ordinary compilation and lint must still
/// use the original source.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn editor_css_source(source: String) -> String {
    stillwind_compiler::editor_css_source(&source).into()
}

/// Returns a short summary of the declarations or rules in the CSS `source` of one
/// selected completion. Returns null when the CSS has nothing to show.
#[napi(ts_return_type = "string | null")]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn completion_detail(source: String) -> Option<NativeText> {
    stillwind_compiler::completion_detail(&source).map(Into::into)
}

/// Returns the paths of the top-level `@plugin` statements in `source`. The method does
/// not load CSS or plugins. When `offset` is given, it is a UTF-8 byte offset of the
/// cursor, and the path that contains it receives a `prefix`.
///
/// # Errors
///
/// Returns an error when a path boundary does not fit in `u32`.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn stylesheet_plugin_paths(
    source: String,
    offset: Option<u32>,
) -> napi::Result<Vec<NativeStylesheetPluginPath>> {
    stillwind_compiler::stylesheet_plugin_paths(&source, offset.map(|offset| offset as usize))
        .into_iter()
        .map(|path| {
            Ok(NativeStylesheetPluginPath {
                start: u32::try_from(path.range.start).map_err(reason)?,
                end: u32::try_from(path.range.end).map_err(reason)?,
                specifier: path.specifier.into(),
                quote: path.quote.to_string(),
                closed: path.closed,
                prefix: path.prefix.map(Into::into),
            })
        })
        .collect()
}

#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
impl NativeEditorQuery {
    /// Returns the location of the definition that the CSS reference at the UTF-8 byte
    /// offset `offset` of `source` refers to. Returns null when the reference does not
    /// resolve, or when the source of the definition has no document URI.
    ///
    /// # Errors
    ///
    /// Returns an error when the location of the definition cannot be converted to
    /// lines and columns.
    #[napi]
    pub fn stylesheet_definition(
        &self,
        source: String,
        offset: u32,
    ) -> napi::Result<Option<NativeStylesheetDefinition>> {
        let Some(range) = self.query.stylesheet_definition(&source, offset as usize) else {
            return Ok(None);
        };
        let Some(uri) = range.source().uri() else {
            return Ok(None);
        };
        let start = range
            .source()
            .position(BytePos(range.start()))
            .map_err(reason)?;
        let end = range
            .source()
            .position(BytePos(range.end()))
            .map_err(reason)?;
        Ok(Some(NativeStylesheetDefinition {
            uri: uri.as_str().into(),
            start_line: start.line,
            start_column: start.utf16_column,
            end_line: end.line,
            end_column: end.utf16_column,
        }))
    }

    /// Returns each known color in the CSS `source`, with its byte range. Incomplete
    /// blocks are also read. A reference that is resolved at runtime uses the active
    /// theme definition.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition, or when an
    /// offset does not fit in `u32`.
    #[napi]
    pub fn stylesheet_colors(&self, source: String) -> napi::Result<Vec<NativeStylesheetColor>> {
        self.query
            .stylesheet_colors(&source)
            .map_err(reason)?
            .into_iter()
            .map(|color| {
                Ok(NativeStylesheetColor {
                    start: u32::try_from(color.range.start).map_err(reason)?,
                    end: u32::try_from(color.range.end).map_err(reason)?,
                    red: color.rgba[0],
                    green: color.rgba[1],
                    blue: color.rgba[2],
                    alpha: color.rgba[3],
                    candidate: color.candidate.map(Into::into),
                })
            })
            .collect()
    }

    /// Returns the edits that sort the utilities of the CSS `source`. The edits do not
    /// overlap. The method does not edit inside the byte ranges `protected`.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition, for a
    /// protected range whose start is after its end or that splits a UTF-8 character,
    /// or when an offset does not fit in `u32`.
    #[napi]
    pub fn sort_stylesheet(
        &self,
        source: String,
        protected: Option<Vec<NativeStylesheetRange>>,
    ) -> napi::Result<Vec<NativeEditorLintEdit>> {
        let protected = protected
            .unwrap_or_default()
            .into_iter()
            .map(|range| {
                let (start, end) = (range.start as usize, range.end as usize);
                if start > end || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
                    return Err(Error::from_reason("Invalid protected CSS range"));
                }
                Ok(start..end)
            })
            .collect::<napi::Result<Vec<_>>>()?;
        self.query
            .sort_stylesheet(&source, &protected)
            .map_err(reason)?
            .into_iter()
            .map(|edit| {
                Ok(NativeEditorLintEdit {
                    start: u32::try_from(edit.range.start).map_err(reason)?,
                    end: u32::try_from(edit.range.end).map_err(reason)?,
                    replacement: edit.replacement.into(),
                })
            })
            .collect()
    }

    /// Returns the completions for the CSS `source` at the UTF-8 byte offset `offset`.
    /// Incomplete blocks and candidates are accepted. An offset that is not a character
    /// boundary returns no completions.
    ///
    /// An omitted `options` offers the syntax that users write. `options` does not
    /// change the compiler settings of the query, or the lists that hovers and
    /// arguments use.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition.
    #[napi]
    pub fn complete_stylesheet(
        &self,
        source: String,
        offset: u32,
        options: Option<NativeStylesheetCompletionOptions>,
    ) -> napi::Result<Vec<NativeStylesheetCompletion>> {
        self.query
            .complete_stylesheet(&source, offset as usize, options.unwrap_or_default().into())
            .map_err(reason)?
            .into_iter()
            .map(|item| {
                Ok(NativeStylesheetCompletion {
                    start: u32::try_from(item.range.start).map_err(reason)?,
                    end: u32::try_from(item.range.end).map_err(reason)?,
                    label: item.label.into(),
                    keyword: item.keyword,
                    variant: item.variant,
                    detail: item.detail.map(Into::into),
                    sort_text: item.sort_text.map(Into::into),
                })
            })
            .collect()
    }

    /// Returns the generated CSS or the documentation for the input at the UTF-8 byte
    /// offset `offset` of the CSS `source`. Returns null when there is nothing to show.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed requirements of a utility definition, or when an
    /// offset does not fit in `u32`.
    #[napi]
    pub fn hover_stylesheet(
        &self,
        source: String,
        offset: u32,
    ) -> napi::Result<Option<NativeStylesheetHover>> {
        self.query
            .hover_stylesheet(&source, offset as usize)
            .map_err(reason)?
            .map(|hover| {
                Ok(NativeStylesheetHover {
                    start: u32::try_from(hover.range.start).map_err(reason)?,
                    end: u32::try_from(hover.range.end).map_err(reason)?,
                    css: hover.css.into(),
                    documentation: hover.documentation.into(),
                })
            })
            .transpose()
    }

    /// Returns the utility keys of host plugins and the `@variant ` requests that the
    /// stylesheet `css` uses in `@apply` and `@variant`, at any nesting level. The
    /// method does not read comments, strings, definition headers, or other directives.
    #[napi]
    pub fn stylesheet_dynamic_candidates(&self, css: String) -> Vec<String> {
        self.query
            .stylesheet_dynamic_candidates(&css)
            .into_iter()
            .map(|value| value.to_string())
            .collect()
    }
}
