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

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_source::{CssSourceName, ModuleId};

use super::generator::{GeneratedName, IdentifierGenerator};
use super::kind::{IdentifierKind, builtin_label_separator, builtin_prefix};
use super::name::{
    AnchorName, ConditionHelperName, ContainerName, KeyframesName, Label, LayerName, RefMarker,
    Token, UtilityClass, ViewTransitionClass, ViewTransitionSlotName,
};

/// The digits a digest is spelled in, in value order.
const BASE36_DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

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
    pub fn to_hex(&self) -> LeanString {
        blake3::Hash::from(self.0).to_hex().to_lean_string()
    }

    /// The low-order `digits` base-36 digits of the digest.
    ///
    /// The digest is one big-endian integer, so these digits are its remainder modulo
    /// `36^digits` and every bit of the digest participates. Asking for more digits
    /// extends the same suffix, and a digit the digest cannot fill is `0`.
    pub(crate) fn short_base36(&self, digits: usize) -> LeanString {
        let mut value = self.0;
        let mut spelled = Vec::with_capacity(digits);
        for _ in 0..digits {
            let mut remainder = 0u32;
            for byte in &mut value {
                let current = (remainder << 8) | u32::from(*byte);
                // `current` is at most `35 * 256 + 255`, so the quotient always fits a byte.
                #[allow(clippy::cast_possible_truncation)]
                let quotient = (current / 36) as u8;
                *byte = quotient;
                remainder = current % 36;
            }
            spelled.push(char::from(BASE36_DIGITS[remainder as usize]));
        }
        spelled.reverse();
        spelled.into_iter().collect()
    }
}

/// The context one utility class is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassContext {
    /// Module identity of the call site.
    pub module: ModuleId,
    /// The caller's class namespace, which separates independently built projects.
    pub namespace: LeanStr,
    /// Byte offset of the call.
    pub call: u32,
    /// Branch discriminator and canonical spelling of the contribution.
    pub branch: LeanStr,
}

/// The context one declaration name is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationContext {
    /// Module identity of the declaring module.
    pub module: ModuleId,
    /// Byte offset of the first character of the declaration's name.
    pub start: u32,
    /// Byte offset just past the declaration's name.
    pub end: u32,
    /// Binding name the declaration is assigned to.
    pub binding: LeanStr,
    /// Authored label, normalized for name use, when the declaration has one.
    pub label: Option<Label>,
}

impl DeclarationIdentity for DeclarationContext {
    fn module(&self) -> &ModuleId {
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

/// The context one written custom-ident is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatorContext {
    /// Module identity of the declaration that owns the name.
    pub module: ModuleId,
    /// Byte offset of the first character of the declaration's name.
    pub start: u32,
    /// Byte offset just past the declaration's name.
    pub end: u32,
    /// Binding name of the declaration that owns the name.
    pub binding: LeanStr,
    /// Authored label, normalized for name use, when the declaration has one.
    pub label: Option<Label>,
    /// Projection the name is written for, when the writer has one.
    pub projection: Option<LocatorProjection>,
}

impl DeclarationIdentity for LocatorContext {
    fn module(&self) -> &ModuleId {
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

/// The ref projection that requests a generated custom identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocatorProjection {
    /// Supplies an anchor name.
    Anchor,
    /// References an anchor from a styled element.
    PositionAnchor,
    /// Supplies a container name.
    Container,
}

impl LocatorProjection {
    /// Returns the spelling passed to naming callbacks.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::PositionAnchor => "positionAnchor",
            Self::Container => "container",
        }
    }
}

/// The context one anonymous layer name is generated from.
///
/// The traversal index is not part of the name: the compiler appends it, so one template's layers
/// are distinct whatever the generator returns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayerContext {
    /// Identity of the template the layer belongs to.
    pub template: CssSourceName,
    /// The caller's class namespace, which separates independently built projects.
    pub namespace: LeanStr,
}

/// What one generated conditional helper is written for.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConditionRole {
    /// A support group shared by one owner's nonconflicting rule interval.
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

    /// The letter the role adds to the compiler's own helper name.
    ///
    /// A fallback helper needs no marker, because it is the one a declaration reads
    /// directly.
    const fn builtin_letter(self) -> &'static str {
        match self {
            Self::Rule | Self::Fallback => "",
            Self::Gate => "g",
            Self::GateValue => "v",
            Self::GateNot => "n",
        }
    }
}

