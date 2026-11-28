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

use stillwind_css::{PREVIEW_SLOT, preview_comment_at_rule};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::LintSettings;
use stillwind_ir::{
    CssCall, KeyframesBody, KeyframesDefinition, MediaCall, ModuleAnalysis, RecipeCall,
    RecipeValueRole, RefDefinition, StoredVariantDefinition, StyleCall, StyleVariant,
    ViewTransitionDefinition,
};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{Fingerprint, ModuleId, Span};

use super::super::class_units::PreviewSlots;
use super::super::conditionals::lower;
use super::super::css_call::{CssOutput, css_call_text};
use super::super::definitions::{emit_keyframes_definition, emit_view_transition_definition};
use super::super::emission::{CallEmitter, CallEmitterInputs};
use super::super::media::media_query;
use super::super::style_variants::StyleVariantPlan;
use super::super::stylesheet::render_authored_variant_preview_path;
use super::super::symbols::{named_identity, ref_context};
use super::input::{
    collect_bases, has_enclosing_variant, identity_wrappers, input_plans, replace_text,
};
use super::resources::preview_resource_urls;
use super::runtime::collect_runtime_names;
use crate::compiler::rewrite_generated_css;
use crate::generated::js_string;
use crate::identifiers::{DeclarationContext, IdentifierContext, PreviewNames};
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
    /// A ref condition or a ref container query used as a variant. Its CSS contains
    /// the selectors and conditions of the variant around an empty rule or block,
    /// without the rules under the variant. The span addresses the condition's variant
    /// text, or the whole call when a ref is called without arguments, or the container
    /// query text.
    Variant,
    /// A `style.media()` call. Its preview is the JavaScript string literal that the
    /// call lowers to.
    Media,
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
    /// Whether an isolated literal input is wrapped in any enclosing variant, such as a
    /// string variant group, a ref condition, a container query, or a stored variant.
    pub enclosing_variant: bool,
    /// Whether emission succeeded. Whole calls and definitions also reject overlapping
    /// source diagnostics; diagnostics elsewhere in the module do not invalidate this
    /// site.
    pub valid: bool,
    /// Generated CSS with element subjects represented by `&` and other generated names
    /// replaced by readable identifiers, or the lowered JavaScript string literal for
    /// [`EditorSiteKind::Media`]. Empty on failure or when the site contributes no
    /// output.
    pub css: LeanStr,
    /// Required styles and global definitions, sorted and deduplicated by identity.
    pub dependencies: Vec<IdentityId>,
    /// Authored expressions that supply view-transition naming keys at runtime, in
    /// source order. Repeated occurrences can contain the same expression text.
    pub runtime_names: Vec<LeanStr>,
    /// Static recipe metadata for filtering previews at `recipe({ ... })` call sites.
    /// Present only on a complete `style.recipe()` factory site.
    pub recipe: Option<EditorRecipe>,
}

/// Static recipe structure retained on a `style.recipe()` site for filtering call-site
/// previews.
#[derive(Clone, Debug)]
pub struct EditorRecipe {
    /// Default option key for each axis declared in `defaultVariants`, in authored order.
    pub default_variants: Vec<(LeanStr, LeanStr)>,
    /// Compiled recipe values with their declaration identities and variant conditions,
    /// in source order.
    pub values: Vec<EditorRecipeValue>,
}

