//! Loading of recipes: v0 `meta.yaml` (conda-build) and v1 `recipe.yaml`
//! (rattler-build).
//!
//! v1 recipes are plain YAML (templating lives inside `${{ ... }}` strings),
//! so they parse directly. v0 `meta.yaml` is Jinja-templated YAML; like the
//! Python linter's `NullUndefined` environment we evaluate the template with
//! minijinja, stubbing out conda-build functions (`compiler`, `pin_subpackage`,
//! ...) so the result parses as YAML while still being recognisable to lints.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use minijinja::value::Value as JinjaValue;
use minijinja::{Environment, UndefinedBehavior};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeVersion {
    /// conda-build `meta.yaml`
    V0,
    /// rattler-build `recipe.yaml`
    V1,
}

impl RecipeVersion {
    pub fn as_int(&self) -> i64 {
        match self {
            RecipeVersion::V0 => 0,
            RecipeVersion::V1 => 1,
        }
    }
}

/// A loaded recipe: raw text plus the leniently-parsed YAML document.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub path: PathBuf,
    pub version: RecipeVersion,
    /// The recipe file exactly as on disk (lints inspect raw lines too).
    pub text: String,
    /// The YAML actually parsed: identical to `text` for v1, the
    /// Jinja-rendered document for v0.
    pub rendered: String,
    /// The parsed document. For v0 this is the *rendered* YAML (Jinja
    /// evaluated with stubs); `None` when the file could not be parsed.
    pub parsed: Option<serde_yaml::Value>,
    /// Parse error message, if parsing failed (surfaced as a lint).
    pub parse_error: Option<String>,
}

impl Recipe {
    /// Locate and load the recipe inside a recipe directory.
    ///
    /// `recipe.yaml` wins over `meta.yaml` when both exist (the duplicate is
    /// reported by the `duplicate_recipes` lint, not here).
    pub fn from_recipe_dir(recipe_dir: &Path) -> Result<Self> {
        let v1 = recipe_dir.join("recipe.yaml");
        let v0 = recipe_dir.join("meta.yaml");
        if v1.exists() {
            Self::from_path(&v1, RecipeVersion::V1)
        } else if v0.exists() {
            Self::from_path(&v0, RecipeVersion::V0)
        } else {
            bail!(
                "no recipe found in {}: expected recipe.yaml or meta.yaml",
                recipe_dir.display()
            )
        }
    }

    pub fn from_path(path: &Path, version: RecipeVersion) -> Result<Self> {
        let text = fs_err::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(Self::from_text(path.to_path_buf(), version, text))
    }

    pub fn from_text(path: PathBuf, version: RecipeVersion, text: String) -> Self {
        let yaml_source = match version {
            RecipeVersion::V1 => text.clone(),
            RecipeVersion::V0 => render_v0_jinja(&text),
        };
        match serde_yaml::from_str::<serde_yaml::Value>(&yaml_source) {
            Ok(parsed) => Recipe {
                path,
                version,
                text,
                rendered: yaml_source,
                parsed: Some(parsed),
                parse_error: None,
            },
            Err(err) => Recipe {
                path,
                version,
                text,
                rendered: yaml_source,
                parsed: None,
                parse_error: Some(err.to_string()),
            },
        }
    }

    /// The package name, looking at `package.name` and (v1) `recipe.name`.
    pub fn package_name(&self) -> Option<String> {
        let doc = self.parsed.as_ref()?;
        for path in ["package.name", "recipe.name"] {
            if let Some(serde_yaml::Value::String(s)) = lookup(doc, path) {
                return Some(s.clone());
            }
        }
        None
    }
}

/// Follow a dotted path (`about.license`) through YAML mappings.
pub fn lookup<'a>(value: &'a serde_yaml::Value, path: &str) -> Option<&'a serde_yaml::Value> {
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

