//! Public orchestration API for the first compiler slice.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use serde::{Deserialize, Serialize};
use stillwind_grammar::{Variant, lex_utilities};
use stillwind_ir::{
    AnimationDefinition, AssetReference, AttributeEntry, HandleProjection, StyleInput,
    ViewTransitionDefinition,
};
use stillwind_semantic::{
    AssetKind, AssetMetadata, Candidates, ExportedValue, IdentityId, TypeFact,
};
use stillwind_source::{Diagnostic, SourceFile, Span, Spanned, normalized_module_path};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedClass {
    pub class_name: String,
    pub call_span: Span,
    pub guard: Option<Span>,
    pub guard_when_true: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PropsTransform {
    pub call_span: Span,
    pub replacement: String,
}

/// A generated CSS identifier with the canonical identity used to allocate it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedSymbol {
    pub name: String,
    pub kind: String,
    pub identity: String,
    pub source_path: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug)]
struct ViewTransitionStyleEntry {
    value: Option<Span>,
    identity: Option<String>,
    candidates: Vec<String>,
    key: Option<Span>,
}

#[derive(Clone, Debug)]
struct DynamicAssetClassEntry {
    value: Span,
    classes: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
struct ReactPropsParts<'a> {
    classes: &'a [GeneratedClass],
    attributes: &'a [AttributeEntry],
    handle_markers: &'a [String],
    external_classes: &'a [Span],
    dynamic_handle_classes: &'a [Span],
    dynamic_asset_classes: &'a [DynamicAssetClassEntry],
    view_transition_style: Option<&'a ViewTransitionStyleEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    pub css: String,
    /// Unwrapped utility rules used to assemble one stylesheet for a module graph.
    #[serde(skip)]
    pub module_css: String,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub transforms: Vec<PropsTransform>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    pub prefix: String,
    pub tailwind_prefix: String,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            prefix: "--sw-".to_owned(),
            tailwind_prefix: "--tw-".to_owned(),
        }
    }
}

impl CompilerOptions {
    /// Validates that generated and Tailwind-compatible namespaces are distinct CSS prefixes.
    ///
    /// # Errors
    ///
    /// Returns an error when a prefix is malformed or both namespaces are identical.
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("prefix", self.prefix.as_str()),
            ("tailwindPrefix", self.tailwind_prefix.as_str()),
        ] {
            if !value.starts_with("--") || !value.ends_with('-') || value.len() < 4 {
                return Err(format!("{name} must start with `--` and end with `-`"));
            }
        }
        if self.prefix == self.tailwind_prefix {
            return Err("prefix and tailwindPrefix must be different".to_owned());
        }
        Ok(())
    }

    fn rewrite_generated(&self, value: &str) -> String {
        value
            .replace("--sw-", "\u{e000}stillwind-prefix\u{e001}")
            .replace("--tw-", "\u{e000}tailwind-prefix\u{e001}")
            .replace("\u{e000}stillwind-prefix\u{e001}", &self.prefix)
            .replace("\u{e000}tailwind-prefix\u{e001}", &self.tailwind_prefix)
    }
}

impl CompileResult {
    /// Applies source-spanned macro replacements from right to left.
    ///
    /// This deliberately keeps the compiler's source mapping contract separate
    /// from any particular bundler. A Vite adapter can use the same plan and
    /// provide a high-resolution source map around these exact byte spans.
    ///
    /// # Errors
    ///
    /// Returns an error when a transform targets another file or overlaps a
    /// previously applied source span.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, String> {
        let mut transforms = self.transforms.clone();
        transforms.sort_by_key(|transform| std::cmp::Reverse(transform.call_span.start.0));

        let mut output = source.text().to_owned();
        let mut previous_start = u32::MAX;
        for transform in transforms {
            if transform.call_span.file != source.id()
                || transform.call_span.end.0 > previous_start
                || transform.call_span.end.0 as usize > output.len()
            {
                return Err("invalid or overlapping stillwind transform plan".to_owned());
            }
            let start = transform.call_span.start.0 as usize;
            let end = transform.call_span.end.0 as usize;
            output.replace_range(start..end, &transform.replacement);
            previous_start = transform.call_span.start.0;
        }
        Ok(output)
    }
}

pub struct Compiler;

impl Compiler {
    pub fn check_javascript(source: &SourceFile) -> Vec<Diagnostic> {
        stillwind_js_oxc::parse(source).diagnostics
    }

    pub fn analyze_javascript(source: &SourceFile) -> stillwind_js_oxc::ParsedJavaScript {
        stillwind_js_oxc::parse(source)
    }

    /// Reads the project's CSS-first theme surface.
    ///
    /// # Errors
    ///
    /// Returns an error when the stylesheet is invalid or contains an
    /// unterminated `@theme` block.
    pub fn read_theme(
        source: &SourceFile,
    ) -> Result<stillwind_css::ThemeIndex, stillwind_css::ThemeError> {
        stillwind_css::parse_tailwind_theme(source)
    }

    /// Generates a module augmentation containing the theme-backed utility vocabulary.
    ///
    /// # Errors
    ///
    /// Returns an error when the CSS theme cannot be parsed.
    pub fn generate_react_types(
        theme_source: &SourceFile,
    ) -> Result<String, stillwind_css::ThemeError> {
        let theme = Self::read_theme(theme_source)?;
        Ok(render_react_types(&theme))
    }

    /// Wraps compiled module CSS fragments in the project's shared stylesheet layers.
    ///
    /// # Errors
    ///
    /// Returns an error when the CSS theme cannot be parsed.
    pub fn render_react_stylesheet(
        theme_source: &SourceFile,
        module_css: &str,
        options: &CompilerOptions,
    ) -> Result<String, stillwind_css::ThemeError> {
        let theme = Self::read_theme(theme_source)?;
        Ok(render_stylesheet(&theme, module_css, options))
    }

    /// Compiles the first static React subset into CSS.
    ///
    /// This temporary API deliberately supports only literal utility-string
    /// arguments. Conditions, attributes, selectors, and transforms are added
    /// in successive compiler slices.
    #[must_use]
    pub fn compile_react(source: &SourceFile, theme_source: &SourceFile) -> CompileResult {
        Self::compile_react_with_options(source, theme_source, &CompilerOptions::default())
    }

    #[must_use]
    pub fn compile_react_with_options(
        source: &SourceFile,
        theme_source: &SourceFile,
        options: &CompilerOptions,
    ) -> CompileResult {
        let parsed = Self::analyze_javascript(source);
        Self::compile_react_parsed(source, theme_source, parsed, options)
    }

