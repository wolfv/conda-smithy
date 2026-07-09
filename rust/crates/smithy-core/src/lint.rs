//! The scriptable lint engine.
//!
//! Every lint rule — including all built-in ones — is a small
//! [Rhai](https://rhai.rs) script. A rule inspects the recipe and calls
//! `lint("...")` (an error) or `hint("...")` (a suggestion). Feedstock
//! maintainers add their own rules by dropping `*.rhai` files into
//! `.smithy/lints/`; no recompilation, no Rust knowledge needed.
//!
//! Every script runs with these variables in scope:
//!
//! | variable            | type          | meaning                                    |
//! |---------------------|---------------|--------------------------------------------|
//! | `recipe`            | map           | the parsed recipe (empty map if unparsable)|
//! | `recipe_text`       | string        | raw recipe file contents                   |
//! | `recipe_yaml`       | string        | the YAML that was parsed (v0: rendered)    |
//! | `recipe_version`    | int           | `0` = meta.yaml, `1` = recipe.yaml         |
//! | `recipe_parse_error`| string        | parse error, `""` when parsing succeeded   |
//! | `config`            | map           | parsed `conda-forge.yml`                   |
//! | `recipe_files`      | array<string> | file names inside the recipe directory     |
//!
//! and these helper functions besides the full Rhai standard library:
//!
//! * `lint(msg)` / `hint(msg)` — report a problem
//! * `get(value, "a.b.c")` — safe dotted-path lookup, returns `()` if absent
//! * `has(value, "a.b.c")` — dotted-path existence check
//! * `is_match(text, regex)` / `find_all(text, regex)` — regular expressions
//! * `keys_in_order(recipe_yaml, "requirements")` — mapping keys in file order
//! * `join(array, ", ")` — join array elements into a string

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use anyhow::{Context, Result};
use rhai::{Array, Dynamic, Engine, Map as RhaiMap, Scope};

use crate::feedstock::Feedstock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Must be fixed (conda-smithy calls these "lints").
    Lint,
    /// Advisory (conda-smithy calls these "hints").
    Hint,
}

#[derive(Debug, Clone)]
pub struct LintMessage {
    /// Rule id — the script file stem, e.g. `license`.
    pub rule: String,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct LintReport {
    pub messages: Vec<LintMessage>,
}

impl LintReport {
    pub fn lints(&self) -> impl Iterator<Item = &LintMessage> {
        self.messages
            .iter()
            .filter(|m| m.severity == Severity::Lint)
    }

    pub fn hints(&self) -> impl Iterator<Item = &LintMessage> {
        self.messages
            .iter()
            .filter(|m| m.severity == Severity::Hint)
    }

    pub fn has_lints(&self) -> bool {
        self.lints().next().is_some()
    }
}

/// A lint rule: a named Rhai script.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Identifier, used in output and in `linter.skip` of `conda-forge.yml`.
    pub id: String,
    pub source: String,
    /// `true` for rules shipped with smithy, `false` for `.smithy/lints/`.
    pub builtin: bool,
}

/// The built-in rules, embedded into the binary. They double as living
/// documentation: copy one into `.smithy/lints/` to tweak it.
pub fn builtin_rules() -> Vec<Rule> {
    macro_rules! rules {
        ($($name:literal),* $(,)?) => {
            vec![$(Rule {
                id: $name.to_string(),
                source: include_str!(concat!("../lints/", $name, ".rhai")).to_string(),
                builtin: true,
            }),*]
        };
    }
    rules![
        "parseable",
        "top_level_sections",
        "about_metadata",
        "maintainers",
        "license",
        "build_number",
        "requirements_order",
        "source_hash",
        "package_name_version",
        "noarch",
        "pin_spacing",
        "stdlib",
        "python_pins",
        "tests",
        "wheels",
        "trailing_newline",
        "selectors",
        "jinja_spacing",
        "pin_subpackage",
        "bundled_licenses",
        "misc_requirements",
        "noarch_selectors",
        "variant_config",
        "forge_yml",
    ]
}

/// Load user rules from a `.smithy/lints/` directory.
pub fn user_rules(dir: &Path) -> Result<Vec<Rule>> {
    let mut rules = Vec::new();
    if !dir.is_dir() {
        return Ok(rules);
    }
    let mut entries: Vec<_> = fs_err::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "rhai"))
        .collect();
    entries.sort();
    for path in entries {
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let source = fs_err::read_to_string(&path)?;
        rules.push(Rule {
            id,
            source,
            builtin: false,
        });
    }
    Ok(rules)
}

