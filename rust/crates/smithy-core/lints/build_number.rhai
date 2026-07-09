// conda-smithy R-008: every recipe needs an explicit build number.

if recipe_parse_error != "" { return; }

if get(recipe, "build.number") == () {
    lint("The recipe must have a `build/number` section.");
}