    #[must_use]
    pub fn compile_react_with_type_facts(
        source: &SourceFile,
        theme_source: &SourceFile,
        type_facts: &[TypeFact],
        assets: &BTreeMap<IdentityId, AssetMetadata>,
    ) -> CompileResult {
        Self::compile_react_with_type_facts_and_options(
            source,
            theme_source,
            type_facts,
            assets,
            &CompilerOptions::default(),
        )
    }

    #[must_use]
    pub fn compile_react_with_type_facts_and_options(
        source: &SourceFile,
        theme_source: &SourceFile,
        type_facts: &[TypeFact],
        assets: &BTreeMap<IdentityId, AssetMetadata>,
        options: &CompilerOptions,
    ) -> CompileResult {
        let parsed = stillwind_js_oxc::parse_with_type_facts(source, type_facts, assets);
        Self::compile_react_parsed(source, theme_source, parsed, options)
    }

    #[allow(clippy::too_many_lines)] // Coordinates source transforms with CSS emission.
    fn compile_react_parsed(
        source: &SourceFile,
        theme_source: &SourceFile,
        parsed: stillwind_js_oxc::ParsedJavaScript,
        options: &CompilerOptions,
    ) -> CompileResult {
        let mut runtime_assets = summary_runtime_assets(&parsed.summary);
        let mut result = CompileResult {
            diagnostics: parsed.diagnostics,
            ..CompileResult::default()
        };
        let theme = match Self::read_theme(theme_source) {
            Ok(theme) => theme,
            Err(error) => {
                result.diagnostics.push(Diagnostic::error(
                    "SHCSS001",
                    error.to_string(),
                    theme_source.full_span(),
                ));
                return result;
            }
        };

        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == stillwind_source::Severity::Error)
        {
            return result;
        }

