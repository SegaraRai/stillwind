//! Stable reporting identities for fatal diagnostics.
//!
//! Codes use `SWnnmmm`: `nn` identifies a category and `mmm` identifies one
//! diagnostic within it. Explicit numbers remain stable when declaration order changes.
//!
//! ## Categories
//!
//! - 01: JavaScript and style inputs
//! - 02: utilities and variants
//! - 03: refs and providers
//! - 04: keyframes and animation
//! - 05: view transitions
//! - 06: composition
//! - 07: generated identifiers and configuration
//! - 08: CSS loading and rendering
//! - 09: source bounds and transforms
//! - 10: projection and coordinates
//! - 11: import edits
//! - 12: checker facts

use serde::{Deserialize, Serialize};

use super::CompileDiagnostic;

macro_rules! define_compile_codes {
    ($($variant:ident $( { $($fields:tt)* } )? $( ( $($tuple:tt)* ) )? => $code:literal,)+) => {
        /// Reporting identity of one fatal diagnostic variant.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        pub enum CompileErrorCode {
            $(#[doc = concat!("Reports `CompileDiagnostic::", stringify!($variant), "` as `", $code, "`.")]
              #[serde(rename = $code)] $variant,)+
        }

        impl CompileErrorCode {
            /// All reporting identities in registry order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// Returns the stable code used in external reports.
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $code,)+ }
            }

            /// Parses an exact code, returning `None` for an unknown code.
            pub fn from_code(code: &str) -> Option<Self> {
                match code { $($code => Some(Self::$variant),)+ _ => None }
            }
        }

        impl CompileDiagnostic {
            /// Returns this diagnostic variant's reporting identity.
            pub const fn code(&self) -> CompileErrorCode {
                match self { $(Self::$variant $({ $($fields)* })? $(($($tuple)*))? => CompileErrorCode::$variant,)+ }
            }
        }
    };
}

