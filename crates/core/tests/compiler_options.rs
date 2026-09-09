use stillwind_core::{CompilerOptions, CompilerOptionsError};

#[test]
fn invalid_options_identify_the_option_and_value() {
    let options = CompilerOptions {
        prefix: "bad".into(),
        ..CompilerOptions::default()
    };
    assert!(
        matches!(options.validate(), Err(CompilerOptionsError::InvalidPrefix { option: "prefix", value }) if value == "bad")
    );
    let options = CompilerOptions {
        tailwind_prefix: "bad".into(),
        ..CompilerOptions::default()
    };
    assert!(
        matches!(options.validate(), Err(CompilerOptionsError::InvalidPrefix { option: "tailwindPrefix", value }) if value == "bad")
    );
    let options = CompilerOptions {
        tailwind_prefix: "--sw-".into(),
        ..CompilerOptions::default()
    };
    assert!(
        matches!(options.validate(), Err(CompilerOptionsError::ConflictingPrefixes { prefix }) if prefix == "--sw-")
    );
}
