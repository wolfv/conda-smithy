// conda-smithy R-031 + R-032: Rust and Go builds statically link their
// dependencies, so their licenses must be collected and shipped.

if recipe_parse_error != "" { return; }

let name = get(recipe, "package.name");
if name == () { name = get(recipe, "recipe.name"); }

// R-031: rust compiler needs cargo-bundle-licenses.
let uses_rust = recipe_yaml.contains("rust_compiler_stub")
    || is_match(recipe_text, "compiler\\([\"']rust[\"']\\)");
if uses_rust && name != "cargo-bundle-licenses"
    && !recipe_text.contains("cargo-bundle-licenses") {
    lint("Rust packages must bundle the licenses of their dependencies: add cargo-bundle-licenses to the build requirements and run it in the build script (THIRDPARTY.yml), and include the file in about.license_file.");
}

// R-032: go compiler needs go-licenses.
let uses_go = recipe_yaml.contains("go_compiler_stub")
    || recipe_yaml.contains("go-cgo_compiler_stub")
    || is_match(recipe_text, "compiler\\([\"']go(-cgo|-nocgo)?[\"']\\)");
if uses_go && name != "go-licenses" && !recipe_text.contains("go-licenses") {
    lint("Go packages must bundle the licenses of their dependencies: add go-licenses to the build requirements and run it in the build script (library_licenses/), and include the directory in about.license_file.");
}
