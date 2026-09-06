//! Public orchestration API for the first compiler slice.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use serde::{Deserialize, Serialize};
use stillwind_grammar::{Variant, lex_utilities, parse_candidate};
use stillwind_ir::{
    AnimationDefinition, AssetReference, AttributeEntry, HandleProjection, StyleInput,
    ViewTransitionDefinition,
};
use stillwind_semantic::{
    AssetKind, AssetMetadata, Candidates, ExportedValue, IdentityId, TypeFact,
};
use stillwind_source::{
    CompileError, LintFinding, SourceFile, Span, Spanned, filter_lint_findings,
    normalized_module_path,
};

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
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub lint_policy: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    pub prefix: String,
    pub tailwind_prefix: String,
    pub forbid_tailwind_relationships: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            prefix: "--sw-".to_owned(),
            tailwind_prefix: "--tw-".to_owned(),
            forbid_tailwind_relationships: false,
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
    pub fn check_javascript(source: &SourceFile) -> Vec<CompileError> {
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
        let mut options = options.clone();
        if let Some(prefix) = &theme.config.prefix {
            options.prefix.clone_from(prefix);
        }
        if let Some(prefix) = &theme.config.tailwind_prefix {
            options.tailwind_prefix.clone_from(prefix);
        }
        Ok(render_stylesheet(&theme, module_css, &options))
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
            lint_findings: parsed.lint_findings,
            ..CompileResult::default()
        };
        let theme = match Self::read_theme(theme_source) {
            Ok(theme) => theme,
            Err(error) => {
                result.diagnostics.push(CompileError::new(
                    "SHCSS001",
                    error.to_string(),
                    theme_source.full_span(),
                ));
                return result;
            }
        };
        let mut options = options.clone();
        if let Some(prefix) = &theme.config.prefix {
            options.prefix.clone_from(prefix);
        }
        if let Some(prefix) = &theme.config.tailwind_prefix {
            options.tailwind_prefix.clone_from(prefix);
        }
        result.lint_policy.clone_from(&theme.config.lint_rules);
        filter_lint_findings(source, &mut result.lint_findings);

        if !result.diagnostics.is_empty() {
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
            let mut compatibility_classes = Vec::new();
            for input in call.inputs {
                if let StyleInput::ViewTransitionUse { transition, key } = input {
                    if view_transition_style.is_some() {
                        result.diagnostics.push(CompileError::new(
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
                        result.diagnostics.push(CompileError::new(
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
                        if let Some(mut utilities) =
                            resolve_utility_targets(&utility.value.raw, &theme)
                        {
                            {
                                for utility in &mut utilities {
                                    utility.container = Some(format!(
                                        "{} ({})",
                                        ref_symbol_name(&handle),
                                        query.value
                                    ));
                                }
                                branch.3.append(&mut utilities);
                            }
                        } else {
                            result.lint_findings.push(LintFinding::new(
                                unresolved_utility_rule(&utility.value.raw),
                                format!("unknown or unavailable utility `{}`", utility.value.raw),
                                utility.span,
                            ));
                            external_classes.push(utility.span);
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
                    if (options.forbid_tailwind_relationships
                        || theme.config.lint_enabled("prefer-typed-relationships"))
                        && stillwind_tailwind::is_tailwind_relationship_candidate(
                            &utility.value.raw,
                        )
                    {
                        result.lint_findings.push(LintFinding::new(
                            "prefer-typed-relationships",
                            format!(
                                "Tailwind relationship utility `{}` is forbidden; use style.ref() or style.scope()",
                                utility.value.raw
                            ),
                            utility.span,
                        ));
                    }
                    if selector.is_none()
                        && let Some(marker) =
                            stillwind_tailwind::compatibility_marker_class(&utility.value.raw)
                    {
                        let class = GeneratedClass {
                            class_name: marker,
                            call_span: call.span,
                            guard,
                            guard_when_true,
                        };
                        if !compatibility_classes.contains(&class) {
                            compatibility_classes.push(class);
                        }
                        continue;
                    }
                    if let Some(selector) = &selector {
                        branch.2.push_str(selector);
                        branch.2.push('\0');
                    }
                    branch.2.push_str(&utility.value.raw);
                    branch.2.push('\0');
                    if let Some(mut utilities) = resolve_utility_targets(&utility.value.raw, &theme)
                    {
                        {
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
                    } else {
                        result.lint_findings.push(LintFinding::new(
                            unresolved_utility_rule(&utility.value.raw),
                            format!("unknown or unavailable utility `{}`", utility.value.raw),
                            utility.span,
                        ));
                        if selector.is_none() {
                            external_classes.push(utility.span);
                        }
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
            for class in compatibility_classes {
                call_classes.push(class.clone());
                result.classes.push(class);
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

        if !result.diagnostics.is_empty() {
            result.css.clear();
            result.classes.clear();
            result.symbols.clear();
            result.transforms.clear();
        } else if !utility_css.is_empty() {
            let utility_css = options.rewrite_generated(&utility_css);
            result.module_css.clone_from(&utility_css);
            result.css = render_stylesheet(&theme, &result.module_css, &options);
        }

        for transform in &mut result.transforms {
            transform.replacement = options.rewrite_generated(&transform.replacement);
        }
        for symbol in &mut result.symbols {
            symbol.name = options.rewrite_generated(&symbol.name);
        }

        filter_lint_findings(source, &mut result.lint_findings);

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
    diagnostics: &mut Vec<CompileError>,
) {
    output.push_str("@keyframes ");
    output.push_str(&symbol_name(source, "animation", &animation.name));
    output.push('{');
    for frame in &animation.frames {
        let mut utilities = Vec::new();
        for utility in lex_utilities(&frame.utilities) {
            match resolve_utility(&utility.value.raw, theme) {
                Some(utility) if utility.variants.is_empty() => utilities.push(utility),
                Some(_) => diagnostics.push(CompileError::new(
                    "SHANIMATION003",
                    "animation keyframes cannot contain variants",
                    utility.span,
                )),
                None => diagnostics.push(CompileError::new(
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
    diagnostics: &mut Vec<CompileError>,
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
    diagnostics: &mut Vec<CompileError>,
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
                _ => diagnostics.push(CompileError::new(
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
    diagnostics: &mut Vec<CompileError>,
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
    diagnostics: &mut Vec<CompileError>,
) -> String {
    let mut utilities = Vec::new();
    for utility in lex_utilities(input) {
        match resolve_utility(&utility.value.raw, theme) {
            Some(utility) if utility.variants.is_empty() => utilities.push(utility),
            Some(_) => diagnostics.push(CompileError::new(
                "SHMOTION001",
                "motion definition utilities cannot contain variants",
                utility.span,
            )),
            None => diagnostics.push(CompileError::new(
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

fn unresolved_utility_rule(raw: &str) -> &'static str {
    if parse_candidate(raw).is_some() {
        "unknown-utility"
    } else {
        "invalid-utility"
    }
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
    for target in &mut targets {
        for nested in &mut target.nested {
            if utility == "divide-y" && nested.declaration.contains("border-bottom-width:") {
                "--tw-divide-y-reverse:0;border-bottom-style:var(--tw-border-style);border-top-style:var(--tw-border-style);border-top-width:calc(1px * var(--tw-divide-y-reverse));border-bottom-width:calc(1px * calc(1 - var(--tw-divide-y-reverse)));".clone_into(&mut nested.declaration);
            } else if utility == "divide-x"
                && nested.declaration.contains("border-inline-end-width:")
            {
                "--tw-divide-x-reverse:0;border-inline-end-style:var(--tw-border-style);border-inline-start-style:var(--tw-border-style);border-inline-start-width:calc(1px * var(--tw-divide-x-reverse));border-inline-end-width:calc(1px * calc(1 - var(--tw-divide-x-reverse)));".clone_into(&mut nested.declaration);
            }
        }
    }
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
        let mut nested_in_rule_css = String::new();
        output.push('.');
        output.push_str(class_name);
        output.push('{');
        for utility in utilities.iter().filter(|utility| {
            utility.variants.is_empty() && utility.selector.is_none() && utility.container.is_none()
        }) {
            for nested in &utility.nested {
                let nested_in_rule = nested.declaration.starts_with("--tw-gradient-")
                    || nested.declaration.starts_with("background-color:color-mix")
                    || nested
                        .declaration
                        .starts_with("--tw-shadow-color:color-mix");
                if !nested_in_rule {
                    continue;
                }
                let mut nested_css = String::new();
                match nested.at_rule {
                    stillwind_tailwind::NestedUtilityAtRule::Media => {
                        nested_css.push_str("@media ");
                        nested_css.push_str(&nested.condition);
                        nested_css.push('{');
                        nested_css.push_str(&nested.declaration);
                        nested_css.push('}');
                    }
                    stillwind_tailwind::NestedUtilityAtRule::Supports => {
                        nested_css.push_str("@supports (");
                        nested_css.push_str(&nested.condition);
                        nested_css.push_str("){\n");
                        nested_css.push_str(&nested.declaration);
                        nested_css.push('}');
                    }
                }
                nested_in_rule_css.push_str(&nested_css);
            }
        }
        if base.contains("background-image:") && !nested_in_rule_css.is_empty() {
            if let Some(position) = base.find("background-image:") {
                output.push_str(&base[..position]);
                output.push_str(&nested_in_rule_css);
                output.push_str(&base[position..]);
            } else {
                output.push_str(&base);
                output.push_str(&nested_in_rule_css);
            }
        } else {
            output.push_str(&base);
            output.push_str(&nested_in_rule_css);
        }
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
            let nested_in_rule = nested.declaration.starts_with("--tw-gradient-")
                || nested.declaration.starts_with("background-color:color-mix")
                || nested
                    .declaration
                    .starts_with("--tw-shadow-color:color-mix");
            if nested_in_rule
                && utility.variants.is_empty()
                && utility.selector.is_none()
                && utility.container.is_none()
            {
                continue;
            }
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
    if selector.contains(">:not(:last-child)") {
        selector = format!(":where({selector})");
    }
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
    let mut ordered = Vec::new();
    for utility in utilities {
        for declaration in utility.declaration.split_terminator(';') {
            if let Some(transform) = declaration.strip_prefix("transform:") {
                transforms.push(transform);
            } else if !declaration.is_empty() {
                // Tailwind's minifier retains a separating space in
                // comma-separated shadow fallback lists. Keep that spelling so
                // parity comparisons are made against the same serialized CSS.
                let declaration = declaration.strip_prefix("box-shadow:").map_or_else(
                    || declaration.to_owned(),
                    |value| format!("box-shadow:{}", value.replace(",var(", ", var(")),
                );
                let declaration = if let Some(value) = declaration.strip_prefix("mask-image:") {
                    format!("mask-image:{}", value.replace(",var(", ", var("))
                } else {
                    declaration
                };
                ordered.push(declaration);
            }
        }
    }
    // Tailwind's generated utility order is independent of the order in which
    // candidates appear in the source. Reorder only declarations belonging to
    // the same competing family; unrelated declarations keep their original
    // slots and source order.
    for family in ["margin", "padding", "inset", "border"] {
        let mut positions = ordered
            .iter()
            .enumerate()
            .filter_map(|(index, declaration)| {
                (declaration_order(declaration).is_some_and(|(name, _)| name == family))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let mut declarations_for_family = positions
            .iter()
            .map(|index| ordered[*index].clone())
            .collect::<Vec<_>>();
        declarations_for_family
            .sort_by_key(|declaration| declaration_order(declaration).unwrap().1);
        for (index, declaration) in positions.drain(..).zip(declarations_for_family) {
            ordered[index] = declaration;
        }
    }
    for declaration in ordered {
        declarations.push_str(&declaration);
        declarations.push(';');
    }
    if !transforms.is_empty() {
        declarations.push_str("transform:");
        declarations.push_str(&transforms.join(" "));
        declarations.push(';');
    }
    declarations
}

fn declaration_order(declaration: &str) -> Option<(&'static str, u16)> {
    let property = declaration.split_once(':').map(|(property, _)| property)?;

    // CSS cascade order used by Tailwind for shorthand, axis and side forms.
    // Keep the table deliberately focused on competing longhands; unrelated
    // properties retain their source order via the stable sequence tie-breaker.
    for (base, shorthand, axis, side) in [
        ("margin", "margin", "margin-inline", "margin-left"),
        ("padding", "padding", "padding-inline", "padding-left"),
        ("inset", "inset", "inset-inline", "left"),
    ] {
        if property == shorthand || property.starts_with(&format!("{shorthand}-")) {
            if property == shorthand {
                return Some((base, 100));
            }
            if property == axis || property == format!("{base}-block") {
                return Some((base, 110));
            }
            if property.contains("inline-start")
                || property.contains("inline-end")
                || property.contains("block-start")
                || property.contains("block-end")
            {
                return Some((base, 120));
            }
            if property.ends_with("-top")
                || property.ends_with("-right")
                || property.ends_with("-bottom")
                || property.ends_with("-left")
            {
                return Some((base, 130));
            }
        } else if property == side || property.starts_with(&format!("{side}-")) {
            return Some((base, 130));
        }
    }

    if property == "border" || property.starts_with("border-") {
        if property == "border" || property == "border-width" || property == "border-style" {
            return Some(("border", 200));
        }
        if property.starts_with("border-inline-") || property.starts_with("border-block-") {
            return Some(("border", 210));
        }
        if property.contains("-top-")
            || property.contains("-right-")
            || property.contains("-bottom-")
            || property.contains("-left-")
        {
            return Some(("border", 230));
        }
    }

    None
}

fn render_stylesheet(
    theme: &stillwind_css::ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
) -> String {
    let imports = &theme.imports;
    let mut css = String::new();
    if imports.layer_order {
        css.push_str("@layer theme, base, components, utilities;\n");
    }
    if let Some(layer) = &imports.theme_layer {
        let required_tokens = theme.tokens_required_by(utility_css, imports.base_layer.is_some());
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
        if !required_tokens.is_empty() || !animations.is_empty() {
            css.push_str("@layer ");
            css.push_str(layer);
            css.push_str("{:root,:host{");
            for token in &required_tokens {
                css.push_str(&token.name.value);
                css.push(':');
                css.push_str(&token.value.value.replace("--theme(", "var("));
                css.push(';');
            }
            css.push_str("}}");
            let keyframes =
                options.rewrite_generated(&stillwind_css::tailwind_v4_keyframes_for(&animations));
            if !keyframes.is_empty() {
                css.push_str("@layer ");
                css.push_str(layer);
                css.push('{');
                css.push_str(&keyframes);
                css.push('}');
            }
            css.push('\n');
        }
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
    let properties = render_used_properties(theme, &css, options);
    if !properties.is_empty() {
        css.insert_str(0, "@layer properties;\n");
        css.push('\n');
        css.push_str(&properties);
    }
    css
}

#[allow(clippy::too_many_lines)]
fn render_used_properties(
    theme: &stillwind_css::ThemeIndex,
    css: &str,
    options: &CompilerOptions,
) -> String {
    let mut used = stillwind_css::used_custom_properties(css);
    // A utility that only assigns a custom property does not make Tailwind
    // register it. Registration is needed when the value participates in a
    // consuming declaration (`var(--tw-...)`) or an explicitly materialized
    // family below.
    for name in [
        "--tw-border-style",
        "--tw-content",
        "--tw-divide-x-reverse",
        "--tw-divide-y-reverse",
        "--tw-mask-radial-shape",
        "--tw-mask-radial-position",
        "--tw-mask-radial-size",
        "--tw-outline-style",
        "--tw-text-shadow-color",
        "--tw-gradient-via-stops",
    ] {
        if css.contains(&format!("{name}:"))
            && !css.contains(&format!("var({name}"))
            && !(name.starts_with("--tw-divide-") && css.contains(":not(:last-child)"))
        {
            used.remove(name);
        }
    }
    // The CPU/GPU transform shorthands intentionally consume the transform
    // axes without registering them; Tailwind keeps these as unregistered
    // fallback variables.
    for name in [
        "--tw-rotate-x",
        "--tw-rotate-y",
        "--tw-rotate-z",
        "--tw-skew-x",
        "--tw-skew-y",
    ] {
        used.remove(name);
    }
    // Tailwind registers the complete transform axis family when any axis is
    // used, including the z-axis default even when the emitted utility does
    // not reference it directly.
    if used.contains("--tw-translate-x") || used.contains("--tw-translate-y") {
        used.insert("--tw-translate-z".to_owned());
    }
    if css.contains("--tw-ring-shadow:")
        || css.contains("--tw-shadow-color:")
        || css.contains("--tw-shadow:")
        || css.contains("--tw-inset-shadow-color:")
        || css.contains("--tw-inset-shadow-alpha:")
        || css.contains("--tw-ring-inset:")
    {
        for name in [
            "--tw-shadow",
            "--tw-shadow-color",
            "--tw-shadow-alpha",
            "--tw-inset-shadow",
            "--tw-inset-shadow-color",
            "--tw-inset-shadow-alpha",
            "--tw-ring-color",
            "--tw-inset-ring-color",
            "--tw-inset-ring-shadow",
            "--tw-ring-inset",
            "--tw-ring-offset-width",
            "--tw-ring-offset-color",
            "--tw-ring-offset-shadow",
        ] {
            used.insert(name.to_owned());
        }
    }
    if css.contains("--tw-drop-shadow:")
        || (css.contains("filter:") && css.contains("var(--tw-drop-shadow"))
    {
        for name in [
            "--tw-drop-shadow",
            "--tw-drop-shadow-color",
            "--tw-drop-shadow-alpha",
            "--tw-drop-shadow-size",
        ] {
            used.insert(name.to_owned());
        }
    }
    if css.contains("--tw-text-shadow-color:") {
        used.insert("--tw-text-shadow-color".to_owned());
        used.insert("--tw-text-shadow-alpha".to_owned());
    }
    let gradient_stops_used = css.contains("--tw-gradient-from:")
        || css.contains("--tw-gradient-via:")
        || css.contains("--tw-gradient-to:");
    let shadow_family_used = css.contains("--tw-shadow:")
        || css.contains("--tw-shadow-color:")
        || css.contains("--tw-ring-shadow:")
        || css.contains("--tw-ring-inset:")
        || css.contains("--tw-inset-shadow-color:")
        || css.contains("--tw-inset-shadow-alpha:");
    let mut seen = BTreeSet::new();
    let builtin_properties = theme
        .imports
        .builtins
        .then(stillwind_tailwind::builtin_property_definitions)
        .unwrap_or_default();
    let mut properties = builtin_properties
        .iter()
        .chain(&theme.properties)
        .rev()
        .filter_map(|property| {
            let name = options.rewrite_generated(&property.name);
            if name.starts_with("--tw-gradient-")
                && (name == "--tw-gradient-position" || name == "--tw-gradient-stops")
                && !gradient_stops_used
            {
                return None;
            }
            if name == "--tw-ring-color" && !shadow_family_used {
                return None;
            }
            let shadow_family_property = shadow_family_used
                && (name.starts_with("--tw-shadow")
                    || name.starts_with("--tw-inset-shadow")
                    || name.starts_with("--tw-ring")
                    || name.starts_with("--tw-inset-ring"));
            let property_used = (gradient_stops_used && name.starts_with("--tw-gradient-"))
                || shadow_family_property
                || used.contains(&name);
            let builtin = builtin_properties
                .iter()
                .any(|candidate| candidate.name == property.name);
            if !property_used || (!builtin && theme.authored_properties.contains(&name)) {
                return None;
            }
            if !seen.insert(name.clone()) {
                return None;
            }
            Some((name, property.body.as_str(), builtin))
        })
        .collect::<Vec<_>>();
    properties.reverse();

    let mut output = String::new();
    let mut builtin_initials = Vec::new();
    for (name, body, builtin) in &properties {
        if *builtin {
            let initial = body
                .lines()
                .find_map(|line| line.trim().strip_prefix("initial-value:"))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("initial");
            builtin_initials.push(format!("{name}:{initial};"));
        }
    }
    if !builtin_initials.is_empty() {
        output.push_str("@layer properties{@supports (((-webkit-hyphens:none)) and (not (margin-trim:inline))) or ((-moz-orient:inline) and (not (color:rgb(from red r g b)))){*,:before,:after,::backdrop{");
        output.push_str(&builtin_initials.concat());
        output.push_str("}}}\n");
    }
    for (name, body, _) in properties {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = writeln!(output, "@property {name} {{");
        for line in body.lines() {
            let _ = writeln!(output, "  {}", line.trim());
        }
        output.push_str("}\n");
    }
    output
}

const DEFAULT_REACT_UTILITIES: &[&str] = &[
    "absolute",
    "block",
    "container",
    "cursor-pointer",
    "cursor-wait",
    "flex",
    "grid",
    "group",
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
    "peer",
    "relative",
    "text-center",
    "text-left",
    "transition",
    "@container",
    "@container-normal",
];

const DEFAULT_REACT_VARIANTS: &[&str] = &[
    "active",
    "dark",
    "disabled",
    "focus",
    "focus-visible",
    "group-focus-within",
    "group-hover",
    "hover",
    "peer-checked",
    "peer-hover",
];

fn render_react_types(theme: &stillwind_css::ThemeIndex) -> String {
    let mut utilities = Vec::new();
    if theme.imports.builtins {
        utilities.extend(DEFAULT_REACT_UTILITIES.iter().copied().map(str::to_owned));
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
    }
    utilities.sort();
    utilities.dedup();
    let entries = utilities
        .into_iter()
        .map(|utility| format!("    \"{utility}\": true;"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut variants = Vec::new();
    if theme.imports.builtins {
        variants.extend(DEFAULT_REACT_VARIANTS.iter().copied());
        variants.extend(
            theme
                .tokens
                .iter()
                .filter_map(|token| token.name.value.strip_prefix("--breakpoint-")),
        );
    }
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

    #[test]
    fn emits_only_used_compiler_owned_property_registrations() {
        let theme = SourceFile::new(
            FileId(2),
            "stillwind.css",
            r#"
                @stillwind-property --example-used {
                  syntax: "<number>";
                  inherits: false;
                  initial-value: 0;
                }
                @stillwind-property --example-unused {
                  syntax: "<number>";
                  inherits: false;
                  initial-value: 0;
                }
                .example { --example-used: 1; }
            "#,
        );

        let css =
            Compiler::render_react_stylesheet(&theme, "", &CompilerOptions::default()).unwrap();

        assert!(css.contains("@property --example-used"));
        assert!(!css.contains("--example-unused"));
        assert!(!css.contains("@stillwind-property"));
    }

    #[test]
    fn preserves_authored_property_rules_over_compiler_defaults() {
        let theme = SourceFile::new(
            FileId(2),
            "stillwind.css",
            r#"
                @property --tw-shadow {
                  syntax: "<color>";
                  inherits: true;
                  initial-value: red;
                }
                .example { color: var(--tw-shadow); }
            "#,
        );

        let css =
            Compiler::render_react_stylesheet(&theme, "", &CompilerOptions::default()).unwrap();

        assert_eq!(css.matches("@property --tw-shadow").count(), 1);
        assert!(css.contains("inherits: true"));
        assert!(!css.contains("initial-value: 0 0 #0000"));
    }

    #[test]
    fn filters_and_rewrites_builtin_properties_for_generated_utility_css() {
        let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");
        let options = CompilerOptions {
            tailwind_prefix: "--internal-".to_owned(),
            ..CompilerOptions::default()
        };

        let css = Compiler::render_react_stylesheet(
            &theme,
            ".example{--internal-translate-x:0;translate:var(--internal-translate-x);}",
            &options,
        )
        .unwrap();

        assert!(css.contains("@property --internal-translate-x"));
        assert!(!css.contains("@property --internal-translate-y"));
        assert!(!css.contains("@property --internal-gradient-from"));
    }

    #[test]
    fn orders_conflicting_shorthands_like_tailwind() {
        let declarations = [
            CompiledUtility {
                variants: Vec::new(),
                declaration: "margin-left:0.5rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
            },
            CompiledUtility {
                variants: Vec::new(),
                declaration: "margin-inline:0.75rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
            },
            CompiledUtility {
                variants: Vec::new(),
                declaration: "margin:1rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
            },
        ];

        let actual = combine_declarations(&declarations.iter().collect::<Vec<_>>());
        assert_eq!(
            actual,
            "margin:1rem;margin-inline:0.75rem;margin-left:0.5rem;"
        );
    }

    #[test]
    fn keeps_unrelated_declarations_stable() {
        let declarations = [
            CompiledUtility {
                variants: Vec::new(),
                declaration: "color:red;display:block;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
            },
            CompiledUtility {
                variants: Vec::new(),
                declaration: "opacity:0.5;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
            },
        ];

        let actual = combine_declarations(&declarations.iter().collect::<Vec<_>>());
        assert_eq!(actual, "color:red;display:block;opacity:0.5;");
    }

    #[test]
    fn makes_margin_utility_order_independent() {
        let source = SourceFile::new(
            FileId(1),
            "App.tsx",
            r#"import { style } from "stillwind/react"; export const props = style("ml-2 mx-3 m-4");"#,
        );
        let theme = SourceFile::new(
            FileId(2),
            "stillwind.css",
            "@theme { --spacing-2: 0.5rem; --spacing-3: 0.75rem; --spacing-4: 1rem; }",
        );
        let result = Compiler::compile_react(&source, &theme);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let margin = result.css.find("margin:").expect("margin shorthand");
        let inline = result.css.find("margin-inline:").expect("margin axis");
        let left = result.css.find("margin-left:").expect("margin side");
        assert!(margin < inline && inline < left, "{0}", result.css);
    }
}
