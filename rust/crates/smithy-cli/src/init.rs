//! `smithy init` — generate a fresh feedstock skeleton.

use std::path::Path;

use anyhow::{bail, Context, Result};

pub enum RecipeFormat {
    V0,
    V1,
}

const CONDA_FORGE_YML: &str = r#"# Feedstock configuration. Every key is optional.
# Reference: https://conda-forge.org/docs/maintainer/conda_forge_yml/

# Which CI provider builds each platform (github_actions, azure, None):
provider:
  linux_64: github_actions
  osx_64: github_actions
  win_64: github_actions

# Cross-compilation: target platform -> platform the build runs on, e.g.
# build_platform:
#   osx_arm64: osx_64

conda_build_tool: rattler-build

# Disable lint rules by id (built-in or from .smithy/lints/):
# linter:
#   skip: [jinja_spacing]
"#;

const RECIPE_V1: &str = r#"schema_version: 1

context:
  version: "0.1.0"

package:
  name: {name}
  version: ${{ version }}

source:
  url: https://example.com/{name}-${{ version }}.tar.gz
  sha256: 0000000000000000000000000000000000000000000000000000000000000000

build:
  number: 0

requirements:
  build: []
  host: []
  run: []

tests:
  - script: {name} --help

about:
  homepage: https://example.com
  license: MIT
  license_file: LICENSE
  summary: TODO

extra:
  recipe-maintainers:
    - TODO
"#;

const RECIPE_V0: &str = r#"{% set version = "0.1.0" %}

package:
  name: {name}
  version: {{ version }}

source:
  url: https://example.com/{name}-{{ version }}.tar.gz
  sha256: 0000000000000000000000000000000000000000000000000000000000000000

build:
  number: 0

requirements:
  build:
  host:
  run:

test:
  commands:
    - {name} --help

about:
  home: https://example.com
  license: MIT
  license_file: LICENSE
  summary: TODO

extra:
  recipe-maintainers:
    - TODO
"#;

const SMITHY_README: &str = r#"# .smithy/ — per-feedstock smithy extensions

* `lints/*.rhai`   — extra lint rules, run by `smithy lint`.
* `templates/*.j2` — CI template overrides and additions, rendered by
  `smithy rerender`. A template named after a built-in replaces it; any
  other `<path>.j2` becomes a new output file (`__` maps to `/`).

See `smithy lint --help`, `smithy rerender --help` and the smithy
documentation for the full scripting API.
"#;

const EXAMPLE_RULE: &str = r#"// An example custom lint rule. Rules are Rhai scripts with the recipe in
// scope; remove the leading `//` of the last block to activate it.
//
// Available: recipe (map), recipe_text, config, lint("..."), hint("..."),
// get(recipe, "a.b.c"), is_match(text, regex), and more.

// if get(recipe, "build.number") == () {
//     lint("please set a build number");
// }
"#;

/// Write `contents` unless `path` already exists. Returns whether written.
fn write_new(path: &Path, contents: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, contents)?;
    Ok(true)
}

/// Create a feedstock skeleton. Existing files are never overwritten.
/// Returns the list of created paths (relative to `target`).
pub fn init_feedstock(target: &Path, name: &str, format: RecipeFormat) -> Result<Vec<String>> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        bail!("`{name}` is not a valid package name (lowercase alphanumerics, -, _, . only)");
    }
    fs_err::create_dir_all(target).context("cannot create the feedstock directory")?;

    let (recipe_file, recipe_template) = match format {
        RecipeFormat::V1 => ("recipe/recipe.yaml", RECIPE_V1),
        RecipeFormat::V0 => ("recipe/meta.yaml", RECIPE_V0),
    };

    let files = [
        ("conda-forge.yml", CONDA_FORGE_YML.to_string()),
        (recipe_file, recipe_template.replace("{name}", name)),
        (".smithy/README.md", SMITHY_README.to_string()),
        (".smithy/lints/example.rhai", EXAMPLE_RULE.to_string()),
        (".gitignore", "build_artifacts/\noutput/\n".to_string()),
    ];

    let mut created = Vec::new();
    for (rel, contents) in files {
        if write_new(&target.join(rel), &contents)? {
            created.push(rel.to_string());
        }
    }
    Ok(created)
}
