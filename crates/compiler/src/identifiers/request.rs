//! The context one name is allocated from, and the request that carries it.
//!
//! A request is what tells the families apart: a ref marker and a keyframes name share their
//! context type but not their family, and an anonymous layer has a context type of its own. Naming
//! the family in the request type keeps the identity text, the kind, and the generator method in
//! one declaration, and gives one compilation one memo entry per family and identity.
//!
//! The identity text is the versioned spelling a request is hashed from. [`NameRequest::hash`]
//! returns the whole digest, which is what identifies the request, and [`NameRequest::builtin`]
//! turns it into the compiler's own name. A family whose spelling is not the prefix followed by the
//! digest declares that spelling here, next to its identity rule.

use super::generator::{GeneratedName, IdentifierGenerator, builtin_text};
use super::kind::{IdentifierKind, builtin_digest_digits, builtin_prefix};
use super::name::{
    AnchorName, ContainerName, CustomPropertyName, KeyframesName, Label, LayerName, RefMarker,
    SubtreeMarker, Token, UtilityClass, ViewTransitionClass,
};

/// The canonical hash of one request, which is what identifies what was asked for.
///
/// A generator receives the whole digest rather than the truncated spelling a built-in name
/// carries, so a generator that wants more of the identity than a name can hold has all of it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdentifierHash([u8; 32]);

impl IdentifierHash {
    /// Hashes one identity text.
    fn of(identity: &str) -> Self {
        Self(*blake3::hash(identity.as_bytes()).as_bytes())
    }

    /// The digest bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The whole digest in lowercase hexadecimal.
    pub fn to_hex(&self) -> String {
        blake3::Hash::from(self.0).to_hex().to_string()
    }

    /// The first `digits` hexadecimal digits of the digest.
    pub(crate) fn short_hex(&self, digits: usize) -> String {
        self.to_hex()[..digits].to_owned()
    }
}

/// The context one utility class is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassContext {
    /// Normalized module path of the call site.
    pub module: String,
    /// The caller's class namespace, which separates independently built projects.
    pub namespace: String,
    /// Byte offset of the call.
    pub call: u32,
    /// Branch discriminator and canonical spelling of the contribution.
    pub branch: String,
}

/// The context one declaration name is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationContext {
    /// Normalized module path of the declaring module.
    pub module: String,
    /// Byte offset of the first character of the declaration's name.
    pub start: u32,
    /// Byte offset just past the declaration's name.
    pub end: u32,
    /// Binding name the declaration is assigned to.
    pub binding: String,
    /// Authored label, normalized for name use, when the declaration has one.
    pub label: Option<Label>,
}

/// The context one written custom-ident is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatorContext {
    /// Normalized module path of the declaration that owns the name.
    pub module: String,
    /// Byte offset of the first character of the declaration's name.
    pub start: u32,
    /// Byte offset just past the declaration's name.
    pub end: u32,
    /// Binding name of the declaration that owns the name.
    pub binding: String,
    /// Projection the name is written for, when the writer has one.
    pub projection: Option<String>,
}

/// The context one anonymous layer name is generated from.
///
/// The traversal index is not part of the name: the compiler appends it, so one template's layers
/// are distinct whatever the generator returns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayerContext {
    /// Identity of the template the layer belongs to.
    pub template: String,
    /// The caller's class namespace, which separates independently built projects.
    pub namespace: String,
}

/// What one generated conditional helper is written for.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConditionRole {
    /// A rule-isolation guard: the support group around one finished rule.
    Rule,
    /// A declaration fallback: the helper a lowered conditional reads.
    Fallback,
    /// A gate combining two conditions as AND.
    Gate,
    /// A gate comparing a custom-property value.
    GateValue,
    /// A gate inverting a condition.
    GateNot,
}

impl ConditionRole {
    /// Stable spelling used in a helper's identity text.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Fallback => "fallback",
            Self::Gate => "gate",
            Self::GateValue => "gate-value",
            Self::GateNot => "gate-not",
        }
    }

    /// The spelling the role adds to the compiler's own helper name.
    ///
    /// A fallback helper needs no marker, because it is the one a declaration reads directly.
    const fn name_infix(self) -> &'static str {
        match self {
            Self::Rule => "rule-",
            Self::Fallback => "",
            Self::Gate => "gate-",
            Self::GateValue => "gate-value-",
            Self::GateNot => "gate-not-",
        }
    }
}

