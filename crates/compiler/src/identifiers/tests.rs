use std::sync::{Arc, Mutex};

use stillwind_diagnostics::CompileErrorKind;
use stillwind_source::{CssSourceName, ModuleId};

use super::*;

/// Answers with a fixed name and records every request.
#[derive(Debug, Default)]
struct RecordingGenerator {
    calls: Mutex<Vec<(&'static str, String)>>,
}

impl RecordingGenerator {
    /// Records one request and answers with the family's fixed body.
    fn record(&self, family: &'static str, hash: &IdentifierHash) -> String {
        self.calls
            .lock()
            .expect("test generator lock")
            .push((family, hash.to_hex()));
        format!("fixed-{family}")
    }
}

impl IdentifierGenerator for RecordingGenerator {
    fn utility_class(
        &self,
        _context: &ClassContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, String> {
        Ok(GeneratedName::provided(
            UtilityClass::new(&self.record("class", hash)).expect("canonical test body"),
        ))
    }

    fn ref_marker(
        &self,
        _context: &DeclarationContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<RefMarker>, String> {
        Ok(GeneratedName::provided(
            RefMarker::new(&self.record("ref-marker", hash)).expect("canonical test body"),
        ))
    }
}

/// Fails the utility class family.
#[derive(Debug)]
struct FailingGenerator;

impl IdentifierGenerator for FailingGenerator {
    fn utility_class(
        &self,
        _context: &ClassContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, String> {
        Err("no name for this call".to_owned())
    }
}

fn class_context(branch: &str) -> ClassContext {
    ClassContext {
        module: ModuleId::from("src/a.tsx"),
        namespace: String::new(),
        call: 12,
        branch: branch.to_owned(),
    }
}

fn declaration_context(label: Option<&str>) -> DeclarationContext {
    DeclarationContext {
        module: ModuleId::from("src/a.tsx"),
        start: 20,
        end: 24,
        binding: "menu".to_owned(),
        label: label.and_then(|label| Label::new(label).ok()),
    }
}

fn locator_context(declaration: &DeclarationContext) -> LocatorContext {
    LocatorContext {
        module: declaration.module.clone(),
        start: declaration.start,
        end: declaration.end,
        binding: declaration.binding.clone(),
        label: declaration.label.clone(),
        projection: None,
    }
}

#[test]
fn a_host_name_is_written_without_the_builtin_prefix() {
    let context = IdentifierContext::new(
        GeneratorSet::new(Arc::new(RecordingGenerator::default())),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    let name = context.class_name(&class_context("0:flex")).unwrap();
    assert_eq!(name.as_str(), "fixed-class");
    let builtin = IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    )
    .class_name(&class_context("0:flex"))
    .unwrap();
    assert!(builtin.as_str().starts_with("sw_"), "{builtin:?}");
}

#[test]
fn every_builtin_family_keeps_its_own_prefix() {
    let context = IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    let declaration = declaration_context(Some("primary"));
    assert!(
        context
            .ref_marker_name(&declaration)
            .unwrap()
            .as_str()
            .starts_with("sw_")
    );
    assert!(
        context
            .anchor_name(&locator_context(&declaration))
            .unwrap()
            .as_str()
            .starts_with("sw-a-")
    );
    assert!(
        context
            .container_name(&locator_context(&declaration))
            .unwrap()
            .as_str()
            .starts_with("sw-c-")
    );
    assert!(
        context
            .keyframes_name(&declaration)
            .unwrap()
            .as_str()
            .starts_with("sw-kf-")
    );
    assert!(
        context
            .view_transition_class(&declaration)
            .unwrap()
            .as_str()
            .starts_with("sw-vt-")
    );
    // The anchor properties take a `<dashed-ident>`, so the two hyphens belong to the write
    // site rather than to the name.
    assert!(
        context
            .anchor_name(&locator_context(&declaration))
            .unwrap()
            .dashed_ident()
            .starts_with("--sw-a-")
    );
}

#[test]
fn a_label_is_spelled_between_the_prefix_and_the_digest() {
    let context = IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    let labelled = declaration_context(Some("primary"));
    assert!(
        context
            .ref_marker_name(&labelled)
            .unwrap()
            .as_str()
            .starts_with("sw_primary_")
    );
    assert!(
        context
            .anchor_name(&locator_context(&labelled))
            .unwrap()
            .as_str()
            .starts_with("sw-a-primary-")
    );
    assert!(
        context
            .container_name(&locator_context(&labelled))
            .unwrap()
            .as_str()
            .starts_with("sw-c-primary-")
    );
    assert!(
        context
            .keyframes_name(&labelled)
            .unwrap()
            .as_str()
            .starts_with("sw-kf-primary-")
    );
    assert!(
        context
            .view_transition_class(&labelled)
            .unwrap()
            .as_str()
            .starts_with("sw-vt-primary-")
    );
    // A label that cleans to nothing is no label, so the name falls back to the digest alone.
    assert_eq!(
        context
            .ref_marker_name(&declaration_context(Some("")))
            .unwrap(),
        context.ref_marker_name(&declaration_context(None)).unwrap()
    );
}

#[test]
fn a_digest_is_spelled_in_base_36_and_the_length_extends_its_suffix() {
    let hash = KeyframesNameRequest(declaration_context(None)).hash();
    let short = hash.short_base36(5);
    let long = hash.short_base36(24);
    assert_eq!(short.len(), 5);
    assert_eq!(long.len(), 24);
    assert!(long.ends_with(&short), "{long} does not end with {short}");
    assert!(
        long.chars()
            .all(|character| character.is_ascii_digit() || character.is_ascii_lowercase()),
        "{long}"
    );
}

#[test]
fn the_compilation_hash_length_decides_the_builtin_spelling() {
    let declaration = declaration_context(None);
    let long = IdentifierContext::new(GeneratorSet::default(), IdentifierMode::Release, 12)
        .keyframes_name(&declaration)
        .unwrap();
    assert_eq!(long.as_str().len(), "sw-kf-".len() + 12, "{long}");
    let short = IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    )
    .keyframes_name(&declaration)
    .unwrap();
    assert_eq!(short.as_str().len(), "sw-kf-".len() + 5, "{short}");
    assert!(long.as_str().ends_with(&short.as_str()["sw-kf-".len()..]));
}

#[test]
fn a_labelled_request_is_not_answered_from_the_unlabelled_memo() {
    let context = IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    let unlabelled = context.ref_marker_name(&declaration_context(None)).unwrap();
    let labelled = context
        .ref_marker_name(&declaration_context(Some("primary")))
        .unwrap();
    assert_ne!(unlabelled, labelled);
    assert!(labelled.as_str().starts_with("sw_primary_"), "{labelled}");
    assert_eq!(
        context.ref_marker_name(&declaration_context(None)).unwrap(),
        unlabelled
    );
}

#[test]
fn repeated_requests_use_one_generator_call() {
    let generator = Arc::new(RecordingGenerator::default());
    let context = IdentifierContext::new(
        GeneratorSet::new(generator.clone()),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    let first = context.class_name(&class_context("0:flex")).unwrap();
    let second = context.class_name(&class_context("0:flex")).unwrap();
    assert_eq!(first, second);
    assert_eq!(generator.calls.lock().expect("lock").len(), 1);
}

#[test]
fn development_mode_substitutes_and_records_a_rejected_name() {
    let context = IdentifierContext::new(
        GeneratorSet::new(Arc::new(FailingGenerator)),
        IdentifierMode::Development,
        DEFAULT_HASH_LENGTH,
    );
    let name = context.class_name(&class_context("0:flex")).unwrap();
    assert!(name.as_str().starts_with("sw_"), "{name:?}");
    let rejected = context.drain_rejected();
    assert_eq!(rejected.entries().len(), 1);
    let entry = &rejected.entries()[0];
    assert_eq!(entry.kind, IdentifierKind::UtilityClass);
    assert_eq!(entry.reason, "no name for this call");
    assert!(entry.substituted);
    assert!(context.drain_rejected().is_empty());
}

#[test]
fn release_mode_fails_on_a_rejected_name() {
    let context = IdentifierContext::new(
        GeneratorSet::new(Arc::new(FailingGenerator)),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    let failure = context.class_name(&class_context("0:flex")).unwrap_err();
    assert!(matches!(
        failure,
        CompileErrorKind::IdentifierGenerationFailed(..)
    ));
    assert!(context.drain_rejected().is_empty());
}

#[test]
fn the_hash_is_stable_for_equal_requests_and_specific_to_the_family() {
    assert_eq!(
        ClassName(class_context("0:flex")).hash(),
        ClassName(class_context("0:flex")).hash()
    );
    assert_ne!(
        ClassName(class_context("0:flex")).hash(),
        ClassName(class_context("0:grid")).hash()
    );
    let declaration = declaration_context(None);
    assert_ne!(
        RefMarkerName(declaration.clone()).hash(),
        KeyframesNameRequest(declaration.clone()).hash()
    );
    assert_eq!(
        AnchorNameRequest(locator_context(&declaration)).hash(),
        RefMarkerName(declaration).hash(),
        "a ref's anchor name and marker share the declaration's identity"
    );
}

#[test]
fn identity_fields_use_nul_boundaries() {
    let class = ClassName(ClassContext {
        module: ModuleId::from("src/a:1.tsx"),
        namespace: "app:one".to_owned(),
        call: 12,
        branch: "0:flex:wide".to_owned(),
    });
    assert_eq!(
        class.identity(),
        "stillwind/style/v1\0app:one\0src/a:1.tsx\x0012\x000:flex:wide\0"
    );
    assert_eq!(
        LayerNameRequest(LayerContext {
            template: CssSourceName::from("a:b.css"),
            namespace: "app:one".to_owned(),
        })
        .identity(),
        "stillwind/layer/v1\0app:one\0a:b.css\0"
    );
    assert_eq!(
        RefMarkerName(DeclarationContext {
            module: ModuleId::from("src/a:1.tsx"),
            start: 2,
            end: 5,
            binding: "a:b".to_owned(),
            label: None,
        })
        .identity(),
        "stillwind/v1\0src/a:1.tsx\0ref\x002:5\0a:b\0"
    );
}

#[test]
fn detached_owner_identities_do_not_reuse_hash_inputs() {
    let context = IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    let helper = ConditionHelperRequest(ConditionContext {
        role: ConditionRole::Gate,
        text: "--a:1".to_owned(),
    });
    context.condition_helper(&helper.0).unwrap();
    let name = context.drain_detached().pop().unwrap();
    assert_eq!(
        name.identity,
        "stillwind-identity/v1\0condition-helper\0gate\0--a:1\0"
    );
    assert_ne!(name.identity, helper.identity());

    let slot = ViewTransitionSlotRequest(class_context("view-transition"));
    context.view_transition_slot(&slot.0).unwrap();
    let name = context.drain_detached().pop().unwrap();
    assert_eq!(
        name.identity,
        "stillwind-identity/v1\0slot\0src/a.tsx\0\x0012\0view-transition\0"
    );
    assert_ne!(name.identity, slot.identity());

    let layer = LayerContext {
        template: CssSourceName::from("theme.css"),
        namespace: "app".to_owned(),
    };
    context.record_layer(&layer, "sw-anon-0");
    let name = context.drain_detached().pop().unwrap();
    assert_eq!(
        name.identity,
        "stillwind-identity/v1\0anonymous-layer\0theme.css\0app\0"
    );
    assert_ne!(name.identity, LayerNameRequest(layer).identity());
}

#[test]
fn a_ref_marker_uses_the_same_class_token_spelling_as_other_classes() {
    let marker = RefMarker::new("__proto__").unwrap();
    assert_eq!(marker.as_str(), "__proto__");
    assert_eq!(marker.selector(), ".__proto__");
    assert_eq!(marker.javascript_string(), "\"__proto__\"");
}

#[test]
fn a_class_token_no_object_holds_keeps_its_spelling() {
    assert_eq!(
        UtilityClass::new("__proto__").unwrap().as_str(),
        "__proto__"
    );
    assert_eq!(
        KeyframesName::new("__proto__").unwrap().as_str(),
        "__proto__"
    );
}

#[test]
fn a_helper_carries_its_role_in_the_hash_and_its_text_beside_it() {
    let helper = |role, text: &str| {
        ConditionHelperRequest(ConditionContext {
            role,
            text: text.to_owned(),
        })
    };
    let gate = helper(ConditionRole::GateValue, "--a:1");
    let inverted = helper(ConditionRole::GateNot, "--a:1");
    // The role is part of the identity, so two helpers for one text never share a memo entry.
    assert_ne!(gate.hash(), inverted.hash());
    // The text's own hash is the same for both, which is what lets a generator name them by the
    // text and spell the role itself.
    assert_eq!(gate.text_hash(), inverted.text_hash());
    assert_ne!(
        gate.text_hash(),
        helper(ConditionRole::GateValue, "--a:2").text_hash()
    );
}

#[test]
fn a_builtin_name_spells_the_role_and_truncates_the_digest() {
    let request = ConditionHelperRequest(ConditionContext {
        role: ConditionRole::Rule,
        text: ".a{color:red}".to_owned(),
    });
    let name = request.builtin(&request.hash(), DEFAULT_HASH_LENGTH);
    let suffix = name.as_str();
    assert_eq!(suffix.len(), DEFAULT_HASH_LENGTH, "{name}");
    assert_eq!(
        name,
        request.builtin(&request.hash(), DEFAULT_HASH_LENGTH),
        "{name}"
    );
    // The name carries no leading hyphens: the custom-property family's render method adds them.
    assert_eq!(
        ConditionHelperName::new(&name).unwrap().property(),
        format!("--{name}"),
        "{name}"
    );
    // Every role letter is its own spelling of one text hash.
    for (role, expected) in [
        (ConditionRole::Fallback, "sw-if-"),
        (ConditionRole::Rule, ""),
        (ConditionRole::Gate, "sw-ifg-"),
        (ConditionRole::GateValue, "sw-ifv-"),
        (ConditionRole::GateNot, "sw-ifn-"),
    ] {
        let spelled = ConditionHelperRequest(ConditionContext {
            role,
            text: ".a{color:red}".to_owned(),
        })
        .builtin(&request.hash(), DEFAULT_HASH_LENGTH);
        assert!(spelled.starts_with(expected), "{spelled}");
        assert!(spelled.ends_with(suffix), "{spelled}");
    }
}

#[test]
fn a_layer_request_names_one_template_and_the_writer_adds_the_index() {
    let request = LayerNameRequest(LayerContext {
        template: CssSourceName::from("entry"),
        namespace: String::new(),
    });
    let name = request.builtin(&request.hash(), DEFAULT_HASH_LENGTH);
    let suffix = name
        .strip_prefix("sw-anon-")
        .unwrap_or_else(|| panic!("{name}"));
    assert_eq!(suffix.len(), DEFAULT_HASH_LENGTH, "{name}");
    // Two layers of one template are distinguished by the index the compiler appends, and a
    // different namespace separates applications that share a template.
    assert_ne!(
        request.hash(),
        LayerNameRequest(LayerContext {
            template: CssSourceName::from("entry"),
            namespace: "other-app".to_owned(),
        })
        .hash()
    );
    assert_ne!(
        request.hash(),
        LayerNameRequest(LayerContext {
            template: CssSourceName::from("other"),
            namespace: String::new(),
        })
        .hash()
    );
}

#[test]
fn a_generator_receives_the_whole_digest() {
    let host = IdentifierContext::new(
        GeneratorSet::new(Arc::new(DigestGenerator)),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    let name = host.class_name(&class_context("0:flex")).unwrap();
    assert_eq!(name.as_str(), "a-full-digest");
}

/// Answers with a body that proves every digest byte arrived.
#[derive(Debug)]
struct DigestGenerator;

impl IdentifierGenerator for DigestGenerator {
    fn utility_class(
        &self,
        _context: &ClassContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, String> {
        let name = if hash.as_bytes().len() == 32 && hash.to_hex().len() == 64 {
            "a-full-digest"
        } else {
            "a-truncated-digest"
        };
        Ok(GeneratedName::provided(
            UtilityClass::new(name).expect("canonical test body"),
        ))
    }
}

#[test]
fn report_spellings_are_distinct_per_family() {
    let kinds = [
        IdentifierKind::UtilityClass,
        IdentifierKind::RefMarker,
        IdentifierKind::AnchorName,
        IdentifierKind::ContainerName,
        IdentifierKind::KeyframesName,
        IdentifierKind::ViewTransitionClass,
        IdentifierKind::ConditionHelper,
        IdentifierKind::RuleBoundary,
        IdentifierKind::ViewTransitionSlot,
        IdentifierKind::LayerName,
    ];
    let mut spellings = kinds.map(IdentifierKind::as_str).to_vec();
    spellings.sort_unstable();
    spellings.dedup();
    assert_eq!(spellings.len(), kinds.len());
}
