//! Recognition of imported values that order CSS between modules.
//!
//! A module that passes a `style()` result or a `style.forward()` value to a component
//! needs the CSS of the module that declares the component first, so the caller's
//! classes win. A module that passes an imported value directly to `style()` needs the
//! CSS of the declaring module first for the same reason. This module records such
//! uses as [`OrderingUse`] records. The host resolves each import to its declaring
//! module after following re-exports.
//!
//! A component reference is resolved through constant aliases, because converted
//! Vue and Svelte components reach the component through generated constants such as
//! `const __VLS_0 = Button` and `const $$_Button = ensureComponent(Button)`.

use std::collections::BTreeSet;

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{
    Argument, BindingPattern, Expression, IdentifierReference, JSXElementName,
    JSXMemberExpressionObject, VariableDeclarator,
};
use oxc::semantic::SymbolId;

use stillwind_ir::OrderingUse;
use stillwind_semantic::ImportBinding;
use stillwind_source::Span;

use crate::state::MacroExtractor;

/// Alias chains longer than this are not followed. Generated component code uses at
/// most two levels.
const MAX_ALIAS_DEPTH: usize = 4;

impl<'a> MacroExtractor<'a> {
    /// Records the symbols that a constant binding forwards to: the initializer itself
    /// when it is an identifier, and the callee and identifier arguments when it is a
    /// call or `new` expression.
    pub(crate) fn record_ordering_alias(&mut self, declarator: &VariableDeclarator<'a>) {
        let (BindingPattern::BindingIdentifier(identifier), Some(init)) =
            (&declarator.id, &declarator.init)
        else {
            return;
        };
        let Some(binding) = identifier.symbol_id.get() else {
            return;
        };
        let mut targets = Vec::new();
        match init.get_inner_expression() {
            Expression::Identifier(reference) => targets.extend(self.reference_symbol(reference)),
            Expression::CallExpression(call) => {
                targets.extend(self.root_symbol(&call.callee).map(|(symbol, _)| symbol));
                targets.extend(self.argument_symbols(&call.arguments));
            }
            Expression::NewExpression(call) => {
                targets.extend(self.root_symbol(&call.callee).map(|(symbol, _)| symbol));
                targets.extend(self.argument_symbols(&call.arguments));
            }
            _ => {}
        }
        if !targets.is_empty() {
            self.ordering_aliases.insert(binding, targets);
        }
    }

    /// Records the component of a JSX element whose attributes contain the `style()`
    /// call at index `first_style` in [`MacroExtractor::style_calls`] or a later one.
    pub(crate) fn record_jsx_ordering(&mut self, name: &JSXElementName<'a>, first_style: usize) {
        let Some(trigger) = self.style_calls.get(first_style).map(|call| call.span) else {
            return;
        };
        let root = match name {
            JSXElementName::IdentifierReference(reference) => self
                .reference_symbol(reference)
                .map(|symbol| (symbol, None)),
            JSXElementName::MemberExpression(member) => match &member.object {
                JSXMemberExpressionObject::IdentifierReference(reference) => self
                    .reference_symbol(reference)
                    .map(|symbol| (symbol, Some(member.property.name.to_lean_str()))),
                _ => None,
            },
            _ => None,
        };
        if let Some((symbol, member)) = root {
            self.ordering_roots.push((symbol, member, trigger));
        }
    }

    /// Records the callee of a call or `new` expression whose arguments contain the
    /// `style()` call at index `first_style` or a later one, or a `style.forward()`
    /// value whose callee has the span `forward`.
    pub(crate) fn record_call_ordering(
        &mut self,
        callee: &Expression<'a>,
        first_style: usize,
        forward: Option<Span>,
    ) {
        let Some(trigger) = self
            .style_calls
            .get(first_style)
            .map(|call| call.span)
            .or(forward)
        else {
            return;
        };
        if let Some((symbol, member)) = self.root_symbol(callee) {
            self.ordering_roots.push((symbol, member, trigger));
        }
    }

