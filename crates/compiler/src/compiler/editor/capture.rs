//! Captures source occurrences and input plans from one finalized module.
//!
//! [`ModuleEditor`] uses compiler emission to render CSS without publishing artifacts.
//! Captured spans address the original source, and input previews retain their
//! enclosing variants and JavaScript conditions.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::LintSettings;
use stillwind_ir::{
    CssCall, KeyframesBody, KeyframesDefinition, ModuleAnalysis, StyleCall, StyleVariant,
    ViewTransitionDefinition,
};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{Fingerprint, ModuleId, Span};

use super::super::class_units::PreviewSlots;
use super::super::conditionals::lower;
use super::super::css_call::{CssOutput, css_call_text};
use super::super::definitions::{emit_keyframes_definition, emit_view_transition_definition};
use super::super::emission::{CallEmitter, CallEmitterInputs};
use super::super::symbols::named_identity;
use super::input::{collect_bases, input_plans, replace_text};
use super::resources::preview_resource_urls;
use super::runtime::collect_runtime_names;
use crate::identifiers::PreviewNames;
use crate::{CandidateContext, CompilerOptions, CssContribution, Identifiers};

/// The source object addressed by a generated preview.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorSiteKind {
    /// A complete style call, including its own generated class units.
    Style,
    /// A reusable keyframes or view-transition declaration.
    Definition,
    /// A rule-bearing use, anchored at its receiver or whole scope call.
    Use,
    /// A compiler-recognized literal input.
    Input,
    /// A `style.css()` call, whose CSS is the string that the call lowers to.
    Stylesheet,
}

/// CSS and dependencies contributed by one original source occurrence.
#[derive(Clone, Debug)]
pub struct EditorSite {
    /// Half-open UTF-8 byte range in the original module. A use can address its
    /// receiver.
    pub span: Span,
    /// Meaning of the source occurrence.
    pub kind: EditorSiteKind,
    /// Declaration identity for reusable values and style calls.
    pub identity: Option<IdentityId>,
    /// Candidate contract for literal inputs. Whole expressions have no context.
    pub context: Option<CandidateContext>,
    /// Whether emission succeeded. Whole calls and definitions also reject overlapping
    /// source diagnostics; diagnostics elsewhere in the module do not invalidate this
    /// site.
    pub valid: bool,
    /// Generated CSS with element subjects represented by `&` and other generated names
    /// replaced by readable identifiers. Empty on failure or when the site contributes
    /// no CSS.
    pub css: LeanStr,
    /// Required styles and global definitions, sorted and deduplicated by identity.
    pub dependencies: Vec<IdentityId>,
    /// Authored expressions that supply view-transition naming keys at runtime, in
    /// source order. Repeated occurrences can contain the same expression text.
    pub runtime_names: Vec<LeanStr>,
}

/// Enclosing IR retained for an occurrence or a replacement literal.
#[derive(Clone)]
pub(super) enum EditorPlan {
    /// Emits element rules with the original call identity.
    Call(
        /// Complete call or one selected input, with its original enclosing conditions.
        StyleCall,
    ),
    /// Emits a global keyframes declaration.
    Keyframes(
        /// Complete definition or the path to a selected frame or condition prefix.
        KeyframesDefinition,
    ),
    /// Emits global view-transition pseudo-element rules.
    Transition(
        /// Complete definition or one selected group or phase input.
        ViewTransitionDefinition,
    ),
    /// Emits the string of a `style.css()` call, or only its rules.
    Css {
        /// Complete call, or the call with one selected input and its options.
        call: CssCall,
        /// The part of the output to render.
        output: CssOutput,
    },
}

