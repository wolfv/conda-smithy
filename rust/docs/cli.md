# CLI reference

```
smithy <command> [options]
```

All commands take `--feedstock-dir <path>` (default: the current
directory).

## `smithy lint`

Runs the [built-in rules](builtin-rules.md) plus every `*.rhai` file in
`.smithy/lints/`.

```console
$ smithy lint --feedstock-dir .
error [build_number] The recipe must have a `build/number` section.
hint  [misc_requirements] Usage of `pypi.io` is discouraged ...

1 lint(s), 1 hint(s)
```

* Exit code `1` when at least one **lint** (error) fired; hints alone
  exit `0`.
* Rules listed in `conda-forge.yml` under `linter.skip` are not run.

### `--format json`

For CI and tooling:

```console
$ smithy lint --format json
{
  "recipe": "recipe/recipe.yaml",
  "lints": [
    { "rule": "build_number", "message": "The recipe must have a `build/number` section." }
  ],
  "hints": []
}
```

The exit code behaves the same as in text mode, so
`smithy lint --format json > report.json` both produces the report and
fails the CI step when needed.

## `smithy rerender`

Regenerates the CI configuration from the build matrix
([variants](variants.md)) and the [templates](templates.md).

```console
$ smithy rerender            # write the files
$ smithy rerender --check    # only print what would be written
```

Files listed in `conda-forge.yml` under `skip_render:` are left alone.

## `smithy init`

Creates a feedstock skeleton; never overwrites existing files.

```console
$ smithy init <package-name> [--feedstock-dir <dir>] [--recipe-format v1|v0]
```

* `--recipe-format v1` (default) writes a rattler-build `recipe.yaml`;
  `v0` writes a conda-build `meta.yaml`.
* The skeleton lints clean and rerenders out of the box; fill in the
  `TODO`s and the source url/checksum.
