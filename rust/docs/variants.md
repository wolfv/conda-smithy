# Build variants

A variant file next to the recipe multiplies the build matrix:
`conda_build_config.yaml` for v0 recipes, `variants.yaml` for v1.

```yaml title="recipe/variants.yaml"
python:
  - "3.12"
  - "3.13"
numpy:
  - "1.26"
  - "2.0"
zip_keys:
  - [python, numpy]
c_stdlib_version:
  - "2.17"   # [linux]
  - "10.13"  # [osx]
```

With this file a feedstock building `linux_64`, `osx_64` and `win_64`
produces six jobs:

```
linux_64_python3.12_numpy1.26   linux_64_python3.13_numpy2.0
osx_64_python3.12_numpy1.26    osx_64_python3.13_numpy2.0
win_64_python3.12_numpy1.26    win_64_python3.13_numpy2.0
```

## The rules

1. **Multi-valued keys fan out.** Each becomes an axis of the matrix.
2. **`zip_keys` groups advance together.** `[python, numpy]` above
   yields 2 combinations, not 4. Groups with mismatched lengths are an
   error.
3. **Single-valued keys are carried, not fanned.** They appear in every
   `.ci_support/<config>.yaml` but don't multiply jobs.
4. **Selectors are per target platform.** A `# [linux]` / `# [not win]`
   comment keeps or drops that line for each platform before parsing.
   The expression is evaluated by the same Rhai engine that runs lint
   rules; known identifiers are `linux`, `osx`, `win`, `unix`,
   `aarch64`, `arm64`, `ppc64le`, `s390x`, `x86`, `x86_64` plus
   `and`/`or`/`not`. Unknown identifiers keep the line.
5. **Unused keys are pruned.** Like conda-smithy, a variant key only
   matters if the recipe *uses* it.

## What counts as "used"

A key survives pruning when any of these hold:

* it names a dependency in any `requirements:`/`run_exports:` section
  (top level or per output) — `python`, `numpy`, `openssl`, ...
* it is referenced as a template variable — `{{ python }}` or
  `${{ python }}`
* it is a compiler/stdlib key for a language the recipe uses —
  `rust_compiler_version` is used when the recipe calls
  `compiler('rust')`, `c_stdlib_version` when it calls `stdlib('c')`
* it shares a `zip_keys` group with a used key (zipped variables must
  stay aligned)
* it is infrastructure that is always kept: `target_platform`,
  `channel_sources`, `channel_targets`, `docker_image`,
  `pin_run_as_build`, `MACOSX_DEPLOYMENT_TARGET`, `MACOSX_SDK_VERSION`,
  `cdt_*`

Everything else is dropped — so pointing a feedstock at a large shared
pinning file does not explode the matrix.

## The `.ci_support` files

One file per job, containing the platform routing and the variant cell:

```yaml title=".ci_support/linux_64_python3.12.yaml"
# This file was generated automatically by smithy. Do not edit.
target_platform: linux-64
build_platform: linux-64
docker_image:
- quay.io/condaforge/linux-anvil-x86_64
c_stdlib_version:
- '2.17'
python:
- '3.12'
```

These are passed to the build tool (`rattler-build build -m ...` /
`conda build -m ...`) by the generated CI, so the values you write in
the variant file are exactly what the build sees.