        let mut utility_css = String::new();
        let mut emitted_utility_classes = BTreeSet::new();
        let module = parsed.module;
        result
            .symbols
            .extend(module.handle_definitions.iter().map(generated_ref_symbol));
        result.symbols.extend(
            module
                .handle_definitions
                .iter()
                .map(generated_ref_marker_symbol),
        );
        result
            .symbols
            .extend(module.scope_definitions.iter().map(generated_scope_symbol));
        result.symbols.extend(
            module
                .scope_definitions
                .iter()
                .map(generated_scope_marker_symbol),
        );
        result.symbols.extend(
            module
                .animation_declarations
                .iter()
                .map(|definition| generated_symbol(source, "animation", &definition.name)),
        );
        result.symbols.extend(
            module
                .view_transition_declarations
                .iter()
                .map(|definition| generated_symbol(source, "view-transition", &definition.name)),
        );
        for animation in &module.animation_declarations {
            emit_animation_definition(
                &mut utility_css,
                source,
                animation,
                &theme,
                &mut result.diagnostics,
            );
        }
        for transition in &module.view_transition_declarations {
            emit_view_transition_definition(
                &mut utility_css,
                source,
                transition,
                &theme,
                &mut result.diagnostics,
            );
        }
        for call in module.style_calls {
            let mut branches = vec![(None, true, String::new(), Vec::new())];
            let mut attributes = Vec::new();
            let mut handle_markers = Vec::new();
            let mut external_classes = Vec::new();
            let mut dynamic_handle_classes = Vec::new();
            let mut dynamic_asset_classes = Vec::new();
            let mut view_transition_style = None;
            for input in call.inputs {
                if let StyleInput::ViewTransitionUse { transition, key } = input {
                    if view_transition_style.is_some() {
                        result.diagnostics.push(Diagnostic::error(
                            "SHVIEW004",
                            "style() accepts at most one view transition",
                            call.span,
                        ));
                        continue;
                    }
                    let name = asset_symbol_name(
                        &transition.module_path,
                        "view-transition",
                        &transition.name,
                    );
                    let branch = &mut branches[0];
                    branch.2.push_str("view-transition");
                    branch.3.push(view_transition_utility());
                    branch.2.push('\0');
                    view_transition_style = Some(ViewTransitionStyleEntry {
                        value: None,
                        identity: Some(name.clone()),
                        candidates: vec![name],
                        key,
                    });
                    continue;
                }
                if let StyleInput::DynamicViewTransitionUse {
                    value,
                    transitions,
                    key,
                } = input
                {
                    if view_transition_style.is_some() {
                        result.diagnostics.push(Diagnostic::error(
                            "SHVIEW004",
                            "style() accepts at most one view transition",
                            call.span,
                        ));
                        continue;
                    }
                    let mut candidates = Vec::new();
                    for transition in transitions {
                        runtime_assets.insert(IdentityId::new(
                            AssetKind::ViewTransition,
                            &transition.module_path,
                            transition.name.span.start.0,
                            transition.name.span.end.0,
                            transition.name.value.clone(),
                        ));
                        let name = asset_symbol_name(
                            &transition.module_path,
                            "view-transition",
                            &transition.name,
                        );
                        candidates.push(name);
                    }
                    let branch = &mut branches[0];
                    branch.2.push_str("view-transition");
                    branch.2.push('\0');
                    branch.3.push(view_transition_utility());
                    view_transition_style = Some(ViewTransitionStyleEntry {
                        value: Some(value),
                        identity: None,
                        candidates,
                        key,
                    });
                    continue;
                }
                if let StyleInput::AnimationUse { animation, options } = input {
                    let branch = &mut branches[0];
                    branch.2.push_str("animation:");
                    branch.2.push_str(&animation.name.value);
                    branch.2.push(':');
                    branch.2.push_str(&options.value);
                    branch.2.push('\0');
                    branch.3.push(resolve_animation_use(
                        &animation,
                        &options,
                        &theme,
                        &mut result.diagnostics,
                    ));
                    continue;
                }
                if let StyleInput::DynamicAnimationUse {
                    value,
                    animations,
                    options,
                } = input
                {
                    let mut classes = Vec::new();
                    for animation in animations {
                        runtime_assets.insert(IdentityId::new(
                            AssetKind::Animation,
                            &animation.module_path,
                            animation.name.span.start.0,
                            animation.name.span.end.0,
                            animation.name.value.clone(),
                        ));
                        let identity =
                            asset_symbol_name(&animation.module_path, "animation", &animation.name);
                        let canonical = format!("dynamic-animation:{identity}:{}", options.value);
                        let class_name = class_name(&canonical);
                        if emitted_utility_classes.insert((class_name.clone(), canonical.clone())) {
                            emit_utility_rules(
                                &mut utility_css,
                                &class_name,
                                &[resolve_animation_use(
                                    &animation,
                                    &options,
                                    &theme,
                                    &mut result.diagnostics,
                                )],
                                &theme,
                            );
                            result.symbols.push(generated_class_symbol(
                                source,
                                &class_name,
                                &canonical,
                                call.span,
                            ));
                        }
                        classes.push((identity, class_name));
                    }
                    dynamic_asset_classes.push(DynamicAssetClassEntry { value, classes });
                    continue;
                }
                if let StyleInput::SelectorAnimationUse {
                    selector,
                    animation,
                    options,
                } = input
                {
                    let branch = &mut branches[0];
                    branch.2.push_str(&selector.value);
                    branch.2.push('\0');
                    branch.2.push_str("animation:");
                    branch.2.push_str(&animation.name.value);
                    branch.2.push(':');
                    branch.2.push_str(&options.value);
                    branch.2.push('\0');
                    let mut utility = resolve_animation_use(
                        &animation,
                        &options,
                        &theme,
                        &mut result.diagnostics,
                    );
                    utility.selector = Some(selector.value);
                    branch.3.push(utility);
                    continue;
                }
                if let StyleInput::DynamicSelectorAnimationUse {
                    selector,
                    value,
                    animations,
                    options,
                } = input
                {
                    let mut classes = Vec::new();
                    for animation in animations {
                        runtime_assets.insert(IdentityId::new(
                            AssetKind::Animation,
                            &animation.module_path,
                            animation.name.span.start.0,
                            animation.name.span.end.0,
                            animation.name.value.clone(),
                        ));
                        let identity =
                            asset_symbol_name(&animation.module_path, "animation", &animation.name);
                        let canonical = format!(
                            "{}\0dynamic-animation:{identity}:{}",
                            selector.value, options.value
                        );
                        let class_name = class_name(&canonical);
                        let mut utility = resolve_animation_use(
                            &animation,
                            &options,
                            &theme,
                            &mut result.diagnostics,
                        );
                        utility.selector = Some(selector.value.clone());
                        if emitted_utility_classes.insert((class_name.clone(), canonical.clone())) {
                            emit_utility_rules(&mut utility_css, &class_name, &[utility], &theme);
                            result.symbols.push(generated_class_symbol(
                                source,
                                &class_name,
                                &canonical,
                                call.span,
                            ));
                        }
                        classes.push((identity, class_name));
                    }
                    dynamic_asset_classes.push(DynamicAssetClassEntry { value, classes });
                    continue;
                }
                if let StyleInput::ContainerQuery {
                    handle,
                    query,
                    value,
                } = input
                {
                    let branch = &mut branches[0];
                    branch.2.push_str("container:");
                    branch.2.push_str(&ref_symbol_name(&handle));
                    branch.2.push(':');
                    branch.2.push_str(&query.value);
                    branch.2.push('\0');
                    for utility in lex_utilities(&value) {
                        branch.2.push_str(&utility.value.raw);
                        branch.2.push('\0');
                        match resolve_utility_targets(&utility.value.raw, &theme) {
                            Some(mut utilities) => {
                                for utility in &mut utilities {
                                    utility.container = Some(format!(
                                        "{} ({})",
                                        ref_symbol_name(&handle),
                                        query.value
                                    ));
                                }
                                branch.3.append(&mut utilities);
                            }
                            None => result.diagnostics.push(Diagnostic::error(
                                "SH001",
                                format!("unknown or unavailable utility `{}`", utility.value.raw),
                                utility.span,
                            )),
                        }
                    }
                    continue;
                }
                if let StyleInput::HandleProjection { handle, projection } = input {
                    let branch = &mut branches[0];
                    let marker = handle_marker_class(&handle);
                    if !handle_markers.contains(&marker) {
                        handle_markers.push(marker);
                    }
                    branch.2.push_str("handle:");
                    branch.2.push_str(&ref_symbol_name(&handle));
                    branch.2.push(':');
                    branch.2.push_str(handle_projection_name(&projection));
                    branch.2.push('\0');
                    branch.3.push(CompiledUtility {
                        variants: Vec::new(),
                        declaration: render_handle_projection(&handle, &projection),
                        selector: None,
                        container: None,
                        nested: Vec::new(),
                    });
                    continue;
                }
                if let StyleInput::DynamicHandleProjection {
                    value,
                    handles,
                    projection,
                } = input
                {
                    dynamic_handle_classes.push(value);
                    for handle in handles {
                        runtime_assets.insert(ref_identity(&handle));
                        let class_name = runtime_projection_class(&handle, &projection);
                        let canonical = format!(
                            "dynamic-handle:{}:{}",
                            ref_symbol_name(&handle),
                            handle_projection_name(&projection)
                        );
                        if emitted_utility_classes.insert((class_name.clone(), canonical.clone())) {
                            emit_utility_rules(
                                &mut utility_css,
                                &class_name,
                                &[CompiledUtility {
                                    variants: Vec::new(),
                                    declaration: render_handle_projection(&handle, &projection),
                                    selector: None,
                                    container: None,
                                    nested: Vec::new(),
                                }],
                                &theme,
                            );
                            result.symbols.push(generated_class_symbol(
                                source,
                                &class_name,
                                &canonical,
                                call.span,
                            ));
                        }
                    }
                    continue;
                }
                if let StyleInput::ScopeAttachment(scope) = input {
                    let marker = scope_marker_class(&scope)
                        .trim_start_matches('.')
                        .to_owned();
                    if !handle_markers.contains(&marker) {
                        handle_markers.push(marker);
                    }
                    branches[0].2.push_str("scope:");
                    branches[0].2.push_str(&scope.name.value);
                    branches[0].2.push('\0');
                    continue;
                }
                if let StyleInput::HandleAttachment(handle) = input {
                    let marker = handle_marker_class(&handle);
                    if !handle_markers.contains(&marker) {
                        handle_markers.push(marker);
                    }
                    branches[0].2.push_str("ref:");
                    branches[0].2.push_str(&handle.name.value);
                    branches[0].2.push('\0');
                    continue;
                }
                if let StyleInput::DynamicScopeAttachment { value, scopes } = input {
                    dynamic_handle_classes.push(value);
                    runtime_assets.extend(scopes.iter().map(scope_identity));
                    continue;
                }
                if let StyleInput::DynamicHandleAttachment { value, handles } = input {
                    dynamic_handle_classes.push(value);
                    runtime_assets.extend(handles.iter().map(ref_identity));
                    continue;
                }
                if let StyleInput::ExternalClass(value) = input {
                    external_classes.push(value);
                    continue;
                }
                let (guard, guard_when_true, selector, input) = match input {
                    StyleInput::UtilityString(input) => (None, true, None, input),
                    StyleInput::ConditionalUtilityString {
                        guard,
                        when_true,
                        value,
                    } => (Some(guard), when_true, None, value),
                    StyleInput::Attributes(entries) => {
                        attributes.extend(entries);
                        continue;
                    }
                    StyleInput::SelectorUtilityString { selector, value } => {
                        (None, true, Some(selector.value), value)
                    }
                    StyleInput::ConditionalSelectorUtilityString {
                        selector,
                        guard,
                        when_true,
                        value,
                    } => (Some(guard), when_true, Some(selector.value), value),
                    StyleInput::HandleSelectorUtilityString {
                        handle,
                        selector,
                        value,
                    } => (
                        None,
                        true,
                        Some(handle_parent_selector(&handle, &selector.value)),
                        value,
                    ),
                    StyleInput::ConditionalHandleSelectorUtilityString {
                        handle,
                        selector,
                        guard,
                        when_true,
                        value,
                    } => (
                        Some(guard),
                        when_true,
                        Some(handle_parent_selector(&handle, &selector.value)),
                        value,
                    ),
                    StyleInput::HandleProjection { .. }
                    | StyleInput::DynamicHandleProjection { .. }
                    | StyleInput::ScopeAttachment(_)
                    | StyleInput::HandleAttachment(_)
                    | StyleInput::DynamicScopeAttachment { .. }
                    | StyleInput::DynamicHandleAttachment { .. }
                    | StyleInput::ExternalClass(_)
                    | StyleInput::ContainerQuery { .. }
                    | StyleInput::AnimationUse { .. }
                    | StyleInput::DynamicAnimationUse { .. }
                    | StyleInput::SelectorAnimationUse { .. }
                    | StyleInput::DynamicSelectorAnimationUse { .. }
                    | StyleInput::ViewTransitionUse { .. }
                    | StyleInput::DynamicViewTransitionUse { .. } => {
                        unreachable!("handled above")
                    }
                };
                let branch_index = if let Some(guard) = guard {
                    branches.push((Some(guard), guard_when_true, String::new(), Vec::new()));
                    branches.len() - 1
                } else {
                    0
                };
                let branch = &mut branches[branch_index];
                for utility in lex_utilities(&input) {
                    if let Some(selector) = &selector {
                        branch.2.push_str(selector);
                        branch.2.push('\0');
                    }
                    branch.2.push_str(&utility.value.raw);
                    branch.2.push('\0');
                    match resolve_utility_targets(&utility.value.raw, &theme) {
                        Some(mut utilities) => {
                            for utility in &mut utilities {
                                if let Some(selector) = &selector {
                                    utility.selector =
                                        Some(utility.selector.as_deref().map_or_else(
                                            || selector.clone(),
                                            |nested| nested.replace('&', selector),
                                        ));
                                }
                            }
                            branch.3.append(&mut utilities);
                        }
                        None => result.diagnostics.push(Diagnostic::error(
                            "SH001",
                            format!("unknown or unavailable utility `{}`", utility.value.raw),
                            utility.span,
                        )),
                    }
                }
            }

            let mut call_classes = Vec::new();
            for (guard, guard_when_true, canonical_input, utilities) in branches {
                if utilities.is_empty() {
                    continue;
                }
                let class_name = class_name(&canonical_input);
                if emitted_utility_classes.insert((class_name.clone(), canonical_input.clone())) {
                    emit_utility_rules(&mut utility_css, &class_name, &utilities, &theme);
                    result.symbols.push(generated_class_symbol(
                        source,
                        &class_name,
                        &canonical_input,
                        call.span,
                    ));
                }
                let generated_class = GeneratedClass {
                    class_name,
                    call_span: call.span,
                    guard,
                    guard_when_true,
                };
                call_classes.push(generated_class.clone());
                result.classes.push(generated_class);
            }
            if let Some(replacement) = render_react_props(
                source,
                &ReactPropsParts {
                    classes: &call_classes,
                    attributes: &attributes,
                    handle_markers: &handle_markers,
                    external_classes: &external_classes,
                    dynamic_handle_classes: &dynamic_handle_classes,
                    dynamic_asset_classes: &dynamic_asset_classes,
                    view_transition_style: view_transition_style.as_ref(),
                },
            ) {
                result.transforms.push(PropsTransform {
                    call_span: call.span,
                    replacement,
                });
            }
        }
        for handle_definition in module.handle_definitions {
            result.transforms.push(PropsTransform {
                call_span: handle_definition.call_span,
                replacement: if runtime_assets.contains(&ref_identity(&handle_definition)) {
                    format!(
                        "stillwindRef(\"{}\")",
                        handle_marker_class(&handle_definition)
                    )
                } else {
                    "undefined".to_owned()
                },
            });
        }
        for scope_definition in module.scope_definitions {
            result.transforms.push(PropsTransform {
                call_span: scope_definition.call_span,
                replacement: if runtime_assets.contains(&scope_identity(&scope_definition)) {
                    format!(
                        "stillwindScope(\"{}\")",
                        scope_marker_class(&scope_definition).trim_start_matches('.')
                    )
                } else {
                    "undefined".to_owned()
                },
            });
        }
        for animation in module.animation_declarations {
            result.transforms.push(PropsTransform {
                call_span: animation.call_span,
                replacement: if runtime_assets.contains(&named_identity(
                    source,
                    AssetKind::Animation,
                    &animation.name,
                )) {
                    format!(
                        "stillwindAnimation(\"{}\")",
                        symbol_name(source, "animation", &animation.name)
                    )
                } else {
                    "undefined".to_owned()
                },
            });
        }
        for transition in module.view_transition_declarations {
            result.transforms.push(PropsTransform {
                call_span: transition.call_span,
                replacement: if runtime_assets.contains(&named_identity(
                    source,
                    AssetKind::ViewTransition,
                    &transition.name,
                )) {
                    format!(
                        "stillwindViewTransition(\"{}\")",
                        symbol_name(source, "view-transition", &transition.name)
                    )
                } else {
                    "undefined".to_owned()
                },
            });
        }

        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == stillwind_source::Severity::Error)
        {
            result.css.clear();
            result.classes.clear();
            result.symbols.clear();
            result.transforms.clear();
        } else if !utility_css.is_empty() {
            let utility_css = options.rewrite_generated(&utility_css);
            result.module_css.clone_from(&utility_css);
            result.css = render_stylesheet(&theme, &result.module_css, options);
        }

        for transform in &mut result.transforms {
            transform.replacement = options.rewrite_generated(&transform.replacement);
        }
        for symbol in &mut result.symbols {
            symbol.name = options.rewrite_generated(&symbol.name);
        }

        result
    }
}

