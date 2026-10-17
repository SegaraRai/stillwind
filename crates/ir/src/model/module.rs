//! Declarations, imports, and style calls extracted from one module.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_source::{Span, Spanned};

use super::{KeyframesDefinition, RefDefinition, StyleCall, ViewTransitionDefinition};
use crate::AdapterKind;

/// Everything a frontend extracted from one module.
///
/// A frontend builds this in one pass. Later phases may reorder [`Self::style_calls`]
/// for emission, but they must not drop entries, duplicate them, or change the identity
/// of a declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleIr {
    /// Framework the module belongs to. It selects the class property, the runtime
    /// import, and the materialized style format.
    pub adapter: AdapterKind,
    /// Bindings and imports needed by code generation.
    #[serde(default)]
    pub imports: ModuleImports,
    /// In source order. A call can emit a base unit plus conditional units, or no CSS.
    pub style_calls: Vec<StyleCall>,
    /// CVA factories and their independently compiled style values, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cva_calls: Vec<CvaCall>,
    /// `style.media()` calls in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media_calls: Vec<MediaCall>,
    /// In source order. Each entry is a `style.ref()` declaration.
    pub ref_definitions: Vec<RefDefinition>,
    /// In source order. Each entry is a `style.keyframes()` declaration.
    pub keyframes_declarations: Vec<KeyframesDefinition>,
    /// In source order. Each entry is a `style.viewTransition()` declaration.
    pub view_transition_declarations: Vec<ViewTransitionDefinition>,
}

impl ModuleIr {
    /// Iterates ordinary style calls and CVA value expressions with their original spans.
    pub fn style_inputs(&self) -> impl Iterator<Item = &StyleCall> {
        self.style_calls
            .iter()
            .chain(self.cva_calls.iter().flat_map(|call| &call.values))
    }

    /// Empty React IR. Frameworks without an explicit adapter start here and
    /// overwrite `adapter` through their own conversion.
    pub const fn react() -> Self {
        Self {
            adapter: AdapterKind::React,
            imports: ModuleImports {
                consumed_uses: Vec::new(),
                reserved_names: Vec::new(),
                macros: Vec::new(),
                module_css: Vec::new(),
            },
            style_calls: Vec::new(),
            cva_calls: Vec::new(),
            media_calls: Vec::new(),
            ref_definitions: Vec::new(),
            keyframes_declarations: Vec::new(),
            view_transition_declarations: Vec::new(),
        }
    }
}

/// A CVA call whose configuration remains ordinary JavaScript after lowering.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CvaCall {
    /// Whole factory call in the original source, used to validate nested macro uses.
    pub span: Span,
    /// Callee expression replaced by the framework's CVA runtime helper.
    pub callee_span: Span,
    /// Base and option expressions compiled independently in source order.
    pub values: Vec<StyleCall>,
    /// Replaces invalid whole calls with an empty factory.
    #[serde(default)]
    pub empty: bool,
    /// Replacements for unsupported configuration entries in original source coordinates.
    #[serde(default)]
    pub replacements: Vec<CvaReplacement>,
}

/// One configuration expression or entry replaced during CVA validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CvaReplacement {
    /// Original expression or entry without its surrounding separator.
    pub span: Span,
    /// Static JavaScript that preserves the enclosing object or array syntax.
    pub replacement: LeanStr,
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
