//! Parsing of `conda-forge.yml`, the per-feedstock configuration file.
//!
//! Only the keys the Rust rewrite acts on are modelled as typed fields;
//! everything else is preserved verbatim in [`ForgeConfig::raw`] so that
//! templates and lint scripts can still reach any custom key.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// CI provider selected for a platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Azure,
    GithubActions,
    Travis,
    Circle,
    Appveyor,
    Drone,
    Woodpecker,
    /// Platform explicitly disabled (`provider: {linux_aarch64: None}`).
    None,
    /// Emulated build on the default provider for the base platform.
    Default,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Azure => "azure",
            Provider::GithubActions => "github_actions",
            Provider::Travis => "travis",
            Provider::Circle => "circle",
            Provider::Appveyor => "appveyor",
            Provider::Drone => "drone",
            Provider::Woodpecker => "woodpecker",
            Provider::None => "none",
            Provider::Default => "default",
        }
    }
}

/// A `provider:` value in `conda-forge.yml` is stringly-typed in the wild:
/// `azure`, `None`, `False`, `default`, or even a list. We accept the common
/// scalar spellings.
fn deserialize_provider<'de, D>(deserializer: D) -> Result<Option<Provider>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_yaml::Value::deserialize(deserializer)?;
    let provider = match &value {
        serde_yaml::Value::Null => Some(Provider::None),
        serde_yaml::Value::Bool(false) => Some(Provider::None),
        serde_yaml::Value::Bool(true) => Some(Provider::Default),
        serde_yaml::Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "azure" => Some(Provider::Azure),
            "github_actions" => Some(Provider::GithubActions),
            "travis" => Some(Provider::Travis),
            "circle" => Some(Provider::Circle),
            "appveyor" => Some(Provider::Appveyor),
            "drone" => Some(Provider::Drone),
            "woodpecker" => Some(Provider::Woodpecker),
            "none" | "false" => Some(Provider::None),
            "default" | "emulated" => Some(Provider::Default),
            other => {
                return Err(D::Error::custom(format!("unknown CI provider `{other}`")));
            }
        },
        other => {
            return Err(D::Error::custom(format!(
                "unsupported provider value: {other:?}"
            )));
        }
    };
    Ok(provider)
}

fn default_true() -> bool {
    true
}

/// `github_actions:` sub-table.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GithubActionsConfig {
    pub self_hosted: bool,
    /// Cancel intermediate builds when a new commit is pushed.
    #[serde(default = "default_true")]
    pub cancel_in_progress: bool,
    /// Maximum number of parallel jobs.
    pub max_parallel: Option<u32>,
    /// Job timeout in minutes.
    pub timeout_minutes: Option<u32>,
}

impl Default for GithubActionsConfig {
    fn default() -> Self {
        Self {
            self_hosted: false,
            cancel_in_progress: true,
            max_parallel: None,
            timeout_minutes: None,
        }
    }
}

/// Typed view over `conda-forge.yml`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ForgeConfig {
    /// Platform (`linux_64`, `osx_arm64`, ...) → CI provider.
    #[serde(deserialize_with = "deserialize_provider_map")]
    pub provider: BTreeMap<String, Provider>,
    /// Target platform → platform the build actually runs on
    /// (cross-compilation), e.g. `osx_arm64: osx_64`.
    pub build_platform: BTreeMap<String, String>,
    /// Directory containing the recipe, relative to the feedstock root.
    pub recipe_dir: Option<String>,
    /// Branch on which package uploads are allowed (e.g. `main`).
    pub upload_on_branch: Option<String>,
    pub github_actions: GithubActionsConfig,
    /// Platforms noarch packages are built on.
    pub noarch_platforms: Vec<String>,
    /// Selected conda build tool (`conda-build`, `rattler-build`, ...).
    pub conda_build_tool: Option<String>,
    /// Files rerender must not touch.
    pub skip_render: Vec<String>,
    /// Test policy: `all`, `native`, `native_and_emulated`.
    pub test: Option<String>,
    /// Anaconda.org channel targets, e.g. `["conda-forge main"]`.
    pub channel_targets: Option<Vec<String>>,
    /// Everything, including keys not modelled above.
    #[serde(skip)]
    pub raw: serde_yaml::Value,
}

fn deserialize_provider_map<'de, D>(deserializer: D) -> Result<BTreeMap<String, Provider>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "deserialize_provider")] Option<Provider>);

    let map = BTreeMap::<String, Wrapper>::deserialize(deserializer)?;
    Ok(map
        .into_iter()
        .filter_map(|(k, Wrapper(v))| v.map(|v| (k, v)))
        .collect())
}

impl ForgeConfig {
    /// Parse from YAML text, keeping the raw document around for templates.
    pub fn from_yaml(text: &str) -> Result<Self> {
        let raw: serde_yaml::Value = if text.trim().is_empty() {
            serde_yaml::Value::Mapping(Default::default())
        } else {
            serde_yaml::from_str(text).context("conda-forge.yml is not valid YAML")?
        };
        let mut config: ForgeConfig =
            serde_yaml::from_value(raw.clone()).context("unsupported value in conda-forge.yml")?;
        config.raw = raw;
        Ok(config)
    }

    /// Load `conda-forge.yml` from a feedstock directory. A missing file is
    /// treated as an empty configuration, mirroring conda-smithy.
    pub fn from_feedstock_dir(dir: &Path) -> Result<Self> {
        let path = dir.join("conda-forge.yml");
        if !path.exists() {
            return Ok(ForgeConfig {
                raw: serde_yaml::Value::Mapping(Default::default()),
                ..Default::default()
            });
        }
        let text = fs_err::read_to_string(&path)?;
        Self::from_yaml(&text)
    }

    /// The recipe directory, defaulting to `recipe/`.
    pub fn recipe_dir(&self) -> &str {
        self.recipe_dir.as_deref().unwrap_or("recipe")
    }

    /// The CI provider for a platform key such as `linux_64`, applying
    /// conda-forge's defaults when the key is absent.
    pub fn provider_for(&self, platform: &str) -> Provider {
        if let Some(p) = self.provider.get(platform) {
            return *p;
        }
        // Fall back to the base platform (`linux_aarch64` → `linux`).
        let base = platform.split('_').next().unwrap_or(platform);
        if let Some(p) = self.provider.get(base) {
            return *p;
        }
        match platform {
            // conda-forge defaults: main three platforms build on Azure.
            "linux_64" | "osx_64" | "osx_arm64" | "win_64" => Provider::Azure,
            // Exotic platforms are opt-in.
            _ => Provider::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_has_defaults() {
        let config = ForgeConfig::from_yaml("").unwrap();
        assert_eq!(config.recipe_dir(), "recipe");
        assert_eq!(config.provider_for("linux_64"), Provider::Azure);
        assert_eq!(config.provider_for("linux_aarch64"), Provider::None);
    }

    #[test]
    fn provider_spellings() {
        let config = ForgeConfig::from_yaml(
            "provider:\n  linux_64: github_actions\n  linux_aarch64: default\n  win: None\n",
        )
        .unwrap();
        assert_eq!(config.provider_for("linux_64"), Provider::GithubActions);
        assert_eq!(config.provider_for("linux_aarch64"), Provider::Default);
        // `win:` covers `win_64` via the base-platform fallback.
        assert_eq!(config.provider_for("win_64"), Provider::None);
    }

    #[test]
    fn raw_preserves_unknown_keys() {
        let config = ForgeConfig::from_yaml("my_custom_key: 42\n").unwrap();
        assert_eq!(config.raw["my_custom_key"], serde_yaml::Value::from(42));
    }
}
