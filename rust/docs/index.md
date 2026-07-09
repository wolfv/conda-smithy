# smithy

**smithy** is a Rust rewrite of the core of
[conda-smithy](https://github.com/conda-forge/conda-smithy): it lints
conda recipes and generates ("rerenders") the CI configuration of a
feedstock.

It is built around one idea:

!!! tip "The parts people want to customise are scripts, not compiled code."

    * Every **lint rule** — including all built-in ones — is a small
      [Rhai](https://rhai.rs) script. Drop a `.rhai` file into
      `.smithy/lints/` in your feedstock and it runs on the next
      `smithy lint`. No Rust, no recompilation.
    * Every **CI file** is rendered from a
      [minijinja](https://github.com/mitsuhiko/minijinja) template
      (Jinja2-compatible syntax). Override a built-in template or add a
      brand new workflow from `.smithy/templates/`.

## A 60-second tour

```console
$ smithy init mytool --feedstock-dir mytool-feedstock
created conda-forge.yml
created recipe/recipe.yaml
created .smithy/README.md
created .smithy/lints/example.rhai

$ cd mytool-feedstock
$ smithy lint
✓ recipe/recipe.yaml looks good — no lints, no hints

$ smithy rerender
wrote README.md
wrote .github/workflows/conda-build.yml
wrote .ci_support/linux_64.yaml
wrote .ci_support/osx_64.yaml
wrote .ci_support/win_64.yaml
```

A custom lint rule is a file:

```rust title=".smithy/lints/no_curl_in_tests.rhai"
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

A custom CI workflow is a template:

```yaml title=".smithy/templates/.github__workflows__docs.yml.j2"
name: Docs for {{ feedstock_name }}
on: [push]
jobs:
  docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: make docs
```

(the file name maps to the output path: `__` becomes `/`, `.j2` is
stripped — this renders to `.github/workflows/docs.yml`).

## What it does today

| Area | Status |
|------|--------|
| Lint v0 `meta.yaml` and v1 `recipe.yaml` recipes | 24 built-in rules, IDs mapped to conda-smithy's `R-###` messages |
| User lint rules in Rhai | `.smithy/lints/*.rhai`, sandboxed, crash-safe |
| `conda-forge.yml` validation | type/value checks + schema-error surfacing |
| Build matrix | providers, `build_platform` cross-compilation, noarch |
| Variants | `conda_build_config.yaml` / `variants.yaml`, `zip_keys`, `# [selector]`s, recipe-aware pruning |
| Rendered CI | GitHub Actions, Azure Pipelines, `README.md`, `.ci_support/*.yaml` |
| User templates | overrides + arbitrary new outputs |
| Tooling | `--format json`, `smithy init`, a reusable GitHub Action |

## What stays in Python (for now)

Provider registration (`register-ci`, tokens), GitHub-API-backed lints,
and the long tail of legacy providers (Travis, Circle, Drone,
Woodpecker). The architecture leaves room for them: matrix entries are
plain data and providers are just templates.