/// The context one conditional helper name is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionContext {
    /// Which helper the name is written for.
    pub role: ConditionRole,
    /// The CSS the helper serves: a rule, a lowered conditional, or a condition.
    pub text: String,
}

/// One family's name request: the context plus the identity it is hashed from.
pub trait NameRequest<T: Token> {
    /// The family this request belongs to.
    fn kind(&self) -> IdentifierKind;

    /// The canonical identity text this request is hashed from.
    ///
    /// A report carries the text rather than the digest, so two requests that name the same
    /// construct compare equal even when their truncated spellings collide.
    fn identity(&self) -> String;

    /// Canonical hash of this request's fields, which is what one compilation memoizes by.
    fn hash(&self) -> IdentifierHash {
        hashed(&self.identity())
    }

    /// Asks the generator for this family, passing this request's own context.
    ///
    /// # Errors
    ///
    /// Returns the generator's own failure message for this request.
    fn name(
        &self,
        generator: &dyn IdentifierGenerator,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<T>, String>;

    /// The compiler's own name for this request.
    ///
    /// A family that spells an authored label writes it, normalized, between the prefix and
    /// the digest; the label is what makes a marker readable in generated markup. A family
    /// whose spelling is neither form overrides this.
    fn builtin(&self, hash: &IdentifierHash) -> String {
        match self.label() {
            Some(label) => format!(
                "{}{}_{}",
                builtin_prefix(self.kind()),
                label.as_str(),
                hash.short_hex(builtin_digest_digits(self.kind()))
            ),
            None => builtin_text(self.kind(), hash),
        }
    }

    /// The authored label this family spells into its built-in name, when it has one.
    fn label(&self) -> Option<&Label> {
        None
    }
}

/// Hashes one identity text into the digest a request is identified by.
fn hashed(identity: &str) -> IdentifierHash {
    IdentifierHash::of(identity)
}

/// Declares one family's request type and its hash rule.
///
/// A request type is what tells the families apart: a ref marker and a keyframes name share their
/// context type but not their family, and an anonymous layer has a context type of its own. Naming
/// the family in the type is what keeps the identity text, the kind, and the generator method in
/// one declaration.
macro_rules! name_requests {
    ($(
        $name:ident($context:ident) : $token:ident as $kind:ident = $identity:expr , asks($generator:ident) $(, labels($labels:ident))? ;
    )*) => {
        $(
            #[doc = concat!("A ", stringify!($kind), " request.")]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct $name(pub $context);

            impl NameRequest<$token> for $name {
                fn kind(&self) -> IdentifierKind {
                    IdentifierKind::$kind
                }

                fn identity(&self) -> String {
                    $identity(&self.0)
                }

                fn name(
                    &self,
                    generator: &dyn IdentifierGenerator,
                    hash: &IdentifierHash,
                ) -> Result<GeneratedName<$token>, String> {
                    generator.$generator(&self.0, hash)
                }

                $(
                    fn label(&self) -> Option<&Label> {
                        self.0.$labels.as_ref()
                    }
                )?
            }
        )*
    };
}

/// Canonical identity of a named declaration, in the versioned identity spelling.
fn declaration_identity(context: &impl DeclarationIdentity, kind: &str) -> String {
    format!(
        "stillwind/v1\0{}\0{kind}\0{}:{}:{}",
        context.module(),
        context.start(),
        context.end(),
        context.binding(),
    )
}

/// The declaration fields one identity is computed from.
trait DeclarationIdentity {
    /// Normalized module path of the declaring module.
    fn module(&self) -> &str;
    /// Byte offset of the first character of the declaration's name.
    fn start(&self) -> u32;
    /// Byte offset just past the declaration's name.
    fn end(&self) -> u32;
    /// Binding name the declaration is assigned to.
    fn binding(&self) -> &str;
}

impl DeclarationIdentity for DeclarationContext {
    fn module(&self) -> &str {
        &self.module
    }

    fn start(&self) -> u32 {
        self.start
    }

    fn end(&self) -> u32 {
        self.end
    }

    fn binding(&self) -> &str {
        &self.binding
    }
}

impl DeclarationIdentity for LocatorContext {
    fn module(&self) -> &str {
        &self.module
    }

    fn start(&self) -> u32 {
        self.start
    }

    fn end(&self) -> u32 {
        self.end
    }

