use std::env;
pub(super) fn argument(name: &str) -> Result<String, ArgumentError> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments
                .next()
                .ok_or_else(|| ArgumentError::MissingValue { name: name.into() });
        }
    }
    Err(ArgumentError::MissingArgument { name: name.into() })
}

pub(super) fn optional_argument(name: &str) -> Option<String> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments.next();
        }
    }
    None
}

pub(super) fn has_flag(name: &str) -> bool {
    env::args().skip(1).any(|argument| argument == name)
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ArgumentError {
    #[error("missing value for {name}")]
    MissingValue { name: String },
    #[error("missing required {name} argument")]
    MissingArgument { name: String },
}
