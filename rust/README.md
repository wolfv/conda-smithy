# smithy — conda-smithy, rewritten in Rust

A Rust rewrite of the core of [conda-smithy](https://github.com/conda-forge/conda-smithy),
built around one idea: **the parts people want to customise — lint rules and
CI templates — are scripts and templates, not compiled code.**

* Lint rules are [Rhai](https://rhai.rs) scripts. All built-in rules are
  written in Rhai too and embedded in the binary, so they double as
  copy-paste examples.
* CI configuration is rendered from [minijinja](https://github.com/mitsuhiko/minijinja)
  templates (Jinja2-compatible syntax).
* Both can be overridden or extended per feedstock from a `.smithy/`
  directory — no recompilation, no Rust knowledge required.

```
rust/
├── crates/
│   ├── smithy-core/          # library: parsing, lint engine, render engine
│   │   ├── lints/*.rhai      # the built-in lint rules (all Rhai!)
│   │   └── templates/*.j2    # the built-in CI templates (minijinja)
│   └── smithy-cli/           # the `smithy` binary
└── examples/demo-feedstock/  # a feedstock exercising every feature
```

## Building & usage

```console
$ cargo build --release
$ ./target/release/smithy lint --feedstock-dir path/to/feedstock
$ ./target/release/smithy rerender --feedstock-dir path/to/feedstock [--check]
```

`smithy lint` runs the built-in rules plus every `*.rhai` file in
`.smithy/lints/`, prints `error`s (lints) and `hint`s, and exits non-zero
if any lint fired. `smithy rerender` regenerates `.ci_support/*.yaml`,
GitHub Actions / Azure Pipelines configuration and `README.md` from the
build matrix defined in `conda-forge.yml`.

## Build variants

A `conda_build_config.yaml` (v0) or `variants.yaml` (v1) next to the
recipe fans the matrix out — every key with more than one value becomes a
job axis, `zip_keys` groups advance together instead of crossing, and
`# [linux]`-style line selectors are evaluated per target platform
(the selector expression is run by the same Rhai engine that powers lint
rules):

```yaml
python: ["3.12", "3.13"]
numpy: ["1.26", "2.0"]
zip_keys:
  - [python, numpy]
c_stdlib_version:
  - "2.17"   # [linux]
  - "10.13"  # [osx]
```

yields jobs like `linux_64_python3.12_numpy1.26`, each with its variant
values written to `.ci_support/<name>.yaml`.

## Writing your own lint rule

Drop a file into `.smithy/lints/` in your feedstock:

```rhai
// .smithy/lints/no_curl_in_tests.rhai
// Team policy: tests must not download things from the internet.
let commands = get(recipe, "test.commands");
if type_of(commands) == "array" {
    for cmd in commands {
        if type_of(cmd) == "string" && cmd.contains("curl ") {
            lint(`test command uses curl: ${cmd}`);
        }
    }
}
```

That's the whole workflow — the rule runs on the next `smithy lint`.

Every rule script gets these variables:

| variable             | type          | meaning                                     |
|----------------------|---------------|---------------------------------------------|
| `recipe`             | map           | the parsed recipe (v0 rendered with Jinja stubs, so `{{ compiler('c') }}` appears as `c_compiler_stub`) |
| `recipe_text`        | string        | the raw recipe file                         |
| `recipe_yaml`        | string        | the YAML that was actually parsed           |
| `recipe_version`     | int           | `0` = `meta.yaml`, `1` = `recipe.yaml`      |
| `recipe_parse_error` | string        | parse error, `""` on success                |
| `config`             | map           | the parsed `conda-forge.yml`                |
| `config_schema_error`| string        | schema problem in `conda-forge.yml`, or `""`|
| `recipe_files`       | array         | file names in the recipe directory          |
| `variant_config_filename` | string   | `conda_build_config.yaml` / `variants.yaml` / `""` |
| `variant_config_text`| string        | raw variant file contents, or `""`          |

and these helpers on top of the full Rhai standard library:

* `lint(msg)` / `hint(msg)` — report an error / a suggestion
* `get(value, "about.license")` — safe dotted-path lookup (returns `()` when absent)
* `has(value, "a.b.c")` — dotted-path existence check
* `is_match(text, regex)` / `find_all(text, regex)` — regular expressions
* `keys_in_order(recipe_yaml, "requirements")` — mapping keys in file order
* `join(array, ", ")` — stringify an array
* `parse_yaml(text)` — parse any YAML string into a scriptable value

Rhai gotchas worth knowing when writing rules: `trim()`, `replace()` and
friends mutate their string in place and return `()` (see the built-in
rules for the pattern), and backtick strings interpolate `${...}` but
cannot contain literal backticks.

To tweak a built-in rule, copy it from `crates/smithy-core/lints/` into
`.smithy/lints/` under a new name and disable the original in
`conda-forge.yml`:

```yaml
linter:
  skip: [license]
```

Rules run sandboxed (bounded operations, no filesystem or network access),
and a crashing rule degrades to a hint instead of failing the run.

## Customising CI templates

Two mechanisms, both in `.smithy/templates/`:

1. **Override a built-in**: copy e.g.
   `crates/smithy-core/templates/github-actions.yml.j2` to
   `.smithy/templates/github-actions.yml.j2` and edit it.
2. **Add a new output**: any other `*.j2` file becomes an extra rendered
   file — the `.j2` suffix is stripped and `__` becomes `/`. So
   `.smithy/templates/.github__workflows__docs.yml.j2` renders to
   `.github/workflows/docs.yml`.

Templates receive a fully precomputed context (see `RenderContext` in
`smithy-core`), most importantly:

* `configs` — the build matrix; each entry has `name`, `target_platform`,
  `build_platform`, `os`, `provider`, `upload`, `gha_runs_on`,
  `docker_image`, `cross_compile`
* `gha_configs` / `azure_configs` — the matrix filtered by provider
* `feedstock_name`, `package_name`, `about`, `maintainers`
* `github_actions` — `cancel_in_progress`, `max_parallel`, `timeout_minutes`, `triggers`
* `build_tool`, `recipe_dir`, `upload_on_branch`, `secrets`
* `forge_config` — the raw `conda-forge.yml`, so custom keys are reachable

## What is (deliberately) not here yet

This is the core of a rewrite, not full parity. The Python conda-smithy
still owns: recipe-aware variant pruning (we fan out every multi-valued
variant key; conda-smithy only fans out keys the recipe actually uses),
provider registration (`register-ci`, token rotation), `ci-skeleton`,
GitHub-API-backed lints (maintainer existence), and the long tail of
legacy providers (Travis, Circle, Drone, Woodpecker templates). The
architecture leaves room for all of these: matrix entries are plain
data, and providers are just templates.

## Rule id ↔ conda-smithy mapping

Each built-in rule's header comment names the conda-smithy message ids it
ports (e.g. `license.rhai` covers R-007, R-010, R-013), so behavior can be
diffed against the Python linter rule by rule.
