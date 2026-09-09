use super::*;

#[test]
fn request_id_exhaustion_is_terminal_and_never_emits_max_or_wraps() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    loader.next_id = u32::MAX - 1;
    let DesignSystemLoadStep::NeedFile(requests) =
        loader.provide(0, "root".into(), "@import 'last';".into())
    else {
        panic!("expected last usable request ID");
    };
    assert_eq!(requests[0].id, u32::MAX - 1);
    assert!(matches!(
        loader.provide(u32::MAX - 1, "last".into(), "@import 'overflow';".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::RequestIdExhausted))
    ));
    assert_eq!(loader.next_id, u32::MAX);
    assert!(matches!(
        loader.provide_many([]),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Protocol { .. }))
    ));
}

#[test]
fn exhausted_batch_finishes_with_error_instead_of_returning_partial_requests() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    loader.next_id = u32::MAX - 1;
    assert!(matches!(
        loader.provide(
            0,
            "root".into(),
            "@import 'last'; @import 'overflow';".into()
        ),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::RequestIdExhausted))
    ));
    assert_eq!(loader.next_id, u32::MAX);
}
