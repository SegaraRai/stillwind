//! Declarations, imports, and style calls extracted from one module.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_source::{Span, Spanned};

use super::{
    KeyframesDefinition, RefDefinition, StoredVariantDefinition, StyleCall,
    ViewTransitionDefinition,
};
use crate::{AdapterKind, OutputFormat};

/// Everything a frontend extracted from one module.
///
/// A frontend builds this in one pass. Later phases may reorder [`Self::style_calls`]
/// for emission, but they must not drop entries, duplicate them, or change the identity
/// of a declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleIr {
    /// Bindings and imports needed by code generation.
    #[serde(default)]
    pub imports: ModuleImports,
    /// In source order. A call can emit a base unit plus conditional units, or no CSS.
    pub style_calls: Vec<StyleCall>,
    /// Recipe factories and their independently compiled style values, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipe_calls: Vec<RecipeCall>,
    /// `style.media()` calls in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media_calls: Vec<MediaCall>,
    /// `style.css()` calls in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub css_calls: Vec<CssCall>,
    /// `style.forward()` calls outside `style()` calls, such as the second argument of a
    /// recipe factory, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forward_calls: Vec<ForwardCall>,
    /// Imported values whose declaring modules must precede this module in CSS order,
    /// in source order. One import can appear at several uses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ordering_uses: Vec<OrderingUse>,
    /// In source order. Each entry is a `style.ref()` declaration.
    pub ref_definitions: Vec<RefDefinition>,
    /// In source order. Each entry is a `style.keyframes()` declaration.
    pub keyframes_declarations: Vec<KeyframesDefinition>,
    /// In source order. Each entry is a `style.viewTransition()` declaration.
    pub view_transition_declarations: Vec<ViewTransitionDefinition>,
    /// In source order. Each entry is a `style.when()` declaration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stored_variants: Vec<StoredVariantDefinition>,
}

impl ModuleIr {
    /// Iterates ordinary style calls and recipe value expressions with their original
    /// spans.
    pub fn style_inputs(&self) -> impl Iterator<Item = &StyleCall> {
        self.style_calls
            .iter()
            .chain(self.recipe_calls.iter().flat_map(|call| &call.values))
    }

    /// Returns an empty module with no extracted declarations or calls.
    pub const fn empty() -> Self {
        Self {
            imports: ModuleImports {
                consumed_uses: Vec::new(),
                reserved_names: Vec::new(),
                macros: Vec::new(),
                module_css: Vec::new(),
            },
            style_calls: Vec::new(),
            recipe_calls: Vec::new(),
            media_calls: Vec::new(),
            css_calls: Vec::new(),
            forward_calls: Vec::new(),
            ordering_uses: Vec::new(),
            ref_definitions: Vec::new(),
            keyframes_declarations: Vec::new(),
            view_transition_declarations: Vec::new(),
            stored_variants: Vec::new(),
        }
    }
}

/// A recipe call whose configuration remains ordinary JavaScript after lowering.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeCall {
    /// Whole factory call in the original source, used to validate nested macro uses.
    pub span: Span,
    /// Prop representation selected by the factory's macro binding.
    pub output: OutputFormat,
    /// Callee expression replaced by the selected output runtime's recipe helper.
    pub callee_span: Span,
    /// Base and option expressions compiled independently in source order.
    pub values: Vec<StyleCall>,
    /// Replaces invalid whole calls with an empty factory.
    #[serde(default)]
    pub empty: bool,
    /// Replacements for unsupported configuration entries in original source
    /// coordinates.
    #[serde(default)]
    pub replacements: Vec<RecipeReplacement>,
}

/// One configuration expression or entry replaced during recipe validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeReplacement {
    /// Original expression or entry without its surrounding separator.
    pub span: Span,
    /// Static JavaScript that preserves the enclosing object or array syntax.
    pub replacement: LeanStr,
}

