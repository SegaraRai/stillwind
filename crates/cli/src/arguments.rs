//! Positional-free argument lookup over `std::env::args`.
//!
//! Arguments are matched by exact name, so a flag never depends on its position and an
//! unknown argument is skipped during name lookup. A value lookup accepts the next
//! argument verbatim, even if it looks like another flag. Only the command word
//! is positional. Nothing here parses `--name=value`; the CLI uses the separated form
//! throughout.

use std::env;

/// Failure to find a required argument or its value.
#[derive(Debug, thiserror::Error)]
pub(super) enum ArgumentError {
    /// The argument was present but nothing followed it.
    #[error("missing value for {name}")]
    MissingValue { name: String },
    /// The argument was never present.
    #[error("missing required {name} argument")]
    MissingArgument { name: String },
}

/// Returns the value following `name`.
///
/// The first occurrence wins; a repeated argument is not an error.
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

/// Returns the value following `name`, or `None` when it is absent.
///
/// An argument present without a value also yields `None`, which is why callers pair
/// this with a default rather than treating absence as an error.
pub(super) fn optional_argument(name: &str) -> Option<String> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments.next();
        }
    }
    None
}

/// True when `name` appears anywhere in the argument list.
pub(super) fn has_flag(name: &str) -> bool {
    env::args().skip(1).any(|argument| argument == name)
}
