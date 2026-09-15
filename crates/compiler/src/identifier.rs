//! Identifier families, CSS namespaces, generation, and write-position escaping.
//!
//! One family is one kind of name the compiler allocates: a utility class, a ref
//! marker, an animation name, and so on. Identity is not the same thing as a CSS
//! namespace, because several families can write into one namespace when the default
//! generators reuse a single string per declaration.
//!
//! [`IdentifierPool`] groups the families whose names must stay unique among
//! themselves, because CSS resolves them in one namespace. Validation compares a name
//! to the identity that produced it, so one declaration that appears in two pools is
//! not a conflict and only distinct identities sharing a spelling are reported.
//!
//! [`GeneratorSet`] is the stage-1 injection point: a host may compute the raw body of
//! any family, and a family the host leaves out keeps the built-in generator. Bodies
//! are hashed from canonical content, so recompiling unchanged sources reproduces the
//! same names; hosts must hold to the same rule, because nothing downstream can tell a
//! deterministic generator from an unstable one.
//!
//! A name becomes a canonical identifier when the characters a write position cannot
//! hold are replaced, and it is escaped separately for each position it is written to.
//! Comparisons, reports, and DOM tokens use the canonical spelling, so a name never
//! changes meaning by being written.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use stillwind_diagnostics::CompileErrorKind;

/// One family of generated names.
///
/// The variant decides which generator a host may override for that family, and which
/// pools the resulting name must be checked in. Serialized spelling is used only in
/// reports, never as name content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IdentifierKind {
    /// A generated utility class, written to the class attribute and class selectors.
    UtilityClass,
    /// A ref marker class, written where a ref is attached.
    RefMarker,
    /// A scope marker class, written where a scope is attached.
    ScopeMarker,
    /// An `anchor-name` custom-ident, which the element being anchored is named by.
    AnchorName,
    /// A `container-name` custom-ident, used by `@container` queries.
    ContainerName,
    /// A `@keyframes` name, referenced by `animation-name`.
    KeyframesName,
    /// A `view-transition-name` custom-ident.
    ViewTransitionName,
    /// A `view-transition-class` custom-ident.
    ViewTransitionClass,
    /// A custom property name, without its leading `--`.
    CustomProperty,
    /// A generated `@layer` name.
    LayerName,
}

impl IdentifierKind {
    /// Stable lowercase spelling used in symbol records and diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UtilityClass => "class",
            Self::RefMarker => "ref-marker",
            Self::ScopeMarker => "scope-marker",
            Self::AnchorName => "anchor",
            Self::ContainerName => "container",
            Self::KeyframesName => "keyframes",
            Self::ViewTransitionName => "view-transition",
            Self::ViewTransitionClass => "view-transition-class",
            Self::CustomProperty => "custom-property",
            Self::LayerName => "layer",
        }
    }

    /// The CSS namespace this family writes into.
    pub const fn pool(self) -> IdentifierPool {
        match self {
            Self::UtilityClass | Self::RefMarker | Self::ScopeMarker => IdentifierPool::ClassToken,
            Self::AnchorName => IdentifierPool::AnchorName,
            Self::ContainerName => IdentifierPool::ContainerName,
            Self::KeyframesName => IdentifierPool::KeyframesName,
            Self::ViewTransitionName => IdentifierPool::ViewTransitionName,
            Self::ViewTransitionClass => IdentifierPool::ViewTransitionClass,
            Self::CustomProperty => IdentifierPool::CustomProperty,
            Self::LayerName => IdentifierPool::LayerName,
        }
    }
}

/// One CSS namespace whose names must be distinct across the document.
///
/// Names in different pools do not interact, so the same spelling may appear in both;
/// within one pool a spelling may only belong to one identity.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum IdentifierPool {
    /// Class tokens and class selectors: utility classes and attachment markers.
    ClassToken,
    /// `anchor-name`, `position-anchor`, and `anchor-scope`.
    AnchorName,
    /// `container-name` and `@container` names.
    ContainerName,
    /// `@keyframes` names and `animation-name` values.
    KeyframesName,
    /// `view-transition-name` values. Runtime-derived keyed names are not allocated here.
    ViewTransitionName,
    /// `view-transition-class` values.
    ViewTransitionClass,
    /// Everything in the `--*` namespace: declarations, references, and registrations.
    CustomProperty,
    /// `@layer` names in the document and in CSS carried by modules.
    LayerName,
}

