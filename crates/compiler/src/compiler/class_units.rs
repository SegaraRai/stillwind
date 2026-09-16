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
use crate::identifiers::{ClassContext, IdentifierContext, escaped_identifier};
use crate::{CompilerOptions, CssContribution, GeneratedSymbol};

/// View-transition data one class unit declares.
///
/// The class property is one declaration, so the unit that owns the element class list
/// references a slot per guarded use and each guarded unit fills its own slot. The resolved
/// list is whatever slots the element actually declares, so two conditions that hold
/// together contribute both tokens instead of replacing one another.
#[derive(Clone, Default)]
pub(super) struct ViewTransitionClassUnit {
    /// Escaped class tokens the unit declares literally, in first-seen order.
    classes: Vec<String>,
    /// The slot custom property this unit fills, and the token it writes into it.
    slot: Option<(String, String)>,
    /// Escaped names the unit declares statically, in order.
    names: Vec<String>,
}

impl ViewTransitionClassUnit {
    /// Whether the unit declares no view-transition data.
    pub(super) fn is_empty(&self) -> bool {
        self.classes.is_empty() && self.slot.is_none() && self.names.is_empty()
    }

    /// Records one unconditional use's class token and static name.
    fn attach(&mut self, token: &str, name: Option<&str>) {
        let token = escaped_identifier(token);
        if !self.classes.contains(&token) {
            self.classes.push(token);
        }
        if let Some(name) = name {
            self.names.push(name.to_owned());
        }
    }

    /// Records the slot one guarded use fills, and its static name.
    fn attach_slot(&mut self, property: String, token: &str, name: Option<&str>) {
        self.slot = Some((property, escaped_identifier(token)));
        if let Some(name) = name {
            self.names.push(name.to_owned());
        }
    }

    /// The rules the unit owning the element class list writes.
    ///
    /// Its own classes come first and each slot follows as a `var()` reference with an empty
    /// fallback, so a slot no unit filled contributes nothing to the resolved list.
    fn class_rules(&self, selector: &str, slots: &[String]) -> String {
        let mut list = self.classes.join(" ");
        for slot in slots {
            if !list.is_empty() {
                list.push(' ');
            }
            list.push_str("var(");
            list.push_str(slot);
            list.push_str(",)");
        }
        let mut css = String::new();
        if !list.is_empty() {
            css.push_str(selector);
            css.push_str("{view-transition-class:");
            css.push_str(&list);
            css.push_str(";}");
        }
        css.push_str(&self.name_rules(selector));
        css
    }

    /// The rules a guarded unit writes: the slot it fills and its static names.
    fn guarded_rules(&self, selector: &str) -> String {
        let mut css = String::new();
        if let Some((property, token)) = &self.slot {
            css.push_str(selector);
            css.push('{');
            css.push_str(property);
            css.push(':');
            css.push_str(token);
            css.push_str(";}");
        }
        css.push_str(&self.name_rules(selector));
        css
    }

