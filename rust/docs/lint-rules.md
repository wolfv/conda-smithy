# Writing lint rules

Lint rules are [Rhai](https://rhai.rs/book/) scripts. Rhai is a small
scripting language with Rust-like syntax — if you can read Python or
JavaScript, you can write a lint rule.

A rule is one `.rhai` file in `.smithy/lints/`. Its **file name is the
rule id** (shown in output, usable in `linter.skip`). The script simply
runs top to bottom and reports problems by calling `lint(...)` or
`hint(...)`.

```rust title=".smithy/lints/require_dev_url.rhai"
// Our team wants every recipe to link its repository.
if get(recipe, "about.dev_url") == () && get(recipe, "about.repository") == () {
    hint("consider adding about.dev_url so users can find the source repo");
}
```

That's the entire workflow. Save the file, run `smithy lint`.

## What's in scope

Every rule sees the same variables:

| variable                  | type   | meaning |
|---------------------------|--------|---------|
| `recipe`                  | map    | the parsed recipe (see below for v0 rendering) |
| `recipe_text`             | string | the raw recipe file, byte for byte |
| `recipe_yaml`             | string | the YAML that was parsed (v0: Jinja rendered) |
| `recipe_version`          | int    | `0` = `meta.yaml`, `1` = `recipe.yaml` |
| `recipe_parse_error`      | string | parse error, `""` on success |
| `config`                  | map    | the parsed `conda-forge.yml` |
| `config_schema_error`     | string | schema problem in `conda-forge.yml`, or `""` |
| `recipe_files`            | array  | file names in the recipe directory |
| `variant_config_filename` | string | `conda_build_config.yaml` / `variants.yaml` / `""` |
| `variant_config_text`     | string | raw variant file contents, or `""` |

and these functions on top of the full
[Rhai standard library](https://rhai.rs/book/language/):

| function | purpose |
|----------|---------|
| `lint(msg)` | report an error (fails `smithy lint`) |
| `hint(msg)` | report a suggestion |
| `get(value, "about.license")` | dotted-path lookup; returns `()` when absent |
| `has(value, "a.b.c")` | dotted-path existence check |
| `is_match(text, regex)` | regular-expression test |
| `find_all(text, regex)` | all regex matches, as an array of strings |
| `keys_in_order(recipe_yaml, "requirements")` | mapping keys in *file* order |
| `join(array, ", ")` | stringify an array |
| `parse_yaml(text)` | parse YAML text into a scriptable value |

## How v0 recipes are presented

`meta.yaml` is Jinja-templated YAML, so smithy evaluates the template
before parsing — with conda-build's functions stubbed out to
recognisable placeholders:

| in `meta.yaml` | in `recipe` / `recipe_yaml` |
|----------------|------------------------------|
| `{{ compiler('c') }}` | `c_compiler_stub` |
| `{{ stdlib('c') }}` | `c_stdlib_stub` |
| `{{ pin_subpackage('x') }}` | `subpackage_pin x` |
| `{{ pin_compatible('x') }}` | `compatible_pin x` |
| `{% set version = "1.2" %}` + `{{ version }}` | `1.2` |

Match on the stubs when you need to detect these constructs (the raw
spellings are still available in `recipe_text`).

## Patterns

**Bail out early when the recipe didn't parse** — structural rules
should stay quiet and let the `parseable` rule do the reporting:

```rust
if recipe_parse_error != "" { return; }
```

**Iterate requirements defensively** — sections may be missing, and v1
entries can be maps (`if:`/`then:` conditions):

```rust
for section in ["build", "host", "run"] {
    let reqs = get(recipe, "requirements." + section);
    if type_of(reqs) != "array" { continue; }
    for req in reqs {
        if type_of(req) != "string" { continue; }
        // ...
    }
}
```

**Work on raw lines for style checks**:

```rust
let line_no = 0;
for line in recipe_text.split("\n") {
    line_no += 1;
    if line.contains("TODO") { hint(`TODO left on line ${line_no}`); }
}
```

## Rhai gotchas

!!! warning "Strings mutate in place"

    `trim()`, `replace()` and friends **mutate the string and return
    `()`**. Writing `let t = s.replace("a", "b");` leaves `t` as unit.
    Do this instead:

    ```rust
    let t = s;
    t.replace("a", "b");
    ```

!!! warning "Backtick strings"

    `` `...${expr}...` `` interpolates, but **cannot contain literal
    backticks**. Use `'single quotes'` inside interpolated messages.

## Safety

Rules run sandboxed: an operation budget (no infinite loops), no
filesystem or network access, and a rule that crashes degrades to a
*hint* explaining the crash instead of failing the whole lint run.

## Tweaking a built-in rule

All built-in rules are Rhai files embedded in the binary — the sources
live in `rust/crates/smithy-core/lints/`. Copy one into
`.smithy/lints/` under a new name, adjust it, and disable the original:

```yaml title="conda-forge.yml"
linter:
  skip: [license]
```
