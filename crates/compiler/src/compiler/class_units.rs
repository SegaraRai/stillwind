//! Class identity allocation and base-before-conditional CSS publication.
//!
//! [`ClassUnits`] owns the call's base unit and deferred leaf rules. Regions reserve
//! their traversal order before their rules are collected. [`ClassUnits::finish`]
//! publishes the base before those regions. The emitter supplies resolved rules;
//! this module allocates identities and CSS output without choosing framework props.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use stillwind_design_system::{ThemeIndex, UtilityOrder};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::{ModuleId, Span};

use super::fragment::Emitted;
use super::literal::{LiteralContribution, TransitionContribution};
use super::props::{GeneratedClass, GuardStep};
use super::symbols::generated_class_symbol;
use super::utilities::emission::emit_utility_rules;
use super::utilities::resolve::CompiledUtility;
use crate::compiler::RuleBoundaries;
use crate::identifiers::{
    ClassContext, ClassName, IdentifierContext, NameRequest, escaped_identifier,
};
use crate::{CompilerOptions, CssContribution, GeneratedSymbol};

/// View-transition declarations owned by one class unit.
///
/// The base unit lists transition tokens through custom-property slots when a use
/// is conditional, is selected at runtime, or has CSS wrappers. The unit for that
/// use fills its slot under the same conditions as its rules. If two conditions
/// hold together, both tokens appear in the element's view-transition-class.
#[derive(Clone, Default)]
pub(super) struct ViewTransitionClassUnit {
    /// Escaped class tokens the unit declares literally, in first-seen order.
    classes: Vec<String>,
    /// Slot names filled by this unit, referenced by the base token list.
    slots: Vec<String>,
    /// Escaped names the unit declares statically, in order.
    names: Vec<String>,
}

