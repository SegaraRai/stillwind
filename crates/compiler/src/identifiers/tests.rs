use std::sync::Arc;
use std::sync::Mutex;

use stillwind_diagnostics::CompileErrorKind;

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
        module: "src/a.tsx".to_owned(),
        namespace: String::new(),
        call: 12,
        branch: branch.to_owned(),
    }
}

fn declaration_context(label: Option<&str>) -> DeclarationContext {
    DeclarationContext {
        module: "src/a.tsx".to_owned(),
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
        projection: None,
    }
}

#[test]
fn a_host_name_is_written_without_the_builtin_prefix() {
    let context =
        IdentifierContext::new(GeneratorSet::new(Arc::new(RecordingGenerator::default())));
    let name = context.class_name(&class_context("0:flex")).unwrap();
    assert_eq!(name.as_str(), "fixed-class");
    let builtin = IdentifierContext::builtin()
        .class_name(&class_context("0:flex"))
        .unwrap();
    assert!(builtin.as_str().starts_with("_s_"), "{builtin:?}");
}

#[test]
fn every_builtin_family_keeps_its_own_prefix() {
    let context = IdentifierContext::builtin();
    let declaration = declaration_context(Some("primary"));
    assert!(
        context
            .ref_marker_name(&declaration)
            .unwrap()
            .as_str()
            .starts_with("s_")
    );
    assert!(
        context
            .subtree_marker_name(&declaration_context(None))
            .unwrap()
            .as_str()
            .starts_with("s_")
    );
    assert!(
        context
            .anchor_name(&locator_context(&declaration))
            .unwrap()
            .as_str()
            .starts_with("--sw-r-")
    );
    assert!(
        context
            .container_name(&locator_context(&declaration))
            .unwrap()
            .as_str()
            .starts_with("--sw-s-")
    );
    assert!(
        context
            .keyframes_name(&declaration)
            .unwrap()
            .as_str()
            .starts_with("--sw-a-")
    );
    assert!(
        context
            .view_transition_class(&declaration)
            .unwrap()
            .as_str()
            .starts_with("--sw-v-")
    );
}

#[test]
fn repeated_requests_use_one_generator_call() {
    let generator = Arc::new(RecordingGenerator::default());
    let context = IdentifierContext::new(GeneratorSet::new(generator.clone()));
    let first = context.class_name(&class_context("0:flex")).unwrap();
    let second = context.class_name(&class_context("0:flex")).unwrap();
    assert_eq!(first, second);
    assert_eq!(generator.calls.lock().expect("lock").len(), 1);
}

#[test]
fn development_mode_substitutes_and_records_a_rejected_name() {
    let context = IdentifierContext::in_mode(
        GeneratorSet::new(Arc::new(FailingGenerator)),
        IdentifierMode::Development,
    );
    let name = context.class_name(&class_context("0:flex")).unwrap();
    assert!(name.as_str().starts_with("_s_"), "{name:?}");
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
    let context = IdentifierContext::new(GeneratorSet::new(Arc::new(FailingGenerator)));
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
fn a_class_token_an_object_already_holds_gains_a_prefix_when_it_is_built() {
    let marker = RefMarker::new("__proto__").unwrap();
    assert_eq!(marker.as_str(), "___proto__");
    assert_eq!(marker.selector(), ".___proto__");
    assert_eq!(marker.javascript_string(), "\"___proto__\"");
    assert_eq!(
        SubtreeMarker::new("hasOwnProperty").unwrap().as_str(),
        "_hasOwnProperty"
    );
    // The prefix is applied once, however many times a name is rebuilt.
    assert_eq!(
        RefMarker::new(marker.as_str()).unwrap().as_str(),
        "___proto__"
    );
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
    let name = request.builtin(&request.hash());
    let suffix = name
        .strip_prefix("sw-if-rule-")
        .unwrap_or_else(|| panic!("{name}"));
    assert_eq!(suffix.len(), 16, "{name}");
    assert_eq!(name, request.builtin(&request.hash()), "{name}");
    // The name carries no leading hyphens: the custom-property family's render method adds them.
    assert_eq!(
        CustomPropertyName::new(&name).unwrap().property(),
        format!("--{name}"),
        "{name}"
    );
}

#[test]
fn a_layer_request_names_one_template_and_the_writer_adds_the_index() {
    let request = LayerNameRequest(LayerContext {
        template: "entry".to_owned(),
        namespace: String::new(),
    });
    let name = request.builtin(&request.hash());
    let suffix = name
        .strip_prefix("sw-anon-")
        .unwrap_or_else(|| panic!("{name}"));
    assert_eq!(suffix.len(), 16, "{name}");
    // Two layers of one template are distinguished by the index the compiler appends, and a
    // different namespace separates applications that share a template.
    assert_ne!(
        request.hash(),
        LayerNameRequest(LayerContext {
            template: "entry".to_owned(),
            namespace: "other-app".to_owned(),
        })
        .hash()
    );
    assert_ne!(
        request.hash(),
        LayerNameRequest(LayerContext {
            template: "other".to_owned(),
            namespace: String::new(),
        })
        .hash()
    );
}

#[test]
fn a_generator_receives_the_whole_digest() {
    let host = IdentifierContext::new(GeneratorSet::new(Arc::new(DigestGenerator)));
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
        IdentifierKind::SubtreeMarker,
        IdentifierKind::AnchorName,
        IdentifierKind::ContainerName,
        IdentifierKind::KeyframesName,
        IdentifierKind::ViewTransitionClass,
        IdentifierKind::CustomProperty,
        IdentifierKind::LayerName,
    ];
    let mut spellings = kinds.map(IdentifierKind::as_str).to_vec();
    spellings.sort_unstable();
    spellings.dedup();
    assert_eq!(spellings.len(), kinds.len());
}
