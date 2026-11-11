//! Collection of ref and stored-variant declarations before the main visit.
//!
//! A `style()` call can use a ref or a stored variant whose declaration comes later in
//! the source, for example inside a component that is defined above a module-level
//! declaration. Expanding a stored variant needs its variant sequence, and checker
//! facts give only its identity. The extractor therefore collects these declarations
//! first, so every use in the main visit can read them.
//!
//! A declaration inside a `style()` or CVA input is skipped. The main visit omits such
//! an input as a nested macro call, and a declaration collected there would add a
//! replacement inside the replaced call.

use oxc::ast::ast::{BindingPattern, CallExpression, Expression, Program, VariableDeclarator};
use oxc::ast_visit::{Visit, walk};

use lean_string::ToLeanStr;
use stillwind_semantic::{AssetKind, ExportedValue, SymbolicValue};
use stillwind_source::Spanned;

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Collects every `style.ref()` and `style.when()` declaration outside macro inputs,
    /// in source order.
    ///
    /// Call this after the style bindings are seeded and before the main visit.
    pub(crate) fn seed_declarations(&mut self, program: &Program<'a>) {
        let mut seeder = DeclarationSeeder {
            extractor: self,
            depth: 0,
        };
        seeder.visit_program(program);
    }

    /// Records a `const name = style.ref()` or `const name = style.when()` declaration.
    /// Other declarators are left to the main visit.
    fn seed_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        let (BindingPattern::BindingIdentifier(identifier), Some(Expression::CallExpression(call))) =
            (&declarator.id, &declarator.init)
        else {
            return;
        };
        let Some(symbol_id) = identifier.symbol_id.get() else {
            return;
        };
        let name = Spanned::new(
            identifier.name.to_lean_str(),
            self.source_span(identifier.span),
        );
        if self.is_style_ref_call(call) {
            let definition = self.collect_ref_definition(name, call);
            self.ref_symbols.insert(symbol_id, definition.clone());
            self.local_values.insert(
                symbol_id,
                ExportedValue::Refs(SymbolicValue::exact(self.identity(
                    AssetKind::Ref,
                    &definition.binding,
                    &definition.key,
                ))),
            );
            self.ref_definitions.push(definition);
        } else if self.is_style_when_call(call) {
            let definition = self.collect_stored_variant(name, call);
            self.local_values.insert(
                symbol_id,
                ExportedValue::StoredVariants(SymbolicValue::exact(self.identity(
                    AssetKind::StoredVariant,
                    &definition.binding,
                    &definition.key,
                ))),
            );
            self.stored_variants.push(definition);
        }
    }
}

/// Walks a program and seeds declarations that are not inside a macro input.
struct DeclarationSeeder<'e, 'a> {
    extractor: &'e mut MacroExtractor<'a>,
    /// Number of enclosing `style()` and CVA calls.
    depth: usize,
}

impl<'a> Visit<'a> for DeclarationSeeder<'_, 'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let input = self.extractor.style_call_output(call).is_some()
            || self
                .extractor
                .is_style_member(call.callee.get_inner_expression(), "cva");
        self.depth += usize::from(input);
        walk::walk_call_expression(self, call);
        self.depth -= usize::from(input);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if self.depth == 0 {
            self.extractor.seed_declarator(declarator);
        }
        walk::walk_variable_declarator(self, declarator);
    }
}
