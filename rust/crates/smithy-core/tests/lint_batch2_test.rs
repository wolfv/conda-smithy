//! Tests for the second batch of built-in lint rules: selectors, jinja
//! spacing, pin_subpackage/compatible, bundled licenses, noarch selectors,
//! variant-config and conda-forge.yml checks.

use std::path::PathBuf;

use smithy_core::lint::{builtin_rules, Linter};
use smithy_core::{Feedstock, ForgeConfig, Recipe, RecipeVersion, Severity};

fn feedstock_from(recipe_text: &str, version: RecipeVersion, forge_yaml: &str) -> Feedstock {
    let file = match version {
        RecipeVersion::V0 => "meta.yaml",
        RecipeVersion::V1 => "recipe.yaml",
    };
    Feedstock {
        root: PathBuf::from("/nonexistent"),
        config: ForgeConfig::from_yaml(forge_yaml).unwrap(),
        recipe: Recipe::from_text(PathBuf::from(file), version, recipe_text.to_string()),
    }
}

fn messages_for(feedstock: &Feedstock, rule: &str) -> Vec<(Severity, String)> {
    let report = Linter::new().run(feedstock, &builtin_rules()).unwrap();
    for m in &report.messages {
        assert!(
            !m.message.contains("crashed"),
            "a rule crashed: {}",
            m.message
        );
    }
    report
        .messages
        .into_iter()
        .filter(|m| m.rule == rule)
        .map(|m| (m.severity, m.message))
        .collect()
}

#[test]
fn untidy_selectors_are_linted() {
    let recipe = "package:\n  name: x\nbuild:\n  number: 0\n  skip: true # [win]\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "selectors");
    // one space before `#` instead of two → untidy
    assert!(
        msgs.iter().any(|(_, m)| m.contains("two spaces")),
        "{msgs:?}"
    );

    let tidy = "package:\n  name: x\nbuild:\n  number: 0\n  skip: true  # [win]\n";
    let feedstock = feedstock_from(tidy, RecipeVersion::V0, "");
    assert!(messages_for(&feedstock, "selectors").is_empty());
}

#[test]
fn old_python_selectors() {
    let recipe = "build:\n  number: 0\n  skip: true  # [py27]\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "selectors");
    assert!(
        msgs.iter().any(|(sev, _)| *sev == Severity::Hint),
        "{msgs:?}"
    );

    let recipe = "build:\n  number: 0\n  skip: true  # [py38]\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "selectors");
    assert!(
        msgs.iter().any(|(sev, _)| *sev == Severity::Lint),
        "{msgs:?}"
    );
}

#[test]
fn comment_selectors_forbidden_in_v1() {
    let recipe = "package:\n  name: x\nbuild:\n  number: 0  # [win]\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V1, "");
    let msgs = messages_for(&feedstock, "selectors");
    assert!(msgs.iter().any(|(_, m)| m.contains("not allowed in v1")));
}

#[test]
fn jinja_set_spacing() {
    let recipe = "{% set version=\"1.0\" %}\npackage:\n  name: x\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "jinja_spacing");
    assert!(
        msgs.iter()
            .any(|(sev, m)| *sev == Severity::Lint && m.contains("{% set <name> = <value> %}")),
        "{msgs:?}"
    );
}

#[test]
fn jinja_reference_padding_hint() {
    let recipe = "{% set version = \"1.0\" %}\npackage:\n  name: x\n  version: {{version}}\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "jinja_spacing");
    assert!(
        msgs.iter().any(|(sev, _)| *sev == Severity::Hint),
        "{msgs:?}"
    );
}

#[test]
fn pin_compatible_on_own_output_is_linted() {
    let recipe = r#"package:
  name: mylib
  version: "1.0"
build:
  number: 0
requirements:
  run:
    - {{ pin_compatible('mylib-core') }}
outputs:
  - name: mylib-core
"#;
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "pin_subpackage");
    assert!(
        msgs.iter()
            .any(|(_, m)| m.contains("pin_subpackage should be used")),
        "{msgs:?}"
    );
}