    /// The static view-transition-name declarations this unit writes.
    fn name_rules(&self, selector: &str) -> String {
        let mut css = String::new();
        for name in &self.names {
            css.push_str(selector);
            css.push_str("{view-transition-name:");
            css.push_str(name);
            css.push_str(";}");
        }
        css
    }
}

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
    /// View-transition declarations per class unit; slot zero is the base unit.
    view_transition_classes: Vec<ViewTransitionClassUnit>,
    /// Class selector of each view-transition slot, index-aligned with `view_transitions`.
    class_selectors: Vec<String>,
    /// Conditional rules retained until the base has been emitted, each tagged with the
    /// view-transition slot it declares.
    runtime_css: Vec<(usize, CssContribution)>,
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
            view_transition_classes: vec![ViewTransitionClassUnit::default()],
            class_selectors: vec![String::new()],
            runtime_css: Vec::new(),
            classes: Vec::new(),
            class_index: 0,
        }
    }

    /// Merges a contribution into the unconditional base unit.
    pub(super) fn fold(&mut self, contribution: LiteralContribution) {
        self.base.canonical.push_str(&contribution.canonical);
        self.base.utilities.extend(contribution.utilities);
        if let Some(token) = &contribution.view_transition_token {
            self.view_transition_classes[0]
                .attach(token, contribution.view_transition_name.as_deref());
        }
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
        if !contribution.utilities.is_empty() || contribution.view_transition_token.is_some() {
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
            self.class_selectors.push(format!(".{name}"));
            let mut unit = ViewTransitionClassUnit::default();
            if guard.is_empty() {
                if let Some(token) = &contribution.view_transition_token {
                    unit.attach(token, contribution.view_transition_name.as_deref());
                }
            } else if let Some(token) = &contribution.view_transition_token {
                // A guarded use contributes its token through the slot its own class fills,
                // so a sibling condition that also holds contributes its token as well.
                let property = self.view_transition_slot(&branch)?;
                unit.attach_slot(
                    property,
                    token,
                    contribution.view_transition_name.as_deref(),
                );
            }
            self.view_transition_classes.push(unit);
            let slot = self.view_transition_classes.len() - 1;
            self.classes.push(GeneratedClass {
                class_name: name,
                call_span: self.call_span,
                guard_path: guard.to_vec(),
            });
            self.publish_unit(slot, rules);
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
        // A call whose transitions are all guarded still needs the class that references
        // their slots, so any unit beyond the base keeps it.
        let base_declares = !self.view_transition_classes[0].is_empty()
            || self
                .view_transition_classes
                .iter()
                .skip(1)
                .any(|unit| unit.slot.is_some());
        if !self.base.utilities.is_empty() || base_declares {
            let branch = format!("0:{}", self.base.canonical);
            let canonical =
                self.options
                    .style_class_input(self.source.path(), self.call_span.start.0, &branch);
            let definitions = std::mem::take(&mut self.base.utilities);
            let (name, rules) =
                self.name_unit(self.call_span.start.0, &branch, &canonical, &definitions)?;
            // The base unit must precede every conditional unit in the stylesheet.
            deferred.extend(rules);
            self.class_selectors[0] = format!(".{name}");
            let slots = self.view_transition_slots();
            let declarations =
                self.view_transition_classes[0].class_rules(&format!(".{name}"), &slots);
            if !declarations.is_empty() {
                deferred.push(CssContribution {
                    css: declarations,
                    required_properties: Vec::new(),
                    keyframes: false,
                });
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
        for (slot, contribution) in std::mem::take(&mut self.runtime_css) {
            deferred.push(contribution);
            if let Some(unit) = self.view_transition_classes.get(slot) {
                let declarations = unit.guarded_rules(&self.class_selectors[slot]);
                if !declarations.is_empty() {
                    deferred.push(CssContribution {
                        css: declarations,
                        required_properties: Vec::new(),
                        keyframes: false,
                    });
                }
            }
        }
        self.utility_css.append(&mut deferred);
        Ok((names, self.classes))
    }

    /// Generates one class unit and returns its rules without publishing them.
    ///
    /// `canonical` is the full identity input the name is generated from. Only the
    /// first caller for a name records its symbol, so an identical call that reuses a
    /// name from an earlier call carries no rules of its own.
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
        let mut contributions = Vec::new();
        if self
            .emitted_utility_classes
            .insert((name.as_str().to_owned(), identity.to_owned()))
        {
            emit_utility_rules(
                &mut contributions,
                &name.selector(),
                definitions,
                self.theme,
                self.identifiers,
            )?;
            self.symbols.push(generated_class_symbol(
                self.source,
                name.as_str(),
                identity,
                self.call_span,
            ));
        }
        Ok((name.as_str().to_owned(), contributions))
    }

    /// Publishes one conditional unit's rules, in traversal order.
    fn publish_unit(&mut self, slot: usize, contributions: Vec<CssContribution>) {
        self.runtime_css.extend(
            contributions
                .into_iter()
                .map(|contribution| (slot, contribution)),
        );
    }

    /// Allocates one class unit that declares only view-transition data.
    ///
    /// A transition the value selects at runtime carries one of several tokens, so each token
    /// needs a class of its own for the class attribute to choose between them.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for the utility class family.
    pub(super) fn view_transition_class_unit(
        &mut self,
        token: &str,
        name: Option<&str>,
        guard: &[GuardStep],
    ) -> Result<String, CompileErrorKind> {
        let contribution = LiteralContribution {
            canonical: format!("view-transition:{token}\u{0}"),
            utilities: Vec::new(),
            markers: Vec::new(),
            external: Vec::new(),
            plain: Vec::new(),
            view_transition_token: Some(token.to_owned()),
            view_transition_name: name.map(str::to_owned),
        };
        let Emitted::Leaf { names } = self.leaf(contribution, guard)? else {
            return Ok(String::new());
        };
        Ok(names.first().cloned().unwrap_or_default())
    }

    /// The slot properties the base unit references, in the order their units were allocated.
    fn view_transition_slots(&self) -> Vec<String> {
        self.view_transition_classes
            .iter()
            .skip(1)
            .filter_map(|unit| unit.slot.as_ref().map(|(property, _)| property.clone()))
            .collect()
    }

    /// The custom property name one guarded unit fills with its class token.
    ///
    /// # Errors
    ///
    /// Returns the host generator failure in release mode.
    fn view_transition_slot(&self, branch: &str) -> Result<String, CompileErrorKind> {
        let context = ClassContext {
            module: normalized_module_path(self.source.path()),
            namespace: self.options.class_namespace.clone(),
            call: self.call_span.start.0,
            branch: branch.to_owned(),
        };
        Ok(self.identifiers.view_transition_slot(&context)?.property())
    }
}
