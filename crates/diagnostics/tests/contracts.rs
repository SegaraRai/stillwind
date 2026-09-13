//! Reporting identity, presentation, and source-location contracts.

use stillwind_diagnostics::{
    CompileError, CompileErrorKind, Label, LintFinding, LintKind, SourceFailureContext,
    UtilityFailure, filter_lint_findings,
};
use stillwind_source::{FileId, SourceError, SourceFile, Span};

#[test]
fn distinct_causes_share_codes_without_sharing_messages() {
    let invalid = CompileErrorKind::InvalidVariantPrefix;
    let unavailable = CompileErrorKind::UnavailableVariantPrefix;
    let unidentified = CompileErrorKind::UnidentifiedRef;
    assert_ne!(invalid, unavailable);
    assert_eq!(invalid.code(), "SHREF003");
    assert_eq!(unavailable.code(), invalid.code());
    assert_eq!(unidentified.code(), invalid.code());
    assert_eq!(invalid.to_string(), "invalid variant prefix");
    assert_eq!(
        unavailable.to_string(),
        "unknown or unavailable variant prefix"
    );
    assert_eq!(
        unidentified.to_string(),
        "ref variant has no known ref identity"
    );
}

#[test]
fn one_message_template_can_report_multiple_codes() {
    let findings = [UtilityFailure::Invalid, UtilityFailure::Unknown].map(|classification| {
        LintKind::UnresolvedUtility {
            utility: "hover:[".into(),
            classification,
        }
    });
    assert_eq!(findings[0].code(), "invalid-utility");
    assert_eq!(findings[1].code(), "unknown-utility");
    assert_eq!(findings[0].to_string(), findings[1].to_string());
    assert_eq!(
        findings[0].to_string(),
        "unknown or unavailable utility `hover:[`"
    );

    for (context, code) in [
        (SourceFailureContext::Utility, "SHSPAN001"),
        (SourceFailureContext::Projection, "SHPRJ001"),
        (SourceFailureContext::Stylesheet, "SHCSS001"),
    ] {
        let error = CompileErrorKind::source_failure(
            &SourceError::InvalidRange { start: 9, end: 2 },
            context,
        );
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), "invalid source byte range 9..2");
    }
}

#[test]
fn wrappers_roundtrip_owned_causes_labels_and_notes() {
    let span = Span::new(FileId(7), 4, 12);
    let mut error = CompileError::new(
        CompileErrorKind::GeneratedIdentifierCollision {
            name: "衝突".into(),
        },
        span,
    );
    error.labels.push(Label {
        span: Span::new(FileId(8), 0, 3),
        message: Some("previous definition".into()),
    });
    error
        .notes
        .push("reported while publishing the module".into());
    let json = serde_json::to_string(&error).unwrap();
    let restored: CompileError = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, error);
    assert_eq!(restored.code(), "SHSYMBOL001");
    assert_eq!(restored.to_string(), "generated identifier collision: 衝突");
    assert_eq!(
        std::error::Error::source(&restored).unwrap().to_string(),
        restored.to_string()
    );

    let finding = LintFinding::new(
        LintKind::MissingAnchor {
            name: "対象".into(),
        },
        span,
    );
    let json = serde_json::to_string(&finding).unwrap();
    assert_eq!(serde_json::from_str::<LintFinding>(&json).unwrap(), finding);
    assert_eq!(finding.code(), "require-anchor-provider");
    assert_eq!(
        finding.to_string(),
        "ref `対象` uses position without an anchor projection"
    );
}

#[test]
fn suppression_uses_primary_label_and_typed_rule_only() {
    let source = SourceFile::new(
        FileId(1),
        "entry.ts",
        "// swlint-disable-next-line require-anchor-provider\nx\ny",
    )
    .unwrap();
    let x = u32::try_from(source.text().find("\nx").unwrap() + 1).unwrap();
    let primary = Span::new(source.id(), x, x + 1);
    let secondary = Span::new(source.id(), x + 2, x + 3);
    let mut suppressed = LintFinding::new(LintKind::MissingAnchor { name: "a".into() }, primary);
    suppressed.labels.push(Label {
        span: secondary,
        message: None,
    });
    let retained = LintFinding::new(LintKind::MissingContainer { name: "a".into() }, primary);
    let mut findings = vec![suppressed, retained.clone()];
    filter_lint_findings(&source, &mut findings);
    assert_eq!(findings, vec![retained]);
}

#[test]
fn source_name_is_wrapper_context_and_survives_serialization() {
    let error = CompileError::new(
        CompileErrorKind::UnterminatedCssBlock { offset: 8 },
        Span::new(FileId(77), 0, 9),
    )
    .with_source_name("theme.css");
    assert_eq!(error.to_string(), format!("theme.css: {}", error.kind));
    assert_eq!(error.code(), error.kind.code());
    let restored: CompileError =
        serde_json::from_str(&serde_json::to_string(&error).unwrap()).unwrap();
    assert_eq!(restored, error);
}