impl IdentifierPool {
    /// Stable lowercase spelling used in diagnostics and reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClassToken => "class",
            Self::AnchorName => "anchor",
            Self::ContainerName => "container",
            Self::KeyframesName => "keyframes",
            Self::ViewTransitionName => "view-transition",
            Self::ViewTransitionClass => "view-transition-class",
            Self::CustomProperty => "custom-property",
            Self::LayerName => "layer",
        }
    }
}

/// A place a generated name is written to, which decides its usable characters.
///
/// The position fixes the intersection of characters the name may contain; the writer
/// escapes whatever remains, so normalization is not repeated per family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamePosition {
    /// A DOM class token. A token is whitespace-separated, so it cannot hold
    /// whitespace, and it is not escaped.
    DomToken,
    /// A class selector, written as `.name`.
    Selector,
    /// A custom-ident value, such as `anchor-name`, a `@container` name, or a
    /// `view-transition-name`.
    CustomIdent,
    /// A custom property name, written with a leading `--`.
    CustomProperty,
    /// A `@layer` name.
    LayerName,
}

impl NamePosition {
    /// Cleans `name` into the canonical spelling this position can hold.
    ///
    /// The compiler replaces what the position cannot carry and leaves everything else
    /// for the writer to escape. A DOM token is the only position whose characters
    /// cannot be escaped, so it also replaces NUL, which CSS cannot represent at all.
    pub fn canonical(self, name: &str) -> String {
        match self {
            Self::DomToken | Self::Selector => name
                .chars()
                .map(|character| {
                    if character.is_whitespace() || character == '\0' {
                        '_'
                    } else {
                        character
                    }
                })
                .collect(),
            Self::CustomIdent | Self::CustomProperty => name.to_owned(),
            Self::LayerName => name.replace('.', "_"),
        }
    }

    /// Escapes the canonical spelling for this position.
    ///
    /// `name` must already be canonical. Layer names are escaped like selectors,
    /// because a layer name is an identifier with the same grammar.
    pub fn escaped(self, name: &str) -> String {
        match self {
            Self::DomToken => name.to_owned(),
            Self::Selector | Self::CustomIdent | Self::LayerName => {
                let mut escaped = String::new();
                let _ = cssparser::serialize_identifier(name, &mut escaped);
                escaped
            }
            Self::CustomProperty => {
                let mut escaped = String::from("--");
                let _ = cssparser::serialize_identifier(name, &mut escaped);
                escaped
            }
        }
    }
}

/// Writes a class selector for a canonical class token, including its leading dot.
pub(crate) fn selector(name: &str) -> String {
    format!(".{}", NamePosition::Selector.escaped(name))
}

/// Writes a canonical custom-ident value for a property or at-rule prelude.
pub(crate) fn custom_ident(name: &str) -> String {
    NamePosition::CustomIdent.escaped(name)
}

/// The prefix that marks a ref marker class token as Stillwind-owned.
pub(crate) const REF_MARKER_PREFIX: &str = "s_";
/// The prefix that marks a scope marker class token as Stillwind-owned.
pub(crate) const SCOPE_MARKER_PREFIX: &str = "s_";
/// The custom-property prefix of a generated ref anchor name.
pub(crate) const ANCHOR_PREFIX: &str = "--sw-r-";
/// The custom-property prefix of a generated scope container name.
pub(crate) const CONTAINER_PREFIX: &str = "--sw-s-";
/// The custom-property prefix of a generated keyframes name.
pub(crate) const ANIMATION_PREFIX: &str = "--sw-a-";
/// The custom-property prefix of a generated view-transition name.
pub(crate) const VIEW_TRANSITION_PREFIX: &str = "--sw-v-";
/// The prefix of a generated utility class token.
pub(crate) const CLASS_PREFIX: &str = "_s_";

/// The raw body a generator produced for one name, before any family prefix.
///
/// The body is the family-specific part of the name, and the writing step adds the
/// prefix. Symbol records and diagnostics report the body, so a failure points at the
/// content the host chose rather than at a name the compiler assembled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedBody {
    /// Host-supplied body, before the family prefix.
    pub body: String,
}

impl GeneratedBody {
    /// Wraps `body` as a generated name body.
    pub fn new(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }
}

