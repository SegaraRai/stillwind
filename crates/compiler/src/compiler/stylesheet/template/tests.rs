//! Assembly contracts spanning dependency selection and template emission.
//!
//! Covers authored placement, carrier layers, dependency destinations, and repeated
//! source mappings through the rendering entry points; it does not test CSS minification.

use std::rc::Rc;
use std::sync::Arc;

use lean_string::{LeanString, ToLeanString};

use stillwind_css::{CssOutputNode, CssOutputTemplate};
use stillwind_design_system::{CssSourceGraph, CustomUtility, ThemeIndex, ThemeToken, parse_theme};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::{CssSourceName, FileId, SourceFile};

use crate::identifiers::{
    DEFAULT_HASH_LENGTH, GeneratedName, GeneratorSet, IdentifierContext, IdentifierGenerator,
    IdentifierHash, Identifiers, LayerContext, LayerName,
};
use crate::{CompilerOptions, IdentifierMode, MappedStylesheet};

/// One test's generated names, which the built-in generator always produces.
fn identifiers() -> IdentifierContext {
    IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    )
}

/// The name one anonymous layer of `template` carries, without its traversal index.
fn layer_name(template: &CssOutputTemplate, options: &CompilerOptions) -> LeanString {
    identifiers()
        .layer_name(&LayerContext {
            template: template.source_identity.clone(),
            namespace: options.class_namespace.clone(),
        })
        .expect("the built-in generator cannot reject a name")
        .custom_ident()
}

/// Renders raw CSS with the compiler's own names.
fn render(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    document: bool,
) -> Result<String, CompileDiagnostic> {
    super::render(
        template,
        theme,
        utility_css,
        options,
        document,
        &identifiers(),
    )
}

/// Renders a mapped stylesheet with the compiler's own names.
fn render_mapped(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &MappedStylesheet,
    options: &CompilerOptions,
    document: bool,
    sources: Option<&CssSourceGraph>,
) -> Result<MappedStylesheet, CompileDiagnostic> {
    super::render_mapped(
        template,
        theme,
        utility_css,
        options,
        document,
        sources,
        &identifiers(),
    )
}

/// Names every anonymous layer with one fixed body.
#[derive(Debug)]
struct FixedLayerGenerator;

impl IdentifierGenerator for FixedLayerGenerator {
    fn layer_name(
        &self,
        _context: &LayerContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<LayerName>, LeanString> {
        Ok(GeneratedName::provided(
            LayerName::new("app-layer").expect("canonical test body"),
        ))
    }
}

#[test]
fn a_host_names_the_anonymous_layers_and_the_compiler_appends_the_index() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "@layer { .base{color:red} } @layer { @layer { @stillwind-slot utilities; } }",
    )
    .unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let identifiers = Identifiers::new(Some(Rc::new(FixedLayerGenerator)), IdentifierMode::Release)
        .context(DEFAULT_HASH_LENGTH);
    let css = super::render(
        theme.output_template(),
        &theme,
        "",
        &CompilerOptions::default(),
        true,
        &identifiers,
    )
    .unwrap();
    // The generator names the layers of one template, and the compiler keeps them distinct by
    // appending the traversal index.
    assert!(css.contains("@layer app-layer-0{"), "{css}");
    assert!(css.contains("@layer app-layer-1{"), "{css}");
    assert!(!css.contains("sw-anon-"), "{css}");
}

#[test]
fn declaration_slots_keep_css_owned_selectors_conditions_and_order() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"
        @theme { --brand: red; }
        @stillwind-property --n { syntax: "<number>"; inherits: false; initial-value: 0; }
        @media print { @layer defaults {
            .theme-scope { color:blue; @stillwind-slot theme; color:green; }
            .reset::before { @stillwind-slot property-initials; }
        } }
        @stillwind-slot properties;
        @stillwind-slot utilities;
    "#,
    )
    .unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let css = render(
        theme.output_template(),
        &theme,
        ".x{color:var(--brand);opacity:var(--n)}",
        &CompilerOptions::default(),
        false,
    )
    .unwrap();
    assert!(
        css.contains("@media print{@layer defaults{.theme-scope{"),
        "{css}"
    );
    assert!(css.contains("--brand:red;"), "{css}");
    assert!(css.find("color:blue").unwrap() < css.find("--brand:red").unwrap());
    assert!(css.find("--brand:red").unwrap() < css.find("color:green").unwrap());
    assert!(css.contains(".reset::before{"), "{css}");
    assert!(!css.contains(":root") && !css.contains(":host") && !css.contains("::backdrop"));
    assert!(!css.contains("@stillwind-slot"));
}