/// The context one conditional helper name is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionContext {
    /// Which helper the name is written for.
    pub role: ConditionRole,
    /// Canonical owner and interval identity for `Rule`; otherwise the conditional CSS.
    pub text: LeanStr,
}

/// One family's name request: the context plus the identity it is hashed from.
pub trait NameRequest<T: Token> {
    /// The family this request belongs to.
    fn kind(&self) -> IdentifierKind;

    /// The canonical input this request is hashed from.
    fn identity(&self) -> LeanString;

    /// Owner identity for a name recorded without a declaration position.
    fn detached_owner_identity(&self) -> Option<LeanString> {
        None
    }

    /// Canonical hash of this request's fields, which is what one compilation memoizes
    /// by together with the label the request spells.
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
    ) -> Result<GeneratedName<T>, LeanString>;

    /// The compiler's own name for this request.
    ///
    /// `hash_length` is how many base-36 digits of a digest this compilation spells. A
    /// family that spells an authored label writes it, normalized, between the prefix
    /// and the digest; the label is what makes a marker readable in generated markup. A
    /// family whose spelling is neither form overrides this.
    fn builtin(&self, hash: &IdentifierHash, hash_length: usize) -> LeanString {
        builtin_spelling(self.kind(), self.label(), &hash.short_base36(hash_length))
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

/// Spells one family's built-in name from its prefix, its label, and the request digest.
fn builtin_spelling(kind: IdentifierKind, label: Option<&Label>, digest: &str) -> LeanString {
    match label {
        Some(label) => format_args!(
            "{}{}{}{}",
            builtin_prefix(kind),
            label.as_str(),
            builtin_label_separator(kind),
            digest,
        )
        .to_lean_string(),
        None => format_args!("{}{}", builtin_prefix(kind), digest).to_lean_string(),
    }
}

/// Declares one family's request type and its hash rule.
///
/// A request type is what tells the families apart: a ref marker and a keyframes name
/// share their context type but not their family, and an anonymous layer has a context
/// type of its own. Naming the family in the type is what keeps the identity text, the
/// kind, and the generator method in one declaration.
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

                fn identity(&self) -> LeanString {
                    $identity(&self.0)
                }

                fn name(
                    &self,
                    generator: &dyn IdentifierGenerator,
                    hash: &IdentifierHash,
                ) -> Result<GeneratedName<$token>, LeanString> {
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
fn declaration_identity(context: &impl DeclarationIdentity, kind: &str) -> LeanString {
    format_args!(
        "stillwind/v1\0{}",
        declaration_fields(
            context.module(),
            kind,
            context.start(),
            context.end(),
            context.binding()
        )
    )
    .to_lean_string()
}

/// Fields shared by the hash request and owner identity of a named declaration.
pub(crate) fn declaration_fields(
    module: &ModuleId,
    kind: &str,
    start: u32,
    end: u32,
    binding: &str,
) -> LeanString {
    format_args!("{module}\0{kind}\0{start}:{end}\0{binding}\0").to_lean_string()
}

/// The declaration fields one identity is computed from.
trait DeclarationIdentity {
    /// Module identity of the declaring module.
    fn module(&self) -> &ModuleId;
    /// Byte offset of the first character of the declaration's name.
    fn start(&self) -> u32;
    /// Byte offset just past the declaration's name.
    fn end(&self) -> u32;
    /// Binding name the declaration is assigned to.
    fn binding(&self) -> &str;
}

/// A conditional helper request.
///
/// The helper's own hash covers the role as well as the text, so two roles never share a name for
/// one text. The text's own hash is provided beside it, so a generator may also name helpers by the
/// text alone and spell the role itself, which is what the built-in name does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionHelperRequest(pub ConditionContext);

impl ConditionHelperRequest {
    /// Hash of the canonical context text, without the role.
    pub(crate) fn text_hash(&self) -> IdentifierHash {
        hashed(&self.0.text)
    }
}

impl NameRequest<ConditionHelperName> for ConditionHelperRequest {
    fn kind(&self) -> IdentifierKind {
        if self.0.role == ConditionRole::Rule {
            IdentifierKind::RuleBoundary
        } else {
            IdentifierKind::ConditionHelper
        }
    }

    fn identity(&self) -> LeanString {
        format_args!(
            "stillwind/condition/v1\0{}\0{}\0",
            self.0.role.as_str(),
            self.0.text,
        )
        .to_lean_string()
    }

    fn detached_owner_identity(&self) -> Option<LeanString> {
        Some(
            format_args!(
                "stillwind-identity/v1\0condition-helper\0{}\0{}\0",
                self.0.role.as_str(),
                self.0.text,
            )
            .to_lean_string(),
        )
    }

    fn name(
        &self,
        generator: &dyn IdentifierGenerator,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<ConditionHelperName>, LeanString> {
        generator.condition_helper(&self.0, hash, &self.text_hash())
    }

    fn builtin(&self, _hash: &IdentifierHash, hash_length: usize) -> LeanString {
        if self.0.role == ConditionRole::Rule {
            return self.text_hash().short_base36(hash_length);
        }
        // The role is a letter of the prefix rather than part of the digest, so the
        // name is the helper's text hash under the spelling of its own role.
        format_args!(
            "{}{}-{}",
            builtin_prefix(self.kind()),
            self.0.role.builtin_letter(),
            self.text_hash().short_base36(hash_length),
        )
        .to_lean_string()
    }
}

/// The custom property one guarded view-transition use writes its class token into.
///
/// The declaring class lists every slot as `var(--slot,)`, so a condition that holds
/// contributes its token and a sibling condition contributes its own: the resolved
/// class list is the union of whatever the element actually declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewTransitionSlotRequest(pub ClassContext);

impl NameRequest<ViewTransitionSlotName> for ViewTransitionSlotRequest {
    fn kind(&self) -> IdentifierKind {
        IdentifierKind::ViewTransitionSlot
    }

    fn identity(&self) -> LeanString {
        format_args!(
            "stillwind/view-transition-slot/v1\0{namespace}\0{module}\0{call}\0{branch}\0",
            namespace = self.0.namespace,
            module = self.0.module,
            call = self.0.call,
            branch = self.0.branch,
        )
        .to_lean_string()
    }

    fn detached_owner_identity(&self) -> Option<LeanString> {
        Some(
            format_args!(
                "stillwind-identity/v1\0slot\0{}\0{}\0{}\0{}\0",
                self.0.module, self.0.namespace, self.0.call, self.0.branch,
            )
            .to_lean_string(),
        )
    }

    fn name(
        &self,
        generator: &dyn IdentifierGenerator,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<ViewTransitionSlotName>, LeanString> {
        generator.view_transition_slot(&self.0, hash)
    }
}

name_requests! {
    ClassName(ClassContext): UtilityClass as UtilityClass = |context: &ClassContext| format_args!(
        "stillwind/style/v1\0{namespace}\0{module}\0{call}\0{branch}\0",
        namespace = context.namespace,
        module = context.module,
        call = context.call,
        branch = context.branch,
    ).to_lean_string(), asks(utility_class);
    RefMarkerName(DeclarationContext): RefMarker as RefMarker = |context: &DeclarationContext| declaration_identity(context, "ref"), asks(ref_marker), labels(label);
    AnchorNameRequest(LocatorContext): AnchorName as AnchorName = |context: &LocatorContext| declaration_identity(context, "ref"), asks(anchor_name), labels(label);
    ContainerNameRequest(LocatorContext): ContainerName as ContainerName = |context: &LocatorContext| declaration_identity(context, "ref"), asks(container_name), labels(label);
    KeyframesNameRequest(DeclarationContext): KeyframesName as KeyframesName = |context: &DeclarationContext| declaration_identity(context, "keyframes"), asks(keyframes_name), labels(label);
    ViewTransitionClassRequest(DeclarationContext): ViewTransitionClass as ViewTransitionClass = |context: &DeclarationContext| declaration_identity(context, "view-transition"), asks(view_transition_class), labels(label);
    LayerNameRequest(LayerContext): LayerName as LayerName = |context: &LayerContext| format_args!(
        "stillwind/layer/v1\0{namespace}\0{template}\0",
        namespace = context.namespace,
        template = context.template,
    ).to_lean_string(), asks(layer_name);
}