#[test]
fn pin_subpackage_on_foreign_package_is_linted_v1() {
    let recipe = r#"package:
  name: mylib
  version: "1.0"
build:
  number: 0
requirements:
  run:
    - ${{ pin_subpackage("numpy") }}
"#;
    let feedstock = feedstock_from(recipe, RecipeVersion::V1, "");
    let msgs = messages_for(&feedstock, "pin_subpackage");
    assert!(
        msgs.iter()
            .any(|(_, m)| m.contains("pin_compatible should be used")),
        "{msgs:?}"
    );
}

#[test]
fn correct_pins_pass() {
    let recipe = r#"package:
  name: mylib
  version: "1.0"
build:
  number: 0
requirements:
  run:
    - {{ pin_subpackage('mylib-core') }}
    - {{ pin_compatible('numpy') }}
outputs:
  - name: mylib-core
"#;
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    assert!(messages_for(&feedstock, "pin_subpackage").is_empty());
}

#[test]
fn rust_without_license_bundling() {
    let recipe = "package:\n  name: x\nrequirements:\n  build:\n    - {{ compiler('rust') }}\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "bundled_licenses");
    assert!(msgs
        .iter()
        .any(|(_, m)| m.contains("cargo-bundle-licenses")));

    let ok = "package:\n  name: x\nrequirements:\n  build:\n    - {{ compiler('rust') }}\n    - cargo-bundle-licenses\n";
    let feedstock = feedstock_from(ok, RecipeVersion::V0, "");
    assert!(messages_for(&feedstock, "bundled_licenses").is_empty());
}

#[test]
fn misc_requirements_rules() {
    let recipe = r#"package:
  name: x
build:
  number: 0
  script: python setup.py install
source:
  url: https://pypi.io/packages/source/x/x-1.0.tar.gz
requirements:
  build:
    - toolchain
  host:
    - numpy x.x
"#;
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "misc_requirements");
    assert!(msgs
        .iter()
        .any(|(sev, m)| *sev == Severity::Lint && m.contains("numpy x.x")));
    assert!(msgs
        .iter()
        .any(|(sev, m)| *sev == Severity::Lint && m.contains("toolchain")));
    assert!(msgs
        .iter()
        .any(|(sev, m)| *sev == Severity::Hint && m.contains("pip")));
    assert!(msgs
        .iter()
        .any(|(sev, m)| *sev == Severity::Hint && m.contains("pypi.org")));
}

#[test]
fn noarch_with_selector_requirements() {
    let recipe = r#"package:
  name: x
build:
  number: 0
  noarch: python
requirements:
  host:
    - python
  run:
    - python
    - pywin32  # [win]
"#;
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let msgs = messages_for(&feedstock, "noarch_selectors");
    assert!(!msgs.is_empty(), "expected a noarch selector lint");

    // With multiple noarch_platforms, platform-only selectors are allowed.
    let feedstock = feedstock_from(
        recipe,
        RecipeVersion::V0,
        "noarch_platforms: [linux_64, win_64]\n",
    );
    assert!(messages_for(&feedstock, "noarch_selectors").is_empty());
}

#[test]
fn noarch_with_skip_is_linted_v1() {
    let recipe = "package:\n  name: x\nbuild:\n  number: 0\n  noarch: python\n  skip: win\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V1, "");
    let msgs = messages_for(&feedstock, "noarch_selectors");
    assert!(msgs.iter().any(|(_, m)| m.contains("skips")));
}

#[test]
fn forge_yml_value_checks() {
    let feedstock = feedstock_from(
        "package:\n  name: x\n",
        RecipeVersion::V0,
        "test: sometimes\nconda_build_tool: meson\nskip_render: README.md\n",
    );
    let msgs = messages_for(&feedstock, "forge_yml");
    assert!(
        msgs.iter().any(|(_, m)| m.contains("test is 'sometimes'")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter()
            .any(|(_, m)| m.contains("conda_build_tool 'meson'")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter().any(|(_, m)| m.contains("skip_render")),
        "{msgs:?}"
    );
    assert!(msgs.iter().all(|(sev, _)| *sev == Severity::Lint));
}

#[test]
fn forge_yml_valid_config_passes() {
    let feedstock = feedstock_from(
        "package:\n  name: x\n",
        RecipeVersion::V0,
        "test: native\nconda_build_tool: rattler-build\nskip_render: [README.md]\n",
    );
    assert!(messages_for(&feedstock, "forge_yml").is_empty());
}
