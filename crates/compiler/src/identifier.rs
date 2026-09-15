//! Generated identifier families and the CSS namespaces they share.
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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
/// names for one declaration.
///
/// Fallible generation memoizes the decoded failure message, and the reported diagnostic
/// is rebuilt from it, so a repeated request reports the same failure without calling
/// the generator again.
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
    /// Repeated requests with the same kind, input, and label reuse the first result,
    /// including a failure, so one compilation reports a failing generator once per
    /// distinct request rather than once per call site.
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
            Some(generator) if generator.handles(kind) => generator
                .generate(kind, input, label)
                .map_err(|message| generation_failure(kind, message)),
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
}

/// Wraps a generator failure with the family it concerned.
fn generation_failure(kind: IdentifierKind, message: String) -> CompileErrorKind {
    CompileErrorKind::IdentifierGenerationFailed(kind.as_str().into(), message.into())
}

/// The Stillwind-owned custom-property prefix before a host-level rewrite.
const STILLWIND_PREFIX: &str = "--sw-";

/// Prefix of a generated ref marker class token.
const REF_MARKER_PREFIX: &str = "s_";
/// Prefix of a generated scope marker class token.
const SCOPE_MARKER_PREFIX: &str = "s_";
/// Prefix of a generated keyframes name.
const ANIMATION_PREFIX: &str = "--sw-a-";
/// Prefix of a generated view-transition name.
const VIEW_TRANSITION_PREFIX: &str = "--sw-v-";
/// Prefix of a generated utility class token.
const CLASS_PREFIX: &str = "_s_";

/// Normalizes an authored label for use inside a generated name.
///
/// Characters are the authored ones so the token stays readable, with two exceptions:
/// whitespace becomes `_`, and NUL becomes U+FFFD because CSS cannot represent it. The
/// result is a DOM class token, so escaping belongs to the CSS and JavaScript
/// boundaries that write it out.
pub(crate) fn marker_label(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_whitespace() {
                '_'
            } else if character == '\0' {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

impl IdentifierContext {
    /// The body for `kind`, or the built-in body, as a diagnostic-carrying result.
    fn required(
        &self,
        kind: IdentifierKind,
        input: &str,
        label: Option<&str>,
    ) -> Result<String, CompileErrorKind> {
        Ok(self.body(kind, input, label)?.body)
    }

    /// A utility class token for one canonical class input.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn class_name(&self, canonical_input: &str) -> Result<String, CompileErrorKind> {
        let body = self.required(IdentifierKind::UtilityClass, canonical_input, None)?;
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
    /// The built-in generator delegates to the ref marker generator, so one declaration
    /// has one string in both namespaces unless a host overrides a family.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn anchor_name(
        &self,
        input: &str,
        label: Option<&str>,
    ) -> Result<String, CompileErrorKind> {
        let body = self.required(IdentifierKind::AnchorName, input, label)?;
        Ok(format!("{STILLWIND_PREFIX}r-{body}"))
    }

    /// A `container-name` custom-ident for a scope's identity.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn container_name(&self, input: &str) -> Result<String, CompileErrorKind> {
        let body = self.required(IdentifierKind::ContainerName, input, None)?;
        Ok(format!("{STILLWIND_PREFIX}s-{body}"))
    }

    /// A `@keyframes` name, referenced by `animation-name`.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn animation_name(&self, input: &str) -> Result<String, CompileErrorKind> {
        let body = self.required(IdentifierKind::KeyframesName, input, None)?;
        Ok(format!("{ANIMATION_PREFIX}{body}"))
    }

    /// A `view-transition-name` custom-ident.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for this family.
    pub(crate) fn view_transition_name(&self, input: &str) -> Result<String, CompileErrorKind> {
        let body = self.required(IdentifierKind::ViewTransitionName, input, None)?;
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
        let body = self.required(kind, input, normalized.as_deref())?;
        Ok(format!("{prefix}{body}"))
    }
}

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
    GeneratedBody::new(body)
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
