// conda-smithy R-020 + R-026: `noarch` must be python or generic, and
// noarch python recipes need a lower bound on python.

if recipe_parse_error != "" { return; }

let noarch = get(recipe, "build.noarch");

if noarch != () && !(noarch in ["python", "generic"]) {
    lint(`Invalid noarch value '${noarch}'. Should be one of: python, generic.`);
}

if noarch == "python" && !has(recipe, "outputs") {
    let run = get(recipe, "requirements.run");
    let has_lower_bound = false;
    if type_of(run) == "array" {
        for req in run {
            if type_of(req) == "string" && req.starts_with("python ") {
                has_lower_bound = true;
            }
        }
    }
    if !has_lower_bound {
        lint("noarch: python recipes are required to have a lower bound on the python version, e.g. `python >={{ python_min }}` in the run section.");
    }
}