/// Captured editor output and replacement plans for one analyzed module.
/// Later input queries use the captured analysis and fork the shared display names.
pub struct ModuleEditor {
    /// Fingerprint of the source, supporting analyses, design system, compiler
    /// settings, and captured output.
    pub revision: Fingerprint,
    /// Recognized occurrences, including invalid inputs, in capture traversal order.
    /// Definitions precede style calls; entries are not sorted by source position.
    pub sites: Vec<EditorSite>,
    /// Finalized analysis and original source retained independently of session
    /// updates.
    module: Arc<ModuleAnalysis>,
    /// Theme and resource metadata used when the occurrences were captured.
    snapshot: DesignSystemSnapshot,
    /// Resolved settings used for emission, CSS conditionals, and prefix rewriting.
    options: CompilerOptions,
    /// Generator configuration retained for replacement previews.
    identifiers: Identifiers,
    /// Original literal spans paired with isolated plans, including currently invalid
    /// inputs.
    plans: Vec<(Span, EditorPlan)>,
    /// Slot allocations from complete calls, keyed by their inclusive UTF-8 start.
    slots: BTreeMap<u32, PreviewSlots>,
    /// Names shared while a batch is captured. Each replacement preview clones this
    /// table.
    names: Rc<RefCell<PreviewNames>>,
}

impl ModuleEditor {
    /// Captures definitions, calls, and isolated inputs from `module`.
    ///
    /// `module` must contain finalized IR with original source spans, and `options`
    /// must be resolved against `snapshot`. `names` must already contain declaration
    /// reservations for the whole capture batch. Emission uses `identifiers` and adds
    /// helper names to that shared table. The returned revision combines the supplied
    /// fingerprint with the captured output. Failed occurrences remain available as
    /// invalid sites; no artifacts are published.
    pub(crate) fn new(
        module: &ModuleAnalysis,
        snapshot: DesignSystemSnapshot,
        options: CompilerOptions,
        identifiers: Identifiers,
        revision: Fingerprint,
        names: Rc<RefCell<PreviewNames>>,
    ) -> Self {
        let mut editor = Self {
            revision,
            sites: Vec::new(),
            module: Arc::new(module.clone()),
            snapshot,
            options,
            identifiers,
            plans: Vec::new(),
            slots: BTreeMap::new(),
            names,
        };
        for definition in &module.ir.keyframes_declarations {
            editor.add_keyframes(definition);
        }
        for definition in &module.ir.view_transition_declarations {
            editor.add_transition(definition);
        }
        for call in module.ir.style_inputs() {
            editor.add_call(call);
        }
        for call in &module.ir.css_calls {
            editor.add_css(call);
        }
        editor.revision = Fingerprint::of(
            format_args!(
                "{:?}{:?}",
                editor.revision,
                editor
                    .sites
                    .iter()
                    .map(|site| (
                        site.span.start,
                        site.span.end,
                        site.kind,
                        &site.identity,
                        site.context,
                        site.valid,
                        &site.css,
                        &site.dependencies
                    ))
                    .collect::<Vec<_>>()
            )
            .to_lean_string(),
        );
        editor
    }

    /// Returns the original module identity shared by all captured source ranges.
    pub fn module_id(&self) -> &ModuleId {
        &self.module.module_id
    }

    /// Records a complete definition and an isolated plan for each frame and condition
    /// prefix. `definition` must belong to this module's finalized analysis.
    fn add_keyframes(&mut self, definition: &KeyframesDefinition) {
        self.add(
            definition.call_span,
            EditorSiteKind::Definition,
            Some(named_identity(
                &self.module.module_id,
                AssetKind::Keyframes,
                &definition.binding,
            )),
            None,
            EditorPlan::Keyframes(definition.clone()),
        );
        collect_frames(&definition.body, &mut |span, context| {
            let mut selected = definition.clone();
            retain_frame(&mut selected.body, span);
            self.add(
                span,
                EditorSiteKind::Input,
                None,
                Some(context),
                EditorPlan::Keyframes(selected),
            );
        });
    }

