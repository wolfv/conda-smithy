# Getting started

## Install

smithy is a single static binary built from the `rust/` workspace of the
conda-smithy repository:

```console
$ git clone https://github.com/wolfv/conda-smithy
$ cd conda-smithy/rust
$ cargo install --locked --path crates/smithy-cli
$ smithy --version
```

## Create a feedstock

```console
$ smithy init mytool --feedstock-dir mytool-feedstock
```

This writes a skeleton and never overwrites existing files:

```
mytool-feedstock/
├── conda-forge.yml            # feedstock configuration (commented)
├── recipe/recipe.yaml         # v1 example recipe (use --recipe-format v0 for meta.yaml)
├── .smithy/
│   ├── README.md              # what goes in here
│   └── lints/example.rhai     # a commented-out custom rule to start from
└── .gitignore
```

Fill in the recipe (`source.url`, `sha256`, requirements, `about`), then:

## Lint

```console
$ smithy lint --feedstock-dir mytool-feedstock
error [build_number] The recipe must have a `build/number` section.
hint  [jinja_spacing] Jinja2 variable references are suggested to take a '{{ <variable name> }}' form ...

1 lint(s), 1 hint(s)
```

* **error** (a *lint* in conda-smithy terms) — must be fixed; the exit
  code is non-zero.
* **hint** — advisory; does not fail the command.

The rule id in brackets (`build_number`) is what you put in
`conda-forge.yml` to disable a rule:

```yaml
linter:
  skip: [build_number]
```

For CI systems, `--format json` prints a machine-readable report — see
[CLI reference](cli.md).

## Rerender

```console
$ smithy rerender --feedstock-dir mytool-feedstock
wrote README.md
wrote .github/workflows/conda-build.yml
wrote .ci_support/linux_64.yaml
...
```

`smithy rerender --check` shows what would be written without touching
anything.

The build matrix comes from `conda-forge.yml` (which platforms, which CI
provider, cross-compilation) and the optional variant file next to your
recipe (which python/numpy/... versions). See
[Build variants](variants.md).

## Extend

Two directories inside the feedstock make smithy yours:

* `.smithy/lints/` — extra lint rules ([writing lint rules](lint-rules.md))
* `.smithy/templates/` — CI template overrides and additions
  ([templates](templates.md))

Both are picked up automatically; there is nothing to register.
