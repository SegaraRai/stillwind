//! Discovery of candidates of utility families that a host plugin defines on demand.
//!
//! A host resolves such candidates outside the compiler and adds their definitions to
//! the design system, so the compiler reports which ones a module needs.
//! [`module_candidates`] lists the utility text and variant prefixes that the IR of a
//! module passes to resolution, and [`dynamic_candidates`] turns such candidates into
//! requests. The listing does not depend on the design system, so a host can collect
//! it before the definitions exist. [`stylesheet_dynamic_candidates`] reads only the
//! candidates that CSS applies, because stylesheets also contain definition headers
//! and registry metadata that name utilities without using them.

use std::collections::BTreeSet;

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::{CssNode, CssNodeKind, parse_structure};
use stillwind_design_system::{
    DynamicFamilyKind, ThemeIndex, compound_variant_inner, resolve_utility,
};
use stillwind_ir::{
    ClassExpression, CssCall, CssInput, GroupEntry, KeyframesBody, KeyframesDefinition, MediaCall,
    ModuleIr, RecipeCall, RefCondition, RuleExpression, StoredVariantDefinition, StyleCall,
    StyleInput, StyleVariant, ViewTransitionDefinition,
};
use stillwind_source::Spanned;
use stillwind_syntax::{
    Utility, lex_utilities, parse_candidate, parse_variant_prefix, utility_registration_key,
};

use crate::compiler::apply_candidates;

/// Returns sorted, unique requests for host plugin families among `candidates`.
///
/// Each item of `candidates` must be one candidate, such as `hover:icon-mdi--close`,
/// or one variant prefix that ends in `:`, such as `data-[open]:`. Other items are
/// skipped.
///
/// Utility names that a `@stillwind-undef-utility` statement of the `default` registry
/// context matches are not requested. Native families report unresolved utility names
/// without modifiers. Tailwind utility families retain negative signs and modifiers
/// even when a `@stillwind-utility` definition already resolves. Variant requests have
/// an `@variant ` prefix followed by the complete variant name.
///
/// A bracket candidate such as `icon-[mdi--close]` is reported by its named spelling,
/// `icon-mdi--close`, when its family accepts both forms, and as written otherwise. A
/// theme without dynamic families returns nothing without reading `candidates`.
pub(crate) fn dynamic_candidates<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    theme: &ThemeIndex,
) -> Vec<LeanStr> {
    if theme.dynamic_utilities().is_empty() {
        return Vec::new();
    }
    requests(candidates, theme)
}

/// Returns the candidates and variant prefixes that compilation of `module` resolves,
/// in IR order and possibly repeated.
///
/// Utility text is split at whitespace, as compilation splits it. Each variant prefix
/// ends in `:`, so that [`dynamic_candidates`] reads it as variants without a utility.
/// A candidate keeps the variants that its own text writes, and the prefixes of the
/// enclosing wrappers are listed separately. Utility text that cannot be tokenized is
/// skipped, because compilation reports it as a fatal diagnostic.
pub fn module_candidates(module: &ModuleIr) -> Vec<LeanStr> {
    // Every field is named, so a new field does not compile until this function lists
    // its candidates or ignores it.
    let ModuleIr {
        imports: _,
        style_calls,
        recipe_calls,
        media_calls,
        css_calls,
        forward_calls: _,
        ordering_uses: _,
        ref_definitions: _,
        keyframes_declarations,
        view_transition_declarations,
        stored_variants,
        conditional_labels: _,
    } = module;
    let mut candidates = Candidates::default();
    for call in style_calls {
        candidates.style_call(call);
    }
    for RecipeCall { values, .. } in recipe_calls {
        for call in values {
            candidates.style_call(call);
        }
    }
    for MediaCall { arguments, .. } in media_calls {
        for argument in arguments {
            candidates.variant_prefix(&argument.value);
        }
    }
    for CssCall { inputs, .. } in css_calls {
        for CssInput {
            variants,
            utilities,
        } in inputs
        {
            for variant in variants {
                candidates.variant_prefix(&variant.value);
            }
            candidates.utilities(utilities);
        }
    }
    for KeyframesDefinition { body, .. } in keyframes_declarations {
        candidates.keyframes_body(body);
    }
    for ViewTransitionDefinition {
        group,
        old,
        new,
        image_pair,
        ..
    } in view_transition_declarations
    {
        for utilities in [group, old, new, image_pair].into_iter().flatten() {
            candidates.utilities(utilities);
        }
    }
    for StoredVariantDefinition { variants, .. } in stored_variants {
        for variant in variants.iter().flatten() {
            candidates.variant(variant);
        }
    }
    candidates.0
}