#[test]
fn generated_keyframes_keep_mapping_while_raw_css_keeps_its_placement() {
    let source = SourceFile::new(FileId(1), None, None, "@layer animations { @stillwind-slot keyframes; } @layer utilities { @stillwind-slot utilities; }").unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let original =
        Arc::new(SourceFile::new(FileId(9), None, None, "日本 animation source").unwrap());
    let mut frames = MappedStylesheet::default();
    frames.text(
        "@keyframes generated{to{opacity:0}}",
        Some((original.clone(), original.full_span())),
    );
    let utilities = MappedStylesheet {
        css: "@media print{@keyframes authored{to{opacity:1}}}.x{content:'@keyframes fake{}'}"
            .into(),
        keyframes: vec![frames],
        ..MappedStylesheet::default()
    };
    let output = render_mapped(
        theme.output_template(),
        &theme,
        &utilities,
        &CompilerOptions::default(),
        true,
        None,
    )
    .unwrap();
    assert!(
        output
            .css
            .contains("@layer animations{@keyframes generated{")
    );
    assert!(
        output
            .css
            .contains("@layer utilities{@media print{@keyframes authored{")
    );
    assert!(output.css.contains("content:'@keyframes fake{}'"));
    let mapping = &output.mappings[0];
    assert_eq!(
        &output.css[mapping.generated.clone()],
        "@keyframes generated{to{opacity:0}}"
    );
    assert_eq!(mapping.original, 0..original.text().len());
    assert!(Arc::ptr_eq(&mapping.source, &original));
}

#[test]
fn anonymous_layers_share_identity_across_document_and_partial_carriers() {
    let source = SourceFile::new(FileId(1), None, None, "@layer { .base{color:red} } @layer /* anonymous */ { @layer { @stillwind-slot utilities; } } @layer named { .end{color:blue} }").unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let template = theme.output_template();
    let options = CompilerOptions::default();
    let prefix = layer_name(template, &options);
    let document = render(template, &theme, "", &options, true).unwrap();
    let module = render(template, &theme, ".x{color:green}", &options, false).unwrap();
    assert!(
        document.contains(
            format_args!("@layer {prefix}-0{{")
                .to_lean_string()
                .as_str()
        ),
        "{document}"
    );
    assert!(
        document.contains(
            format_args!("@layer {prefix}-1{{@layer {prefix}-2{{}}}}")
                .to_lean_string()
                .as_str()
        )
    );
    assert_eq!(
        module,
        format_args!(
            "@layer {prefix}-0;@layer {prefix}-1{{@layer {prefix}-2{{.x{{color:green}}}}}}@layer named;"
        ).to_lean_string()
    );
    assert!(document.contains("@layer named"));
    let isolated = CompilerOptions {
        class_namespace: "other-app".into(),
        ..options
    };
    assert_ne!(prefix, layer_name(template, &isolated));
    let other_source = SourceFile::new(FileId(2), None, None, source.text()).unwrap();
    let other_theme = parse_theme(&other_source, &CssSourceName::from("other.css")).unwrap();
    assert_ne!(
        prefix,
        layer_name(other_theme.output_template(), &CompilerOptions::default())
    );
}

#[test]
fn slots_and_support_preserve_authored_conditions_in_document_and_module_outputs() {
    let source = SourceFile::new(FileId(1), None, None, "@layer theme, utilities; @layer theme { :root,:host { @stillwind-slot theme; } @theme { --color-brand: red; --unused:blue; } } .before{display:block;} @layer utilities { @supports (display:grid) { @media print { @stillwind-slot utilities; } } } .after{color:blue;}").unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let template = theme.output_template();
    let utility = ".x{color:var(--color-brand);}";
    let css = render(template, &theme, utility, &CompilerOptions::default(), true).unwrap();
    assert!(css.contains("@layer theme{:root,:host{--color-brand:red;}}"));
    assert!(css.contains(
        "@layer utilities{@supports (display:grid){@media print{.x{color:var(--color-brand);}}}}"
    ));
    assert!(css.find(".before").unwrap() < css.find(".x{").unwrap());
    assert!(css.find(".x{").unwrap() < css.find(".after").unwrap());
    assert!(!css.contains("--unused"));
    let module = render(
        template,
        &theme,
        utility,
        &CompilerOptions::default(),
        false,
    )
    .unwrap();
    assert!(!module.contains(".before"));
    assert!(!module.contains(".after"));
    assert!(module.contains("@media print{.x{"));
    assert!(module.contains("--color-brand:red;"));
}

#[test]
fn slots_relocate_definitions_without_hoisting_conditions_or_layers() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"
        @layer theme, utilities;
        @layer theme { :root,:host { @stillwind-slot theme; } }
        @layer utilities { @stillwind-slot utilities; }
        @media print { @layer motion { @stillwind-slot keyframes; } }
        @stillwind-slot properties;
    *,:before,:after,::backdrop { @stillwind-slot property-initials; }
        @theme { --color-brand: red; --animate-spin: spin 1s;
            @keyframes spin { to { opacity: 0; } }
        }
        @stillwind-property --length { syntax: "<length>"; inherits: false; initial-value: 0px; }
    "#,
    )
    .unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let css = render(
        theme.output_template(),
        &theme,
        ".x{color:var(--color-brand);animation:var(--animate-spin);width:var(--length)}",
        &CompilerOptions::default(),
        true,
    )
    .unwrap();
    assert!(
        css.contains("@layer theme{:root,:host{--color-brand:red;--animate-spin:spin 1s;}}"),
        "{css}"
    );
    assert!(
        css.contains("@media print{@layer motion{@keyframes spin"),
        "{css}"
    );
    assert_eq!(css.matches("@keyframes spin").count(), 1);
    assert!(css.find("@layer utilities{").unwrap() < css.find("@keyframes spin").unwrap());
    assert!(css.find("@property --length").unwrap() > css.find("@keyframes spin").unwrap());
}