    /// Records imported values passed directly to the `style()` call at `trigger`: an
    /// identifier or member argument, or the callee of a call argument.
    pub(crate) fn record_style_argument_ordering(
        &mut self,
        arguments: &[Argument<'a>],
        trigger: Span,
    ) {
        for argument in arguments {
            let Some(expression) = argument.as_expression() else {
                continue;
            };
            let root = match expression.get_inner_expression() {
                Expression::CallExpression(call) => self.root_symbol(&call.callee),
                other => self.root_symbol(other),
            };
            if let Some((symbol, member)) = root {
                self.ordering_roots.push((symbol, member, trigger));
            }
        }
    }

    /// Resolves the recorded roots to imports, in recording order.
    ///
    /// Each root follows constant aliases up to [`MAX_ALIAS_DEPTH`] levels. Roots that
    /// reach no import binding, or reach only macro imports, produce no use. A namespace
    /// import with a member read uses the member name as the imported name.
    pub(crate) fn ordering_uses(&self) -> Vec<OrderingUse> {
        let mut uses = Vec::new();
        let mut seen = BTreeSet::new();
        for (symbol, member, span) in &self.ordering_roots {
            let mut pending = vec![(*symbol, 0)];
            let mut visited = BTreeSet::new();
            while let Some((symbol, depth)) = pending.pop() {
                if !visited.insert(symbol) {
                    continue;
                }
                if self.scoping.symbol_flags(symbol).is_import() {
                    let local = self.scoping.symbol_name(symbol);
                    let Some(binding) = self.imports.iter().find(|import| import.local == local)
                    else {
                        continue;
                    };
                    let imported = match (binding.imported.as_str(), member) {
                        ("*", Some(member)) if depth == 0 => member.clone(),
                        _ => binding.imported.clone(),
                    };
                    if seen.insert((
                        binding.source.clone(),
                        imported.clone(),
                        span.start,
                        span.end,
                    )) {
                        uses.push(OrderingUse {
                            source: binding.source.clone(),
                            imported,
                            span: *span,
                        });
                    }
                } else if depth < MAX_ALIAS_DEPTH
                    && let Some(targets) = self.ordering_aliases.get(&symbol)
                {
                    pending.extend(targets.iter().map(|target| (*target, depth + 1)));
                }
            }
        }
        uses
    }

    /// Returns the root symbol of an identifier or a static member chain, with the
    /// member name when the root is read through one member.
    fn root_symbol(&self, expression: &Expression<'a>) -> Option<(SymbolId, Option<LeanStr>)> {
        match expression.get_inner_expression() {
            Expression::Identifier(reference) => self
                .reference_symbol(reference)
                .map(|symbol| (symbol, None)),
            Expression::StaticMemberExpression(member) => {
                match member.object.get_inner_expression() {
                    Expression::Identifier(reference) => self
                        .reference_symbol(reference)
                        .map(|symbol| (symbol, Some(member.property.name.to_lean_str()))),
                    other => self.root_symbol(other).map(|(symbol, _)| (symbol, None)),
                }
            }
            _ => None,
        }
    }

    /// Returns the symbols of identifier arguments.
    fn argument_symbols(&self, arguments: &[Argument<'a>]) -> Vec<SymbolId> {
        arguments
            .iter()
            .filter_map(Argument::as_expression)
            .filter_map(|argument| match argument.get_inner_expression() {
                Expression::Identifier(reference) => self.reference_symbol(reference),
                _ => None,
            })
            .collect()
    }

    /// Returns the symbol that an identifier reference resolves to.
    fn reference_symbol(&self, reference: &IdentifierReference<'a>) -> Option<SymbolId> {
        self.scoping
            .get_reference(reference.reference_id.get()?)
            .symbol_id()
    }
}

/// Returns the imports that `uses` read, deduplicated and sorted by source and imported
/// name.
///
/// A use whose name a namespace import reads takes the local name of that namespace
/// import. The local name is empty when no import declaration in `imports` matches.
pub(crate) fn ordering_imports(
    uses: &[OrderingUse],
    imports: &[ImportBinding],
) -> Vec<ImportBinding> {
    uses.iter()
        .map(|use_| (use_.source.clone(), use_.imported.clone()))
        .collect::<BTreeSet<(LeanStr, LeanStr)>>()
        .into_iter()
        .map(|(source, imported)| {
            let local = imports
                .iter()
                .find(|import| import.source == source && import.imported == imported)
                .or_else(|| {
                    imports
                        .iter()
                        .find(|import| import.source == source && import.imported == "*")
                })
                .map_or_default(|import| import.local.clone());
            ImportBinding {
                local,
                source,
                imported,
            }
        })
        .collect()
}
