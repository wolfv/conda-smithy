// conda-smithy R0-005 + R-025: templating style.
//
// `{% set name = value %}` definitions (v0) and `{{ var }}` / `${{ var }}`
// references should use exactly one space of padding.

// R0-005: {% set %} spacing (v0 only).
if recipe_version == 0 {
    let line_no = 0;
    for line in recipe_text.split("\n") {
        line_no += 1;
        if is_match(line, "\\{%.*\\bset\\b")
            && !is_match(line, "^\\s*\\{% set \\S+ = \\S.* %\\}\\s*$") {
            lint(`Jinja2 variable definitions are suggested to take a '{% set <name> = <value> %}' form (line ${line_no}).`);
        }
    }
}

// R-025 (hint): reference padding, `{{ var }}` not `{{var}}` / `{{ var}}`.
let pattern = if recipe_version == 1 {
    "\\$\\{\\{[^{}]*\\}\\}"
} else {
    "\\{\\{[^{}]*\\}\\}"
};
let opener = if recipe_version == 1 { "${{ " } else { "{{ " };

let bad_refs = [];
for reference in find_all(recipe_text, pattern) {
    if !reference.starts_with(opener) || !reference.ends_with(" }}")
        || reference.contains("  ") {
        bad_refs += reference;
    }
}
if bad_refs.len() > 0 {
    hint(`Jinja2 variable references are suggested to take a '{{ <variable name> }}' form with single spaces. See: ${bad_refs}`);
}