    /// Records a complete definition and each present group or phase input separately.
    /// `definition` must belong to this module's finalized analysis.
    fn add_transition(&mut self, definition: &ViewTransitionDefinition) {
        self.add(
            definition.call_span,
            EditorSiteKind::Definition,
            Some(named_identity(
                &self.module.module_id,
                AssetKind::ViewTransition,
                &definition.binding,
            )),
            None,
            EditorPlan::Transition(definition.clone()),
        );
        for (index, input) in [
            &definition.group,
            &definition.old,
            &definition.new,
            &definition.image_pair,
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(input) = input {
                let mut selected = definition.clone();
                selected.group = (index == 0).then(|| input.clone());
                selected.old = (index == 1).then(|| input.clone());
                selected.new = (index == 2).then(|| input.clone());
                selected.image_pair = (index == 3).then(|| input.clone());
                self.add(
                    input.span,
                    EditorSiteKind::Input,
                    None,
                    Some(CandidateContext::ViewTransitionUtilities),
                    EditorPlan::Transition(selected),
                );
            }
        }
    }

    /// Records a `style.css()` call with its whole string and each input literal with
    /// its rules. `call` must retain its original spans in this module.
    ///
    /// An input plan keeps the call's options and the input's group keys, so a
    /// replacement preview resolves like the input in the call.
    fn add_css(&mut self, call: &CssCall) {
        self.add(
            call.span,
            EditorSiteKind::Stylesheet,
            None,
            None,
            EditorPlan::Css {
                call: call.clone(),
                output: CssOutput::Template,
            },
        );
        let context = if call.selector.is_some() {
            CandidateContext::AnchoredDocument
        } else {
            CandidateContext::Document
        };
        for input in &call.inputs {
            self.add(
                input.utilities.span,
                EditorSiteKind::Input,
                None,
                Some(context),
                EditorPlan::Css {
                    call: CssCall {
                        inputs: vec![input.clone()],
                        ..call.clone()
                    },
                    output: CssOutput::Rules,
                },
            );
        }
    }

    /// Captures a complete call's slot allocations before recording its isolated inputs
    /// and uses. `call` must retain its original spans in this module.
    fn add_call(&mut self, call: &StyleCall) {
        let mut slots = PreviewSlots::capture();
        let plan = EditorPlan::Call(call.clone());
        let output = self.render_with_slots(&plan, &mut slots);
        self.slots.insert(call.span.start.0, slots);
        self.record(
            call.span,
            EditorSiteKind::Style,
            Some(IdentityId::new(
                AssetKind::Style,
                self.module.module_id.clone(),
                call.span.start.0,
                call.span.end.0,
                "",
            )),
            None,
            plan,
            output,
        );
        for input in &call.inputs {
            for (span, context, input) in input_plans(input) {
                self.add(
                    span,
                    if context.is_some() {
                        EditorSiteKind::Input
                    } else {
                        EditorSiteKind::Use
                    },
                    None,
                    context,
                    EditorPlan::Call(StyleCall {
                        span: call.span,
                        inputs: vec![input],
                    }),
                );
            }
        }
    }

    /// Renders decoded replacement text for the literal whose original inclusive UTF-8
    /// byte start is `start`. `candidate` replaces the whole selected literal and may
    /// contain several tokens. The plan retains its enclosing variants and conditions.
    /// The captured analysis, slot allocations, and names remain unchanged.
    /// Returns `None` for an unknown start, failed emission, or output with no CSS.
    pub fn preview_input(&self, start: u32, candidate: &str) -> Option<LeanString> {
        let (span, plan) = self.plans.iter().find(|(span, _)| span.start.0 == start)?;
        let mut plan = plan.clone();
        replace_text(&mut plan, *span, candidate);
        let names = Rc::new(RefCell::new(self.names.borrow().clone()));
        self.render(&plan, &names)
            .map(|(css, _)| css)
            .filter(|css| !css.is_empty())
    }