/// Candidates and variant prefixes that [`module_candidates`] collects, in IR order.
#[derive(Default)]
struct Candidates(Vec<LeanStr>);

impl Candidates {
    /// Appends the whitespace-separated candidates of `text`.
    fn utilities(&mut self, text: &Spanned<LeanStr>) {
        if let Ok(tokens) = lex_utilities(text) {
            self.0
                .extend(tokens.into_iter().map(|token| token.value.raw));
        }
    }

    /// Appends a variant prefix and adds its final `:` when it is missing. Empty text
    /// has no variants and is not appended.
    fn variant_prefix(&mut self, prefix: &str) {
        if prefix.is_empty() {
            return;
        }
        self.0.push(if prefix.ends_with(':') {
            prefix.into()
        } else {
            format_args!("{prefix}:").to_lean_str()
        });
    }

    /// Appends the candidates of every input of `call`.
    fn style_call(&mut self, call: &StyleCall) {
        for input in &call.inputs {
            self.style_input(input);
        }
    }

    /// Appends the candidates of `input` and its nested inputs.
    fn style_input(&mut self, input: &StyleInput) {
        match input {
            StyleInput::Array(inputs) => {
                for input in inputs {
                    self.style_input(input);
                }
            }
            StyleInput::Conditional { branches, .. } => {
                for input in branches.values().flatten() {
                    self.style_input(input);
                }
            }
            StyleInput::Class(expression) => {
                self.class_expression(expression);
            }
            // These inputs refer to declarations, whose own modules list their candidates.
            StyleInput::Composition { .. }
            | StyleInput::RefAttachment { .. }
            | StyleInput::DynamicRefAttachment { .. }
            | StyleInput::ViewTransitionUse { .. }
            | StyleInput::DynamicViewTransitionUse { .. }
            | StyleInput::Forward { .. } => {}
        }
    }

    /// Appends the candidates of `expression` and its nested expressions.
    fn class_expression(&mut self, expression: &ClassExpression) {
        match expression {
            ClassExpression::Array(elements) => {
                for element in elements {
                    self.class_expression(element);
                }
            }
            ClassExpression::Conditional { branches, .. } => {
                for expression in branches.values().flatten() {
                    self.class_expression(expression);
                }
            }
            ClassExpression::Rule(rule) => {
                self.rule_expression(rule);
            }
            ClassExpression::Group { entries, .. } => {
                for entry in entries {
                    match entry {
                        GroupEntry::Class(expression) => {
                            self.class_expression(expression);
                        }
                        GroupEntry::CustomProperty(_) => {}
                    }
                }
            }
            // A `class` value holds external class names, which compilation does not
            // resolve.
            ClassExpression::ClassValue { .. } => {}
        }
    }

    /// Appends the candidates of `expression` and its nested expressions.
    fn rule_expression(&mut self, expression: &RuleExpression) {
        match expression {
            RuleExpression::Utilities(text) => {
                self.utilities(text);
            }
            RuleExpression::Array(elements) => {
                for element in elements {
                    self.rule_expression(element);
                }
            }
            RuleExpression::Conditional { branches, .. } => {
                for element in branches.values().flatten() {
                    self.rule_expression(element);
                }
            }
            RuleExpression::Variant { variant, inputs } => {
                self.variant(variant);
                for input in inputs {
                    self.rule_expression(input);
                }
            }
            RuleExpression::Group { inputs, .. } => {
                for input in inputs {
                    self.rule_expression(input);
                }
            }
            RuleExpression::KeyframesUse { options, .. }
            | RuleExpression::DynamicKeyframesUse { options, .. } => {
                self.utilities(options);
            }
            RuleExpression::AnchorScope { .. }
            | RuleExpression::RefProjection { .. }
            | RuleExpression::DynamicRefProjection { .. }
            | RuleExpression::ViewTransitionUse { .. }
            | RuleExpression::DynamicViewTransitionUse { .. } => {}
        }
    }

