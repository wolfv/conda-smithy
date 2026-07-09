// Checks on conda-forge.yml itself (a slimmed-down version of
// conda-smithy's JSON-schema validation).

// The engine reports typed-deserialization failures here; rerender falls
// back to default settings until this is fixed.
if config_schema_error != "" {
    lint(`In conda-forge.yml: ${config_schema_error}`);
}

// `test:` policy.
let test = get(config, "test");
if test != () && !(test in ["all", "native", "native_and_emulated"]) {
    lint(`In conda-forge.yml: test is '${test}' but must be one of: all, native, native_and_emulated.`);
}

// `conda_build_tool:` must be a tool smithy knows how to drive.
let tool = get(config, "conda_build_tool");
let known_tools = ["conda-build", "conda-build+classic",
                   "conda-build+conda-libmamba-solver", "mambabuild",
                   "rattler-build"];
if tool != () && !(tool in known_tools) {
    lint(`In conda-forge.yml: unknown conda_build_tool '${tool}'. Expected one of: ${join(known_tools, ", ")}.`);
}

// Type checks for commonly mistyped keys.
if has(config, "skip_render") && type_of(get(config, "skip_render")) != "array" {
    lint("In conda-forge.yml: skip_render must be a list of file paths.");
}
if has(config, "upload_on_branch") && type_of(get(config, "upload_on_branch")) != "string" {
    lint("In conda-forge.yml: upload_on_branch must be a string (a branch name).");
}
if has(config, "noarch_platforms") {
    let value = get(config, "noarch_platforms");
    if type_of(value) != "array" && type_of(value) != "string" {
        lint("In conda-forge.yml: noarch_platforms must be a platform string or a list of platforms.");
    }
}
if has(config, "linter.skip") && type_of(get(config, "linter.skip")) != "array" {
    lint("In conda-forge.yml: linter.skip must be a list of rule ids.");
}