fn handle_projection_name(projection: &HandleProjection) -> &'static str {
    match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(_) => "container",
    }
}

fn runtime_projection_class(
    handle: &stillwind_ir::HandleDefinition,
    projection: &HandleProjection,
) -> String {
    let suffix = match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(kind) => kind.value.as_str(),
    };
    format!("_swp_{}_{suffix}", handle_marker_class(handle))
}

fn symbol_identity(source: &SourceFile, kind: &str, definition: &Spanned<String>) -> String {
    format!(
        "stillwind/v1\0{}\0{kind}\0{}:{}:{}",
        normalized_module_path(source.path()),
        definition.span.start.0,
        definition.span.end.0,
        definition.value,
    )
}

fn asset_identity(module_path: &str, kind: &str, definition: &Spanned<String>) -> String {
    format!(
        "stillwind/v1\0{}\0{kind}\0{}:{}:{}",
        normalized_module_path(module_path),
        definition.span.start.0,
        definition.span.end.0,
        definition.value,
    )
}

fn ref_symbol_name(definition: &stillwind_ir::HandleDefinition) -> String {
    let identity = asset_identity(&definition.module_path, "ref", &definition.name);
    format!(
        "--sw-r-{}",
        &blake3::hash(identity.as_bytes()).to_hex()[..8]
    )
}