    /// Appends the variant prefixes of `variant`.
    fn variant(&mut self, variant: &StyleVariant) {
        match variant {
            StyleVariant::Prefix(prefix) => {
                self.variant_prefix(&prefix.value);
            }
            // The query is CSS text, and the refs supply container names.
            StyleVariant::Container { .. } => {}
            StyleVariant::Ref { condition, scopes } => {
                for RefCondition { prefix, .. } in std::iter::once(condition).chain(scopes) {
                    self.variant_prefix(&prefix.value);
                }
            }
            StyleVariant::Alternatives(alternatives) => {
                for variant in alternatives.iter().flatten() {
                    self.variant(variant);
                }
            }
        }
    }

    /// Appends the candidates of the frames of `body` and of its conditional
    /// replacements.
    fn keyframes_body(&mut self, body: &KeyframesBody) {
        for frame in &body.frames {
            self.utilities(&frame.utilities);
        }
        for branch in &body.branches {
            self.variant(&branch.variant);
            self.keyframes_body(&branch.body);
        }
    }
}

/// Returns sorted, unique requests for host plugin families that `css` applies, in the
/// forms that [`dynamic_candidates`] returns.
///
/// The candidates of every `@apply` statement and the variant sequences of every
/// `@variant` rule count, at any nesting depth, including the bodies of definitions.
/// A sequence requests its variants, as a `data-[open]:` style key does in a module.
/// Comments, strings, declarations, and at-rule preludes other than `@variant` are not
/// read. Incomplete CSS reports the uses that its structure contains.
pub(crate) fn stylesheet_dynamic_candidates(css: &str, theme: &ThemeIndex) -> Vec<LeanStr> {
    if theme.dynamic_utilities().is_empty() {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    applied_tokens(css, &parse_structure(css), &mut tokens);
    requests(tokens.iter().map(LeanStr::as_str), theme)
}

/// Appends the candidates and variant prefixes that `nodes` apply to `tokens`.
/// Node ranges must address `css`. A variant sequence gains a trailing `:`, so it
/// parses as variants without a utility.
fn applied_tokens(css: &str, nodes: &[CssNode], tokens: &mut Vec<LeanStr>) {
    for node in nodes {
        match &node.kind {
            CssNodeKind::StyleRule { block, .. } => {
                applied_tokens(css, &block.children, tokens);
            }
            CssNodeKind::AtRule {
                name,
                prelude,
                block,
            } => {
                if let Some(candidates) = apply_candidates(css, node) {
                    tokens.extend(
                        candidates
                            .into_iter()
                            .map(|range| LeanStr::from(&css[range])),
                    );
                }
                if name == "variant" {
                    let prelude = &css[prelude.clone()];
                    tokens.extend(
                        super::variant_sequences(prelude)
                            .into_iter()
                            .filter(|range| !range.is_empty())
                            .map(|range| format_args!("{}:", &prelude[range]).to_lean_str()),
                    );
                }
                if let Some(block) = block {
                    applied_tokens(css, &block.children, tokens);
                }
            }
            CssNodeKind::Declaration { .. } => {}
        }
    }
}

/// Returns sorted, unique requests for the family members among `tokens`. The theme
/// must declare at least one dynamic family.
fn requests<'a>(tokens: impl IntoIterator<Item = &'a str>, theme: &ThemeIndex) -> Vec<LeanStr> {
    let families = theme.dynamic_utilities();
    let mut names = BTreeSet::new();
    for token in tokens {
        if !families
            .iter()
            .any(|family| token.contains(family.prefix.as_str()))
        {
            continue;
        }
        // A style object key such as `"data-[open]:"` carries variants without a utility.
        let (variants, candidate) = match parse_variant_prefix(token) {
            Some(variants) => (variants, None),
            None => match parse_candidate(token) {
                Some(candidate) => (candidate.variants.clone(), Some(candidate)),
                None => continue,
            },
        };
        for variant in &variants {
            // A compound variant such as `group-state-[closed]` requests its inner
            // variant. The registry's variant-stack definitions decide which variants
            // are compound.
            let mut value = LeanString::from(variant.raw.as_str());
            loop {
                if families.iter().any(|family| {
                    family.kind == DynamicFamilyKind::Variant
                        && family_member(&value, &family.prefix)
                }) {
                    names.insert(format_args!("@variant {value}").to_lean_str());
                }
                match compound_variant_inner(&value, theme) {
                    Some(inner) if inner != value => {
                        value = inner;
                    }
                    _ => {
                        break;
                    }
                }
            }
        }
        let Some(candidate) = candidate else {
            continue;
        };
        let Utility::Named { name, modifier } = &candidate.utility else {
            continue;
        };
        // The host adds its definitions to the `@plugin` answer, which registers before
        // the other statements, so an undefinition outside the answer removes them again.
        if theme.undefines_utility(name) {
            continue;
        }
        if families.iter().any(|family| {
            family.kind == DynamicFamilyKind::Utility && family_member(name, &family.prefix)
        }) {
            names.insert(
                utility_registration_key(name, modifier.as_ref(), candidate.negative).to_lean_str(),
            );
        }
        if theme.dynamic_family(name).is_none() || matches!(resolve_utility(name, theme), Ok(Ok(_)))
        {
            continue;
        }
        names.insert(
            theme
                .dynamic_named_utility(name)
                .map_or_else(|| name.to_lean_str(), |named| named.to_lean_str()),
        );
    }
    names.into_iter().collect()
}