impl ViewTransitionClassUnit {
    /// Whether the unit declares no view-transition data.
    pub(super) fn is_empty(&self) -> bool {
        self.classes.is_empty() && self.slots.is_empty() && self.names.is_empty()
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

    /// The rules the unit owning the element class list writes.
    ///
    /// Its own classes come first and each slot follows as a `var()` reference with an
    /// empty fallback, so a slot no unit filled contributes nothing to the resolved
    /// list.
    fn class_rules(&self, selector: &str, slots: &[String]) -> String {
        let list = self.class_value(slots);
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

    /// Returns this unit's tokens followed by references to `slots` in their given
    /// order. Slot names must be escaped CSS custom-property identifiers. Each
    /// reference uses an empty fallback so an unfilled slot contributes no token.
    /// Returns an empty string when the unit has no tokens and the caller provides no
    /// slots.
    fn class_value(&self, slots: &[String]) -> String {
        let mut list = self.classes.join(" ");
        for slot in slots {
            if !list.is_empty() {
                list.push(' ');
            }
            list.push_str("var(");
            list.push_str(slot);
            list.push_str(",)");
        }
        list
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
    /// Hash fragments for rule-bearing base contributions in source order.
    pub(super) canonical: String,
    /// Resolved utilities, in their encountered order.
    pub(super) utilities: Vec<CompiledUtility>,
    /// Transition uses kept until the base unit's CSS is generated.
    transitions: Vec<TransitionContribution>,
    /// Marker and external names retained alongside the generated base class.
    pub(super) names: Vec<String>,
}

/// Per-call class allocation and deferred stylesheet output.
pub(super) struct ClassUnits<'a> {
    /// Compiler identity included in class hashes and symbol provenance.
    module_id: &'a ModuleId,
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
    /// `(source traversal order, CSS contribution)`
    /// retained until the base has been emitted.
    runtime_css: Vec<(u32, CssContribution)>,
    /// `(source traversal order, class metadata)`; the base is inserted before
    /// conditional classes at finish.
    classes: Vec<(u32, GeneratedClass)>,
    /// Next leaf identity; zero is reserved for the base.
    class_index: u32,
}

impl<'a> ClassUnits<'a> {
    /// Starts one call without publishing any CSS or allocating a class identity.
    #[allow(clippy::too_many_arguments)] // One borrow per compilation-wide destination.
    pub(super) fn new(
        module_id: &'a ModuleId,
        options: &'a CompilerOptions,
        identifiers: &'a IdentifierContext,
        theme: &'a ThemeIndex,
        call_span: Span,
        utility_css: &'a mut Vec<CssContribution>,
        symbols: &'a mut Vec<GeneratedSymbol>,
        emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    ) -> Self {
        Self {
            module_id,
            options,
            identifiers,
            theme,
            call_span,
            utility_css,
            symbols,
            emitted_utility_classes,
            base: BaseUnit::default(),
            view_transition_classes: vec![ViewTransitionClassUnit::default()],
            runtime_css: Vec::new(),
            classes: Vec::new(),
            class_index: 0,
        }
    }

    /// Adds `contribution` to the unconditional base in encounter order.
    /// The caller must ensure that the contribution applies unconditionally.
    /// This keeps its canonical input, rules, transition uses, and class names without
    /// allocating a class or publishing CSS.
    pub(super) fn fold(&mut self, contribution: LiteralContribution) {
        self.base.canonical.push_str(&contribution.canonical);
        self.base.utilities.extend(contribution.utilities);
        self.base.transitions.extend(contribution.view_transitions);
        self.base.names.extend(contribution.markers);
        self.base.names.extend(contribution.external);
        self.base.names.extend(contribution.plain);
    }

    /// Allocates a class unit for `contribution` under the JavaScript conditions in
    /// `guard`. It reserves the next traversal order, so identical rules at different
    /// positions receive distinct unit identities.
    ///
    /// Returns a leaf containing generated and retained class names. Class metadata is
    /// recorded and CSS is queued for `finish` to publish.
    ///
    /// # Errors
    ///
    /// Returns an error if generating a class or slot name, or emitting utilities, fails.
    /// Earlier allocations and shared registry changes remain. The caller must discard
    /// the failed compilation.
    pub(super) fn leaf(
        &mut self,
        contribution: LiteralContribution,
        guard: &[GuardStep],
    ) -> Result<Emitted, CompileDiagnostic> {
        let order = self.reserve_branch();
        self.leaf_at(contribution, guard, order)
    }

    /// Reserves and returns the next source traversal order for a conditional region
    /// before visiting its children. Zero remains reserved for the base.
    pub(super) fn reserve_branch(&mut self) -> u32 {
        self.class_index += 1;
        self.class_index
    }

    /// Allocates a class unit for `contribution` under the JavaScript conditions in
    /// `guard`. The caller must pass a unique `order` obtained from `reserve_branch`.
    ///
    /// Returns a leaf containing the generated class and any marker or external names.
    /// Class metadata is recorded and CSS is queued for `finish` to publish.
    ///
    /// # Errors
    ///
    /// Returns an error if generating a class or slot name, or emitting utilities, fails.
    /// Earlier allocations and registry changes remain.
    pub(super) fn leaf_at(
        &mut self,
        mut contribution: LiteralContribution,
        guard: &[GuardStep],
        order: u32,
    ) -> Result<Emitted, CompileDiagnostic> {
        let mut names = Vec::new();
        if !contribution.utilities.is_empty() || !contribution.view_transitions.is_empty() {
            let branch = format!("{}:{}", order, contribution.canonical);
            let unit = self.materialize_transitions(
                &branch,
                contribution.view_transitions,
                &mut contribution.utilities,
                false,
            )?;
            let (name, rules) =
                self.name_unit(self.call_span.start.0, &branch, &contribution.utilities)?;
            names.push(name.clone());
            self.view_transition_classes.push(unit);
            self.classes.push((
                order,
                GeneratedClass {
                    class_name: name,
                    call_span: self.call_span,
                    guard_path: guard.to_vec(),
                },
            ));
            self.publish_unit(order, rules);
        }
        names.extend(contribution.markers);
        names.extend(contribution.external);
        names.extend(contribution.plain);
        Ok(Emitted::Leaf { names })
    }

    /// Consumes the allocator and publishes the base CSS before the queued leaf CSS.
    /// Leaves are published in traversal order. Transition slots can require a base
    /// class even when there are no ordinary base utilities.
    ///
    /// Returns unconditional class names, generated class metadata, and the base
    /// transition class list including its slot references. An empty list writes no
    /// transition-class declaration. The base name comes before marker and external
    /// names. The base metadata comes before leaf metadata, which follows traversal
    /// order.
    ///
    /// # Errors
    ///
    /// Returns an error if generating base class or slot names, emitting utilities, or
    /// rendering rule boundaries fails. Shared symbols and allocations may already have
    /// changed. CSS is appended to the destination only after rendering succeeds.
    pub(super) fn finish(
        mut self,
    ) -> Result<(Vec<String>, Vec<GeneratedClass>, String), CompileDiagnostic> {
        let mut names = Vec::new();
        let mut deferred = Vec::new();
        let base_branch = format!("0:{}", self.base.canonical);
        let transitions = std::mem::take(&mut self.base.transitions);
        let mut utilities = std::mem::take(&mut self.base.utilities);
        self.view_transition_classes[0] =
            self.materialize_transitions(&base_branch, transitions, &mut utilities, true)?;
        self.base.utilities = utilities;
        // A call whose transitions are all guarded still needs the class that references
        // their slots, so any unit beyond the base keeps it.
        let base_declares = !self.view_transition_classes[0].is_empty()
            || self
                .view_transition_classes
                .iter()
                .skip(1)
                .any(|unit| !unit.slots.is_empty());
        let transition_list =
            self.view_transition_classes[0].class_value(&self.view_transition_slots());
        if !self.base.utilities.is_empty() || base_declares {
            let branch = format!("0:{}", self.base.canonical);
            let definitions = std::mem::take(&mut self.base.utilities);
            let (name, rules) = self.name_unit(self.call_span.start.0, &branch, &definitions)?;
            // The base unit must precede every conditional unit in the stylesheet.
            deferred.extend(rules);
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
                (
                    0,
                    GeneratedClass {
                        class_name: name.clone(),
                        call_span: self.call_span,
                        guard_path: Vec::new(),
                    },
                ),
            );
            names.push(name);
        }
        names.extend(std::mem::take(&mut self.base.names));
        self.runtime_css.sort_by_key(|(order, _)| *order);
        deferred.extend(
            std::mem::take(&mut self.runtime_css)
                .into_iter()
                .map(|(_, contribution)| contribution),
        );
        let owner = format!(
            "style\0{}\0{}\0{}",
            self.module_id, self.options.class_namespace, self.call_span.start.0
        );
        let mut boundaries = RuleBoundaries::new(owner, self.identifiers);
        for contribution in &mut deferred {
            contribution.css = boundaries.render(&contribution.css)?;
        }
        self.utility_css.append(&mut deferred);
        self.classes.sort_by_key(|(order, _)| *order);
        Ok((
            names,
            self.classes.into_iter().map(|(_, class)| class).collect(),
            transition_list,
        ))
    }

    /// Generates a class name from `call` and the canonical `branch`, then returns
    /// that name and the unpublished CSS for `definitions`.
    ///
    /// Rules and a symbol are recorded only for the first occurrence of each pair of
    /// generated name and identity. Later occurrences return no CSS.
    ///
    /// # Errors
    ///
    /// Returns an error if class generation or utility emission fails. The pair may
    /// already be in the deduplication registry. The caller must discard the failed
    /// compilation.
    fn name_unit(
        &mut self,
        call: u32,
        branch: &str,
        definitions: &[CompiledUtility],
    ) -> Result<(String, Vec<CssContribution>), CompileDiagnostic> {
        let context = ClassContext {
            module: self.module_id.clone(),
            namespace: self.options.class_namespace.clone(),
            call,
            branch: branch.to_owned(),
        };
        let identity = ClassName(context.clone()).identity();
        let name = self.identifiers.class_name(&context)?;
        let mut contributions = Vec::new();
        if self
            .emitted_utility_classes
            .insert((name.as_str().to_owned(), identity.clone()))
        {
            emit_utility_rules(
                &mut contributions,
                &name.selector(),
                definitions,
                self.theme,
                self.identifiers,
            )?;
            self.symbols.push(generated_class_symbol(
                self.module_id,
                name.as_str(),
                &identity,
                self.call_span,
            ));
        }
        Ok((name.as_str().to_owned(), contributions))
    }

    /// Queues one unit's CSS contributions with their source order for publication after the base.
    fn publish_unit(&mut self, order: u32, contributions: Vec<CssContribution>) {
        self.runtime_css.extend(
            contributions
                .into_iter()
                .map(|contribution| (order, contribution)),
        );
    }

    /// Converts `transitions` into token metadata and appends their rules to `utilities`.
    /// The transitions keep their input order, and `branch` distinguishes their slots
    /// from those of other units in this call.
    ///
    /// When `base` is true, a transition without a CSS plan keeps its token as a literal.
    /// Other transitions use slots and apply their CSS plans. If a transition has a
    /// dynamic-name property, the caller must provide both a plan and a runtime value
    /// for that property.
    ///
    /// # Errors
    ///
    /// Returns an error if slot generation fails. Rules already appended to `utilities`
    /// remain when a later transition fails.
    fn materialize_transitions(
        &self,
        branch: &str,
        transitions: Vec<TransitionContribution>,
        utilities: &mut Vec<CompiledUtility>,
        base: bool,
    ) -> Result<ViewTransitionClassUnit, CompileDiagnostic> {
        let mut unit = ViewTransitionClassUnit::default();
        for (index, transition) in transitions.into_iter().enumerate() {
            if base && transition.plan.is_none() {
                unit.attach(&transition.token, transition.name.as_deref());
                continue;
            }
            let property = self.view_transition_slot(&format!("{branch}\0{index}"))?;
            let mut declaration = format!("{property}:{};", escaped_identifier(&transition.token));
            unit.slots.push(property);
            if let Some(name) = transition.name {
                let _ = write!(declaration, "view-transition-name:{name};");
            }
            if let Some(property) = transition.dynamic_name_property {
                let _ = write!(declaration, "view-transition-name:var({property});");
            }
            let mut utility = CompiledUtility {
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration,
                selector: None,
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(None),
                candidate: String::new(),
            };
            if let Some(plan) = transition.plan {
                plan.apply_rule(&mut utility);
            }
            utilities.push(utility);
        }
        Ok(unit)
    }

    /// Returns a distinct custom-property name for one dynamic transition name use.
    /// `source_offset` is the original UTF-8 start byte of the receiver expression.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for the slot name family.
    pub(super) fn view_transition_dynamic_name_slot(
        &self,
        source_offset: u32,
    ) -> Result<String, CompileDiagnostic> {
        self.view_transition_slot(&format!("view-transition-name:{source_offset}"))
    }

    /// The slot properties the base unit references, in the order their units were allocated.
    fn view_transition_slots(&self) -> Vec<String> {
        self.view_transition_classes
            .iter()
            .flat_map(|unit| unit.slots.iter().cloned())
            .collect()
    }

    /// Returns the generated custom-property name for a slot in this call.
    /// The caller must use a stable `branch` value that distinguishes this slot from
    /// other slots used for transition tokens or names.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured slot generator fails.
    fn view_transition_slot(&self, branch: &str) -> Result<String, CompileDiagnostic> {
        let context = ClassContext {
            module: self.module_id.clone(),
            namespace: self.options.class_namespace.clone(),
            call: self.call_span.start.0,
            branch: branch.to_owned(),
        };
        Ok(self.identifiers.view_transition_slot(&context)?.property())
    }
}