/// Evaluate the Jinja in a v0 `meta.yaml` just enough for the result to be
/// valid YAML. conda-build functions become recognisable placeholder strings
/// (`compiler('c')` → `c_compiler_stub`), mirroring the Python linter, and
/// unknown variables render as empty strings.
fn render_v0_jinja(text: &str) -> String {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Lenient);

    env.add_function("compiler", |lang: String| format!("{lang}_compiler_stub"));
    env.add_function("stdlib", |lang: String| format!("{lang}_stdlib_stub"));
    env.add_function("pin_subpackage", |name: String| {
        format!("subpackage_pin {name}")
    });
    env.add_function("pin_compatible", |name: String| {
        format!("compatible_pin {name}")
    });
    env.add_function("cdt", |name: String| format!("cdt_{name}"));
    env.add_function(
        "load_file_regex",
        |_args: minijinja::value::Rest<JinjaValue>| JinjaValue::UNDEFINED,
    );
    env.add_function(
        "load_setup_py_data",
        |_args: minijinja::value::Rest<JinjaValue>| {
            JinjaValue::from(std::collections::BTreeMap::<String, String>::new())
        },
    );
    env.add_function("environ", |_name: String| String::new());
    env.add_global(
        "environ",
        JinjaValue::from(std::collections::BTreeMap::<String, String>::new()),
    );
    env.add_global("target_platform", "linux-64");
    env.add_global("build_platform", "linux-64");
    env.add_global("python_min", "3.9");
    // `os.environ.get(...)` and `datetime` appear in the wild; a permissive
    // undefined swallows attribute access on them.

    match env.render_str(text, minijinja::context! {}) {
        Ok(rendered) => rendered,
        // If Jinja evaluation fails (exotic constructs), fall back to a
        // crude strip so YAML parsing still gets a chance.
        Err(_) => strip_jinja(text),
    }
}

/// Last-resort v0 preprocessing: drop `{% ... %}` statements and blank out
/// `{{ ... }}` expressions.
fn strip_jinja(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("{%") {
            continue;
        }
        let mut line = line.to_string();
        while let (Some(start), Some(end)) = (line.find("{{"), line.find("}}")) {
            if end < start {
                break;
            }
            line.replace_range(start..end + 2, "jinja_stub");
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const V0_RECIPE: &str = r#"{% set name = "foo" %}
{% set version = "1.2.3" %}

package:
  name: {{ name }}
  version: {{ version }}

build:
  number: 0

requirements:
  build:
    - {{ compiler('c') }}
    - {{ stdlib('c') }}
  run:
    - {{ pin_subpackage('foo-core') }}

about:
  license: MIT
"#;

    #[test]
    fn v0_jinja_renders_with_stubs() {
        let recipe = Recipe::from_text(
            PathBuf::from("meta.yaml"),
            RecipeVersion::V0,
            V0_RECIPE.to_string(),
        );
        assert!(recipe.parse_error.is_none(), "{:?}", recipe.parse_error);
        let doc = recipe.parsed.as_ref().unwrap();
        assert_eq!(recipe.package_name().as_deref(), Some("foo"));
        assert_eq!(
            lookup(doc, "package.version").unwrap().as_str(),
            Some("1.2.3")
        );
        let build_reqs = lookup(doc, "requirements.build").unwrap();
        assert_eq!(build_reqs[0].as_str(), Some("c_compiler_stub"));
        assert_eq!(build_reqs[1].as_str(), Some("c_stdlib_stub"));
        let run_reqs = lookup(doc, "requirements.run").unwrap();
        assert_eq!(run_reqs[0].as_str(), Some("subpackage_pin foo-core"));
    }

    #[test]
    fn v1_parses_directly() {
        let recipe = Recipe::from_text(
            PathBuf::from("recipe.yaml"),
            RecipeVersion::V1,
            "schema_version: 1\npackage:\n  name: bar\n  version: ${{ version }}\n".to_string(),
        );
        assert_eq!(recipe.package_name().as_deref(), Some("bar"));
    }

    #[test]
    fn broken_yaml_reports_parse_error() {
        let recipe = Recipe::from_text(
            PathBuf::from("meta.yaml"),
            RecipeVersion::V0,
            "package:\n  name: [unclosed\n".to_string(),
        );
        assert!(recipe.parsed.is_none());
        assert!(recipe.parse_error.is_some());
    }
}
