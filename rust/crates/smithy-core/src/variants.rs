//! Build-variant handling: `conda_build_config.yaml` (v0 recipes) and
//! `variants.yaml` (v1 recipes).
//!
//! A variant file maps variable names to lists of values, e.g.
//!
//! ```yaml
//! python:
//!   - "3.11"
//!   - "3.12"    # [not win]
//! numpy:
//!   - "1.26"
//!   - "2.0"
//! zip_keys:
//!   - [python, numpy]
//! ```
//!
//! Keys with more than one value fan out into separate CI jobs; `zip_keys`
//! groups advance together instead of forming a cross product. Line
//! selectors (`# [linux]`, `# [not win]`, ...) are evaluated per target
//! platform before parsing — the boolean expression is handed to a tiny
//! Rhai engine, the same language used for lint rules.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};

use crate::recipe::Recipe;

/// Variant file names, in the order they are looked up.
pub const VARIANT_FILE_NAMES: &[&str] = &["conda_build_config.yaml", "variants.yaml"];

/// Variant keys that are always kept even when the recipe never mentions
/// them — they steer the build environment rather than a dependency.
const ALWAYS_USED: &[&str] = &[
    "target_platform",
    "channel_sources",
    "channel_targets",
    "docker_image",
    "pin_run_as_build",
    "MACOSX_DEPLOYMENT_TARGET",
    "MACOSX_SDK_VERSION",
    "cdt_name",
];

/// The parsed variant configuration for one target platform.
#[derive(Debug, Clone, Default)]
pub struct VariantConfig {
    /// Variable → list of values (stringified), in file order.
    pub variants: Vec<(String, Vec<String>)>,
    /// Groups of keys that are zipped together.
    pub zip_keys: Vec<Vec<String>>,
}

/// One matrix cell: variable → chosen value.
pub type VariantCell = BTreeMap<String, String>;

/// What a recipe references, used to decide which variant keys matter.
/// conda-smithy only fans the matrix out over variables the recipe
/// actually uses; everything else is noise from shared pinning files.
#[derive(Debug, Default, Clone)]
pub struct UsedVars {
    /// Package names appearing in any requirements section.
    pub requirements: BTreeSet<String>,
    /// The full recipe text (raw + rendered), for `{{ var }}` lookups and
    /// compiler/stdlib detection.
    haystack: String,
}

impl UsedVars {
    pub fn from_recipe(recipe: &Recipe) -> Self {
        let mut requirements = BTreeSet::new();
        if let Some(doc) = &recipe.parsed {
            collect_requirement_names(doc, &mut requirements);
        }
        UsedVars {
            requirements,
            haystack: format!("{}\n{}", recipe.text, recipe.rendered),
        }
    }

