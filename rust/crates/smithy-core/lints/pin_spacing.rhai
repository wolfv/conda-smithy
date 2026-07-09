// conda-smithy R-021 + R-022: version pins are spelled `name >=1.2`,
// with a space between name and pin but none inside the pin.

if recipe_parse_error != "" { return; }

for section in ["build", "host", "run"] {
    let reqs = get(recipe, "requirements." + section);
    if type_of(reqs) != "array" { continue; }

    for req in reqs {
        if type_of(req) != "string" { continue; }
        // Leave templated requirements alone.
        if req.contains("{{") || req.contains("${{") { continue; }

        let parts = req.split(" ").filter(|p| p != "");
        if parts.len() == 0 { continue; }

        if parts.len() > 2 && (parts[1] in ["!=", "=", "==", ">", "<", "<=", ">="]) {
            lint(`requirements: ${section}: '${req}' should not contain a space between the relational operator and the version.`);
        } else if is_match(parts[0], "^[a-zA-Z0-9_.\\-]+[<>=]") {
            lint(`requirements: ${section}: '${req}' must contain a space between the package name and the pin.`);
        }
    }
}
