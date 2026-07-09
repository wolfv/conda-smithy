# Architecture

```
rust/
├── crates/
│   ├── smithy-core/           # the library
│   │   ├── src/
│   │   │   ├── config.rs      # conda-forge.yml model (serde, fault-tolerant)
│   │   │   ├── recipe.rs      # v0/v1 recipe loading (v0: minijinja + stubs)
│   │   │   ├── feedstock.rs   # ties root + config + recipe together
│   │   │   ├── lint.rs        # the Rhai lint engine
│   │   │   ├── variants.rs    # variant parsing, zip, selectors, pruning
│   │   │   └── render.rs      # matrix + minijinja rendering
│   │   ├── lints/*.rhai       # built-in rules (embedded via include_str!)
│   │   └── templates/*.j2     # built-in templates (embedded)
│   └── smithy-cli/            # the `smithy` binary (clap)
├── action/                    # composite GitHub Action
├── docs/ + mkdocs.yml         # this documentation
└── examples/demo-feedstock/   # exercises every feature; used in tests
```

## Design decisions

**Rules are data, not code.** Built-in lint rules are `.rhai` files
embedded with `include_str!`. The engine treats built-in and user rules
identically — same scope, same API, same sandbox — which guarantees the
scripting surface stays honest: if the built-ins can't do something,
users can't either, and we notice immediately.

**One scripting language everywhere.** Rhai runs the lint rules *and*
evaluates conda selector expressions (`# [linux and not aarch64]`) in
variant files. One language to learn, one engine to maintain.

**Templates only print.** All matrix logic (providers, cross-compilation,
variant fanout, pruning) happens in Rust; templates receive ready-to-use
values. This keeps user-edited templates trivial and side-steps the
Jinja2-constructs-minijinja-lacks problem (`namespace()`, `{% do %}`)
that a 1:1 port of conda-smithy's templates would hit.

**v0 recipes are rendered with minijinja.** `meta.yaml`'s Jinja is
evaluated with conda-build functions stubbed to recognisable
placeholders (`compiler('c')` → `c_compiler_stub`), mirroring the Python
linter's `NullUndefined` environment. Lints match on the stubs.

**Fail soft.** A crashing lint rule becomes a hint; an unparsable
recipe is a lint (not a crash); a mistyped `conda-forge.yml` falls back
to defaults and surfaces the schema error as a lint. `smithy lint`
should never be the thing that breaks.

## Rule-id compatibility

Every built-in rule's header comment names the conda-smithy message ids
it ports (`R-###`, `R0-###`, `R1-###`, `RC-###`, ...), so behaviour can
be compared rule by rule against
[`conda_smithy/linter/`](https://github.com/conda-forge/conda-smithy/tree/master/conda_smithy/linter).

## Testing

* Unit tests live next to each module; integration tests in
  `crates/*/tests/` cover the lint engine, rendering, variants and the
  CLI end to end (the demo feedstock is a fixture).
* The linter is crash-swept against every recipe fixture of the Python
  test suite (`tests/recipes/`).
* CI (`.github/workflows/rust-ci.yml`) runs fmt, clippy `-D warnings`,
  the test suite, and a smoke test of the GitHub Action against the
  demo feedstock.