fn symbol_name(source: &SourceFile, kind: &str, definition: &Spanned<String>) -> String {
    asset_symbol_name(source.path(), kind, definition)
}

fn asset_symbol_name(module_path: &str, kind: &str, definition: &Spanned<String>) -> String {
    let prefix = match kind {
        "ref" => "--sw-r-",
        "scope" => "--sw-s-",
        "animation" => "--sw-a-",
        "view-transition" => "--sw-v-",
        _ => unreachable!("only named Stillwind asset kinds are allocated"),
    };
    let identity = asset_identity(module_path, kind, definition);
    let hash = blake3::hash(identity.as_bytes());
    format!("{prefix}{}", &hash.to_hex()[..8])
}

fn generated_symbol(
    source: &SourceFile,
    kind: &str,
    definition: &Spanned<String>,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: symbol_name(source, kind, definition),
        kind: kind.to_owned(),
        identity: symbol_identity(source, kind, definition),
        source_path: normalized_module_path(source.path()),
        start: definition.span.start.0,
        end: definition.span.end.0,
    }
}

fn generated_ref_symbol(definition: &stillwind_ir::HandleDefinition) -> GeneratedSymbol {
    let identity = asset_identity(&definition.module_path, "ref", &definition.name);
    GeneratedSymbol {
        name: ref_symbol_name(definition),
        kind: "ref".to_owned(),
        identity,
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
}

fn generated_scope_symbol(definition: &stillwind_ir::ScopeDefinition) -> GeneratedSymbol {
    let identity = asset_identity(&definition.module_path, "scope", &definition.name);
    GeneratedSymbol {
        name: format!(
            "--sw-s-{}",
            &blake3::hash(identity.as_bytes()).to_hex()[..8]
        ),
        kind: "scope".to_owned(),
        identity,
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
}

fn generated_ref_marker_symbol(definition: &stillwind_ir::HandleDefinition) -> GeneratedSymbol {
    GeneratedSymbol {
        name: handle_marker_class(definition),
        kind: "ref-marker".to_owned(),
        identity: asset_identity(&definition.module_path, "ref-marker", &definition.name),
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
}

fn generated_scope_marker_symbol(definition: &stillwind_ir::ScopeDefinition) -> GeneratedSymbol {
    GeneratedSymbol {
        name: scope_marker_class(definition)
            .trim_start_matches('.')
            .to_owned(),
        kind: "scope-marker".to_owned(),
        identity: asset_identity(&definition.module_path, "scope-marker", &definition.name),
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
}

fn generated_class_symbol(
    source: &SourceFile,
    name: &str,
    canonical_input: &str,
    span: Span,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: name.to_owned(),
        kind: "class".to_owned(),
        identity: format!("stillwind/v1\0class\0{canonical_input}"),
        source_path: normalized_module_path(source.path()),
        start: span.start.0,
        end: span.end.0,
    }
}

fn emit_animation_definition(
    output: &mut String,
    source: &SourceFile,
    animation: &AnimationDefinition,
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    output.push_str("@keyframes ");
    output.push_str(&symbol_name(source, "animation", &animation.name));
    output.push('{');
    for frame in &animation.frames {
        let mut utilities = Vec::new();
        for utility in lex_utilities(&frame.utilities) {
            match resolve_utility(&utility.value.raw, theme) {
                Some(utility) if utility.variants.is_empty() => utilities.push(utility),
                Some(_) => diagnostics.push(Diagnostic::error(
                    "SHANIMATION003",
                    "animation keyframes cannot contain variants",
                    utility.span,
                )),
                None => diagnostics.push(Diagnostic::error(
                    "SH001",
                    format!("unknown or unavailable utility `{}`", utility.value.raw),
                    utility.span,
                )),
            }
        }
        output.push_str(&frame.selector.value);
        output.push('{');
        output.push_str(&combine_declarations(&utilities.iter().collect::<Vec<_>>()));
        output.push('}');
    }
    output.push('}');
}

fn resolve_animation_use(
    animation: &AssetReference,
    options: &Spanned<String>,
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> CompiledUtility {
    let mut declaration = format!(
        "animation-name:{};",
        asset_symbol_name(&animation.module_path, "animation", &animation.name)
    );
    declaration.push_str(&resolve_motion_options(options, theme, diagnostics));
    CompiledUtility {
        variants: Vec::new(),
        declaration,
        selector: None,
        container: None,
        nested: Vec::new(),
    }
}

fn resolve_motion_options(
    options: &Spanned<String>,
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let mut declaration = String::new();
    for option in lex_utilities(options) {
        match option.value.raw.as_str() {
            "none" | "forwards" | "backwards" | "both" => {
                write!(
                    &mut declaration,
                    "animation-fill-mode:{};",
                    option.value.raw
                )
                .expect("writing to a String cannot fail");
            }
            "running" | "paused" => {
                write!(
                    &mut declaration,
                    "animation-play-state:{};",
                    option.value.raw
                )
                .expect("writing to a String cannot fail");
            }
            "normal" | "reverse" | "alternate" | "alternate-reverse" => {
                write!(
                    &mut declaration,
                    "animation-direction:{};",
                    option.value.raw
                )
                .expect("writing to a String cannot fail");
            }
            "infinite" => declaration.push_str("animation-iteration-count:infinite;"),
            value => match resolve_utility(value, theme) {
                Some(utility)
                    if utility.variants.is_empty()
                        && (utility.declaration.starts_with("transition-duration:")
                            || utility
                                .declaration
                                .starts_with("transition-timing-function:")) =>
                {
                    declaration.push_str(&utility.declaration.replace("transition-", "animation-"));
                }
                _ => diagnostics.push(Diagnostic::error(
                    "SHANIMATION002",
                    format!("unknown animation option `{value}`"),
                    option.span,
                )),
            },
        }
    }
    declaration
}

fn emit_view_transition_definition(
    output: &mut String,
    source: &SourceFile,
    transition: &ViewTransitionDefinition,
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = symbol_name(source, "view-transition", &transition.name);
    if let Some(group) = &transition.group {
        output.push_str("::view-transition-group(");
        output.push('.');
        output.push_str(&name);
        output.push_str("){ ");
        output.push_str(&resolve_motion_options(group, theme, diagnostics));
        output.push('}');
    }
    for (pseudo, utilities) in [
        ("old", transition.old.as_ref()),
        ("new", transition.new.as_ref()),
    ] {
        let Some(utilities) = utilities else {
            continue;
        };
        output.push_str("::view-transition-");
        output.push_str(pseudo);
        output.push('(');
        output.push('.');
        output.push_str(&name);
        output.push_str("){ ");
        output.push_str(&resolve_static_utility_list(utilities, theme, diagnostics));
        output.push('}');
    }
}

fn resolve_static_utility_list(
    input: &Spanned<String>,
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let mut utilities = Vec::new();
    for utility in lex_utilities(input) {
        match resolve_utility(&utility.value.raw, theme) {
            Some(utility) if utility.variants.is_empty() => utilities.push(utility),
            Some(_) => diagnostics.push(Diagnostic::error(
                "SHMOTION001",
                "motion definition utilities cannot contain variants",
                utility.span,
            )),
            None => diagnostics.push(Diagnostic::error(
                "SH001",
                format!("unknown or unavailable utility `{}`", utility.value.raw),
                utility.span,
            )),
        }
    }
    combine_declarations(&utilities.iter().collect::<Vec<_>>())
}

fn render_handle_projection(
    handle: &stillwind_ir::HandleDefinition,
    projection: &HandleProjection,
) -> String {
    let name = ref_symbol_name(handle);
    match projection {
        HandleProjection::Anchor => format!("anchor-name:{name};"),
        HandleProjection::Position => format!("position-anchor:{name};"),
        HandleProjection::Scope => format!("anchor-scope:{name};"),
        HandleProjection::Container(kind) => {
            format!("container-name:{name};container-type:{};", kind.value)
        }
    }
}

fn handle_marker_class(handle: &stillwind_ir::HandleDefinition) -> String {
    let hash = &blake3::hash(asset_identity(&handle.module_path, "ref", &handle.name).as_bytes())
        .to_hex()[..8];
    match &handle.label {
        Some(label) => format!("s_{}_{}", label.value, hash),
        None => format!("s_{hash}"),
    }
}

fn handle_parent_selector(handle: &stillwind_ir::HandleDefinition, selector: &str) -> String {
    let marker = handle_marker_class(handle);
    let reference = match selector.strip_prefix('&') {
        Some("") => format!(".{marker}"),
        Some(suffix) => format!(".{marker}:where({suffix})"),
        None => unreachable!("the extractor requires ref selectors to begin with `&`"),
    };
    match &handle.scope {
        Some(scope) => format!("{}:has({reference}) &", scope_marker_class(scope)),
        None => format!("{reference} &"),
    }
}

fn scope_marker_class(scope: &stillwind_ir::ScopeDefinition) -> String {
    let hash = &blake3::hash(asset_identity(&scope.module_path, "scope", &scope.name).as_bytes())
        .to_hex()[..8];
    match &scope.label {
        Some(label) => format!(".s_{}_{}", label.value, hash),
        None => format!(".s_{hash}"),
    }
}

fn ref_identity(handle: &stillwind_ir::HandleDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Ref,
        &handle.module_path,
        handle.name.span.start.0,
        handle.name.span.end.0,
        handle.name.value.clone(),
    )
}

fn scope_identity(scope: &stillwind_ir::ScopeDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Scope,
        &scope.module_path,
        scope.name.span.start.0,
        scope.name.span.end.0,
        scope.name.value.clone(),
    )
}

fn named_identity(source: &SourceFile, kind: AssetKind, name: &Spanned<String>) -> IdentityId {
    IdentityId::new(
        kind,
        source.path(),
        name.span.start.0,
        name.span.end.0,
        name.value.clone(),
    )
}

fn summary_runtime_assets(summary: &stillwind_semantic::ModuleSummary) -> BTreeSet<IdentityId> {
    let mut identities = BTreeSet::new();
    for value in summary.exports.values() {
        match value {
            ExportedValue::Refs(alternatives) => {
                for alternative in alternatives {
                    identities.insert(alternative.reference.clone());
                    if let Some(scopes) = &alternative.permitted_scopes
                        && let Candidates::Finite(scopes) = &scopes.candidates
                    {
                        identities.extend(scopes.iter().cloned());
                    }
                }
            }
            ExportedValue::Scopes(value)
            | ExportedValue::Animations(value)
            | ExportedValue::ViewTransitions(value) => {
                if let Candidates::Finite(candidates) = &value.candidates {
                    identities.extend(candidates.iter().cloned());
                }
            }
        }
    }
    identities
}

fn render_react_props(source: &SourceFile, parts: &ReactPropsParts<'_>) -> Option<String> {
    let ReactPropsParts {
        classes,
        attributes,
        handle_markers,
        external_classes,
        dynamic_handle_classes,
        dynamic_asset_classes,
        view_transition_style,
    } = *parts;
    let mut entries = Vec::new();
    if !classes.is_empty()
        || !handle_markers.is_empty()
        || !external_classes.is_empty()
        || !dynamic_handle_classes.is_empty()
        || !dynamic_asset_classes.is_empty()
    {
        let mut class_names = handle_markers
            .iter()
            .map(|marker| format!("\"{marker}\""))
            .collect::<Vec<_>>();
        class_names.extend(
            classes
                .iter()
                .map(|class| match class.guard {
                    Some(guard) => source.slice(guard).ok().map(|guard| {
                        let guard = if class.guard_when_true {
                            guard.to_owned()
                        } else {
                            format!("!{guard}")
                        };
                        format!("{guard}&&\"{}\"", class.class_name)
                    }),
                    None => Some(format!("\"{}\"", class.class_name)),
                })
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            external_classes
                .iter()
                .map(|value| source.slice(*value).ok().map(str::to_owned))
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            dynamic_handle_classes
                .iter()
                .map(|value| {
                    source
                        .slice(*value)
                        .ok()
                        .map(|value| format!("stillwindHandleClass({value})"))
                })
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            dynamic_asset_classes
                .iter()
                .map(|entry| {
                    let value = source.slice(entry.value).ok()?;
                    let classes = entry
                        .classes
                        .iter()
                        .map(|(identity, class_name)| format!("\"{identity}\":\"{class_name}\""))
                        .collect::<Vec<_>>()
                        .join(",");
                    Some(format!("stillwindAssetClass({value},{{{classes}}})"))
                })
                .collect::<Option<Vec<_>>>()?,
        );
        let class_value = if external_classes.is_empty() && class_names.len() == 1 {
            class_names.into_iter().next()?
        } else {
            format!("stillwindClsx({})", class_names.join(","))
        };
        entries.push(format!("className:{class_value}"));
    }
    for attribute in attributes {
        let value = source.slice(attribute.value).ok()?;
        entries.push(format!("\"{}\":{value}", attribute.name.value));
    }
    if let Some(entry) = view_transition_style {
        let value = match (entry.value, &entry.identity) {
            (Some(value), None) => source.slice(value).ok()?.to_owned(),
            (None, Some(identity)) => format!("\"{identity}\""),
            _ => return None,
        };
        let key = match entry.key {
            Some(key) => source.slice(key).ok()?.to_owned(),
            None => "undefined".to_owned(),
        };
        let candidates = entry
            .candidates
            .iter()
            .map(|candidate| format!("\"{candidate}\""))
            .collect::<Vec<_>>()
            .join(",");
        entries.push(format!(
            "style:stillwindViewTransitionStyle({value},{key},[{candidates}],\"--sw-vt-name\",\"--sw-vt-class\")"
        ));
    }
    Some(format!("{{{}}}", entries.join(",")))
}

fn view_transition_utility() -> CompiledUtility {
    CompiledUtility {
        variants: Vec::new(),
        declaration: "--sw-vt-name:none;--sw-vt-class:none;view-transition-name:var(--sw-vt-name);view-transition-class:var(--sw-vt-class);".to_owned(),
        selector: None,
        container: None,
        nested: Vec::new(),
    }
}

#[derive(Clone, Debug)]
struct CompiledUtility {
    variants: Vec<Variant>,
    declaration: String,
    selector: Option<String>,
    container: Option<String>,
    nested: Vec<stillwind_tailwind::NestedUtilityRule>,
}

fn resolve_utility(utility: &str, theme: &stillwind_css::ThemeIndex) -> Option<CompiledUtility> {
    resolve_utility_targets(utility, theme)?.into_iter().next()
}

fn resolve_utility_targets(
    utility: &str,
    theme: &stillwind_css::ThemeIndex,
) -> Option<Vec<CompiledUtility>> {
    let resolved = stillwind_tailwind::resolve_utility(utility, theme)?;
    let mut targets = vec![stillwind_tailwind::ResolvedUtilityTarget {
        declaration: resolved.declaration,
        selector: resolved.selector,
        nested: resolved.nested,
    }];
    targets.extend(resolved.additional);
    Some(
        targets
            .into_iter()
            .map(|target| CompiledUtility {
                variants: resolved.candidate.variants.clone(),
                declaration: target.declaration,
                selector: target.selector,
                container: None,
                nested: target.nested,
            })
            .collect(),
    )
}

fn emit_utility_rules(
    output: &mut String,
    class_name: &str,
    utilities: &[CompiledUtility],
    theme: &stillwind_css::ThemeIndex,
) {
    let base = utilities
        .iter()
        .filter(|utility| {
            utility.variants.is_empty() && utility.selector.is_none() && utility.container.is_none()
        })
        .collect::<Vec<_>>();
    let base = combine_declarations(&base);
    if !base.is_empty() {
        output.push('.');
        output.push_str(class_name);
        output.push('{');
        output.push_str(&base);
        output.push('}');
    }
    for utility in utilities.iter().filter(|utility| {
        !utility.variants.is_empty() || utility.selector.is_some() || utility.container.is_some()
    }) {
        output.push_str(&render_utility_rule(
            class_name,
            utility,
            theme,
            &combine_declarations(&[utility]),
            None,
        ));
    }
    for utility in utilities {
        for nested in &utility.nested {
            output.push_str(&render_utility_rule(
                class_name,
                utility,
                theme,
                &nested.declaration,
                Some(nested),
            ));
        }
    }
}

fn render_utility_rule(
    class_name: &str,
    utility: &CompiledUtility,
    theme: &stillwind_css::ThemeIndex,
    declaration: &str,
    nested_rule: Option<&stillwind_tailwind::NestedUtilityRule>,
) -> String {
    let class_selector = format!(".{class_name}");
    let mut selector = utility.selector.as_deref().map_or_else(
        || class_selector.clone(),
        |selector| selector.replace('&', &class_selector),
    );
    let mut media = Vec::new();
    let mut containers = Vec::new();
    let mut supports = Vec::new();
    let mut starting_style = false;
    for variant in &utility.variants {
        match stillwind_tailwind::resolve_variant(variant, theme) {
            Some(stillwind_tailwind::VariantApplication::SelectorSuffix(suffix)) => {
                selector.push_str(suffix);
            }
            Some(stillwind_tailwind::VariantApplication::SelectorTemplate(template)) => {
                selector = template.replace('&', &selector);
            }
            Some(stillwind_tailwind::VariantApplication::SelectorMedia {
                selector: template,
                condition,
            }) => {
                selector = template.replace('&', &selector);
                media.push(condition);
            }
            Some(stillwind_tailwind::VariantApplication::Media(condition)) => {
                media.push(condition);
            }
            Some(stillwind_tailwind::VariantApplication::Container(condition)) => {
                containers.push(condition);
            }
            Some(stillwind_tailwind::VariantApplication::Supports(condition)) => {
                supports.push(condition);
            }
            Some(stillwind_tailwind::VariantApplication::StartingStyle) => {
                starting_style = true;
            }
            None => {}
        }
    }
    let mut rule = format!("{selector}{{{declaration}}}");
    if let Some(nested) = nested_rule {
        rule = match nested.at_rule {
            stillwind_tailwind::NestedUtilityAtRule::Media => {
                format!("@media {}{{{rule}}}", nested.condition)
            }
            stillwind_tailwind::NestedUtilityAtRule::Supports => {
                format!("@supports ({}){{{rule}}}", nested.condition)
            }
        };
    }
    for condition in supports.into_iter().rev() {
        rule = format!("@supports ({condition}){{{rule}}}");
    }
    for condition in media.into_iter().rev() {
        rule = format!("@media {condition}{{{rule}}}");
    }
    for condition in containers.into_iter().rev() {
        rule = format!("@container {condition}{{{rule}}}");
    }
    if starting_style {
        rule = format!("@starting-style{{{rule}}}");
    }
    if let Some(container) = &utility.container {
        rule = format!("@container {container}{{{rule}}}");
    }
    rule
}

fn combine_declarations(utilities: &[&CompiledUtility]) -> String {
    let mut declarations = String::new();
    let mut transforms = Vec::new();
    for utility in utilities {
        for declaration in utility.declaration.split_terminator(';') {
            if let Some(transform) = declaration.strip_prefix("transform:") {
                transforms.push(transform);
            } else if !declaration.is_empty() {
                declarations.push_str(declaration);
                declarations.push(';');
            }
        }
    }
    if !transforms.is_empty() {
        declarations.push_str("transform:");
        declarations.push_str(&transforms.join(" "));
        declarations.push(';');
    }
    declarations
}

fn render_stylesheet(
    theme: &stillwind_css::ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
) -> String {
    let imports = &theme.imports;
    // Tailwind only emits typed custom-property registrations when a generated
    // utility actually participates in one of those protocols. An import is
    // permission to use the layer, not a request to eagerly dump it.
    let properties =
        imports.properties || (imports.has_any() && utility_css.contains(&options.tailwind_prefix));
    let mut css = String::new();
    if properties {
        css.push_str("@layer properties;\n");
    }
    if imports.has_any() {
        css.push_str("@layer theme, base, components, utilities;\n");
    }
    if let Some(layer) = &imports.theme_layer {
        css.push_str("@layer ");
        css.push_str(layer);
        css.push_str("{:root,:host{");
        let required_tokens = theme.tokens_required_by(utility_css, imports.base_layer.is_some());
        for token in &required_tokens {
            css.push_str(&token.name.value);
            css.push(':');
            css.push_str(&token.value.value.replace("--theme(", "var("));
            css.push(';');
        }
        css.push_str("}}\n");
        css.push_str("@layer ");
        css.push_str(layer);
        css.push('{');
        let animations = required_tokens
            .iter()
            .filter_map(|token| match &token.kind {
                stillwind_css::ThemeTokenKind::Token {
                    namespace: stillwind_css::ThemeNamespace::Animate,
                    ..
                } => token
                    .value
                    .value
                    .split_whitespace()
                    .next()
                    .map(str::to_owned),
                _ => None,
            })
            .collect::<Vec<_>>();
        css.push_str(
            &options.rewrite_generated(&stillwind_css::tailwind_v4_keyframes_for(&animations)),
        );
        css.push_str("}\n");
    }
    if let Some(layer) = &imports.base_layer {
        css.push_str("@layer ");
        css.push_str(layer);
        css.push('{');
        css.push_str(&options.rewrite_generated(&stillwind_css::tailwind_v4_preflight()));
        css.push('}');
    }
    if !theme.custom_css().trim().is_empty() {
        css.push('\n');
        css.push_str(theme.custom_css());
        css.push('\n');
    }
    if let Some(layer) = &imports.utilities_layer {
        css.push_str("\n@layer ");
        css.push_str(layer);
        css.push('{');
        css.push_str(utility_css);
        css.push_str("}\n");
    }
    if properties {
        css.push_str(&options.rewrite_generated(stillwind_css::tailwind_v4_properties()));
    }
    css
}

fn render_react_types(theme: &stillwind_css::ThemeIndex) -> String {
    let mut utilities = vec![
        "absolute",
        "block",
        "cursor-pointer",
        "cursor-wait",
        "flex",
        "grid",
        "hidden",
        "inline-block",
        "inline-flex",
        "items-center",
        "items-end",
        "items-start",
        "justify-between",
        "justify-center",
        "opacity-0",
        "opacity-60",
        "opacity-100",
        "overflow-hidden",
        "relative",
        "text-center",
        "text-left",
        "transition",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for token in &theme.tokens {
        let Some((namespace, name)) = token.name.value.trim_start_matches("--").split_once('-')
        else {
            continue;
        };
        for (prefix, expected_namespace) in [
            ("bg", "color"),
            ("text", "color"),
            ("border", "color"),
            ("p", "spacing"),
            ("px", "spacing"),
            ("py", "spacing"),
            ("m", "spacing"),
            ("mx", "spacing"),
            ("my", "spacing"),
            ("gap", "spacing"),
            ("w", "spacing"),
            ("h", "spacing"),
            ("rounded", "radius"),
            ("shadow", "shadow"),
            ("duration", "duration"),
            ("ease", "ease"),
        ] {
            if namespace == expected_namespace {
                utilities.push(format!("{prefix}-{name}"));
            }
        }
    }
    utilities.sort();
    utilities.dedup();
    let entries = utilities
        .into_iter()
        .map(|utility| format!("    \"{utility}\": true;"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut variants = vec![
        "active",
        "dark",
        "disabled",
        "focus",
        "focus-visible",
        "hover",
    ];
    variants.extend(
        theme
            .tokens
            .iter()
            .filter_map(|token| token.name.value.strip_prefix("--breakpoint-")),
    );
    variants.sort_unstable();
    variants.dedup();
    let variants = variants
        .into_iter()
        .map(|variant| format!("    \"{variant}\": true;"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "// Generated by Stillwind from @theme. Do not edit.\nimport \"stillwind/react\";\n\ndeclare module \"stillwind/react\" {{\n  interface StillwindUtilityVocabulary {{\n{entries}\n  }}\n  interface StillwindVariantVocabulary {{\n{variants}\n  }}\n}}\n\nexport {{}};\n"
    )
}

fn class_name(canonical_input: &str) -> String {
    let hash = blake3::hash(canonical_input.as_bytes());
    format!("_s_{}", &hash.to_hex()[..8])
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn exposes_the_first_frontends_through_one_compiler_api() {
        let js = SourceFile::new(FileId(1), "App.tsx", "export const App = () => <main />;");
        let css = SourceFile::new(
            FileId(2),
            "stillwind.css",
            "@theme { --color-accent: red; }",
        );

        assert!(Compiler::check_javascript(&js).is_empty());
        let theme = Compiler::read_theme(&css).unwrap();
        assert_eq!(theme.value("color", "accent"), Some("red"));
        assert!(theme.token("color", "red-500").is_some());
    }

    #[test]
    fn generates_types_from_the_css_theme_vocabulary() {
        let css = SourceFile::new(
            FileId(2),
            "stillwind.css",
            "@theme { --color-accent: red; --spacing-4: 1rem; }",
        );
        let types = Compiler::generate_react_types(&css).unwrap();

        assert!(types.contains("\"bg-accent\": true"));
        assert!(types.contains("\"px-4\": true"));
    }
}
