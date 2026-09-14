//! Class identity allocation and base-before-conditional CSS publication.
//!
//! [`ClassUnits`] owns the call's base unit and deferred leaf rules.
//! [`ClassUnits::leaf`] allocates identities in traversal order; [`ClassUnits::finish`]
//! publishes the base before those leaves. JavaScript tree traversal and rendering do
//! not choose stylesheet order.

use std::collections::BTreeSet;

use stillwind_design_system::ThemeIndex;
use stillwind_source::{SourceFile, Span};

use super::fragment::Emitted;
use super::literal::LiteralContribution;
use super::props::{GeneratedClass, GuardStep};
use super::symbols::{class_name, generated_class_symbol};
use super::utilities::emission::emit_utility_rules;
use super::utilities::resolve::CompiledUtility;
use crate::{CompilerOptions, CssContribution, GeneratedSymbol};

/// Unconditional class contributions, accumulated in source order before publication.
#[derive(Default)]
pub(super) struct BaseUnit {
    /// Exact canonical spelling used as input to the base class hash.
    pub(super) canonical: String,
    /// Resolved utilities, in their encountered order.
    pub(super) utilities: Vec<CompiledUtility>,
    /// Marker and external names retained alongside the generated base class.
    pub(super) names: Vec<String>,
}

/// Per-call class allocation and deferred stylesheet output.
pub(super) struct ClassUnits<'a> {
    /// Source identity included in class hashes and symbol provenance.
    source: &'a SourceFile,
    /// Generation settings for canonical class inputs.
    options: &'a CompilerOptions,
    /// Registry used to emit resolved utilities.
    theme: &'a ThemeIndex,
    /// Original call span used in class identities and guard metadata.
    call_span: Span,
    /// Published module CSS; the base is appended before deferred leaves.
    utility_css: &'a mut Vec<CssContribution>,
    /// Symbol records shared across calls in this module.
    symbols: &'a mut Vec<GeneratedSymbol>,
    /// Previously emitted (class name, canonical input) pairs.
    emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    /// Unconditional contributions accumulated across all argument positions.
    pub(super) base: BaseUnit,
    /// Conditional rules retained until the base has been emitted.
    runtime_css: Vec<CssContribution>,
    /// Class metadata, with the base inserted before conditional classes at finish.
    classes: Vec<GeneratedClass>,
    /// Next leaf identity; zero is reserved for the base.
    class_index: u32,
}

impl<'a> ClassUnits<'a> {
    /// Starts one call without publishing any CSS or allocating a class identity.
    pub(super) fn new(
        source: &'a SourceFile,
        options: &'a CompilerOptions,
        theme: &'a ThemeIndex,
        call_span: Span,
        utility_css: &'a mut Vec<CssContribution>,
        symbols: &'a mut Vec<GeneratedSymbol>,
        emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    ) -> Self {
        Self {
            source,
            options,
            theme,
            call_span,
            utility_css,
            symbols,
            emitted_utility_classes,
            base: BaseUnit::default(),
            runtime_css: Vec::new(),
            classes: Vec::new(),
            class_index: 0,
        }
    }

    /// Merges a contribution into the unconditional base unit.
    pub(super) fn fold(&mut self, contribution: LiteralContribution) {
        self.base.canonical.push_str(&contribution.canonical);
        self.base.utilities.extend(contribution.utilities);
        self.base.names.extend(contribution.markers);
        self.base.names.extend(contribution.external);
        self.base.names.extend(contribution.plain);
    }

    /// Turns one contribution into a leaf expression, generating its class unit.
    ///
    /// The branch index is part of the class identity, so identical utilities in two
    /// branches still produce distinct units and the later unit can override the
    /// earlier one.
    pub(super) fn leaf(
        &mut self,
        contribution: LiteralContribution,
        guard: &[GuardStep],
    ) -> Emitted {
        let mut names = Vec::new();
        if !contribution.utilities.is_empty() {
            self.class_index += 1;
            let canonical = format!("{}:{}", self.class_index, contribution.canonical);
            let canonical = self.options.style_class_input(
                self.source.path(),
                self.call_span.start.0,
                &canonical,
            );
            let name = class_name(&canonical);
            if self
                .emitted_utility_classes
                .insert((name.clone(), canonical.clone()))
            {
                emit_utility_rules(
                    &mut self.runtime_css,
                    &name,
                    &contribution.utilities,
                    self.theme,
                );
                self.symbols.push(generated_class_symbol(
                    self.source,
                    &name,
                    &canonical,
                    self.call_span,
                ));
            }
            names.push(name.clone());
            self.classes.push(GeneratedClass {
                class_name: name,
                call_span: self.call_span,
                guard_path: guard.to_vec(),
            });
        }
        names.extend(contribution.markers);
        names.extend(contribution.external);
        names.extend(contribution.plain);
        Emitted::Leaf { names }
    }

    /// Publishes base CSS before leaf CSS and returns unconditional names and class
    /// metadata. The generated base name precedes retained marker/external names, and
    /// its metadata precedes leaf metadata. Leaf metadata retains allocation order.
    pub(super) fn finish(mut self) -> (Vec<String>, Vec<GeneratedClass>) {
        let mut names = Vec::new();
        if !self.base.utilities.is_empty() {
            let canonical = format!("0:{}", self.base.canonical);
            let canonical = self.options.style_class_input(
                self.source.path(),
                self.call_span.start.0,
                &canonical,
            );
            let name = class_name(&canonical);
            if self
                .emitted_utility_classes
                .insert((name.clone(), canonical.clone()))
            {
                emit_utility_rules(self.utility_css, &name, &self.base.utilities, self.theme);
                self.symbols.push(generated_class_symbol(
                    self.source,
                    &name,
                    &canonical,
                    self.call_span,
                ));
            }
            self.classes.insert(
                0,
                GeneratedClass {
                    class_name: name.clone(),
                    call_span: self.call_span,
                    guard_path: Vec::new(),
                },
            );
            names.push(name);
        }
        names.extend(std::mem::take(&mut self.base.names));
        // The base unit must precede every conditional unit in the stylesheet.
        self.utility_css.append(&mut self.runtime_css);
        (names, self.classes)
    }
}
