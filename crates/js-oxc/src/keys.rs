//! Declaration keys that survive edits elsewhere in a module.
//!
//! A [`DeclarationKey`] places a declaration by the named declarations that enclose it
//! and by an ordinal among its siblings. [`DeclarationScopes`] records the named
//! declarations of one program with the range that each one encloses, and
//! [`MacroExtractor::declaration_key`] and [`MacroExtractor::style_key`] allocate
//! ordinals as declarations are collected.

use std::collections::HashMap;

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{
    BindingPattern, Class, Function, MethodDefinition, ObjectProperty, Program, PropertyDefinition,
    VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::ScopeFlags;
use oxc::span::{GetSpan, Span as OxcSpan};

use stillwind_semantic::{AssetKind, DeclarationKey};
use stillwind_source::{SourceFile, Span};

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;

/// Named declarations of one program and the analysis-buffer range that each encloses.
#[derive(Debug, Default)]
pub(crate) struct DeclarationScopes {
    /// Segments in pre-order, so an enclosing segment precedes the segments inside it.
    segments: Vec<Segment>,
}

/// One named declaration and the range whose declarations it encloses.
#[derive(Debug)]
struct Segment {
    /// Inclusive UTF-8 offset in the analysis buffer.
    start: u32,
    /// Exclusive UTF-8 offset in the analysis buffer.
    end: u32,
    /// Declared name.
    name: LeanStr,
}

impl DeclarationScopes {
    /// Records the named declarations of `program`, which Oxc parsed from `source`.
    ///
    /// A variable binding encloses its initializer only, so a declaration assigned to a
    /// binding is not inside its own segment. Named functions, classes, methods, class
    /// fields, and object properties with a static key enclose their bodies or values.
    ///
    /// `coordinates` maps a span of `source` to `original`. A declaration counts only
    /// when the source text of its name maps to the same text in `original`. A framework projection may wrap
    /// authored code in generated declarations, such as the `$$render` function of
    /// svelte2tsx, and those names must not enter the keys of authored declarations.
    pub(crate) fn collect(
        program: &Program<'_>,
        source: &SourceFile,
        original: &SourceFile,
        coordinates: &dyn Fn(Span) -> Option<Span>,
    ) -> Self {
        let mut collector = ScopeCollector {
            scopes: DeclarationScopes::default(),
            authored: &|name: OxcSpan| {
                let text = source.text().get(name.start as usize..name.end as usize);
                coordinates(Span::new(source.id(), name.start, name.end)).is_some_and(|mapped| {
                    original
                        .text()
                        .get(mapped.start.0 as usize..mapped.end.0 as usize)
                        == text
                })
            },
        };
        collector.visit_program(program);
        collector.scopes
    }

    /// Returns the names of the segments that contain `start..end`, from the outermost
    /// to the innermost.
    fn path(&self, start: u32, end: u32) -> Vec<LeanStr> {
        self.segments
            .iter()
            .filter(|segment| segment.start <= start && end <= segment.end)
            .map(|segment| segment.name.clone())
            .collect()
    }
}

/// Visitor that fills [`DeclarationScopes`].
struct ScopeCollector<'c> {
    /// Segments collected so far, in pre-order.
    scopes: DeclarationScopes,
    /// Reports whether the name at a span of the analysis buffer is authored text.
    authored: &'c dyn Fn(OxcSpan) -> bool,
}

impl ScopeCollector<'_> {
    /// Records a segment for `name` over `span` when the name at `name_span` is authored.
    fn push(&mut self, span: OxcSpan, name_span: OxcSpan, name: &str) {
        if !(self.authored)(name_span) {
            return;
        }
        self.scopes.segments.push(Segment {
            start: span.start,
            end: span.end,
            name: name.to_lean_str(),
        });
    }
}

impl<'a> Visit<'a> for ScopeCollector<'_> {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(identifier), Some(init)) =
            (&declarator.id, &declarator.init)
        {
            self.push(init.span(), identifier.span, &identifier.name);
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(identifier) = &function.id {
            self.push(function.span, identifier.span, &identifier.name);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        if let Some(identifier) = &class.id {
            self.push(class.body.span, identifier.span, &identifier.name);
        }
        walk::walk_class(self, class);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        if let Some(name) = method.key.static_name()
            && let Some(name) = name.as_str()
        {
            self.push(method.value.span, method.key.span(), name);
        }
        walk::walk_method_definition(self, method);
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'a>) {
        if let (Some(name), Some(value)) = (property.key.static_name(), &property.value)
            && let Some(name) = name.as_str()
        {
            self.push(value.span(), property.key.span(), name);
        }
        walk::walk_property_definition(self, property);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if !property.computed
            && let Some(name) = property.key.static_name()
            && let Some(name) = name.as_str()
        {
            self.push(property.value.span(), property.key.span(), name);
        }
        walk::walk_object_property(self, property);
    }
}

/// Next ordinal for each kind, enclosing path, and binding name.
pub(crate) type Ordinals = HashMap<(AssetKind, Vec<LeanStr>, LeanStr), u32>;

/// Allocated keys by kind and the place of the declaration.
pub(crate) type AllocatedKeys = HashMap<(AssetKind, KeySite), DeclarationKey>;

/// Place of a declaration that receives a key.
///
/// A framework projection may copy authored code, such as Vue component options in
/// older language-core versions. Each copy then has its own analysis span, but all
/// copies map to one original span and must receive one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum KeySite {
    /// UTF-8 span in the original source.
    Original(LocalSpan),
    /// UTF-8 span in the analysis buffer, for a declaration without an original span.
    Analysis(LocalSpan),
}

impl MacroExtractor<'_> {
    /// Returns the key of the declaration of `kind` at `span`, allocating it on the
    /// first request.
    ///
    /// `span` must be in the analysis buffer: the binding name for a named declaration,
    /// or the call for a `style()` call or CVA value. The ordinal counts the earlier
    /// declarations with the same kind, path, and binding name in the order of their
    /// first request. Callers request declarations of one kind in source order, so the
    /// order is deterministic. For style keys, seeding requests the calls assigned to a
    /// binding before the main visit requests the others.
    pub(crate) fn declaration_key(
        &self,
        kind: AssetKind,
        span: Span,
        binding: &str,
    ) -> DeclarationKey {
        let site = match (self.coordinates)(span) {
            Some(original) => KeySite::Original(LocalSpan::new(original.start.0, original.end.0)),
            None => KeySite::Analysis(LocalSpan::new(span.start.0, span.end.0)),
        };
        if let Some(key) = self.keys.borrow().get(&(kind, site)) {
            return key.clone();
        }
        let path = self.scopes.path(span.start.0, span.end.0);
        let mut ordinals = self.ordinals.borrow_mut();
        let next = ordinals
            .entry((kind, path.clone(), binding.to_lean_str()))
            .or_default();
        let key = DeclarationKey::new(path, *next);
        *next += 1;
        self.keys.borrow_mut().insert((kind, site), key.clone());
        key
    }

    /// Returns the key of the `style()` call or CVA value at `span`.
    ///
    /// `span` must be in the analysis buffer.
    pub(crate) fn style_key(&self, span: Span) -> DeclarationKey {
        self.declaration_key(AssetKind::Style, span, "")
    }
}
