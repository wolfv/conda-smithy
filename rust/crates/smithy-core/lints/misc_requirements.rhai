// conda-smithy R-017 + R0-006 + R-049 + R-050: assorted requirement and
// packaging checks.

if recipe_parse_error != "" { return; }

for section in ["build", "host"] {
    let reqs = get(recipe, "requirements." + section);
    if type_of(reqs) != "array" { continue; }
    for req in reqs {
        if type_of(req) != "string" { continue; }
        // R-017: `numpy x.x` build-time pinning is deprecated.
        if req == "numpy x.x" {
            lint("pinned numpy (numpy x.x) is a deprecated pattern; use `numpy` in host and `{{ pin_compatible('numpy') }}` in run instead.");
        }
        // R0-006: the ancient `toolchain` meta package.
        if req == "toolchain" {
            lint("Using the `toolchain` package directly is deprecated. Use {{ compiler('<language>') }} instead.");
        }
    }
}

// R-049 (hint): prefer pip over setup.py.
if is_match(recipe_text, "python\\s+setup\\.py\\s+install") {
    hint("Whenever possible python packages should use pip: replace `python setup.py install` with `{{ PYTHON }} -m pip install . -vv --no-deps --no-build-isolation`.");
}

// R-050 (hint): pypi.io is a legacy alias.
if recipe_text.contains("pypi.io/") {
    hint("Usage of `pypi.io` is discouraged; use the standard `pypi.org` domain instead.");
}
