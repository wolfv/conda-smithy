//! Integration tests for the Rhai lint engine.

use std::path::{Path, PathBuf};

use smithy_core::lint::{builtin_rules, lint_feedstock, Linter, Rule};
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

fn run_builtins(feedstock: &Feedstock) -> Vec<(String, Severity, String)> {
    let report = Linter::new().run(feedstock, &builtin_rules()).unwrap();
    report
        .messages
        .into_iter()
        .map(|m| (m.rule, m.severity, m.message))
        .collect()
}

const GOOD_V0: &str = r#"{% set version = "2.0" %}

package:
  name: goodpkg
  version: {{ version }}

source:
  url: https://example.com/goodpkg-2.0.tar.gz
  sha256: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef

build:
  number: 3

requirements:
  build:
    - {{ compiler('c') }}
    - {{ stdlib('c') }}
  host:
    - python
  run:
    - python

test:
  imports:
    - goodpkg

about:
  home: https://example.com
  license: MIT
  license_file: LICENSE
  summary: A good package

extra:
  recipe-maintainers:
    - somebody
"#;

#[test]
fn good_v0_recipe_has_no_lints() {
    let feedstock = feedstock_from(GOOD_V0, RecipeVersion::V0, "");
    let messages = run_builtins(&feedstock);
    let lints: Vec<_> = messages
        .iter()
        .filter(|(_, sev, _)| *sev == Severity::Lint)
        .collect();
    assert!(lints.is_empty(), "unexpected lints: {lints:?}");
}

#[test]
fn bad_recipe_triggers_expected_rules() {
    // Sections out of order, about incomplete, no maintainers, no build
    // number, no tests, unknown license, unhashed source, bad pin spacing.
    let bad = r#"source:
  url: https://example.com/x.tar.gz

package:
  name: BadPkg
  version: 1.0

requirements:
  run:
    - python >= 3.8
    - numpy>=1.20

about:
  license: unknown
"#;
    let feedstock = feedstock_from(bad, RecipeVersion::V0, "");
    let messages = run_builtins(&feedstock);
    let fired: Vec<&str> = messages.iter().map(|(rule, _, _)| rule.as_str()).collect();

    for expected in [
        "top_level_sections",   // wrong order
        "about_metadata",       // missing home/summary
        "maintainers",          // missing
        "license",              // unknown
        "build_number",         // missing
        "source_hash",          // no checksum
        "package_name_version", // uppercase name + float version
        "pin_spacing",          // `python >= 3.8` and `numpy>=1.20`
        "tests",                // no tests
        "trailing_newline",     // ok actually -- see below
    ] {
        if expected == "trailing_newline" {
            // control: this one must NOT fire (file ends with one newline)
            assert!(!fired.contains(&expected), "trailing_newline misfired");
        } else {
            assert!(
                fired.contains(&expected),
                "rule {expected} did not fire; got {fired:?}"
            );
        }
    }
}

#[test]
fn stdlib_rule_catches_compiler_without_stdlib() {
    let recipe = r#"package:
  name: x
  version: "1"
build:
  number: 0
requirements:
  build:
    - {{ compiler('c') }}
about:
  home: h
  license: MIT
  license_file: L
  summary: s
"#;
    let feedstock = feedstock_from(recipe, RecipeVersion::V0, "");
    let messages = run_builtins(&feedstock);
    assert!(
        messages
            .iter()
            .any(|(rule, _, msg)| rule == "stdlib" && msg.contains("stdlib")),
        "stdlib rule did not fire: {messages:?}"
    );
}

#[test]
fn v1_recipe_rules_use_v1_spellings() {
    // v1 wants about.homepage, and license_file is always required.
    let recipe = "schema_version: 1\npackage:\n  name: y\n  version: \"1\"\n";
    let feedstock = feedstock_from(recipe, RecipeVersion::V1, "");
    let messages = run_builtins(&feedstock);
    assert!(messages
        .iter()
        .any(|(_, _, msg)| msg.contains("The homepage item is expected")));
}

#[test]
fn linter_skip_disables_rules() {
    let recipe = "package:\n  name: z\n";
    let all = run_builtins(&feedstock_from(recipe, RecipeVersion::V0, ""));
    assert!(all.iter().any(|(rule, _, _)| rule == "build_number"));

    let skipped = run_builtins(&feedstock_from(
        recipe,
        RecipeVersion::V0,
        "linter:\n  skip: [build_number]\n",
    ));
    assert!(!skipped.iter().any(|(rule, _, _)| rule == "build_number"));
}

#[test]
fn unparseable_recipe_reports_single_parse_lint() {
    let feedstock = feedstock_from("package: [unclosed\n", RecipeVersion::V0, "");
    let messages = run_builtins(&feedstock);
    assert!(messages
        .iter()
        .any(|(rule, sev, _)| rule == "parseable" && *sev == Severity::Lint));
    // Structural rules must stay quiet when the recipe didn't parse.
    assert!(!messages.iter().any(|(rule, _, _)| rule == "build_number"));
}

#[test]
fn broken_user_rule_becomes_hint_not_crash() {
    let feedstock = feedstock_from(GOOD_V0, RecipeVersion::V0, "");
    let rules = vec![Rule {
        id: "explodes".into(),
        source: "this_function_does_not_exist(recipe);".into(),
        builtin: false,
    }];
    let report = Linter::new().run(&feedstock, &rules).unwrap();
    assert_eq!(report.messages.len(), 1);
    assert_eq!(report.messages[0].severity, Severity::Hint);
    assert!(report.messages[0].message.contains("crashed"));
}

#[test]
fn custom_rule_can_lint_and_hint() {
    let feedstock = feedstock_from(GOOD_V0, RecipeVersion::V0, "");
    let rules = vec![Rule {
        id: "team_policy".into(),
        source: r#"
            // Example of the user-facing API surface.
            if get(recipe, "build.number") == 3 { lint("no build number 3 allowed!"); }
            if has(recipe, "requirements.run") { hint("has run requirements"); }
            if is_match(get(recipe, "package.name"), "^good") { hint("name starts with good"); }
        "#
        .into(),
        builtin: false,
    }];
    let report = Linter::new().run(&feedstock, &rules).unwrap();
    let texts: Vec<_> = report.messages.iter().map(|m| m.message.as_str()).collect();
    assert!(texts.contains(&"no build number 3 allowed!"));
    assert!(texts.contains(&"has run requirements"));
    assert!(texts.contains(&"name starts with good"));
}

#[test]
fn demo_feedstock_is_lint_clean() {
    let demo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo-feedstock");
    let feedstock = Feedstock::load(&demo).unwrap();
    let report = lint_feedstock(&feedstock).unwrap();
    assert!(
        !report.has_lints(),
        "demo feedstock has lints: {:?}",
        report.messages
    );
    // The custom .smithy/lints rule fires as hints.
    assert!(report.hints().any(|m| m.rule == "no_example_urls"));
}