/// A host-provided generator for one or more identifier families.
///
/// Implementations must be pure: the same request must produce the same body, and the
/// result must not depend on call order, call count, time, randomness, or the build
/// mode. A family the implementation does not handle keeps the built-in generator, so
/// overriding a single family is a supported use.
///
/// Sharing a generator across threads only forwards calls, because the purity contract
/// already forbids state-dependent output. Generators are still used synchronously
/// within one compilation.
pub trait IdentifierGenerator: fmt::Debug + Send + Sync {
    /// Whether this generator produces names of `kind`.
    ///
    /// A generator that answers true must return a body for every later request of
    /// that kind, or fail the compilation.
    fn handles(&self, kind: IdentifierKind) -> bool;

    /// Produces the body of one name.
    ///
    /// `input` is the canonical identity input for the family, and `label` is the
    /// authored label normalized for name use, present only where the family has one.
    /// Repeated requests with the same kind, input, and label reuse the first result,
    /// so implementations do not need memoization of their own.
    ///
    /// # Errors
    ///
    /// Returns a failure that becomes a diagnostic at the declaration being named.
    /// Implementations must not panic: a panic aborts the host's build.
    fn generate(
        &self,
        kind: IdentifierKind,
        input: &str,
        label: Option<&str>,
    ) -> Result<GeneratedBody, String>;
}

/// The generators one compilation uses, with the built-in generator as fallback.
///
/// Equality compares generator identity, so rebuilding a set from the same generator
/// compares equal while a different generator never does.
#[derive(Clone, Debug, Default)]
pub struct GeneratorSet {
    generator: Option<Arc<dyn IdentifierGenerator>>,
}

impl GeneratorSet {
    /// A set that generates every name with the built-in generator.
    pub fn builtin() -> Self {
        Self::default()
    }

    /// A set that asks `generator` first and falls back per family.
    pub fn new(generator: Arc<dyn IdentifierGenerator>) -> Self {
        Self {
            generator: Some(generator),
        }
    }

    /// The host generator, if any.
    pub fn generator(&self) -> Option<&dyn IdentifierGenerator> {
        self.generator.as_deref()
    }
}

impl PartialEq for GeneratorSet {
    fn eq(&self, other: &Self) -> bool {
        match (&self.generator, &other.generator) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for GeneratorSet {}

/// One compilation's name allocation: host generator first, built-in fallback, memoized.
///
/// A kind and input pair always yields one result within a compilation, so a helper
/// asked twice cannot observe two calls of an impure generator and cannot produce two
/// names for one declaration. Fallible generation memoizes the decoded failure message,
/// so a repeated request reports the same failure without calling the generator again.
#[derive(Clone, Debug)]
pub struct IdentifierContext {
    generators: GeneratorSet,
    memo: RefCell<BTreeMap<MemoKey, Result<String, String>>>,
}

/// Memo key: the family, its canonical input, and the normalized label if it has one.
type MemoKey = (IdentifierKind, String, Option<String>);

impl IdentifierContext {
    /// Starts a context for `generators` with an empty memo.
    pub fn new(generators: GeneratorSet) -> Self {
        Self {
            generators,
            memo: RefCell::new(BTreeMap::new()),
        }
    }

    /// Starts a context that uses the built-in generator for every family.
    pub fn builtin() -> Self {
        Self::new(GeneratorSet::builtin())
    }

    /// The generators this context was built from.
    pub fn generators(&self) -> &GeneratorSet {
        &self.generators
    }

    /// Whether a host generator supplies the bodies of `kind`.
    pub fn handles(&self, kind: IdentifierKind) -> bool {
        self.generators
            .generator()
            .is_some_and(|generator| generator.handles(kind))
    }

    /// The body for one name, or the built-in body when no generator handles `kind`.
    ///
    /// The body is canonical: characters the family's write positions cannot carry are
    /// already replaced.
    ///
    /// # Errors
    ///
    /// Returns [`CompileErrorKind::IdentifierGenerationFailed`] when the host generator
    /// handles `kind` and rejects this input.
    pub fn body(
        &self,
        kind: IdentifierKind,
        input: &str,
        label: Option<&str>,
    ) -> Result<GeneratedBody, CompileErrorKind> {
        let key = (kind, input.to_owned(), label.map(str::to_owned));
        if let Some(cached) = self.memo.borrow().get(&key) {
            return cached
                .clone()
                .map(GeneratedBody::new)
                .map_err(|message| generation_failure(kind, message));
        }
        let generated = match self.generators.generator() {
            Some(generator) if generator.handles(kind) => {
                let positions = write_positions(kind);
                generator
                    .generate(kind, input, label)
                    .map(|body| GeneratedBody::new(canonicalize(positions, &body.body)))
                    .map_err(|message| generation_failure(kind, message))
            }
            _ => Ok(builtin_body(kind, input, label)),
        };
        self.memo.borrow_mut().insert(
            key,
            generated
                .as_ref()
                .map(|body| body.body.clone())
                .map_err(ToString::to_string),
        );
        generated
    }

    /// A utility class token for one canonical class input.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn class_name(&self, canonical_input: &str) -> Result<String, CompileErrorKind> {
        let body = self
            .body(IdentifierKind::UtilityClass, canonical_input, None)?
            .body;
        Ok(format!("{CLASS_PREFIX}{body}"))
    }

    /// A ref marker class token, without a leading dot.
    ///
    /// `label` is the authored label; whitespace and NUL are normalized before the
    /// generator sees it.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn ref_marker_name(
        &self,
        input: &str,
        label: Option<&str>,
    ) -> Result<String, CompileErrorKind> {
        self.marker_name(IdentifierKind::RefMarker, REF_MARKER_PREFIX, input, label)
    }

