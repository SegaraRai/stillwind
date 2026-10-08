//! Property keys used when generated JavaScript stores compiled asset identities.

/// Object key under which a compiled ref record carries its canonical marker token.
pub(in crate::compiler) const CLASS_KEY: &str = "c";

/// Object key under which a keyframes value carries its generated keyframes name.
///
/// The name is what the declaration's `animation-name` states, and a use selects its
/// prebuilt class by reading it.
pub(in crate::compiler) const KEYFRAMES_KEY: &str = "k";

/// Object key under which a view-transition value carries the class token that marks
/// its participants.
pub(in crate::compiler) const VIEW_TRANSITION_CLASS_KEY: &str = "v";
