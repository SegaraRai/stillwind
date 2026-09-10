use oxc::ast::ast::{CallExpression, Expression, ImportDeclaration};

use stillwind_ir::framework::from_macro_import;

use super::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(crate) fn is_style_import(declaration: &ImportDeclaration<'a>) -> bool {
        from_macro_import(declaration.source.value.as_str()).is_some()
    }

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

    pub(crate) fn is_style_ref_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "ref")
    }

    pub(crate) fn is_style_scope_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "scope")
    }

    pub(crate) fn is_style_animation_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "animation")
    }

    pub(crate) fn is_style_view_transition_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "viewTransition")
    }
}