    /// A scope marker class token, without a leading dot.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn scope_marker_name(
        &self,
        input: &str,
        label: Option<&str>,
    ) -> Result<String, CompileErrorKind> {
        self.marker_name(
            IdentifierKind::ScopeMarker,
            SCOPE_MARKER_PREFIX,
            input,
            label,
        )
    }

    /// An `anchor-name` custom-ident for a ref's identity.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn anchor_name(
        &self,
        input: &str,
        label: Option<&str>,
    ) -> Result<String, CompileErrorKind> {
        let body = self.body(IdentifierKind::AnchorName, input, label)?.body;
        Ok(format!("{ANCHOR_PREFIX}{body}"))
    }

    /// A `container-name` custom-ident for a scope's identity.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn container_name(&self, input: &str) -> Result<String, CompileErrorKind> {
        let body = self.body(IdentifierKind::ContainerName, input, None)?.body;
        Ok(format!("{CONTAINER_PREFIX}{body}"))
    }

    /// A `@keyframes` name, referenced by `animation-name`.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn animation_name(&self, input: &str) -> Result<String, CompileErrorKind> {
        let body = self.body(IdentifierKind::KeyframesName, input, None)?.body;
        Ok(format!("{ANIMATION_PREFIX}{body}"))
    }

    /// A `view-transition-name` custom-ident.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn view_transition_name(&self, input: &str) -> Result<String, CompileErrorKind> {
        let body = self
            .body(IdentifierKind::ViewTransitionName, input, None)?
            .body;
        Ok(format!("{VIEW_TRANSITION_PREFIX}{body}"))
    }

    /// A marker class token with `prefix` and an optional normalized label.
    fn marker_name(
        &self,
        kind: IdentifierKind,
        prefix: &str,
        input: &str,
        label: Option<&str>,
    ) -> Result<String, CompileErrorKind> {
        let normalized = label.map(marker_label);
        let body = self.body(kind, input, normalized.as_deref())?.body;
        Ok(format!("{prefix}{body}"))
    }
}

/// The write positions one family appears in.
///
/// A name may be written more than once; the canonical spelling must be usable in every
/// position, so a character is only kept when every position can hold it.
fn write_positions(kind: IdentifierKind) -> &'static [NamePosition] {
    match kind {
        IdentifierKind::UtilityClass | IdentifierKind::RefMarker | IdentifierKind::ScopeMarker => {
            &[NamePosition::DomToken, NamePosition::Selector]
        }
        IdentifierKind::AnchorName
        | IdentifierKind::ContainerName
        | IdentifierKind::KeyframesName
        | IdentifierKind::ViewTransitionName
        | IdentifierKind::ViewTransitionClass => &[NamePosition::CustomIdent],
        IdentifierKind::CustomProperty => &[NamePosition::CustomProperty],
        IdentifierKind::LayerName => &[NamePosition::LayerName],
    }
}

