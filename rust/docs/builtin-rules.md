# Built-in rules

All 24 built-in rules are Rhai scripts embedded in the binary; the
sources are in
[`rust/crates/smithy-core/lints/`](https://github.com/wolfv/conda-smithy/tree/master/rust/crates/smithy-core/lints)
and double as examples for [writing your own](lint-rules.md). Disable
any rule via `linter.skip` in `conda-forge.yml`.

The last column maps to the message ids of the Python conda-smithy
linter, so behaviour can be diffed rule by rule.

| rule id | checks | conda-smithy ids |
|---------|--------|------------------|
| `parseable` | recipe parses as YAML; not both `meta.yaml` and `recipe.yaml` | R-036, R-051 |
| `top_level_sections` | only expected top-level sections, canonical order (per format) | R-000, R-001 |
| `about_metadata` | `about.home`/`homepage`, `license`, `summary` present | R-002 |
| `maintainers` | maintainers present, a list; `feedstock-name` suffix | R-003, R-004, R-039 |
| `license` | not "unknown"; no word "License"; `license_file` where required | R-007, R-010, R-013 |
| `build_number` | explicit `build.number` | R-008 |
| `requirements_order` | `build`, `host`, `run` in order | R-009 |
| `source_hash` | url sources carry sha256/sha1/md5 | R-019 |
| `package_name_version` | valid name; version present, spec-conform, not a float | R-014..R-016, R-040 |
| `noarch` | noarch value valid; noarch python has a python lower bound | R-020, R-026 |
| `pin_spacing` | `name >=1.2` spacing of version pins | R-021, R-022 |
| `stdlib` | compiler ⇒ `stdlib('c')`; no sysroot/`__osx` pins | R-033..R-035 |
| `python_pins` | python/r-base host↔run symmetry, no manual bounds | R-023, R-024 |
| `tests` | recipe (or every output) has tests | R-005, R-006 |
| `wheels` | no compiled wheels; pure wheels discouraged | R-028..R-030 |
| `trailing_newline` | exactly one newline at EOF | R-011, R-012 |
| `selectors` | tidy `# [expr]` form; no `py27`-style; none at all in v1 | R0-001..003, R1-001 |
| `jinja_spacing` | `{% set x = y %}` form; `{{ var }}` padding | R0-005, R-025 |
| `pin_subpackage` | `pin_subpackage` vs `pin_compatible` against own outputs | R-027 |
| `bundled_licenses` | rust ⇒ cargo-bundle-licenses; go ⇒ go-licenses | R-031, R-032 |
| `misc_requirements` | `numpy x.x`, `toolchain`, setup.py install, pypi.io | R-017, R0-006, R-049, R-050 |
| `noarch_selectors` | skips/selectors contradict noarch (with `noarch_platforms` allowance) | R0-004, R1-002 |
| `variant_config` | one variant file; macOS SDK ≥ deployment target; `c_stdlib_version` hint | RC-000, RC-002, CBC-001 |
| `forge_yml` | `conda-forge.yml` value/type checks, schema-error surfacing | FC/CF subset |

!!! note "Not ported (yet)"

    Rules that need network access (GitHub maintainer checks, pinning
    hints from conda-forge-pinning) and shellcheck-based build-script
    hints are still Python-only.