#[test]
fn theme_declarations_activate_registrations_in_document_and_carrier_outputs() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"
        @stillwind-property --length { syntax: "<length>"; inherits: false; initial-value: 0px; }
        @theme { --spacing-brand: var(--length); --spacing-unused: var(--unused); }
        @media print { :root { @stillwind-slot theme; } }
        @stillwind-slot utilities;
        @stillwind-slot properties;
        * { @stillwind-slot property-initials; }
    "#,
    )
    .unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    for document in [true, false] {
        let css = render(
            theme.output_template(),
            &theme,
            ".x{width:var(--spacing-brand)}",
            &CompilerOptions::default(),
            document,
        )
        .unwrap();
        assert!(css.contains("@property --length"), "{css}");
        assert!(
            css.contains("@media print{:root{--spacing-brand:var(--length);}}"),
            "{css}"
        );
        assert!(!css.contains("--unused"), "{css}");
    }
}

#[test]
fn registrations_require_a_destination_instead_of_implicit_tail_output() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"
        @stillwind-property --length { syntax: "<length>"; inherits: false; initial-value: 0px; }
        @stillwind-slot utilities;
    "#,
    )
    .unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    assert!(matches!(
        render(
            theme.output_template(),
            &theme,
            ".x{width:var(--length)}",
            &CompilerOptions::default(),
            true
        ),
        Err(CompileDiagnostic::MissingSupportSlot { slot }) if slot == "properties"
    ));
}

#[test]
fn missing_slots_are_reported_and_no_import_has_no_reset_or_defaults() {
    let source = SourceFile::new(FileId(1), None, None, ".a{color:red;}").unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let template = theme.output_template();
    assert_eq!(
        render(template, &theme, "", &CompilerOptions::default(), true).unwrap(),
        ".a{color:red;}"
    );
    assert!(matches!(
        render(
            template,
            &theme,
            ".x{display:block}",
            &CompilerOptions::default(),
            true
        ),
        Err(CompileDiagnostic::MissingUtilitiesSlot)
    ));
    assert_eq!(theme.registry().utilities(), [] as [Arc<CustomUtility>; 0]);
    assert_eq!(theme.tokens(), [] as [Arc<ThemeToken>; 0]);
}

#[test]
fn repeated_keyframe_slots_preserve_each_mapping_and_wrapper() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "@media print { @stillwind-slot keyframes; } @layer motion { @stillwind-slot keyframes; }",
    )
    .unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let original = Arc::new(SourceFile::new(FileId(2), None, None, "animation").unwrap());
    let mut frames = MappedStylesheet::default();
    let css = "@keyframes generated{to{opacity:0}}";
    frames.text(css, Some((original.clone(), original.full_span())));
    let utility = MappedStylesheet {
        keyframes: vec![frames],
        ..MappedStylesheet::default()
    };
    for document in [true, false] {
        let output = render_mapped(
            theme.output_template(),
            &theme,
            &utility,
            &CompilerOptions::default(),
            document,
            None,
        )
        .unwrap();
        assert_eq!(
            output.css,
            format_args!("@media print{{{css}}}@layer motion{{{css}}}").to_lean_string()
        );
        assert_eq!(output.mappings.len(), 2);
        for mapping in &output.mappings {
            assert_eq!(&output.css[mapping.generated.clone()], css);
            assert_eq!(mapping.original, 0..original.text().len());
            assert!(Arc::ptr_eq(&mapping.source, &original));
        }
    }
}

#[test]
fn registration_destinations_are_checked_before_theme_lowering() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"
        @stillwind-property --length { syntax: "<length>"; inherits: false; initial-value: 0px; }
        @theme { --brand: --if(invalid); }
        :root { @stillwind-slot theme; }
        @stillwind-slot utilities;
    "#,
    )
    .unwrap();
    let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
    let utility = ".x{color:var(--brand);width:var(--length)}";
    assert!(matches!(render(theme.output_template(), &theme, utility,
        &CompilerOptions::default(), false),
        Err(CompileDiagnostic::MissingSupportSlot { slot }) if slot == "properties"));
    let mut template = theme.output_template().clone();
    template
        .nodes
        .extend([CssOutputNode::Properties, CssOutputNode::PropertyInitials]);
    assert!(matches!(
        render(
            &template,
            &theme,
            utility,
            &CompilerOptions::default(),
            false
        ),
        Err(CompileDiagnostic::InvalidRuntimeConditional { .. })
    ));
}