    fn binding(&self) -> &str {
        &self.binding
    }
}

/// A conditional helper request.
///
/// The helper's own hash covers the role as well as the text, so two roles never share a name for
/// one text. The text's own hash is provided beside it, so a generator may also name helpers by the
/// text alone and spell the role itself, which is what the built-in name does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionHelperRequest(pub ConditionContext);

impl ConditionHelperRequest {
    /// Hash of the CSS the helper serves, without the role.
    ///
    /// The text is hashed as it stands: a helper is a helper for exactly this CSS, so its identity
    /// is the text itself rather than a spelling the compiler invented.
    pub(crate) fn text_hash(&self) -> IdentifierHash {
        hashed(&self.0.text)
    }
}

impl NameRequest<CustomPropertyName> for ConditionHelperRequest {
    fn kind(&self) -> IdentifierKind {
        IdentifierKind::CustomProperty
    }

    fn identity(&self) -> String {
        format!(
            "stillwind/condition/v1\0{}\0{}",
            self.0.role.as_str(),
            self.0.text,
        )
    }

    fn name(
        &self,
        generator: &dyn IdentifierGenerator,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<CustomPropertyName>, String> {
        generator.condition_helper(&self.0, hash, &self.text_hash())
    }

    fn builtin(&self, _hash: &IdentifierHash) -> String {
        format!(
            "{}{}{}",
            builtin_prefix(self.kind()),
            self.0.role.name_infix(),
            self.text_hash().short_hex(16),
        )
    }
}

/// The custom property one guarded view-transition use writes its class token into.
///
/// The declaring class lists every slot as `var(--slot,)`, so a condition that holds
/// contributes its token and a sibling condition contributes its own: the resolved
/// class list is the union of whatever the element actually declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewTransitionSlotRequest(pub ClassContext);

impl NameRequest<CustomPropertyName> for ViewTransitionSlotRequest {
    fn kind(&self) -> IdentifierKind {
        IdentifierKind::CustomProperty
    }

    fn identity(&self) -> String {
        format!(
            "stillwind/view-transition-slot/v1:{module}:{module_len}:{namespace_len}:{namespace}\u{0}{branch_len}:{branch}:{call}:{branch}",
            module = self.0.module,
            module_len = self.0.module.len(),
            namespace_len = self.0.namespace.len(),
            namespace = self.0.namespace,
            branch_len = self.0.branch.len(),
            branch = self.0.branch,
            call = self.0.call,
        )
    }

    fn name(
        &self,
        generator: &dyn IdentifierGenerator,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<CustomPropertyName>, String> {
        generator.view_transition_slot(&self.0, hash)
    }

    fn builtin(&self, hash: &IdentifierHash) -> String {
        format!("sw-vtc-{}", hash.short_hex(16))
    }
}

name_requests! {
    ClassName(ClassContext): UtilityClass as UtilityClass = |context: &ClassContext| format!(
        "stillwind/style/v1:{module}:{module_len}:{namespace_len}:{namespace}\u{0}{branch_len}:{branch}:{call}:{branch}",
        module = context.module,
        module_len = context.module.len(),
        namespace_len = context.namespace.len(),
        namespace = context.namespace,
        branch_len = context.branch.len(),
        branch = context.branch,
        call = context.call,
    ), asks(utility_class);
    RefMarkerName(DeclarationContext): RefMarker as RefMarker = |context: &DeclarationContext| declaration_identity(context, "ref"), asks(ref_marker), labels(label);
    SubtreeMarkerName(DeclarationContext): SubtreeMarker as SubtreeMarker = |context: &DeclarationContext| declaration_identity(context, "scope"), asks(subtree_marker), labels(label);
    AnchorNameRequest(LocatorContext): AnchorName as AnchorName = |context: &LocatorContext| declaration_identity(context, "ref"), asks(anchor_name);
    ContainerNameRequest(LocatorContext): ContainerName as ContainerName = |context: &LocatorContext| declaration_identity(context, "scope"), asks(container_name);
    KeyframesNameRequest(DeclarationContext): KeyframesName as KeyframesName = |context: &DeclarationContext| declaration_identity(context, "animation"), asks(keyframes_name);
    ViewTransitionClassRequest(DeclarationContext): ViewTransitionClass as ViewTransitionClass = |context: &DeclarationContext| declaration_identity(context, "view-transition"), asks(view_transition_class);
    LayerNameRequest(LayerContext): LayerName as LayerName = |context: &LayerContext| format!(
        "stillwind/layer/v1:{namespace_len}:{namespace}{template}",
        namespace_len = context.namespace.len(),
        namespace = context.namespace,
        template = context.template,
    ), asks(layer_name);
}
