// conda-smithy R-000 + R-001: only expected top-level sections, in the
// canonical order.

if recipe_parse_error != "" { return; }

// The canonical section order depends on the recipe format.
let order = if recipe_version == 0 {
    // v0 meta.yaml
    ["package", "source", "build", "requirements", "test", "app",
     "outputs", "about", "extra"]
} else if has(recipe, "outputs") {
    // v1 recipe.yaml with multiple outputs
    ["schema_version", "context", "recipe", "source", "build",
     "outputs", "about", "extra"]
} else {
    // v1 recipe.yaml with a single output
    ["schema_version", "context", "package", "source", "build",
     "requirements", "tests", "about", "extra"]
};

// For the "unexpected key" check, v1 accepts both layouts.
let allowed = order;
if recipe_version == 1 {
    for extra_key in ["package", "requirements", "tests", "recipe", "outputs"] {
        if !(extra_key in allowed) { allowed += extra_key; }
    }
}

let seen = [];
for section in keys_in_order(recipe_yaml, "") {
    if !(section in allowed) {
        lint(`The top level meta key ${section} is unexpected`);
    } else if section in order {
        seen += section;
    }
}

let indices = seen.map(|s| order.index_of(s));
let sorted = indices;
sorted.sort();
if indices != sorted {
    lint(`The top level meta keys are in an unexpected order. Expecting [${join(order, ", ")}].`);
}
