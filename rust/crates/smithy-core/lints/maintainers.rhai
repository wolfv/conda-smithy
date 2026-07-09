// conda-smithy R-003 + R-004 + R-039: recipe maintainers must be a
// non-empty list, and `extra.feedstock-name` must not end in "-feedstock".

if recipe_parse_error != "" { return; }

let maintainers = get(recipe, "extra.recipe-maintainers");
if maintainers == () || maintainers == [] {
    lint("The recipe could do with some maintainers listed in the `extra/recipe-maintainers` section.");
} else if type_of(maintainers) != "array" {
    lint("Recipe maintainers should be a json list.");
}

let feedstock_name = get(recipe, "extra.feedstock-name");
if type_of(feedstock_name) == "string" && feedstock_name.ends_with("-feedstock") {
    lint("The feedstock-name in the `extra` section must not end with '-feedstock'.");
}
