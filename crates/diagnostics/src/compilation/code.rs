//! Stable reporting identities for fatal diagnostics.
//!
//! Codes use `SWnnmmm`: `nn` identifies a category and `mmm` identifies one
//! diagnostic within it. Each [`CompileErrorClass`] reserves a range of categories and
//! tells a host who must act on the error. Explicit numbers remain stable when
//! declaration order changes. Until the first release, codes can be renumbered so that
//! each category stays contiguous. After it, a removed code is not reused for another
//! diagnostic.
//!
//! ## Categories
//!
//! Configuration (01-09):
//!
//! - 01: compiler options, generated names, and the CSS entry
//!
//! Authoring (10-29):
//!
//! - 10: JavaScript and component source
//! - 11: style composition
//! - 12: declaration identities checked by TypeScript
//! - 13: authored CSS syntax
//! - 14: stylesheet rendering
//! - 15: source size limits
//!
//! Host (30-49):
//!
//! - 30: TypeScript checker facts
//! - 31: component projections and their coordinates
//! - 32: CSS loader protocol
//! - 33: host-supplied source coordinates
//! - 34: host integration inputs
//!
//! Internal (90-99):
//!
//! - 90: compiler invariants

use serde::{Deserialize, Serialize};

use super::CompileDiagnostic;

/// Party that must act to resolve a fatal diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompileErrorClass {
    /// The user must change JavaScript, component, or CSS source.
    Authoring,
    /// The user must change compiler options or the framework constraint.
    Configuration,
    /// The host or integration violated an input or protocol contract.
    Host,
    /// The compiler violated its own invariant; the error indicates a compiler bug.
    Internal,
}

impl CompileErrorClass {
    /// Returns the camel-case spelling used in external reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authoring => "authoring",
            Self::Configuration => "configuration",
            Self::Host => "host",
            Self::Internal => "internal",
        }
    }
}

macro_rules! define_compile_codes {
    ($($class:ident { $($variant:ident $( { $($fields:tt)* } )? => $code:literal,)+ })+) => {
        /// Reporting identity of one fatal diagnostic variant.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        pub enum CompileErrorCode {
            $($(#[doc = concat!("Reports `CompileDiagnostic::", stringify!($variant), "` as `", $code, "`.")]
              #[serde(rename = $code)] $variant,)+)+
        }

        impl CompileErrorCode {
            /// All reporting identities in registry order.
            pub const ALL: &'static [Self] = &[$($(Self::$variant,)+)+];

            /// Returns the stable code used in external reports.
            pub const fn as_str(self) -> &'static str {
                match self { $($(Self::$variant => $code,)+)+ }
            }

            /// Returns the party that must act on this diagnostic.
            pub const fn class(self) -> CompileErrorClass {
                match self { $($(Self::$variant => CompileErrorClass::$class,)+)+ }
            }
        }

        impl CompileDiagnostic {
            /// Returns this diagnostic variant's reporting identity.
            pub const fn code(&self) -> CompileErrorCode {
                match self { $($(Self::$variant $({ $($fields)* })? => CompileErrorCode::$variant,)+)+ }
            }
        }
    };
}

