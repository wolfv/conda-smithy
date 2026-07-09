// conda-smithy R-005 + R-006: the recipe must have some tests; outputs
// without tests get a hint.

if recipe_parse_error != "" { return; }

if recipe_version == 1 {
    let tests = get(recipe, "tests");
    let outputs = get(recipe, "outputs");

    if type_of(outputs) == "array" {
        let any_output_tested = false;
        for output in outputs {
            let name = get(output, "package.name");
            if get(output, "tests") == () {
                hint(`It looks like the '${name}' output doesn't have any tests.`);
            } else {
                any_output_tested = true;
            }
        }
        if !any_output_tested && tests == () {
            lint("The recipe must have some tests.");
        }
    } else if tests == () || tests == [] {
        lint("The recipe must have some tests.");
    }
} else {
    // v0: a test section with imports/commands, a run_test.* file, or
    // per-output tests.
    let has_file_test = false;
    for f in ["run_test.py", "run_test.sh", "run_test.bat", "run_test.pl"] {
        if f in recipe_files { has_file_test = true; }
    }

    let test = get(recipe, "test");
    let has_test = has_file_test || has(test, "imports") || has(test, "commands");
    if has_test { return; }

    let outputs = get(recipe, "outputs");
    if type_of(outputs) != "array" {
        lint("The recipe must have some tests.");
        return;
    }

    let any_output_tested = false;
    let untested = [];
    for output in outputs {
        let t = get(output, "test");
        if has(t, "imports") || has(t, "commands") || has(t, "script") {
            any_output_tested = true;
        } else {
            untested += get(output, "name");
        }
    }
    if any_output_tested {
        for name in untested {
            hint(`It looks like the '${name}' output doesn't have any tests.`);
        }
    } else {
        lint("The recipe must have some tests.");
    }
}
