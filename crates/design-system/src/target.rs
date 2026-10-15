//! Browser versions used to remove generated color compatibility branches.

use lean_string::{LeanStr, ToLeanStr};
use serde::{Deserialize, Serialize};

/// A browser target whose name or version cannot be interpreted.
#[derive(Clone, Debug, thiserror::Error)]
#[error("invalid browser target `{target}`: {reason}")]
pub struct BrowserTargetError {
    target: LeanStr,
    reason: &'static str,
}

/// Minimum browser versions guaranteed by the host. An empty list keeps fallbacks.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<LeanStr>", into = "Vec<LeanStr>")]
pub struct BrowserTargets(Vec<BrowserTarget>);

impl BrowserTargets {
    /// Parses browser names followed by one to three decimal version components.
    ///
    /// Accepts `android`, `chrome`, `edge`, `firefox`, `ie`, `ios`, `opera`,
    /// `safari`, and `samsung`, for example `chrome111` or `safari16.2`.
    /// An empty iterator preserves compatibility branches.
    ///
    /// # Errors
    ///
    /// Returns the rejected target when its browser is unknown or a version
    /// component is absent, nonnumeric, or greater than 65535.
    pub fn parse<I, S>(targets: I) -> Result<Self, BrowserTargetError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        targets
            .into_iter()
            .map(|target| BrowserTarget::parse(target.as_ref()))
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    /// Whether every target supports the generated two-color `color-mix()`.
    ///
    /// `current_color` includes color variables whose runtime value may be
    /// `currentcolor`. Safari and iOS require version 17 for those values,
    /// matching the preset's guard against earlier `WebKit` failures.
    pub(crate) fn supports_color_mix(&self, current_color: bool) -> bool {
        !self.0.is_empty()
            && self.0.iter().all(|target| {
                let minimum = match target.browser {
                    Browser::Android | Browser::Chrome | Browser::Edge => [111, 0, 0],
                    Browser::Firefox => [113, 0, 0],
                    Browser::Safari | Browser::Ios if current_color => [17, 0, 0],
                    Browser::Safari | Browser::Ios => [16, 2, 0],
                    Browser::Opera => [97, 0, 0],
                    Browser::Samsung => [22, 0, 0],
                    Browser::Ie => return false,
                };
                target.version >= minimum
            })
    }
}

impl TryFrom<Vec<LeanStr>> for BrowserTargets {
    type Error = BrowserTargetError;

    fn try_from(targets: Vec<LeanStr>) -> Result<Self, Self::Error> {
        Self::parse(targets)
    }
}

impl From<BrowserTargets> for Vec<LeanStr> {
    fn from(targets: BrowserTargets) -> Self {
        targets.0.into_iter().map(|target| target.source).collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BrowserTarget {
    source: LeanStr,
    browser: Browser,
    version: [u16; 3],
}

impl BrowserTarget {
    fn parse(source: &str) -> Result<Self, BrowserTargetError> {
        let error = |reason| BrowserTargetError {
            target: source.to_lean_str(),
            reason,
        };
        let split = source
            .find(|character: char| character.is_ascii_digit())
            .ok_or_else(|| error("expected a browser name and version"))?;
        let browser = match &source[..split] {
            "android" => Browser::Android,
            "chrome" => Browser::Chrome,
            "edge" => Browser::Edge,
            "firefox" => Browser::Firefox,
            "ie" => Browser::Ie,
            "ios" => Browser::Ios,
            "opera" => Browser::Opera,
            "safari" => Browser::Safari,
            "samsung" => Browser::Samsung,
            _ => return Err(error("unknown browser")),
        };
        let mut version = [0; 3];
        for (index, component) in source[split..].split('.').enumerate() {
            if index >= version.len()
                || component.is_empty()
                || !component.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(error("expected one to three decimal version components"));
            }
            version[index] = component
                .parse()
                .map_err(|_| error("version component exceeds 65535"))?;
        }
        Ok(Self {
            source: source.to_lean_str(),
            browser,
            version,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Browser {
    Android,
    Chrome,
    Edge,
    Firefox,
    Ie,
    Ios,
    Opera,
    Safari,
    Samsung,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_targets_must_support_color_mix() {
        for targets in [vec![], vec!["chrome110"], vec!["chrome111", "firefox112"]] {
            assert!(
                !BrowserTargets::parse(targets)
                    .unwrap()
                    .supports_color_mix(false)
            );
        }
        let targets = BrowserTargets::parse(["chrome111", "firefox113", "safari16.2"]).unwrap();
        assert!(targets.supports_color_mix(false));
        assert!(!targets.supports_color_mix(true));
        assert!(
            BrowserTargets::parse(["safari17", "ios17"])
                .unwrap()
                .supports_color_mix(true)
        );
    }

    #[test]
    fn malformed_targets_are_rejected() {
        for target in [
            "chrome",
            "es2022",
            "chrome111beta",
            "safari16..2",
            "chrome65536",
            "chrome1.2.3.4",
        ] {
            assert!(BrowserTargets::parse([target]).is_err(), "{target}");
        }
    }
}