/// One compiled value of a recipe paired with the variant axis conditions that select it.
#[derive(Clone, Debug)]
pub struct EditorRecipeValue {
    /// Declaration identity of the value's compiled `StyleCall` site.
    pub identity: IdentityId,
    /// Variant axis conditions that select this value. Empty for unconditional values.
    pub conditions: Vec<(LeanStr, Vec<LeanStr>)>,
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
        for definition in &module.ir.ref_definitions {
            editor.add_ref(definition);
        }
        for definition in &module.ir.stored_variants {
            editor.add_stored_variant(definition);
        }
        for definition in &module.ir.keyframes_declarations {
            editor.add_keyframes(definition);
        }
        for definition in &module.ir.view_transition_declarations {
            editor.add_transition(definition);
        }
        for call in module.ir.style_inputs() {
            editor.add_call(call);
        }
        for recipe in &module.ir.recipe_calls {
            editor.add_recipe(recipe);
        }
        for call in &module.ir.media_calls {
            editor.add_media(call);
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
                        site.enclosing_variant,
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

    /// Records a `style.ref()` declaration with its preview marker selector.
    fn add_ref(&mut self, definition: &RefDefinition) {
        let context = self
            .identifiers
            .context(self.options.hash_length)
            .with_preview_names(Rc::clone(&self.names));
        let info = ref_context(definition, None);
        let css = context
            .ref_marker_name(&DeclarationContext {
                module: info.module,
                key: info.key,
                binding: info.binding,
                label: info.label,
            })
            .ok()
            .map(|marker| {
                format_args!("&:where({}){{{PREVIEW_SLOT}}}", marker.selector()).to_lean_string()
            });
        self.sites.push(EditorSite {
            span: definition.call_span,
            kind: EditorSiteKind::Variant,
            identity: Some(named_identity(
                &self.module.module_id,
                AssetKind::Ref,
                &definition.binding,
                &definition.key,
            )),
            context: None,
            enclosing_variant: false,
            valid: css.is_some(),
            css: css.unwrap_or_default().into(),
            dependencies: Vec::new(),
            runtime_names: Vec::new(),
            recipe: None,
        });
    }

    /// Records a `style.when()` stored variant declaration with its expanded wrappers.
    fn add_stored_variant(&mut self, definition: &StoredVariantDefinition) {
        let css = definition
            .variants
            .as_deref()
            .filter(|variants| !variants.is_empty())
            .and_then(|variants| self.render_variants(variants));
        self.sites.push(EditorSite {
            span: definition.call_span,
            kind: EditorSiteKind::Variant,
            identity: Some(named_identity(
                &self.module.module_id,
                AssetKind::StoredVariant,
                &definition.binding,
                &definition.key,
            )),
            context: None,
            enclosing_variant: false,
            valid: css.is_some(),
            css: css.unwrap_or_default().into(),
            dependencies: Vec::new(),
            runtime_names: Vec::new(),
            recipe: None,
        });
    }

    /// Records a `style.media()` call with the JavaScript string literal it lowers to.
    fn add_media(&mut self, call: &MediaCall) {
        let mut findings = Vec::new();
        let query = media_query(call, self.snapshot.theme(), &mut findings);
        let literal =
            (findings.is_empty() && !call.arguments.is_empty()).then(|| js_string(&query));
        self.sites.push(EditorSite {
            span: call.span,
            kind: EditorSiteKind::Media,
            identity: None,
            context: None,
            enclosing_variant: false,
            valid: literal.is_some(),
            css: literal.unwrap_or_default().into(),
            dependencies: Vec::new(),
            runtime_names: Vec::new(),
            recipe: None,
        });
    }

    /// Records a `style.recipe()` factory call and each of its scoped configuration
    /// regions (`slots`, variant axes, variant options, and compound entries).
    fn add_recipe(&mut self, recipe: &RecipeCall) {
        let valid = !recipe.empty
            && !self
                .module
                .diagnostics
                .iter()
                .flat_map(|error| &error.labels)
                .any(|label| {
                    label.span.file == recipe.span.file
                        && label.span.start < recipe.span.end
                        && recipe.span.start < label.span.end
                });
        let value_id = |value: &StyleCall| {
            IdentityId::new(
                AssetKind::Style,
                self.module.module_id.clone(),
                value.key.clone(),
                "",
            )
        };
        let mut dependencies = Vec::new();
        let mut seen = BTreeSet::new();
        let mut recipe_values = Vec::new();
        if valid {
            for value in &recipe.values {
                let id = value_id(value);
                if seen.insert(id.clone()) {
                    dependencies.push(id.clone());
                }
                let conditions = recipe
                    .roles
                    .iter()
                    .find(|role| role.span == value.span)
                    .map_or_default(|role| {
                        role.conditions
                            .iter()
                            .map(|cond| (cond.axis.clone(), cond.values.clone()))
                            .collect()
                    });
                recipe_values.push(EditorRecipeValue {
                    identity: id,
                    conditions,
                });
            }
        }
        let editor_recipe = valid.then(|| EditorRecipe {
            default_variants: recipe
                .default_variants
                .iter()
                .map(|default| (default.axis.clone(), default.value.clone()))
                .collect(),
            values: recipe_values,
        });
        self.sites.push(EditorSite {
            span: recipe.span,
            kind: EditorSiteKind::Style,
            identity: None,
            context: None,
            enclosing_variant: false,
            valid,
            css: LeanStr::new(),
            dependencies,
            runtime_names: Vec::new(),
            recipe: editor_recipe,
        });
        if valid {
            let mut scope_spans = BTreeSet::new();
            for scope in &recipe.preview_scopes {
                if scope.span == recipe.span
                    || !scope_spans.insert((scope.span.start.0, scope.span.end.0))
                {
                    continue;
                }
                let mut scope_deps = Vec::new();
                let mut scope_seen = BTreeSet::new();
                for span in &scope.values {
                    if let Some(value) = recipe.values.iter().find(|value| value.span == *span) {
                        let id = value_id(value);
                        if scope_seen.insert(id.clone()) {
                            scope_deps.push(id);
                        }
                    }
                }
                if !scope_deps.is_empty() {
                    self.sites.push(EditorSite {
                        span: scope.span,
                        kind: EditorSiteKind::Style,
                        identity: None,
                        context: None,
                        enclosing_variant: false,
                        valid: true,
                        css: LeanStr::new(),
                        dependencies: scope_deps,
                        runtime_names: Vec::new(),
                        recipe: None,
                    });
                }
            }
        }
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
                &definition.key,
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
                &definition.key,
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
                call.key.clone(),
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
                        key: call.key.clone(),
                        output: call.output,
                        inputs: vec![input],
                    }),
                );
            }
        }
        let mut wrappers = Vec::new();
        for input in &call.inputs {
            identity_wrappers(input, &mut wrappers);
        }
        let mut spans = BTreeSet::new();
        for wrapper in wrappers {
            let span = match wrapper {
                StyleVariant::Ref { condition, .. } => condition.prefix.span,
                StyleVariant::Container { query, .. } => query.span,
                StyleVariant::Prefix(_) | StyleVariant::Alternatives(_) => continue,
            };
            // A stored variant that a call uses several times yields the same wrapper.
            if spans.insert((span.start.0, span.end.0)) {
                let css = self.render_variants(std::slice::from_ref(wrapper));
                self.sites.push(EditorSite {
                    span,
                    kind: EditorSiteKind::Variant,
                    identity: None,
                    context: None,
                    enclosing_variant: false,
                    valid: css.is_some(),
                    css: css.unwrap_or_default().into(),
                    dependencies: Vec::new(),
                    runtime_names: Vec::new(),
                    recipe: None,
                });
            }
        }
    }

    /// Renders the selectors and conditions of `wrappers` around a preview placeholder,
    /// with the batch's display names. Returns `None` when the wrappers do not resolve,
    /// for example because variant text cannot test a referenced element, or when
    /// conditional lowering fails.
    fn render_variants(&self, wrappers: &[StyleVariant]) -> Option<LeanString> {
        let context = self
            .identifiers
            .context(self.options.hash_length)
            .with_preview_names(Rc::clone(&self.names));
        let theme = self.snapshot.theme();
        let plans =
            StyleVariantPlan::resolve(&context, wrappers, theme, &mut Vec::new(), &mut Vec::new())?;
        let mut css = LeanString::new();
        for plan in &plans {
            for path in plan.applications() {
                css.push_str(
                    &render_authored_variant_preview_path(path, &context, self.options.use_css_if)
                        .ok()??,
                );
            }
        }
        let lowered = lower(&css, self.options.use_css_if, &context).ok()?;
        let css = rewrite_generated_css(&lowered.css, theme, &self.options);
        Some(preview_resource_urls(css.to_lean_string(), &self.snapshot))
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
        let enclosing_variant = kind == EditorSiteKind::Input && has_enclosing_variant(&plan);
        self.sites.push(EditorSite {
            span,
            kind,
            identity,
            context,
            enclosing_variant,
            valid,
            css: (css).into(),
            dependencies,
            runtime_names,
            recipe: None,
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
                .map_or_default(PreviewSlots::replay),
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

    /// Returns the preview of a `style.css()` call, or `None` when it reports
    /// diagnostics.
    fn render_css_call(
        &self,
        call: &CssCall,
        output: CssOutput,
        context: &IdentifierContext,
    ) -> Option<(LeanString, Vec<IdentityId>)> {
        let mut diagnostics = Vec::new();
        let css = css_call_text(
            &self.module.module_id,
            call,
            self.snapshot.theme(),
            &self.options,
            context,
            output,
            &mut diagnostics,
            &mut Vec::new(),
        );
        diagnostics
            .is_empty()
            .then(|| (preview_resource_urls(css, &self.snapshot), Vec::new()))
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
        match plan {
            EditorPlan::Css { call, output } => {
                // The text is already lowered and rewritten, so it skips the steps below.
                return self.render_css_call(call, *output, &context);
            }
            EditorPlan::Call(call) => {
                for input in &call.inputs {
                    collect_bases(input, &mut dependencies);
                }
                let recipe = self
                    .module
                    .ir
                    .recipe_calls
                    .iter()
                    .any(|factory| factory.values.iter().any(|value| value.span == call.span));
                let run = if recipe {
                    CallEmitter::run_recipe
                } else {
                    CallEmitter::run
                };
                run(CallEmitterInputs {
                    preview: Some(slots),
                    conditional_labels: &self.module.ir.conditional_labels,
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
                    &LintSettings::default(),
                    &mut diagnostics,
                    &mut lint_findings,
                );
                contributions.push(contribution);
            }
        }
        if !diagnostics.is_empty() {
            return None;
        }
        let css = contributions
            .into_iter()
            .map(|part| {
                lower(&part.css, self.options.use_css_if, &context).map(|lowered| {
                    rewrite_generated_css(&lowered.css, self.snapshot.theme(), &self.options)
                })
            })
            .collect::<Result<LeanString, _>>()
            .ok()?;
        let mut css = preview_resource_urls(css, &self.snapshot);
        if !css.is_empty()
            && let EditorPlan::Call(call) = plan
            && let Some(comment) = self.recipe_role_comment(call.span)
        {
            css = format_args!("{comment}{css}").to_lean_string();
        }
        Some((css, dependencies.into_iter().collect()))
    }

    /// Returns a [`preview_comment_at_rule`] describing the slot and variant conditions of
    /// the recipe value at `span`, or `None` when `span` is not a recipe value or is an
    /// unconditional base value of a non-slotted recipe.
    fn recipe_role_comment(&self, span: Span) -> Option<LeanString> {
        let recipe = self
            .module
            .ir
            .recipe_calls
            .iter()
            .find(|factory| factory.values.iter().any(|value| value.span == span))?;
        let role = recipe.roles.iter().find(|role| role.span == span)?;
        let label = format_recipe_role(recipe, role);
        let comment = preview_comment_at_rule(&label);
        (!comment.is_empty()).then_some(comment)
    }
}

/// Formats a recipe value's slot names and variant conditions for a rule-leading comment.
fn format_recipe_role(recipe: &RecipeCall, role: &RecipeValueRole) -> LeanString {
    let slot_prefix = if recipe.has_slots && !role.slots.is_empty() {
        Some(format_args!("[{}]", role.slots.join(", ")).to_lean_string())
    } else {
        None
    };
    let multi_axis = role.conditions.len() > 1;
    let cond_parts: Vec<LeanString> = role
        .conditions
        .iter()
        .filter(|cond| !cond.values.is_empty())
        .map(|cond| {
            let exprs: Vec<LeanString> = cond
                .values
                .iter()
                .map(|val| {
                    let lit = if val == "true" || val == "false" {
                        val.as_str().to_lean_string()
                    } else {
                        serde_json::Value::String(val.as_str().into()).to_lean_string()
                    };
                    format_args!("{} === {lit}", cond.axis).to_lean_string()
                })
                .collect();
            if exprs.len() > 1 && multi_axis {
                format_args!("({})", exprs.join(" || ")).to_lean_string()
            } else {
                exprs.join(" || ").to_lean_string()
            }
        })
        .collect();
    let cond_suffix = (!cond_parts.is_empty()).then(|| cond_parts.join(" && ").to_lean_string());
    match (slot_prefix, cond_suffix) {
        (Some(slots), Some(cond)) => format_args!("{slots} {cond}").to_lean_string(),
        (Some(slots), None) => slots,
        (None, Some(cond)) => cond,
        (None, None) => LeanString::new(),
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
