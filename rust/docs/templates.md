# Templates

`smithy rerender` generates every CI file from a
[minijinja](https://github.com/mitsuhiko/minijinja) template — minijinja
implements the Jinja2 syntax you already know from conda recipes.

## Built-in templates

| template | renders to |
|----------|-----------|
| `github-actions.yml.j2` | `.github/workflows/conda-build.yml` |
| `azure-pipelines.yml.j2` | `azure-pipelines.yml` |
| `README.md.j2` | `README.md` |

plus one `.ci_support/<config>.yaml` per matrix entry (plain YAML dump,
no template involved). Provider templates are skipped when no matrix
entry uses that provider. Files listed under `skip_render:` in
`conda-forge.yml` are never written.

## Overriding a built-in

Copy the template from
[`rust/crates/smithy-core/templates/`](https://github.com/wolfv/conda-smithy/tree/master/rust/crates/smithy-core/templates)
into `.smithy/templates/` (same file name) and edit. The next
`smithy rerender` uses your copy.

## Adding new outputs

Any *other* `*.j2` file in `.smithy/templates/` becomes an additional
rendered file. The output path is the file name with `.j2` stripped and
`__` replaced by `/`:

```
.smithy/templates/.github__workflows__docs.yml.j2  →  .github/workflows/docs.yml
.smithy/templates/Makefile.j2                      →  Makefile
```

New outputs get the same context as the built-ins.

## Template context

The context is precomputed in Rust — templates only print values, which
keeps them easy to edit. The most useful values:

| value | shape | notes |
|-------|-------|-------|
| `configs` | list of config | the whole build matrix |
| `gha_configs`, `azure_configs` | list of config | filtered by provider |
| `feedstock_name`, `package_name` | string | |
| `about` | map | `home`, `summary`, `license`, `dev_url`, `doc_url` |
| `maintainers` | list of string | GitHub handles |
| `github_actions` | map | `cancel_in_progress`, `max_parallel`, `timeout_minutes`, `triggers` |
| `build_tool` | string | `conda-build` or `rattler-build` |
| `recipe_dir` | string | usually `recipe` |
| `upload_on_branch` | string or none | |
| `secrets` | list of string | e.g. `["BINSTAR_TOKEN"]` |
| `forge_config` | map | the **raw** `conda-forge.yml` — custom keys included |
| `smithy_version` | string | |

Each entry of `configs` has:

```yaml
name: linux_64_python3.12       # job + .ci_support file name
target_platform: linux-64
build_platform: linux-64        # differs when cross-compiling
os: linux                       # linux | osx | win
provider: github_actions
upload: true
gha_runs_on: [ubuntu-latest]
docker_image: quay.io/condaforge/linux-anvil-x86_64   # linux only
cross_compile: false
variant: {python: "3.12", c_stdlib_version: "2.17"}
```

!!! tip "Custom conda-forge.yml keys"

    Anything you put in `conda-forge.yml` is reachable through
    `forge_config`, so user templates can have their own settings:

    ```yaml title="conda-forge.yml"
    my_docs:
      python_version: "3.12"
    ```

    ```jinja title=".smithy/templates/.github__workflows__docs.yml.j2"
    python-version: "{{ forge_config.my_docs.python_version }}"
    ```

## Escaping GitHub Actions expressions

GitHub's own `${{ ... }}` syntax collides with Jinja. Wrap it in
`{% raw %}`:

```jinja
runs-on: {% raw %}${{ matrix.runs_on }}{% endraw %}
```
