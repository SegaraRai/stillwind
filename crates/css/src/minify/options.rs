//! Caller options for final asset minification and their conversion to Lightning CSS
//! browser targets.

use std::collections::BTreeMap;

use lean_string::{LeanStr, ToLeanStr};
use lightningcss::targets::{Browsers, Features, Targets};

use super::asset::CssMinifyError;

/// Controls final asset optimization independently of compiler generation options.
#[derive(Clone, Debug)]
pub struct CssMinifyOptions {
    /// Input identity used in errors and generated source maps. Defaults to `input.css`.
    pub filename: LeanStr,
    /// Browser names (`android`, `chrome`, `edge`, `firefox`, `ie`, `ios_saf`,
    /// `opera`, `safari`, `samsung`) mapped to `major << 16 | minor << 8 | patch`.
    /// Each component occupies one byte. `None` leaves browser targets unspecified.
    pub targets: Option<BTreeMap<LeanStr, u32>>,
    /// Lightning CSS feature names to always transform, such as `Nesting` or
    /// `LogicalProperties`. Unknown names and overlap with `exclude` are errors.
    pub include: Vec<LeanStr>,
    /// Lightning CSS feature names to preserve even when targets lack support.
    pub exclude: Vec<LeanStr>,
    /// Generate a separate v3 JSON map. Defaults to false.
    pub source_map: bool,
    /// Ordinary v3 JSON mapping the input CSS to original sources. Requires
    /// `source_map`; indexed maps are unsupported. Inline map comments are ignored.
    pub input_source_map: Option<LeanStr>,
    /// Include source text in the output map. Defaults to true.
    pub include_sources_content: bool,
    /// Names the caller has proven unused, interpreted by Lightning CSS.
    pub unused_symbols: Vec<LeanStr>,
    /// Allow the parser to discard invalid rules/declarations and return warnings.
    /// Defaults to false.
    pub error_recovery: bool,
}

impl Default for CssMinifyOptions {
    fn default() -> Self {
        Self {
            filename: "input.css".into(),
            targets: None,
            include: Vec::new(),
            exclude: Vec::new(),
            source_map: false,
            input_source_map: None,
            include_sources_content: true,
            unused_symbols: Vec::new(),
            error_recovery: false,
        }
    }
}

impl CssMinifyOptions {
    /// Converts the browser versions and feature names to Lightning CSS targets.
    ///
    /// # Errors
    ///
    /// Returns [`CssMinifyError::Options`] for an unknown browser or feature name, a
    /// version wider than 24 bits, or a feature that both `include` and `exclude` name.
    pub(super) fn targets(&self) -> Result<Targets, CssMinifyError> {
        let browsers = self
            .targets
            .as_ref()
            .map(|entries| {
                let mut browsers = Browsers::default();
                for (name, version) in entries {
                    if *version > 0x00ff_ffff {
                        return Err(CssMinifyError::Options(
                            format_args!("browser version exceeds 24 bits: {name}").to_lean_str(),
                        ));
                    }
                    let slot = match name.as_str() {
                        "android" => &mut browsers.android,
                        "chrome" => &mut browsers.chrome,
                        "edge" => &mut browsers.edge,
                        "firefox" => &mut browsers.firefox,
                        "ie" => &mut browsers.ie,
                        "ios_saf" => &mut browsers.ios_saf,
                        "opera" => &mut browsers.opera,
                        "safari" => &mut browsers.safari,
                        "samsung" => &mut browsers.samsung,
                        _ => {
                            return Err(CssMinifyError::Options(
                                format_args!("unknown browser: {name}").to_lean_str(),
                            ));
                        }
                    };
                    *slot = Some(*version);
                }
                Ok(browsers)
            })
            .transpose()?;
        let include = features(&self.include)?;
        let exclude = features(&self.exclude)?;
        if include.intersects(exclude) {
            return Err(CssMinifyError::Options(
                "include and exclude overlap".into(),
            ));
        }
        Ok(Targets {
            browsers,
            include,
            exclude,
        })
    }
}

fn features(names: &[LeanStr]) -> Result<Features, CssMinifyError> {
    names.iter().try_fold(Features::empty(), |flags, name| {
        Features::from_name(name)
            .map(|feature| flags | feature)
            .ok_or_else(|| {
                CssMinifyError::Options(format_args!("unknown CSS feature: {name}").to_lean_str())
            })
    })
}
