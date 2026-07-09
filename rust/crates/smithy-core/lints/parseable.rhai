// conda-smithy R-036: the recipe must be parseable.
//
// Every lint rule is a plain Rhai script like this one. It sees the
// variables `recipe`, `recipe_text`, `recipe_yaml`, `recipe_version`,
// `recipe_parse_error`, `config` and `recipe_files`, and reports problems
// with `lint("...")` (error) or `hint("...")` (suggestion).

if recipe_parse_error != "" {
    lint(`The recipe is not parseable: ${recipe_parse_error}`);
}

// R-051: a recipe directory must contain either meta.yaml or recipe.yaml,
// never both.
if "meta.yaml" in recipe_files && "recipe.yaml" in recipe_files {
    lint("Found both meta.yaml and recipe.yaml in the recipe directory; remove one of them.");
}