    /// Renders `plan` with the batch's display names and records its original `span`.
    /// A present `context` also retains the plan for later replacement queries.
    /// `identity` identifies declarations or style calls that other sites can depend
    /// on.
    fn add(
        &mut self,
        span: Span,
        kind: EditorSiteKind,
        identity: Option<IdentityId>,
        context: Option<CandidateContext>,
        plan: EditorPlan,
    ) {
        let output = self.render(&plan, &self.names);
        self.record(span, kind, identity, context, plan, output);
    }

    /// Appends a site from an emission result, rejecting diagnostics that overlap a
    /// whole call or definition. Failed output has empty CSS and dependency lists.
    /// Successful calls also retain authored runtime keys from the original source.
    /// A present `context` adds `(span, plan)` to the replacement plans even after
    /// failure.
    fn record(
        &mut self,
        span: Span,
        kind: EditorSiteKind,
        identity: Option<IdentityId>,
        context: Option<CandidateContext>,
        plan: EditorPlan,
        output: Option<(LeanString, Vec<IdentityId>)>,
    ) {
        let output = output.filter(|_| {
            !matches!(
                kind,
                EditorSiteKind::Style | EditorSiteKind::Definition | EditorSiteKind::Stylesheet
            ) || !self
                .module
                .diagnostics
                .iter()
                .flat_map(|error| &error.labels)
                .any(|label| {
                    label.span.file == span.file
                        && label.span.start < span.end
                        && span.start < label.span.end
                })
        });
        let valid = output.is_some();
        let (css, dependencies) = output.unwrap_or_default();
        let mut keys = BTreeMap::new();
        if valid && let EditorPlan::Call(call) = &plan {
            for input in &call.inputs {
                collect_runtime_names(input, &mut keys);
            }
        }
        let runtime_names = keys
            .into_values()
            .filter_map(|span| self.module.source.slice(span).ok().map(LeanStr::from))
            .collect();
        self.sites.push(EditorSite {
            span,
            kind,
            identity,
            context,
            valid,
            css: (css).into(),
            dependencies,
            runtime_names,
        });
        if context.is_some() {
            self.plans.push((span, plan));
        }
    }

    /// Renders an original or isolated plan with `names`. Call plans replay slots from
    /// their complete call when available. Returns CSS and sorted unique dependencies,
    /// or `None` when emission fails. New requests can add names to the supplied table.
    fn render(
        &self,
        plan: &EditorPlan,
        names: &Rc<RefCell<PreviewNames>>,
    ) -> Option<(LeanString, Vec<IdentityId>)> {
        let mut slots = match plan {
            EditorPlan::Call(call) => self
                .slots
                .get(&call.span.start.0)
                .map(PreviewSlots::replay)
                .unwrap_or_default(),
            _ => PreviewSlots::default(),
        };
        self.render_with_names(plan, &mut slots, names)
    }

    /// Renders with the batch's name table and the caller's slot capture or replay
    /// state. Returns the same output as `render_with_names` and updates the supplied
    /// slots.
    fn render_with_slots(
        &self,
        plan: &EditorPlan,
        slots: &mut PreviewSlots,
    ) -> Option<(LeanString, Vec<IdentityId>)> {
        self.render_with_names(plan, slots, &self.names)
    }

