// conda-smithy R-027: pin_subpackage is for this recipe's own outputs,
// pin_compatible for everything else.
//
// In rendered v0 recipes the pins appear as `subpackage_pin <name>` /
// `compatible_pin <name>`; in v1 recipes the raw `${{ pin_subpackage("x") }}`
// strings survive parsing. Both spellings are handled below.

if recipe_parse_error != "" { return; }

// -> ["subpackage"|"compatible", name], or [] if the string is not a pin.
fn pin_target(s) {
    if type_of(s) != "string" { return []; }
    if s.starts_with("subpackage_pin ") {
        return ["subpackage", s.sub_string(15).split(" ")[0]];
    }
    if s.starts_with("compatible_pin ") {
        return ["compatible", s.sub_string(15).split(" ")[0]];
    }
    let t = s;
    t.replace("'", "\""); // note: Rhai's replace() mutates in place
    if t.contains("pin_subpackage(\"") {
        let rest = t.sub_string(t.index_of("pin_subpackage(\"") + 16);
        return ["subpackage", rest.split("\"")[0]];
    }
    if t.contains("pin_compatible(\"") {
        let rest = t.sub_string(t.index_of("pin_compatible(\"") + 16);
        return ["compatible", rest.split("\"")[0]];
    }
    []
}

// Requirement-ish strings of one recipe/output section group.
fn section_strings(node) {
    let found = [];
    for path in ["requirements.host", "requirements.run",
                 "requirements.run_exports", "build.run_exports"] {
        let entries = get(node, path);
        if type_of(entries) == "map" {
            // run_exports may be split into strong/weak lists.
            for key in entries.keys() {
                let sub = entries[key];
                if type_of(sub) == "array" { found += sub; }
            }
        } else if type_of(entries) == "array" {
            found += entries;
        } else if type_of(entries) == "string" {
            found += entries;
        }
    }
    found
}

// The recipe's own package names: top-level plus every output.
let own_names = [];
for path in ["package.name", "recipe.name"] {
    let name = get(recipe, path);
    if type_of(name) == "string" { own_names += name; }
}
let candidates = section_strings(recipe);
let outputs = get(recipe, "outputs");
if type_of(outputs) == "array" {
    for output in outputs {
        for path in ["name", "package.name"] {
            let name = get(output, path);
            if type_of(name) == "string" { own_names += name; }
        }
        candidates += section_strings(output);
    }
}

for candidate in candidates {
    let pin = pin_target(candidate);
    if pin.len() != 2 { continue; }
    let kind = pin[0];
    let name = pin[1];
    if kind == "compatible" && name in own_names {
        lint(`pin_subpackage should be used instead of pin_compatible for '${name}' because it is one of the known outputs of this recipe.`);
    } else if kind == "subpackage" && !(name in own_names) {
        lint(`pin_compatible should be used instead of pin_subpackage for '${name}' because it is not a known output of this recipe (${own_names}).`);
    }
}