/// A `style.forward()` call outside `style()` whose callee lowering replaces with the
/// runtime helper that creates a forwarded fragment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardCall {
    /// The `style.forward` callee in the original source.
    pub callee_span: Span,
    /// Prop representation selected by the macro binding.
    pub output: OutputFormat,
    /// Whether the framework accepts an array of style values.
    pub style_arrays: bool,
}

/// A use that orders the CSS of the declaring module of an imported value before the CSS
/// of this module.
///
/// The use is a component that receives a `style()` result or a `style.forward()` value
/// in its props, or an imported value passed directly to `style()`. The host resolves
/// [`Self::source`] and [`Self::imported`] to the declaring module.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderingUse {
    /// Module specifier of the import declaration, as written.
    pub source: LeanStr,
    /// Exported name that the import reads: `default`, a named export, or the member
    /// name read from a namespace import.
    pub imported: LeanStr,
    /// The `style()` or `style.forward()` call that makes the use, in the original
    /// source.
    pub span: Span,
}

/// A `style.media()` call that lowering replaces with a media query list string.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCall {
    /// Whole call in the original source.
    pub span: Span,
    /// Static arguments in source order, each a variant list with an optional final
    /// `:`. The span covers the literal content without its delimiters. Unsupported
    /// arguments are absent because they match no environment.
    pub arguments: Vec<Spanned<LeanStr>>,
}

/// A `style.css()` call that lowering replaces with a stylesheet string.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssCall {
    /// Whole call in the original source.
    pub span: Span,
    /// Utility literals in source order. Unsupported inputs are absent because they
    /// contribute no rules.
    pub inputs: Vec<CssInput>,
    /// Name of the output template, without quotes. `None` selects `default`.
    pub template: Option<Spanned<LeanStr>>,
    /// Selector of the element that a utility styles when its own variants select no
    /// element. `None` omits such utilities.
    pub selector: Option<Spanned<LeanStr>>,
}

/// One utility literal of a `style.css()` call with its enclosing group keys.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssInput {
    /// Keys of the enclosing groups, outermost first. Each key is a variant prefix that
    /// ends in `:` as written; the keys `""` and `":"` add no variants and are absent.
    pub variants: Vec<Spanned<LeanStr>>,
    /// Whitespace-separated candidates. The span covers the literal content without
    /// its delimiters.
    pub utilities: Spanned<LeanStr>,
}

/// Binding-aware import information. All spans belong to the editable source.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleImports {
    /// Ranges in the original source where the frontend accepted nested macro uses.
    /// Import removal uses a range only if it is inside a transformed style call and
    /// the reference to the macro binding starts at the same position.
    #[serde(default)]
    pub consumed_uses: Vec<Span>,
    /// Includes nested bindings and unresolved reads to avoid capturing user code.
    pub reserved_names: Vec<LeanStr>,
    /// Macro imports that code generation may remove once the call is lowered.
    pub macros: Vec<MacroImport>,
    /// Source-literal spans of explicit module CSS value imports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub module_css: Vec<Span>,
}

/// One `import` statement that mentions the framework macro.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroImport {
    /// Adapter selected by the imported macro entry point.
    pub adapter: AdapterKind,
    /// Span of the module specifier literal, including its quotes.
    pub source_span: Span,
    /// Span of the whole import statement in the editable source.
    pub span: Span,
    /// In source order, including specifiers that are not the macro itself.
    pub specifiers: Vec<MacroImportSpecifier>,
}

/// One binding of a macro import statement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroImportSpecifier {
    /// Span of the binding as written, local name included.
    pub span: Span,
    /// Name the module binds the import to.
    pub local: LeanStr,
    /// Only value imports of the framework macro may be removed after lowering.
    pub is_macro: bool,
    /// Incomplete reference correspondence forbids deleting this binding.
    pub has_unmapped_references: bool,
    /// Every read of `local`, in source order, excluding the import itself.
    pub references: Vec<Span>,
}