/// Normalizes a generated body for every position its family is written to.
fn canonicalize(positions: &[NamePosition], name: &str) -> String {
    positions
        .iter()
        .fold(name.to_owned(), |name, position| position.canonical(&name))
}

/// Normalizes an authored label for use inside a generated marker token.
///
/// A marker token is written to the DOM and to a class selector, so whitespace and NUL
/// are replaced and every other authored character is kept readable. Escaping for the
/// selector happens where the selector is written, which is what lets a label keep
/// characters such as `:`.
pub(crate) fn marker_label(label: &str) -> String {
    NamePosition::DomToken.canonical(label)
}

/// Wraps a generator failure with the family it concerned.
fn generation_failure(kind: IdentifierKind, message: String) -> CompileErrorKind {
    CompileErrorKind::IdentifierGenerationFailed(kind.as_str().into(), message.into())
}

/// The built-in body for one family: a hash of the identity input.
///
/// Class names hash the input alone, because a class is unique per call and branch
/// rather than per declaration, and ref-like families mix in the normalized label so
/// the generated name stays readable where a DOM token shows it.
pub(crate) fn builtin_body(
    kind: IdentifierKind,
    input: &str,
    label: Option<&str>,
) -> GeneratedBody {
    let hash = blake3::hash(input.as_bytes());
    let hex = hash.to_hex();
    let body = match kind {
        IdentifierKind::RefMarker
        | IdentifierKind::ScopeMarker
        | IdentifierKind::AnchorName
        | IdentifierKind::ContainerName => match label {
            Some(label) => format!("{label}_{}", &hex[..8]),
            None => hex[..8].to_owned(),
        },
        IdentifierKind::UtilityClass
        | IdentifierKind::KeyframesName
        | IdentifierKind::ViewTransitionName
        | IdentifierKind::ViewTransitionClass
        | IdentifierKind::CustomProperty
        | IdentifierKind::LayerName => hex[..8].to_owned(),
    };
    GeneratedBody::new(canonicalize(write_positions(kind), &body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Records requests so memoization and per-family fallback are observable.
    #[derive(Debug, Default)]
    struct CountingGenerator {
        handled: Vec<IdentifierKind>,
        calls: Mutex<Vec<(IdentifierKind, String, Option<String>)>>,
    }

    impl IdentifierGenerator for CountingGenerator {
        fn handles(&self, kind: IdentifierKind) -> bool {
            self.handled.contains(&kind)
        }

        fn generate(
            &self,
            kind: IdentifierKind,
            input: &str,
            label: Option<&str>,
        ) -> Result<GeneratedBody, String> {
            self.calls.lock().expect("test generator lock").push((
                kind,
                input.to_owned(),
                label.map(str::to_owned),
            ));
            Ok(GeneratedBody::new(format!("gen-{kind:?}")))
        }
    }

    #[test]
    fn unhandled_families_keep_the_builtin_body() {
        let generator = Arc::new(CountingGenerator {
            handled: vec![IdentifierKind::RefMarker],
            ..CountingGenerator::default()
        });
        let context = IdentifierContext::new(GeneratorSet::new(generator.clone()));
        let custom = context
            .body(IdentifierKind::RefMarker, "ref\0x", Some("label"))
            .unwrap();
        let builtin = context
            .body(IdentifierKind::UtilityClass, "class\0x", None)
            .unwrap();
        assert_eq!(custom.body, "gen-RefMarker");
        assert_eq!(
            builtin,
            builtin_body(IdentifierKind::UtilityClass, "class\0x", None)
        );
        assert_eq!(generator.calls.lock().expect("lock").len(), 1);
    }

    #[test]
    fn repeated_requests_reuse_one_generator_call() {
        let generator = Arc::new(CountingGenerator {
            handled: vec![IdentifierKind::KeyframesName],
            ..CountingGenerator::default()
        });
        let context = IdentifierContext::new(GeneratorSet::new(generator.clone()));
        let first = context
            .body(IdentifierKind::KeyframesName, "spin", None)
            .unwrap();
        let second = context
            .body(IdentifierKind::KeyframesName, "spin", None)
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(generator.calls.lock().expect("lock").len(), 1);
        let labelled = context
            .body(IdentifierKind::KeyframesName, "spin", Some("enter"))
            .unwrap();
        assert_eq!(labelled.body, first.body);
        assert_eq!(generator.calls.lock().expect("lock").len(), 2);
        assert_eq!(
            generator.calls.lock().expect("lock")[1],
            (
                IdentifierKind::KeyframesName,
                "spin".to_owned(),
                Some("enter".to_owned())
            )
        );
    }

    #[test]
    fn a_generator_body_is_normalized_for_every_position_it_is_written_to() {
        let generator = Arc::new(PositionalGenerator);
        let context = IdentifierContext::new(GeneratorSet::new(generator));
        let marker = context
            .body(IdentifierKind::RefMarker, "ref", None)
            .unwrap();
        assert_eq!(marker.body, "fixed_token");
        let animation = context
            .body(IdentifierKind::KeyframesName, "spin", None)
            .unwrap();
        assert_eq!(animation.body, "fixed-ok");
    }

    #[test]
    fn builtin_bodies_hash_the_input_and_keep_labels_readable() {
        assert_eq!(
            builtin_body(IdentifierKind::UtilityClass, "flex", None),
            builtin_body(IdentifierKind::UtilityClass, "flex", None)
        );
        assert_ne!(
            builtin_body(IdentifierKind::UtilityClass, "flex", None),
            builtin_body(IdentifierKind::UtilityClass, "grid", None)
        );
        let labelled = builtin_body(IdentifierKind::RefMarker, "ref", Some("primary"));
        assert!(labelled.body.starts_with("primary_"), "{labelled:?}");
        assert_eq!(labelled.body.len(), "primary_".len() + 8);
    }

    #[test]
    fn positions_replace_what_they_cannot_hold_and_escape_the_rest() {
        assert_eq!(NamePosition::DomToken.canonical("a b\0c"), "a_b_c");
        assert_eq!(NamePosition::Selector.canonical("a\tb"), "a_b");
        assert_eq!(NamePosition::CustomIdent.canonical("a b"), "a b");
        assert_eq!(NamePosition::LayerName.canonical("sw.anon"), "sw_anon");
        assert_eq!(selector("a:b/c"), r".a\:b\/c");
        assert_eq!(custom_ident("a:b"), r"a\:b");
        assert_eq!(NamePosition::CustomProperty.escaped("a:b"), r"--a\:b");
        assert_eq!(NamePosition::LayerName.escaped("sw_anon"), "sw_anon");
    }

    /// Emits a body that each position must clean.
    #[derive(Debug)]
    struct PositionalGenerator;

    impl IdentifierGenerator for PositionalGenerator {
        fn handles(&self, kind: IdentifierKind) -> bool {
            matches!(
                kind,
                IdentifierKind::RefMarker | IdentifierKind::KeyframesName
            )
        }

        fn generate(
            &self,
            kind: IdentifierKind,
            _input: &str,
            _label: Option<&str>,
        ) -> Result<GeneratedBody, String> {
            Ok(GeneratedBody::new(match kind {
                IdentifierKind::RefMarker => "fixed token".to_owned(),
                _ => "fixed-ok".to_owned(),
            }))
        }
    }

    #[test]
    fn families_map_to_the_namespace_they_are_written_in() {
        assert_eq!(
            IdentifierKind::UtilityClass.pool(),
            IdentifierPool::ClassToken
        );
        assert_eq!(IdentifierKind::RefMarker.pool(), IdentifierPool::ClassToken);
        assert_eq!(
            IdentifierKind::AnchorName.pool(),
            IdentifierPool::AnchorName
        );
        assert_eq!(
            IdentifierKind::ViewTransitionClass.pool(),
            IdentifierPool::ViewTransitionClass
        );
        assert_eq!(
            IdentifierKind::KeyframesName.pool(),
            IdentifierPool::KeyframesName
        );
    }

    #[test]
    fn report_spellings_are_distinct_per_family() {
        let kinds = [
            IdentifierKind::UtilityClass,
            IdentifierKind::RefMarker,
            IdentifierKind::ScopeMarker,
            IdentifierKind::AnchorName,
            IdentifierKind::ContainerName,
            IdentifierKind::KeyframesName,
            IdentifierKind::ViewTransitionName,
            IdentifierKind::ViewTransitionClass,
            IdentifierKind::CustomProperty,
            IdentifierKind::LayerName,
        ];
        let mut spellings = kinds.map(IdentifierKind::as_str).to_vec();
        spellings.sort_unstable();
        spellings.dedup();
        assert_eq!(spellings.len(), kinds.len());
    }
}
