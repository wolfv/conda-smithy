// conda-smithy R-002: the about section needs homepage, license and summary.

if recipe_parse_error != "" { return; }

let home_key = if recipe_version == 1 { "homepage" } else { "home" };

for item in [home_key, "license", "summary"] {
    let value = get(recipe, "about." + item);
    if value == () || value == "" {
        lint(`The ${item} item is expected in the about section.`);
    }
}