    /// Is a single variant key used by the recipe?
    pub fn contains(&self, key: &str) -> bool {
        if ALWAYS_USED.contains(&key) || key.starts_with("cdt_") {
            return true;
        }
        // A dependency of the recipe, e.g. `python`, `numpy`, `openssl`.
        if self.requirements.contains(key) {
            return true;
        }
        // Referenced as a template variable: `{{ python }}` / `${{ numpy }}`.
        if self.haystack.contains(&format!("{{{{ {key} }}}}"))
            || self.haystack.contains(&format!("{{{{{key}}}}}"))
        {
            return true;
        }
        // Compiler / stdlib keys: `rust_compiler_version` is used when the
        // recipe calls `compiler('rust')`, `c_stdlib_version` when it calls
        // `stdlib('c')`.
        for (suffixes, stub, call) in [
            (
                ["_compiler", "_compiler_version"],
                "_compiler_stub",
                "compiler(",
            ),
            (["_stdlib", "_stdlib_version"], "_stdlib_stub", "stdlib("),
        ] {
            for suffix in suffixes {
                if let Some(lang) = key.strip_suffix(suffix) {
                    if self.haystack.contains(&format!("{lang}{stub}"))
                        || self.haystack.contains(&format!("{call}'{lang}'"))
                        || self.haystack.contains(&format!("{call}\"{lang}\""))
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Walk the document and collect the bare package names (`numpy >=1.20`
/// → `numpy`) of everything inside `requirements:` / `run_exports:`
/// subtrees, wherever they appear (top level or per output).
fn collect_requirement_names(value: &serde_yaml::Value, into: &mut BTreeSet<String>) {
    fn walk(value: &serde_yaml::Value, in_reqs: bool, into: &mut BTreeSet<String>) {
        match value {
            serde_yaml::Value::Mapping(mapping) => {
                for (key, sub) in mapping {
                    let flag = in_reqs
                        || matches!(key.as_str(), Some("requirements") | Some("run_exports"));
                    walk(sub, flag, into);
                }
            }
            serde_yaml::Value::Sequence(seq) => {
                for item in seq {
                    match item {
                        serde_yaml::Value::String(s) if in_reqs => {
                            if let Some(name) = s.split_whitespace().next() {
                                into.insert(name.to_string());
                            }
                        }
                        other => walk(other, in_reqs, into),
                    }
                }
            }
            _ => {}
        }
    }
    walk(value, false, into);
}

/// Evaluate a conda selector expression (`linux and not aarch64`) for a
/// target platform like `linux_64`. Unknown identifiers make the selector
/// pass, which errs on keeping values.
pub fn eval_selector(expr: &str, target_platform: &str) -> bool {
    let (os, arch) = match target_platform.split_once('_') {
        Some(pair) => pair,
        None => (target_platform, "64"),
    };

    let mut scope = rhai::Scope::new();
    for name in [
        "linux",
        "osx",
        "win",
        "unix",
        "aarch64",
        "arm64",
        "ppc64le",
        "s390x",
        "x86",
        "x86_64",
        "emscripten",
        "wasm32",
    ] {
        let value = match name {
            "unix" => os == "linux" || os == "osx",
            "x86_64" | "x86" => arch == "64",
            other => other == os || other == arch,
        };
        scope.push_constant(name, value);
    }

    // Python's boolean operators → Rhai's.
    let expr = expr
        .replace(" and ", " && ")
        .replace(" or ", " || ")
        .replace("not ", "!");

    let engine = rhai::Engine::new_raw();
    engine
        .eval_expression_with_scope::<bool>(&mut scope, &expr)
        .unwrap_or(true)
}

/// Drop lines whose `# [selector]` doesn't match the platform, and strip
/// matching selectors so plain YAML remains.
fn apply_selectors(text: &str, target_platform: &str) -> String {
    let selector_re = regex::Regex::new(r"^(?<code>.*?)\s*#\s*\[(?<expr>[^\]]+)\]\s*$").unwrap();
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        if let Some(caps) = selector_re.captures(line) {
            if eval_selector(&caps["expr"], target_platform) {
                out.push_str(&caps["code"]);
                out.push('\n');
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn value_to_string(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => s.clone(),
        other => serde_yaml::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

impl VariantConfig {
    /// Parse a variant file for one target platform.
    pub fn from_yaml(text: &str, target_platform: &str) -> Result<Self> {
        let resolved = apply_selectors(text, target_platform);
        if resolved.trim().is_empty() {
            return Ok(Self::default());
        }
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&resolved).context("variant file is not valid YAML")?;
        let Some(mapping) = doc.as_mapping() else {
            return Ok(Self::default());
        };

        let mut config = Self::default();
        for (key, value) in mapping {
            let Some(key) = key.as_str() else { continue };
            if key == "zip_keys" {
                if let Some(groups) = value.as_sequence() {
                    for group in groups {
                        if let Some(group) = group.as_sequence() {
                            config.zip_keys.push(
                                group
                                    .iter()
                                    .filter_map(|k| k.as_str().map(str::to_string))
                                    .collect(),
                            );
                        }
                    }
                }
                continue;
            }
            // `pin_run_as_build` and friends are conda-build internals we
            // pass through untouched into the ci_support file — they never
            // fan out. Treat any mapping value as a single opaque value.
            let values = match value {
                serde_yaml::Value::Sequence(seq) => seq.iter().map(value_to_string).collect(),
                other => vec![value_to_string(other)],
            };
            if !values.is_empty() {
                config.variants.push((key.to_string(), values));
            }
        }
        Ok(config)
    }

    /// Locate and parse the variant file of a recipe directory. Returns an
    /// empty config when no variant file exists.
    pub fn from_recipe_dir(recipe_dir: &Path, target_platform: &str) -> Result<Self> {
        for name in VARIANT_FILE_NAMES {
            let path = recipe_dir.join(name);
            if path.is_file() {
                let text = fs_err::read_to_string(&path)?;
                return Self::from_yaml(&text, target_platform)
                    .with_context(|| format!("failed to parse {}", path.display()));
            }
        }
        Ok(Self::default())
    }

    /// Drop every variant key the recipe doesn't use. A key survives when
    /// [`UsedVars::contains`] says so, or when it shares a `zip_keys`
    /// group with a surviving key (zipped variables must stay aligned).
    pub fn prune(&mut self, used: &UsedVars) {
        let directly_used: BTreeSet<String> = self
            .variants
            .iter()
            .map(|(k, _)| k.clone())
            .filter(|k| used.contains(k))
            .collect();
        let keep: BTreeSet<String> = self
            .variants
            .iter()
            .map(|(k, _)| k.clone())
            .filter(|key| {
                directly_used.contains(key)
                    || self.zip_keys.iter().any(|group| {
                        group.iter().any(|k| k == key)
                            && group.iter().any(|k| directly_used.contains(k))
                    })
            })
            .collect();
        self.variants.retain(|(k, _)| keep.contains(k));
        self.zip_keys
            .retain(|group| group.iter().filter(|k| keep.contains(*k)).count() > 1);
    }

    /// Expand into matrix cells: the cartesian product over all axes,
    /// where a zip group forms a single axis. Single-valued variables are
    /// carried into every cell but do not multiply the matrix.
    ///
    /// The returned cells pair with [`Self::fanout_keys`] for naming.
    pub fn expand(&self) -> Result<Vec<VariantCell>> {
        // Build axes: each axis is a list of partial cells.
        let mut axes: Vec<Vec<VariantCell>> = Vec::new();
        let mut consumed: Vec<&str> = Vec::new();

        for group in &self.zip_keys {
            let members: Vec<(&String, &Vec<String>)> = self
                .variants
                .iter()
                .filter(|(k, _)| group.contains(k))
                .map(|(k, v)| (k, v))
                .collect();
            if members.is_empty() {
                continue;
            }
            let len = members[0].1.len();
            for (key, values) in &members {
                if values.len() != len {
                    anyhow::bail!(
                        "zip_keys group {group:?} has mismatched lengths: `{}` has {} values, `{}` has {}",
                        members[0].0, len, key, values.len()
                    );
                }
            }
            let axis: Vec<VariantCell> = (0..len)
                .map(|i| {
                    members
                        .iter()
                        .map(|(k, v)| ((*k).clone(), v[i].clone()))
                        .collect()
                })
                .collect();
            consumed.extend(members.iter().map(|(k, _)| k.as_str()));
            axes.push(axis);
        }

        for (key, values) in &self.variants {
            if consumed.contains(&key.as_str()) {
                continue;
            }
            let axis: Vec<VariantCell> = values
                .iter()
                .map(|v| BTreeMap::from([(key.clone(), v.clone())]))
                .collect();
            axes.push(axis);
        }

        // Cartesian product.
        let mut cells: Vec<VariantCell> = vec![BTreeMap::new()];
        for axis in axes {
            let mut next = Vec::with_capacity(cells.len() * axis.len());
            for cell in &cells {
                for partial in &axis {
                    let mut merged = cell.clone();
                    merged.extend(partial.iter().map(|(k, v)| (k.clone(), v.clone())));
                    next.push(merged);
                }
            }
            cells = next;
        }
        Ok(cells)
    }

    /// Keys that actually fan the matrix out (more than one value, or a
    /// member of a multi-entry zip group). Used for job names.
    pub fn fanout_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        for (key, values) in &self.variants {
            let fans = if let Some(group) = self.zip_keys.iter().find(|g| g.contains(key)) {
                // A zip group fans if its members have >1 value.
                self.variants
                    .iter()
                    .any(|(k, v)| group.contains(k) && v.len() > 1)
            } else {
                values.len() > 1
            };
            if fans {
                keys.push(key.clone());
            }
        }
        keys
    }
}

/// Human/file-name-friendly job suffix for a cell: `python3.10_numpy1.26`.
/// Only fanned-out keys participate; `target_platform`-style keys never do.
pub fn cell_name_suffix(cell: &VariantCell, fanout_keys: &[String]) -> String {
    let mut parts = Vec::new();
    for key in fanout_keys {
        if let Some(value) = cell.get(key) {
            let sanitize = |s: &str| {
                s.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
                    .collect::<String>()
            };
            parts.push(format!("{}{}", sanitize(key), sanitize(value)));
        }
    }
    parts.join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CBC: &str = r#"python:
  - "3.11"
  - "3.12"
numpy:
  - "1.26"
  - "2.0"
zip_keys:
  - [python, numpy]
c_stdlib_version:
  - "2.17"   # [linux]
  - "10.13"  # [osx]
MACOSX_DEPLOYMENT_TARGET:  # [osx]
  - "10.13"  # [osx]
"#;

    #[test]
    fn selectors_filter_per_platform() {
        let linux = VariantConfig::from_yaml(CBC, "linux_64").unwrap();
        let stdlib = linux
            .variants
            .iter()
            .find(|(k, _)| k == "c_stdlib_version")
            .unwrap();
        assert_eq!(stdlib.1, ["2.17"]);
        assert!(!linux
            .variants
            .iter()
            .any(|(k, _)| k == "MACOSX_DEPLOYMENT_TARGET"));

        let osx = VariantConfig::from_yaml(CBC, "osx_arm64").unwrap();
        assert!(osx
            .variants
            .iter()
            .any(|(k, _)| k == "MACOSX_DEPLOYMENT_TARGET"));
    }

    #[test]
    fn zip_keys_advance_together() {
        let config = VariantConfig::from_yaml(CBC, "linux_64").unwrap();
        let cells = config.expand().unwrap();
        // python×numpy zipped → 2 cells, not 4.
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0]["python"], "3.11");
        assert_eq!(cells[0]["numpy"], "1.26");
        assert_eq!(cells[1]["python"], "3.12");
        assert_eq!(cells[1]["numpy"], "2.0");
        // Single-valued keys are carried along without fanning out.
        assert_eq!(cells[0]["c_stdlib_version"], "2.17");
    }

    #[test]
    fn mismatched_zip_lengths_error() {
        let bad = "a: [1, 2]\nb: [1, 2, 3]\nzip_keys:\n  - [a, b]\n";
        let config = VariantConfig::from_yaml(bad, "linux_64").unwrap();
        assert!(config.expand().is_err());
    }

    #[test]
    fn unzipped_keys_cross_product() {
        let text = "python: [\"3.11\", \"3.12\"]\ncuda: [\"11.8\", \"12.0\"]\n";
        let config = VariantConfig::from_yaml(text, "linux_64").unwrap();
        assert_eq!(config.expand().unwrap().len(), 4);
        assert_eq!(config.fanout_keys(), ["python", "cuda"]);
    }

    #[test]
    fn cell_names_are_stable_and_sanitised() {
        let config =
            VariantConfig::from_yaml("python: [\"3.10\", \"3.11 *_cpython\"]\n", "linux_64")
                .unwrap();
        let cells = config.expand().unwrap();
        let keys = config.fanout_keys();
        assert_eq!(cell_name_suffix(&cells[0], &keys), "python3.10");
        assert_eq!(cell_name_suffix(&cells[1], &keys), "python3.11cpython");
    }

    #[test]
    fn selector_expressions() {
        assert!(eval_selector("linux", "linux_64"));
        assert!(!eval_selector("win", "linux_64"));
        assert!(eval_selector("not win", "linux_64"));
        assert!(eval_selector("linux and x86_64", "linux_64"));
        assert!(!eval_selector("linux and aarch64", "linux_64"));
        assert!(eval_selector("osx or win", "win_64"));
        assert!(eval_selector("unix", "osx_arm64"));
        assert!(eval_selector("arm64", "osx_arm64"));
        // Unknown identifiers keep the line.
        assert!(eval_selector("py >= 38", "linux_64"));
    }

    #[test]
    fn empty_or_missing_file_yields_one_cell() {
        let config = VariantConfig::default();
        let cells = config.expand().unwrap();
        assert_eq!(cells.len(), 1);
        assert!(cells[0].is_empty());
        assert!(config.fanout_keys().is_empty());
    }
}

#[cfg(test)]
mod prune_tests {
    use super::*;
    use crate::recipe::RecipeVersion;
    use std::path::PathBuf;

    fn used(recipe_text: &str) -> UsedVars {
        let recipe = Recipe::from_text(
            PathBuf::from("meta.yaml"),
            RecipeVersion::V0,
            recipe_text.to_string(),
        );
        UsedVars::from_recipe(&recipe)
    }

    const RECIPE: &str = r#"package:
  name: x
  version: "1"
requirements:
  build:
    - {{ compiler('c') }}
    - {{ stdlib('c') }}
  host:
    - python
  run:
    - python
"#;

    #[test]
    fn unused_keys_are_pruned() {
        let mut config = VariantConfig::from_yaml(
            "python: [\"3.12\", \"3.13\"]\nnumpy: [\"1.26\", \"2.0\"]\nrust_compiler_version: [\"1.80\"]\n",
            "linux_64",
        )
        .unwrap();
        config.prune(&used(RECIPE));
        let keys: Vec<_> = config.variants.iter().map(|(k, _)| k.as_str()).collect();
        // python is a requirement; numpy and the rust compiler are not used.
        assert_eq!(keys, ["python"]);
        assert_eq!(config.expand().unwrap().len(), 2);
    }

    #[test]
    fn compiler_stdlib_and_always_used_keys_survive() {
        let mut config = VariantConfig::from_yaml(
            "c_compiler_version: [\"13\"]\nc_stdlib_version: [\"2.17\"]\nchannel_sources: [\"conda-forge\"]\nMACOSX_DEPLOYMENT_TARGET: [\"11.0\"]\n",
            "osx_64",
        )
        .unwrap();
        config.prune(&used(RECIPE));
        let keys: Vec<_> = config.variants.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            [
                "c_compiler_version",
                "c_stdlib_version",
                "channel_sources",
                "MACOSX_DEPLOYMENT_TARGET"
            ]
        );
    }

    #[test]
    fn jinja_reference_counts_as_usage() {
        let recipe = "package:\n  name: x\nbuild:\n  string: py{{ python }}\n";
        let mut config =
            VariantConfig::from_yaml("python: [\"3.12\", \"3.13\"]\n", "linux_64").unwrap();
        config.prune(&used(recipe));
        assert_eq!(config.variants.len(), 1);
    }

    #[test]
    fn zip_partner_of_used_key_survives() {
        let mut config = VariantConfig::from_yaml(
            "python: [\"3.12\", \"3.13\"]\npython_impl: [\"cpython\", \"cpython\"]\nzip_keys:\n  - [python, python_impl]\n",
            "linux_64",
        )
        .unwrap();
        config.prune(&used(RECIPE));
        let keys: Vec<_> = config.variants.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["python", "python_impl"]);
        assert_eq!(config.zip_keys.len(), 1);
    }

    #[test]
    fn requirements_in_outputs_count() {
        let recipe = "package:\n  name: x\noutputs:\n  - name: sub\n    requirements:\n      run:\n        - numpy >=1.20\n";
        let mut config =
            VariantConfig::from_yaml("numpy: [\"1.26\", \"2.0\"]\n", "linux_64").unwrap();
        config.prune(&used(recipe));
        assert_eq!(config.variants.len(), 1);
    }
}