define_compile_codes! {
    // 01: JavaScript and style inputs
    FrameworkConflict => "SW01001",
    StyleArraySpread => "SW01002",
    StyleArrayHole => "SW01003",
    InvalidStyleInput => "SW01004",
    AttributeSpread => "SW01005",
    InvalidAttributeProperty => "SW01006",
    InvalidComputedAttribute => "SW01007",
    InvalidAttributeKey => "SW01008",
    MixedFrameworks => "SW01009",
    StyleArgumentSpread => "SW01010",
    UnconsumedMacroUse { .. } => "SW01011",
    JavaScriptParse => "SW01012",
    // 02: utilities and variants
    InvalidVariantInput { .. } => "SW02001",
    InvalidUtilityTemplate => "SW02002",
    UnavailableUtility { .. } => "SW02003",
    // 03: refs and providers
    RefFactsRequired => "SW03001",
    InvalidRefArgument => "SW03002",
    InvalidRefBoundary => "SW03003",
    InvalidRefLabelArgument => "SW03004",
    InvalidRefArguments => "SW03005",
    NonStaticRefVariant => "SW03006",
    UnidentifiedRefVariant => "SW03007",
    InvalidContainerType => "SW03008",
    InvalidAnchorScope => "SW03009",
    RefProjectionFactsRequired => "SW03010",
    InvalidContainerQuery => "SW03011",
    InvalidPositionAnchorArguments => "SW03012",
    InvalidRefProjection => "SW03013",
    InvalidVariantPrefix => "SW03014",
    UnavailableVariantPrefix => "SW03015",
    UnidentifiedRef => "SW03016",
    // 04: keyframes and animation
    InvalidKeyframesOptions => "SW04001",
    KeyframesUseNotAClassInput => "SW04002",
    InvalidKeyframesDefinition => "SW04003",
    KeyframesSpread => "SW04004",
    InvalidKeyframesProperty => "SW04005",
    InvalidKeyframesFrameKey => "SW04006",
    InvalidKeyframesFrameSelector => "SW04007",
    InvalidKeyframesFrameValue => "SW04008",
    EmptyKeyframes => "SW04009",
    InvalidKeyframesLabel => "SW04010",
    KeyframesFrameVariants => "SW04011",
    InvalidKeyframesVariant => "SW04012",
    UnknownAnimationOption { .. } => "SW04013",
    // 05: view transitions
    InvalidViewTransitionKeyExpression => "SW05001",
    InvalidViewTransitionDefinition => "SW05002",
    ViewTransitionSpread => "SW05003",
    InvalidViewTransitionProperty => "SW05004",
    InvalidViewTransitionKey => "SW05005",
    InvalidViewTransitionValue => "SW05006",
    EmptyViewTransition => "SW05007",
    InvalidViewTransitionLabel => "SW05008",
    InvalidViewTransitionVariant => "SW05009",
    // 06: composition
    NestedStyleCall => "SW06001",
    CompositionCycle => "SW06002",
    CompositionModuleCycle => "SW06003",
    UnavailableStyleBase => "SW06004",
    StyleBaseWithProps => "SW06005",
    StyleBaseFrameworkMismatch => "SW06006",
    // 07: generated identifiers and configuration
    GeneratedIdentifierCollision { .. } => "SW07001",
    IdentifierGenerationFailed(..) => "SW07002",
    InvalidPropertyPrefix { .. } => "SW07003",
    InvalidHashLength { .. } => "SW07004",
    ConflictingPropertyPrefixes { .. } => "SW07005",
    // 08: CSS loading and rendering
    InvalidGeneratedCss { .. } => "SW08001",
    MissingUtilitiesSlot => "SW08002",
    MissingPropertySlot { .. } => "SW08003",
    InvalidPropertyRequirement => "SW08004",
    MalformedPropertyRequirement { .. } => "SW08005",
    MalformedUtilityRequirement { .. } => "SW08006",
    InvalidRuntimeConditional { .. } => "SW08007",
    PropertyUsage { .. } => "SW08008",
    MissingRule => "SW08009",
    InvalidCssGraph => "SW08010",
    CssRequestIdExhausted => "SW08011",
    CssSourceIdExhausted => "SW08012",
    CssLoadAlreadyComplete => "SW08013",
    InvalidCssInitialBatch { .. } => "SW08014",
    UnknownCssRequest { .. } => "SW08015",
    ConflictingCssSource { .. } => "SW08016",
    CssImportCycle { .. } => "SW08017",
    UnresolvedCssImports { .. } => "SW08018",
    ConditionalDefinition { .. } => "SW08019",
    UnsupportedOutputSlot { .. } => "SW08020",
    InvalidOutputSlotContext { .. } => "SW08021",
    InvalidCssOutputBlock { .. } => "SW08022",
    CssOffsetTooLarge { .. } => "SW08023",
    CssParse { .. } => "SW08024",
    UnterminatedTheme { .. } => "SW08025",
    UnterminatedCssBlock { .. } => "SW08026",
    CssNestingTooDeep { .. } => "SW08027",
    UnsupportedImportModifier { .. } => "SW08028",
    InvalidImportModifierOrder { .. } => "SW08029",
    InvalidImportBlock { .. } => "SW08030",
    InvalidImportSpecifier { .. } => "SW08031",
    InvalidImportModifiers { .. } => "SW08032",
    InvalidImportModifierBlock { .. } => "SW08033",
    UnresolvedThemeReference => "SW08034",
    InvalidThemeReference { .. } => "SW08035",
    // 09: source bounds and transforms
    SourceInternalMap { .. } => "SW09001",
    SourceTooLarge { .. } => "SW09002",
    SourceInvalidRange { .. } => "SW09003",
    SourceWrongFile { .. } => "SW09004",
    SourceInvalidSpan { .. } => "SW09005",
    SourceInvalidOffset { .. } => "SW09006",
    InvalidTransformExpressionMapping { .. } => "SW09007",
    InvalidTransformOverlap { .. } => "SW09008",
    InvalidTransformSize => "SW09009",
    InvalidTransformState => "SW09010",
    // 10: projection and coordinates
    ProjectionMap { .. } => "SW10001",
    ProjectionUnmapped { .. } => "SW10002",
    ProjectionContent { .. } => "SW10003",
    InvalidUtf16Offset { .. } => "SW10004",
    InvalidUtf16Position { .. } => "SW10005",
    InvalidUtf8Offset { .. } => "SW10006",
    // 11: import edits
    ImportSourceInternalMap => "SW11001",
    ImportSourceTooLarge { .. } => "SW11002",
    ImportSourceInvalidRange { .. } => "SW11003",
    ImportSourceWrongFile { .. } => "SW11004",
    ImportSourceInvalidSpan { .. } => "SW11005",
    ImportSourceInvalidOffset { .. } => "SW11006",
    MissingRuntimeAnchor { .. } => "SW11007",
    InvalidImportMetadata { .. } => "SW11008",
    MissingModuleCssTarget { .. } => "SW11009",
    UntransformedMacroReference { .. } => "SW11010",
    UnmappedMacroReference { .. } => "SW11011",
    // 12: checker facts
    InvalidSymbolicFacts => "SW12001",
    InvalidTypeScriptFacts => "SW12002",
}

impl std::fmt::Display for CompileErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn codes_are_unique_and_round_trip_as_strings() {
        let mut seen = BTreeSet::new();
        for &code in CompileErrorCode::ALL {
            let text = code.as_str();
            assert!(seen.insert(text));
            assert_eq!(text.len(), 7);
            assert!(text.starts_with("SW"));
            assert!(text[2..].bytes().all(|byte| byte.is_ascii_digit()));
            assert_eq!(CompileErrorCode::from_code(text), Some(code));
            assert_eq!(serde_json::to_value(code).unwrap(), text);
            assert_eq!(
                serde_json::from_value::<CompileErrorCode>(serde_json::json!(text)).unwrap(),
                code
            );
        }
        assert_eq!(CompileErrorCode::from_code("SHCSS001"), None);
    }
}
