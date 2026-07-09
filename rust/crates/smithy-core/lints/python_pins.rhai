// conda-smithy R-023 + R-024: in non-noarch single-output recipes,
// python/r-base in host must also appear in run, and neither may carry
// version bounds (the build matrix pins them).

if recipe_parse_error != "" { return; }
if has(recipe, "build.noarch") || has(recipe, "outputs") { return; }

for lang in ["python", "r-base"] {
    let in_host = false;
    let in_run = false;
    let constrained = false;

    for section in ["host", "run"] {
        let reqs = get(recipe, "requirements." + section);
        if type_of(reqs) != "array" { continue; }
        for req in reqs {
            if type_of(req) != "string" { continue; }
            let parts = req.split(" ").filter(|p| p != "");
            if parts.len() == 0 || parts[0] != lang { continue; }

            if section == "host" { in_host = true; } else { in_run = true; }
            if parts.len() > 1 && (parts[1].starts_with(">") || parts[1].starts_with("<")) {
                constrained = true;
            }
        }
    }

    if in_host && !in_run {
        lint(`If ${lang} is a host requirement, it should be a run requirement.`);
    }
    if constrained {
        lint(`Non noarch packages should have ${lang} requirement without any version constraints.`);
    }
}