    /// Emits `plan` with readable names and `&` as the element subject. `slots` records
    /// or replays transition allocations, and `names` receives any new name requests.
    /// Successful output applies the configured CSS conditional mode and prefix
    /// rewrite, restores authored resource URLs, and reports sorted unique style or
    /// definition dependencies. Emission diagnostics or failed conditional lowering
    /// return `None`.
    fn render_with_names(
        &self,
        plan: &EditorPlan,
        slots: &mut PreviewSlots,
        names: &Rc<RefCell<PreviewNames>>,
    ) -> Option<(LeanString, Vec<IdentityId>)> {
        let context = self
            .identifiers
            .context(self.options.hash_length)
            .with_preview_names(Rc::clone(names));
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();
        let mut contributions = Vec::new();
        let mut dependencies = BTreeSet::new();
        if let EditorPlan::Css { call, output } = plan {
            // The text is already lowered and rewritten, so it skips the steps below.
            let css = css_call_text(
                call,
                self.snapshot.theme(),
                &self.options,
                &context,
                *output,
                &mut diagnostics,
                &mut lint_findings,
            );
            return diagnostics
                .is_empty()
                .then(|| (preview_resource_urls(css, &self.snapshot), Vec::new()));
        }
        match plan {
            EditorPlan::Call(call) => {
                for input in &call.inputs {
                    collect_bases(input, &mut dependencies);
                }
                let cva = self
                    .module
                    .ir
                    .cva_calls
                    .iter()
                    .any(|factory| factory.values.iter().any(|value| value.span == call.span));
                let run = if cva {
                    CallEmitter::run_cva
                } else {
                    CallEmitter::run
                };
                run(CallEmitterInputs {
                    preview: Some(slots),
                    source: &self.module.source,
                    module_id: &self.module.module_id,
                    options: &self.options,
                    identifiers: &context,
                    theme: self.snapshot.theme(),
                    call,
                    diagnostics: &mut diagnostics,
                    lint_findings: &mut Vec::new(),
                    utility_css: &mut contributions,
                    symbols: &mut Vec::new(),
                    emitted_utility_classes: &mut BTreeSet::new(),
                    runtime_assets: &mut BTreeSet::new(),
                    required_definitions: &mut dependencies,
                })?;
            }
            EditorPlan::Keyframes(definition) => {
                let mut contribution = CssContribution::default();
                emit_keyframes_definition(
                    &mut contribution,
                    &self.module.module_id,
                    &context,
                    definition,
                    self.snapshot.theme(),
                    &LintSettings::default(),
                    &mut diagnostics,
                    &mut lint_findings,
                );
                contributions.push(contribution);
            }
            EditorPlan::Transition(definition) => {
                let mut contribution = CssContribution::default();
                emit_view_transition_definition(
                    &mut contribution,
                    &self.module.module_id,
                    &context,
                    definition,
                    self.snapshot.theme(),
                    &mut diagnostics,
                    &mut lint_findings,
                );
                contributions.push(contribution);
            }
            EditorPlan::Css { .. } => {}
        }
        if !diagnostics.is_empty() {
            return None;
        }
        let css = contributions
            .into_iter()
            .map(|part| {
                lower(&part.css, self.options.use_css_if, &context)
                    .map(|lowered| self.options.rewrite_generated(&lowered.css))
            })
            .collect::<Result<LeanString, _>>()
            .ok()?;
        Some((
            preview_resource_urls(css, &self.snapshot),
            dependencies.into_iter().collect(),
        ))
    }
}

/// Visits frame literals before nested branches, including each authored condition
/// prefix before its body. Spans and contexts identify inputs for replacement previews.
fn collect_frames(body: &KeyframesBody, visit: &mut impl FnMut(Span, CandidateContext)) {
    for frame in &body.frames {
        visit(frame.utilities.span, CandidateContext::KeyframesFrame);
    }
    for branch in &body.branches {
        if let StyleVariant::Prefix(prefix) = &branch.variant {
            visit(prefix.span, CandidateContext::DefinitionVariants);
        }
        collect_frames(&branch.body, visit);
    }
}

/// Keeps only frames at `span` and the branches that contain them. A matching condition
/// prefix keeps its entire branch body. Returns whether any frame or branch remains.
fn retain_frame(body: &mut KeyframesBody, span: Span) -> bool {
    body.frames.retain(|frame| frame.utilities.span == span);
    body.branches.retain_mut(|branch| {
        matches!(&branch.variant, StyleVariant::Prefix(prefix) if prefix.span == span)
            || retain_frame(&mut branch.body, span)
    });
    !body.frames.is_empty() || !body.branches.is_empty()
}
