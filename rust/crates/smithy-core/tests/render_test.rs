//! Integration tests for matrix computation and template rendering.

use std::path::{Path, PathBuf};

use smithy_core::render::{compute_matrix, render_feedstock, rerender};
use smithy_core::{Feedstock, ForgeConfig, Recipe, RecipeVersion};

const RECIPE: &str = r#"schema_version: 1
package:
  name: demo
  version: "1.0"
build:
  number: 0
about:
  homepage: https://example.com
  license: MIT
  license_file: LICENSE
  summary: demo
extra:
  recipe-maintainers:
    - wolfv
"#;

fn feedstock_with_config(forge_yaml: &str) -> Feedstock {
    Feedstock {
        root: PathBuf::from("/nonexistent"),
        config: ForgeConfig::from_yaml(forge_yaml).unwrap(),
        recipe: Recipe::from_text(
            PathBuf::from("recipe.yaml"),
            RecipeVersion::V1,
            RECIPE.to_string(),
        ),
    }
}

#[test]
fn default_matrix_is_three_azure_platforms() {
    let feedstock = feedstock_with_config("");
    let matrix = compute_matrix(&feedstock);
    let names: Vec<_> = matrix.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["linux_64", "osx_64", "win_64"]);
    assert!(matrix.iter().all(|c| c.provider == "azure"));
    assert!(matrix.iter().all(|c| !c.cross_compile));
}

#[test]
fn cross_compile_and_extra_platforms() {
    let feedstock = feedstock_with_config(
        "provider:\n  linux_aarch64: github_actions\nbuild_platform:\n  osx_arm64: osx_64\n",
    );
    let matrix = compute_matrix(&feedstock);

    let aarch64 = matrix.iter().find(|c| c.name == "linux_aarch64").unwrap();
    assert_eq!(aarch64.provider, "github_actions");
    assert_eq!(
        aarch64.docker_image.as_deref(),
        Some("quay.io/condaforge/linux-anvil-aarch64")
    );

    let arm = matrix.iter().find(|c| c.name == "osx_arm64").unwrap();
    assert!(arm.cross_compile);
    assert_eq!(arm.build_platform, "osx-64");
    assert_eq!(arm.target_platform, "osx-arm64");
}

#[test]
fn provider_none_disables_platform() {
    let feedstock = feedstock_with_config("provider:\n  win_64: None\n");
    let matrix = compute_matrix(&feedstock);
    assert!(!matrix.iter().any(|c| c.name == "win_64"));
}

#[test]
fn noarch_builds_on_noarch_platforms_only() {
    let mut feedstock = feedstock_with_config("");
    feedstock.recipe = Recipe::from_text(
        PathBuf::from("recipe.yaml"),
        RecipeVersion::V1,
        RECIPE.replace(
            "build:\n  number: 0",
            "build:\n  number: 0\n  noarch: python",
        ),
    );
    let matrix = compute_matrix(&feedstock);
    let names: Vec<_> = matrix.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["linux_64"]);
}

#[test]
fn rendered_output_is_valid_yaml_without_jinja_leftovers() {
    let feedstock =
        feedstock_with_config("provider:\n  linux_64: github_actions\n  osx_64: azure\n");
    let files = render_feedstock(&feedstock).unwrap();

    let paths: Vec<_> = files
        .iter()
        .map(|f| f.path.to_string_lossy().into_owned())
        .collect();
    assert!(paths.contains(&".github/workflows/conda-build.yml".to_string()));
    assert!(paths.contains(&"azure-pipelines.yml".to_string()));
    assert!(paths.contains(&"README.md".to_string()));
    assert!(paths.contains(&".ci_support/linux_64.yaml".to_string()));

    for file in &files {
        // No un-rendered minijinja syntax may survive. (GitHub's own
        // ${{ }} expressions are fine.)
        assert!(
            !file.contents.contains("{%"),
            "{}: leftover template tag",
            file.path.display()
        );
        if file
            .path
            .extension()
            .is_some_and(|e| e == "yml" || e == "yaml")
        {
            serde_yaml::from_str::<serde_yaml::Value>(&file.contents)
                .unwrap_or_else(|e| panic!("{} is not valid YAML: {e}", file.path.display()));
        }
    }
}

#[test]
fn readme_mentions_package_and_maintainers() {
    let feedstock = feedstock_with_config("");
    let files = render_feedstock(&feedstock).unwrap();
    let readme = files
        .iter()
        .find(|f| f.path == Path::new("README.md"))
        .unwrap();
    assert!(readme.contents.contains("pixi add demo"));
    assert!(readme.contents.contains("@wolfv"));
}

#[test]
fn user_template_override_and_extra_templates() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs_err::create_dir_all(root.join("recipe")).unwrap();
    fs_err::write(root.join("recipe/recipe.yaml"), RECIPE).unwrap();
    fs_err::write(root.join("conda-forge.yml"), "").unwrap();
    fs_err::create_dir_all(root.join(".smithy/templates")).unwrap();
    // Override a built-in template...
    fs_err::write(
        root.join(".smithy/templates/README.md.j2"),
        "# custom readme for {{ package_name }}\n",
    )
    .unwrap();
    // ...and add a brand new output file.
    fs_err::write(
        root.join(".smithy/templates/.github__dependabot.yml.j2"),
        "version: 2\nupdates: [] # {{ feedstock_name }}\n",
    )
    .unwrap();

    let feedstock = Feedstock::load(root).unwrap();
    let written = rerender(&feedstock).unwrap();

    let readme = fs_err::read_to_string(root.join("README.md")).unwrap();
    assert_eq!(readme, "# custom readme for demo\n");

    let dependabot = fs_err::read_to_string(root.join(".github/dependabot.yml")).unwrap();
    assert!(dependabot.contains("# demo"));

    assert!(written.contains(&PathBuf::from(".github/dependabot.yml")));
}

#[test]
fn skip_render_is_honoured() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs_err::create_dir_all(root.join("recipe")).unwrap();
    fs_err::write(root.join("recipe/recipe.yaml"), RECIPE).unwrap();
    fs_err::write(
        root.join("conda-forge.yml"),
        "skip_render:\n  - README.md\n",
    )
    .unwrap();

    let feedstock = Feedstock::load(root).unwrap();
    rerender(&feedstock).unwrap();
    assert!(!root.join("README.md").exists());
    assert!(root.join("azure-pipelines.yml").exists());
}
