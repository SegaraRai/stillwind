//! Editor requests for the theme tokens and shared assets that JavaScript plugins read.

use std::collections::BTreeMap;

use lean_string::LeanStr;
use napi_derive::napi;

use stillwind_design_system::{LegacyThemeExposure, ThemeValueMode, ThemeValuePrecedence};

use super::NativeEditorQuery;
use crate::output::NativeText;

/// An active theme token. JavaScript plugins read it when they resolve theme values.
#[napi(object)]
pub struct NativeThemeToken {
    /// The complete custom property name, including the leading `--`.
    pub name: String,
    /// The CSS value as written, including references to other tokens.
    pub value: String,
    /// Whether an ordinary `@theme` definition overrides this definition. This is true
    /// for definitions from `@theme default` and from the theme configuration of a
    /// plugin.
    pub fallback: bool,
    /// Whether utilities write the value of the token instead of a reference to it.
    pub inline: bool,
}

/// The theme namespace that one Tailwind v3 config key reads.
#[napi(object, object_from_js = false)]
pub struct NativeLegacyThemeKey {
    /// The v3 config key, such as `fontSize`.
    pub key: String,
    /// The namespace name without `--` and `-*`, such as `text`.
    pub namespace: String,
    /// How plugin configuration sees the CSS values of the namespace under the key.
    /// `replace` replaces the v3 default values, and `extend` adds the CSS values to
    /// them. The field is absent when plugin configuration sees only the v3 default
    /// values.
    #[napi(ts_type = "'extend' | 'replace'")]
    pub exposure: Option<String>,
}

/// An active shared asset and the setting that controls how the output uses it.
#[napi(object)]
pub struct NativeSharedAsset {
    /// The complete custom property name. Case matters.
    pub name: String,
    /// The CSS value as written, including references to variables.
    pub value: String,
    /// Whether an ordinary definition overrides this definition.
    pub fallback: bool,
    /// How the output uses the asset. `default` writes a reference to the asset,
    /// `inline` writes its value, and `static` keeps the asset without a reference.
    #[napi(ts_type = "'default' | 'inline' | 'static'")]
    pub mode: String,
}

#[napi]
impl NativeEditorQuery {
    /// Returns the value of each active theme token, keyed by its complete custom
    /// property name.
    #[napi(ts_return_type = "Record<string, string>")]
    pub fn theme_values(&self) -> BTreeMap<LeanStr, NativeText> {
        self.query
            .theme_values()
            .into_iter()
            .map(|(name, value)| (name, value.into()))
            .collect()
    }

    /// Returns the theme namespace that each Tailwind v3 config key reads, sorted by
    /// config key. A config key that is not included reads the namespace with the
    /// kebab-case name of the key.
    #[napi]
    pub fn legacy_theme_keys(&self) -> Vec<NativeLegacyThemeKey> {
        self.query
            .legacy_theme_keys()
            .into_iter()
            .map(|(key, legacy)| NativeLegacyThemeKey {
                key: key.to_string(),
                namespace: legacy.namespace.to_string(),
                exposure: match legacy.exposure {
                    LegacyThemeExposure::None => None,
                    LegacyThemeExposure::Extend => Some("extend".into()),
                    LegacyThemeExposure::Replace => Some("replace".into()),
                },
            })
            .collect()
    }

    /// Returns the active theme tokens, with the settings that plugin configuration
    /// uses.
    #[napi]
    pub fn theme_tokens(&self) -> Vec<NativeThemeToken> {
        self.query
            .theme_tokens()
            .into_iter()
            .map(|token| NativeThemeToken {
                name: token.name.value.to_string(),
                value: token.value.value.to_string(),
                fallback: token.precedence != ThemeValuePrecedence::Ordinary,
                inline: token.mode == ThemeValueMode::Inline,
            })
            .collect()
    }

    /// Returns the active shared assets, sorted by custom property name.
    #[napi]
    pub fn shared_assets(&self) -> Vec<NativeSharedAsset> {
        self.query
            .shared_assets()
            .into_iter()
            .map(|asset| NativeSharedAsset {
                name: asset.name.value.to_string(),
                value: asset.value.value.to_string(),
                fallback: asset.precedence != ThemeValuePrecedence::Ordinary,
                mode: match asset.mode {
                    ThemeValueMode::Default => "default",
                    ThemeValueMode::Inline => "inline",
                    ThemeValueMode::Static => "static",
                }
                .into(),
            })
            .collect()
    }
}