/// Convert a YAML value into a Rhai [`Dynamic`]. Mapping keys are
/// stringified so selector-style keys (`1.0`, `true`) don't break scripts.
fn yaml_to_dynamic(value: &serde_yaml::Value) -> Dynamic {
    match value {
        serde_yaml::Value::Null => Dynamic::UNIT,
        serde_yaml::Value::Bool(b) => (*b).into(),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into()
            } else {
                n.as_f64().unwrap_or(f64::NAN).into()
            }
        }
        serde_yaml::Value::String(s) => s.clone().into(),
        serde_yaml::Value::Sequence(seq) => {
            let array: Array = seq.iter().map(yaml_to_dynamic).collect();
            array.into()
        }
        serde_yaml::Value::Mapping(mapping) => {
            let mut map = RhaiMap::new();
            for (k, v) in mapping {
                let key = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => serde_yaml::to_string(other)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                };
                map.insert(key.into(), yaml_to_dynamic(v));
            }
            map.into()
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_dynamic(&tagged.value),
    }
}

/// Dotted-path lookup on a [`Dynamic`] (maps and integer array indices).
fn dynamic_get(value: &Dynamic, path: &str) -> Dynamic {
    let mut current = value.clone();
    for part in path.split('.') {
        if current.is_map() {
            let map = current.cast::<RhaiMap>();
            match map.get(part) {
                Some(next) => current = next.clone(),
                None => return Dynamic::UNIT,
            }
        } else if current.is_array() {
            let array = current.cast::<Array>();
            match part.parse::<usize>().ok().and_then(|i| array.get(i)) {
                Some(next) => current = next.clone(),
                None => return Dynamic::UNIT,
            }
        } else {
            return Dynamic::UNIT;
        }
    }
    current
}

/// The lint engine: a configured Rhai [`Engine`] plus the message sink.
pub struct Linter {
    engine: Engine,
    sink: Rc<RefCell<Vec<LintMessage>>>,
    current_rule: Rc<RefCell<String>>,
}

impl Default for Linter {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter {
    pub fn new() -> Self {
        let sink: Rc<RefCell<Vec<LintMessage>>> = Rc::default();
        let current_rule: Rc<RefCell<String>> = Rc::default();
        let mut engine = Engine::new();

        // Untrusted scripts shouldn't be able to hang the linter.
        engine.set_max_operations(1_000_000);
        engine.set_max_call_levels(64);
        // ...but the default parse-time complexity limits are tight enough
        // to reject legitimate rules (nested for + if + method chains).
        engine.set_max_expr_depths(256, 256);

        {
            let sink = sink.clone();
            let rule = current_rule.clone();
            engine.register_fn("lint", move |msg: &str| {
                sink.borrow_mut().push(LintMessage {
                    rule: rule.borrow().clone(),
                    severity: Severity::Lint,
                    message: msg.to_string(),
                });
            });
        }
        {
            let sink = sink.clone();
            let rule = current_rule.clone();
            engine.register_fn("hint", move |msg: &str| {
                sink.borrow_mut().push(LintMessage {
                    rule: rule.borrow().clone(),
                    severity: Severity::Hint,
                    message: msg.to_string(),
                });
            });
        }

        // NOTE: the exact `Map` overloads are required — Rhai ships a
        // built-in `get(map, key)` that would otherwise win overload
        // resolution and break dotted paths. The `Dynamic` overloads keep
        // `get((), "x")` and friends from erroring in scripts.
        engine.register_fn("get", |map: RhaiMap, path: &str| {
            dynamic_get(&Dynamic::from(map), path)
        });
        engine.register_fn("get", |value: Dynamic, path: &str| {
            dynamic_get(&value, path)
        });
        engine.register_fn("has", |map: RhaiMap, path: &str| {
            !dynamic_get(&Dynamic::from(map), path).is_unit()
        });
        engine.register_fn("has", |value: Dynamic, path: &str| {
            !dynamic_get(&value, path).is_unit()
        });

        // Mapping keys in *document order*. Rhai maps are sorted, so rules
        // that check ordering (sections, requirements) read the YAML text.
        engine.register_fn("keys_in_order", |yaml: &str, path: &str| -> Array {
            let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(yaml) else {
                return Array::new();
            };
            let target = if path.is_empty() {
                Some(&doc)
            } else {
                crate::recipe::lookup(&doc, path)
            };
            match target.and_then(|v| v.as_mapping()) {
                Some(mapping) => mapping
                    .keys()
                    .filter_map(|k| k.as_str())
                    .map(|k| Dynamic::from(k.to_string()))
                    .collect(),
                None => Array::new(),
            }
        });

        engine.register_fn("join", |array: Array, sep: &str| -> String {
            array
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(sep)
        });

        // Parse any YAML string into a scriptable value; `()` on failure.
        // Lets rules inspect auxiliary files like conda_build_config.yaml.
        engine.register_fn("parse_yaml", |text: &str| -> Dynamic {
            serde_yaml::from_str::<serde_yaml::Value>(text)
                .map(|v| yaml_to_dynamic(&v))
                .unwrap_or(Dynamic::UNIT)
        });

        // Regex helpers, cached per pattern.
        let cache: Rc<RefCell<HashMap<String, regex::Regex>>> = Rc::default();
        fn compiled(
            cache: &Rc<RefCell<HashMap<String, regex::Regex>>>,
            pattern: &str,
        ) -> Result<regex::Regex, Box<rhai::EvalAltResult>> {
            if let Some(re) = cache.borrow().get(pattern) {
                return Ok(re.clone());
            }
            let re = regex::Regex::new(pattern)
                .map_err(|e| format!("invalid regex `{pattern}`: {e}"))?;
            cache.borrow_mut().insert(pattern.to_string(), re.clone());
            Ok(re)
        }
        {
            let cache = cache.clone();
            engine.register_fn(
                "is_match",
                move |text: &str, pattern: &str| -> Result<bool, Box<rhai::EvalAltResult>> {
                    Ok(compiled(&cache, pattern)?.is_match(text))
                },
            );
        }
        {
            let cache = cache.clone();
            engine.register_fn(
                "find_all",
                move |text: &str, pattern: &str| -> Result<Array, Box<rhai::EvalAltResult>> {
                    Ok(compiled(&cache, pattern)?
                        .find_iter(text)
                        .map(|m| Dynamic::from(m.as_str().to_string()))
                        .collect())
                },
            );
        }

        Linter {
            engine,
            sink,
            current_rule,
        }
    }

