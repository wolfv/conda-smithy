// conda-smithy R-009: requirements subsections come in the order
// build, host, run.

if recipe_parse_error != "" { return; }

let order = ["build", "host", "run"];
let seen = keys_in_order(recipe_yaml, "requirements").filter(|k| k in order);

let indices = seen.map(|k| order.index_of(k));
let sorted = indices;
sorted.sort();
if indices != sorted {
    lint(`The requirements/ sections should be defined in the following order: build, host, run; instead saw: ${join(seen, ", ")}.`);
}
