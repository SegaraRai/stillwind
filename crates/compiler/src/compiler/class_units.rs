//! Class identity allocation and base-before-conditional CSS publication.
//!
//! [`ClassUnits`] owns the call's base unit and deferred leaf rules.
//! [`ClassUnits::leaf`] allocates identities in traversal order; [`ClassUnits::finish`]
//! publishes the base before those leaves. JavaScript tree traversal and rendering do
//! not choose stylesheet order.

use std::collections::BTreeSet;

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::CompileErrorKind;
use stillwind_source::{SourceFile, Span, normalized_module_path};

use super::fragment::Emitted;
use super::literal::LiteralContribution;
use super::props::{GeneratedClass, GuardStep};
use super::symbols::generated_class_symbol;
use super::utilities::emission::emit_utility_rules;
use super::utilities::resolve::CompiledUtility;
use crate::identifier::{ClassContext, IdentifierContext};
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
    /// Name allocation shared with the rest of this compilation.
    identifiers: &'a IdentifierContext,
    /// Registry used to emit resolved utilities.
    theme: &'a ThemeIndex,
    /// Original call span used in class identities and guard metadata.
    call_span: Span,
    /// Published module CSS; the base is appended before deferred leaves.
    utility_css: &'a mut Vec<CssContribution>,
    /// Symbol records shared across calls in this module.
    symbols: &'a mut Vec<GeneratedSymbol>,
    /// Previously emitted (class name, canonical input) pairs, so identical units are
    /// written once per module.
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
    #[allow(clippy::too_many_arguments)] // One borrow per compilation-wide destination.
    pub(super) fn new(
        source: &'a SourceFile,
        options: &'a CompilerOptions,
        identifiers: &'a IdentifierContext,
        theme: &'a ThemeIndex,
        call_span: Span,
        utility_css: &'a mut Vec<CssContribution>,
        symbols: &'a mut Vec<GeneratedSymbol>,
        emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    ) -> Self {
        Self {
            source,
            options,
            identifiers,
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
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for the utility class family.
    pub(super) fn leaf(
        &mut self,
        contribution: LiteralContribution,
        guard: &[GuardStep],
    ) -> Result<Emitted, CompileErrorKind> {
        let mut names = Vec::new();
        if !contribution.utilities.is_empty() {
            self.class_index += 1;
            let branch = format!("{}:{}", self.class_index, contribution.canonical);
            let canonical =
                self.options
                    .style_class_input(self.source.path(), self.call_span.start.0, &branch);
            let (name, rules) = self.name_unit(
                self.call_span.start.0,
                &branch,
                &canonical,
                &contribution.utilities,
            )?;
            names.push(name.clone());
            self.classes.push(GeneratedClass {
                class_name: name,
                call_span: self.call_span,
                guard_path: guard.to_vec(),
            });
            self.publish_unit(rules);
        }
        names.extend(contribution.markers);
        names.extend(contribution.external);
        names.extend(contribution.plain);
        Ok(Emitted::Leaf { names })
    }

    /// Publishes base CSS before leaf CSS and returns unconditional names and class
    /// metadata. The generated base name precedes retained marker/external names, and
    /// its metadata precedes leaf metadata. Leaf metadata retains allocation order.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for the base unit's utility class.
    pub(super) fn finish(mut self) -> Result<(Vec<String>, Vec<GeneratedClass>), CompileErrorKind> {
        let mut names = Vec::new();
        let mut deferred = Vec::new();
        if !self.base.utilities.is_empty() {
            let branch = format!("0:{}", self.base.canonical);
            let canonical =
                self.options
                    .style_class_input(self.source.path(), self.call_span.start.0, &branch);
            let definitions = std::mem::take(&mut self.base.utilities);
            let (name, rules) =
                self.name_unit(self.call_span.start.0, &branch, &canonical, &definitions)?;
            // The base unit must precede every conditional unit in the stylesheet.
            deferred.extend(rules);
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
        deferred.append(&mut self.runtime_css);
        self.utility_css.append(&mut deferred);
        Ok((names, self.classes))
    }

    /// Generates one class unit and returns its rules without publishing them.
    ///
    /// `canonical` is the full identity input the name is generated from. Only the first
    /// caller for a name records its symbol, so an identical call that reuses a name
    /// from an earlier call carries no rules of its own.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for the utility class family.
    fn name_unit(
        &mut self,
        call: u32,
        branch: &str,
        identity: &str,
        definitions: &[CompiledUtility],
    ) -> Result<(String, Vec<CssContribution>), CompileErrorKind> {
        let context = ClassContext {
            module: normalized_module_path(self.source.path()),
            namespace: self.options.class_namespace.clone(),
            call,
            branch: branch.to_owned(),
        };
        let name = self.identifiers.class_name(&context)?;
        let mut contribution = Vec::new();
        if self
            .emitted_utility_classes
            .insert((name.clone(), identity.to_owned()))
        {
            emit_utility_rules(&mut contribution, &name, definitions, self.theme);
            self.symbols.push(generated_class_symbol(
                self.source,
                &name,
                identity,
                self.call_span,
            ));
        }
        Ok((name, contribution))
    }

    /// Publishes one conditional unit's rules, in traversal order.
    fn publish_unit(&mut self, contribution: Vec<CssContribution>) {
        self.runtime_css.extend(contribution);
    }
}