    /// Build the variable scope shared by every rule.
    fn scope_for(feedstock: &Feedstock) -> Scope<'static> {
        let mut scope = Scope::new();
        let recipe = feedstock
            .recipe
            .parsed
            .as_ref()
            .map(yaml_to_dynamic)
            .filter(|d| d.is_map())
            .unwrap_or_else(|| Dynamic::from(RhaiMap::new()));
        scope.push_constant("recipe", recipe);
        scope.push_constant("recipe_text", feedstock.recipe.text.clone());
        scope.push_constant("recipe_yaml", feedstock.recipe.rendered.clone());
        scope.push_constant("recipe_version", feedstock.recipe.version.as_int());
        scope.push_constant(
            "recipe_parse_error",
            feedstock.recipe.parse_error.clone().unwrap_or_default(),
        );
        scope.push_constant("config", yaml_to_dynamic(&feedstock.config.raw));
        scope.push_constant(
            "config_schema_error",
            feedstock.config.schema_error.clone().unwrap_or_default(),
        );

        let recipe_files: Array = std::fs::read_dir(feedstock.recipe_dir())
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| Dynamic::from(e.file_name().to_string_lossy().into_owned()))
                    .collect()
            })
            .unwrap_or_default();
        scope.push_constant("recipe_files", recipe_files);

        // The variant file (conda_build_config.yaml / variants.yaml), if
        // any: its file name and raw text, for rules like the macOS
        // deployment-target checks.
        let (variant_name, variant_text) = crate::variants::VARIANT_FILE_NAMES
            .iter()
            .find_map(|name| {
                let path = feedstock.recipe_dir().join(name);
                fs_err::read_to_string(&path)
                    .ok()
                    .map(|text| (name.to_string(), text))
            })
            .unwrap_or_default();
        scope.push_constant("variant_config_filename", variant_name);
        scope.push_constant("variant_config_text", variant_text);
        scope
    }

    /// Rule ids disabled via `linter.skip` in `conda-forge.yml`.
    fn skipped_rules(feedstock: &Feedstock) -> Vec<String> {
        crate::recipe::lookup(&feedstock.config.raw, "linter.skip")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Run a set of rules against a feedstock.
    pub fn run(&mut self, feedstock: &Feedstock, rules: &[Rule]) -> Result<LintReport> {
        let base_scope = Self::scope_for(feedstock);
        let skipped = Self::skipped_rules(feedstock);

        for rule in rules {
            if skipped.iter().any(|s| s == &rule.id) {
                continue;
            }
            *self.current_rule.borrow_mut() = rule.id.clone();
            let mut scope = base_scope.clone();
            let ast = self
                .engine
                .compile(&rule.source)
                .with_context(|| format!("lint rule `{}` failed to compile", rule.id))?;
            if let Err(err) = self.engine.run_ast_with_scope(&mut scope, &ast) {
                // A broken rule must never take the whole lint run down;
                // surface it as a hint instead.
                self.sink.borrow_mut().push(LintMessage {
                    rule: rule.id.clone(),
                    severity: Severity::Hint,
                    message: format!("lint rule `{}` crashed: {err}", rule.id),
                });
            }
        }

        Ok(LintReport {
            messages: self.sink.borrow_mut().drain(..).collect(),
        })
    }
}

/// Lint a feedstock with the built-in rules plus any user rules found in
/// `.smithy/lints/`.
pub fn lint_feedstock(feedstock: &Feedstock) -> Result<LintReport> {
    let mut rules = builtin_rules();
    rules.extend(user_rules(&feedstock.user_lints_dir())?);
    Linter::new().run(feedstock, &rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_get_nested() {
        let yaml: serde_yaml::Value =
            serde_yaml::from_str("about:\n  homepage: https://x\n  license: MIT\n").unwrap();
        let dynamic = yaml_to_dynamic(&yaml);
        assert!(dynamic.is_map());
        let hit = dynamic_get(&dynamic, "about.homepage");
        assert_eq!(hit.to_string(), "https://x", "got: {hit:?}");
    }
}
