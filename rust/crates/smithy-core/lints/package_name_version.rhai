// conda-smithy R-014 + R-015 + R-016 + R-040: package name and version
// must be present and well-formed.

if recipe_parse_error != "" { return; }

let name = get(recipe, "package.name");
if name == () { name = get(recipe, "recipe.name"); }
if type_of(name) == "string" && !name.contains("${{") {
    if !is_match(name, "^[a-z0-9_\\-.]+$") {
        lint("Recipe name has invalid characters. only lowercase alpha, numeric, underscores, hyphens and dots allowed");
    }
}

let version = get(recipe, "package.version");
if version == () { version = get(recipe, "recipe.version"); }
if version == () {
    lint("Package version is missing.");
} else if type_of(version) == "f64" {
    // YAML parsed the version as a number (e.g. `version: 1.10` == 1.1).
    // Only flag it when the raw recipe really contains a bare float — the
    // v0 Jinja rendering drops quotes from `{% set %}` substitutions.
    if is_match(recipe_text, "(?m)^\\s*version:\\s*[0-9]+\\.[0-9]+\\s*$") {
        lint(`Package version ${version} was parsed as a floating point number; please quote it.`);
    }
} else {
    let v = `${version}`;
    if !v.contains("${{") && !is_match(v, "^[A-Za-z0-9._!+]+$") {
        lint(`Package version ${v} doesn't match conda spec.`);
    }
}