/// Whether a candidate selects a registered Tailwind root or one of its values.
fn family_member(candidate: &str, root: &str) -> bool {
    candidate == root
        || candidate
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use stillwind_design_system::parse_theme;
    use stillwind_diagnostics::CompileError;
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_source::{FileId, ModuleId, SourceFile};

    use super::*;

    /// Parses `css` as a theme.
    fn theme(css: &str) -> ThemeIndex {
        parse_theme(
            &SourceFile::new(FileId(1), None, None, css).unwrap(),
            &"t.css".into(),
        )
        .unwrap()
    }

    /// Parses `code` after a macro import and returns [`module_candidates`] of its IR.
    fn candidates(code: &str) -> Vec<LeanStr> {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("import {{ style }} from 'stillwind/react'; {code}"),
        )
        .unwrap();
        let parsed = stillwind_js_oxc::parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
        module_candidates(&parsed.module)
    }

    #[test]
    fn modules_list_the_candidates_and_variant_prefixes_of_every_input() {
        assert_eq!(
            candidates(
                r#"const open = style.when("state-[open]");
                const fade = style.keyframes({ from: "icon-a", "state-[x]:": { to: "icon-b" } });
                const vt = style.viewTransition({ old: "icon-c" });
                export const a = style("p-1 hover:icon-d", { "md:": ["icon-e", cond ? "icon-f" : "icon-g"] }, { [open]: "icon-h" }, fade("icon-i"), { class: "icon-external" });
                export const r = style.recipe("icon-j", { variants: { size: { small: "icon-k" } } });
                export const m = style.media("state-[media]");
                export const sheet = style.css({ "dark:": "icon-l" });"#
            ),
            [
                "p-1",
                "hover:icon-d",
                "md:",
                "icon-e",
                "icon-f",
                "icon-g",
                "state-[open]:",
                "icon-h",
                "icon-i",
                "icon-j",
                "icon-k",
                "state-[media]:",
                "dark:",
                "icon-l",
                "icon-a",
                "state-[x]:",
                "icon-b",
                "icon-c",
                "state-[open]:",
            ]
        );
    }

    #[test]
    fn modules_do_not_list_text_outside_style_inputs() {
        assert_eq!(
            candidates(
                r#"// icon-comment
                export const re = /["']/;
                const label = "icon-string";
                export const a = style("icon-lucide--anchor");"#
            ),
            ["icon-lucide--anchor"]
        );
    }

    #[test]
    fn families_report_only_the_forms_that_they_accept() {
        let text = "icon-mdi--close icon-[lucide--x]";
        assert_eq!(
            dynamic_candidates(
                text.split_whitespace(),
                &theme("@stillwind-dynamic icon-*;")
            ),
            ["icon-mdi--close"]
        );
        assert_eq!(
            dynamic_candidates(
                text.split_whitespace(),
                &theme("@stillwind-dynamic icon-[*];")
            ),
            ["icon-[lucide--x]"]
        );
        assert_eq!(
            dynamic_candidates(
                ["icon-[lucide--x]", "icon-[mdi--home]"],
                &theme(r"@stillwind-dynamic icon-[*]; @utility icon-[mdi--home] { color: red; }")
            ),
            ["icon-[lucide--x]"]
        );
    }

    #[test]
    fn unresolved_family_members_are_reported_by_their_named_spelling() {
        let theme = theme(
            "@stillwind-dynamic icon-*; @stillwind-dynamic icon-[*]; @utility icon-mdi--home { color: red; }",
        );
        let candidates = [
            "icon-mdi--close",
            "hover:icon-[lucide--x]",
            "icon-mdi--home",
            "md:!icon-mdi--close",
            "iconography",
            "icon-",
            "p-4",
        ];
        assert_eq!(
            dynamic_candidates(candidates, &theme),
            ["icon-lucide--x", "icon-mdi--close"]
        );
        assert_eq!(
            dynamic_candidates(candidates, &ThemeIndex::default()),
            [] as [LeanStr; 0]
        );
    }

    #[test]
    fn requests_depend_only_on_default_definitions() {
        let text = "icon-mdi--close icon-lucide--x";
        let undefined = theme("@stillwind-dynamic icon-*; @stillwind-undef-utility icon-mdi-*;");
        assert_eq!(
            dynamic_candidates(text.split_whitespace(), &undefined),
            ["icon-lucide--x"]
        );
        // `default` keeps the definitions that the host adds when only a context
        // undefines the family, so the context does not suppress the request.
        let scoped = theme(
            "@stillwind-dynamic icon-*; @stillwind-registry keyframe { @stillwind-undef-utility icon-*; }",
        );
        assert_eq!(
            dynamic_candidates(text.split_whitespace(), &scoped),
            ["icon-lucide--x", "icon-mdi--close"]
        );
        let defined = theme(
            "@stillwind-dynamic icon-*; @stillwind-registry keyframe { @stillwind-undef-utility icon-*; } @utility icon-mdi--close { color: red; }",
        );
        assert_eq!(
            dynamic_candidates(text.split_whitespace(), &defined),
            ["icon-lucide--x"]
        );
    }

    #[test]
    fn bracket_values_that_another_definition_accepts_are_not_reported() {
        let theme = theme(
            "@stillwind-dynamic icon-*; @stillwind-dynamic icon-[*]; @utility icon-* { --size: --value([length]); }",
        );
        assert_eq!(
            dynamic_candidates(["icon-[2rem]", "icon-[mdi--close]"], &theme),
            ["icon-mdi--close"]
        );
        let bracket = self::theme(
            "@stillwind-dynamic icon-[*]; @utility icon-* { --size: --value([length]); }",
        );
        assert_eq!(
            dynamic_candidates(["icon-[2rem]", "icon-[mdi--close]"], &bracket),
            ["icon-[mdi--close]"]
        );
    }

    #[test]
    fn stylesheets_report_only_applied_candidates_and_variant_sequences() {
        let theme = theme(
            "@stillwind-plugin-family utility probe; @stillwind-plugin-family variant state; @stillwind-dynamic icon-*;",
        );
        let css = r#"
/* @apply probe-comment; */
@stillwind-utility probe-* { width: --value(integer)px; }
.a { content: "@apply probe-string"; @apply probe-4 hover:icon-mdi--close bg-[url('a;b')]; }
@media print { .b { @variant state-[open], dark:state-[closed] { color: red; } } }
@custom-variant x { @variant state-[x] { @slot; } }
.c { @apply probe-[1px] "#;
        assert_eq!(
            stylesheet_dynamic_candidates(css, &theme),
            [
                "@variant state-[closed]",
                "@variant state-[open]",
                "@variant state-[x]",
                "icon-mdi--close",
                "probe-4",
                "probe-[1px]",
            ]
        );
        assert_eq!(
            stylesheet_dynamic_candidates(css, &ThemeIndex::default()),
            [] as [LeanStr; 0]
        );
    }

    #[test]
    fn tailwind_requests_retain_typed_values_modifiers_and_compound_variants() {
        let theme = theme(
            "@stillwind-plugin-family utility probe; @stillwind-plugin-family variant state; @utility probe-4 { width:1px; } @stillwind-variant group-* (value: [variant]) { &:is(:where(.group):--value() *) { @slot; } }",
        );
        assert_eq!(
            dynamic_candidates(
                [
                    "probe-4",
                    "-probe-[length:calc(2px_+_3px)]/[mirror]",
                    "state-[open]:",
                    "probe-(--width)",
                    "group-state-[closed]:probe-4",
                    "probe-one/[foo_bar]",
                ],
                &theme
            ),
            [
                "-probe-[length:calc(2px_+_3px)]/[mirror]",
                "@variant state-[closed]",
                "@variant state-[open]",
                "probe-(--width)",
                "probe-4",
                // The modifier is encoded again, so the key contains no whitespace.
                "probe-one/[foo_bar]"
            ]
        );
    }
}
