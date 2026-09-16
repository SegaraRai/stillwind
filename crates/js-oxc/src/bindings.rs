//! Symbol-based recognition of the `style` macro and its members.
//!
//! Recognition is by resolved symbol, never by spelling: `style.ref()` counts only when
//! `style` refers to the imported macro binding, so an unrelated local function of the
//! same name is left alone. [`MacroExtractor::is_style_import`] is the exception,
//! because it classifies an import declaration before any symbol exists.

use oxc::ast::ast::{CallExpression, Expression, ImportDeclaration};

use stillwind_ir::from_macro_import;

use super::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// True when the import declaration's source is a Stillwind macro entry.
    ///
    /// Type-only imports still answer `true` here; callers must check
    /// [`ImportDeclaration::import_kind`] themselves if that matters.
    pub(crate) fn is_style_import(declaration: &ImportDeclaration<'a>) -> bool {
        from_macro_import(declaration.source.value.as_str()).is_some()
    }

    /// True when `expression` is `style.<property>` on the imported macro.
    ///
    /// The receiver must resolve to one of the module's style symbols, so an aliased
    /// import (`import { style as s }`) works while a shadowing local binding does not.
    pub(crate) fn is_style_member(&self, expression: &Expression<'a>, property: &str) -> bool {
        let Expression::StaticMemberExpression(member) = expression else {
            return false;
        };
        let Expression::Identifier(identifier) = &member.object else {
            return false;
        };
        let Some(reference_id) = identifier.reference_id.get() else {
            return false;
        };
        self.scoping
            .get_reference(reference_id)
            .symbol_id()
            .is_some_and(|symbol| self.style_symbols.contains(&symbol))
            && member.property.name.as_str() == property
    }

    /// True for a `style.ref(...)` call.
    pub(crate) fn is_style_ref_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "ref")
    }

    /// True for a `style.subtree(...)` call.
    pub(crate) fn is_style_subtree_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "subtree")
    }

    /// True for a `style.animation(...)` call.
    pub(crate) fn is_style_animation_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "animation")
    }

    /// True for a `style.viewTransition(...)` call.
    pub(crate) fn is_style_view_transition_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "viewTransition")
    }
}