define_compile_codes! {
    Configuration {
        // 01: compiler options, generated names, and the CSS entry
        FrameworkConstraintConflict => "SW01001",
        InvalidPropertyPrefix { .. } => "SW01002",
        ConflictingPropertyPrefixes { .. } => "SW01003",
        InvalidHashLength { .. } => "SW01004",
        GeneratedIdentifierCollision { .. } => "SW01005",
        InvalidLintConfiguration { .. } => "SW01006",
        IdentifierGenerationFailed { .. } => "SW01007",
        UnresolvedCssRoot => "SW01008",
    }
    Authoring {
        // 10: JavaScript and component source
        JavaScriptParse => "SW10001",
        ComponentSyntax { .. } => "SW10002",
        MixedFrameworks => "SW10003",
        UnconsumedMacroUse { .. } => "SW10004",
        UntransformedMacroReference { .. } => "SW10005",
        // 11: style composition
        CompositionCycle => "SW11001",
        CompositionModuleCycle => "SW11002",
        UnavailableStyleBase => "SW11003",
        // 12: declaration identities checked by TypeScript
        IdentityAssignment { .. } => "SW12001",
        IdentityUnavailable { .. } => "SW12002",
        // 13: authored CSS syntax
        CssParse { .. } => "SW13001",
        UnterminatedCssBlock { .. } => "SW13002",
        InvalidCssBlock { .. } => "SW13003",
        CssNestingTooDeep { .. } => "SW13004",
        // 14: stylesheet rendering
        MissingUtilitiesSlot => "SW14001",
        MissingSupportSlot { .. } => "SW14002",
        MalformedPropertyRequirement { .. } => "SW14003",
        MalformedUtilityRequirement { .. } => "SW14004",
        UnknownRequiredProperty { .. } => "SW14005",
        PropertyUsageParse { .. } => "SW14006",
        InvalidThemeReference { .. } => "SW14007",
        // 15: source size limits
        CssOffsetTooLarge { .. } => "SW15001",
        RenderedSourceTooLarge => "SW15002",
        SourceTooLarge { .. } => "SW15003",
    }
    Host {
        // 30: TypeScript checker facts
        CheckerFactsRequired { .. } => "SW30001",
        InvalidTypeScriptFacts => "SW30002",
        InvalidSymbolicFacts => "SW30003",
        CheckerTargetUnavailable { .. } => "SW30004",
        // 31: component projections and their coordinates
        ProjectionMap { .. } => "SW31001",
        ProjectionUnmapped { .. } => "SW31002",
        ProjectionContent { .. } => "SW31003",
        ProjectionSyntax { .. } => "SW31004",
        ProjectionPreparation { .. } => "SW31005",
        InvalidUtf16Offset { .. } => "SW31006",
        InvalidUtf16Position { .. } => "SW31007",
        InvalidUtf8Offset { .. } => "SW31008",
        UnmappedMacroReference { .. } => "SW31009",
        EditOutsideEditableRegion { .. } => "SW31010",
        AttributeQuoteMismatch { .. } => "SW31011",
        // 32: CSS loader protocol
        UnresolvedCssImports { .. } => "SW32001",
        InvalidCssInitialBatch { .. } => "SW32002",
        UnknownCssRequest { .. } => "SW32003",
        ConflictingCssSource { .. } => "SW32004",
        CssLoadAlreadyComplete => "SW32005",
        CssRequestIdExhausted => "SW32006",
        CssSourceIdExhausted => "SW32007",
        // 33: host-supplied source coordinates
        SourceInternalMap { .. } => "SW33001",
        SourceInvalidRange { .. } => "SW33002",
        SourceWrongFile { .. } => "SW33003",
        SourceInvalidSpan { .. } => "SW33004",
        SourceInvalidOffset { .. } => "SW33005",
        // 34: host integration inputs
        MissingRuntimeAnchor { .. } => "SW34001",
        MissingModuleCssTarget { .. } => "SW34002",
        TransformAfterFailedCompilation => "SW34003",
    }
    Internal {
        // 90: compiler invariants
        InvalidGeneratedCss { .. } => "SW90001",
        MissingRule => "SW90002",
        InvalidCssGraph => "SW90003",
        InvalidMacroImportMetadata { .. } => "SW90004",
        MissingAnchorEdit { .. } => "SW90005",
        InvalidTransformExpressionMapping { .. } => "SW90006",
        InvalidTransformOverlap { .. } => "SW90007",
        InvalidInternalSourceCoordinates { .. } => "SW90008",
    }
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
            assert_eq!(serde_json::to_value(code).unwrap(), text);
            assert_eq!(
                serde_json::from_value::<CompileErrorCode>(serde_json::json!(text)).unwrap(),
                code
            );
        }
    }

    #[test]
    fn categories_lie_in_their_class_range_in_registry_order() {
        let mut previous = "";
        for &code in CompileErrorCode::ALL {
            let text = code.as_str();
            assert!(previous < text, "{text} follows {previous}");
            previous = text;
            let category: u8 = text[2..4].parse().unwrap();
            let range = match code.class() {
                CompileErrorClass::Configuration => 1..=9,
                CompileErrorClass::Authoring => 10..=29,
                CompileErrorClass::Host => 30..=49,
                CompileErrorClass::Internal => 90..=99,
            };
            assert!(range.contains(&category), "{text}");
        }
        for class in [
            CompileErrorClass::Authoring,
            CompileErrorClass::Configuration,
            CompileErrorClass::Host,
            CompileErrorClass::Internal,
        ] {
            assert_eq!(serde_json::to_value(class).unwrap(), class.as_str());
        }
    }
}
