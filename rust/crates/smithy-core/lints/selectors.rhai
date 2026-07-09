// conda-smithy R0-001 + R0-002/R0-003 + R1-001: selector hygiene.
//
// v0: `# [linux]` selectors must be written `<code><2 spaces>#<1 space>[expr]`,
//     and old-style Python selectors (py27, py38, ...) are deprecated.
// v1: comment selectors don't exist — use `if:` conditions.

let line_no = 0;
let untidy = [];

for line in recipe_text.split("\n") {
    line_no += 1;
    if !(line.contains("# [") || line.contains("#[")) { continue; }

    if recipe_version == 1 {
        // R1-001: no comment selectors in recipe.yaml.
        lint(`Selectors in comments are not allowed in v1 recipes (line ${line_no}). Use 'if:' conditions instead.`);
        continue;
    }

    // R0-001: tidy form.
    if !is_match(line, "\\S.*\\s\\s#\\s\\[[^\\]]+\\]\\s*$") {
        untidy += line_no;
    }

    // R0-002 / R0-003: numeric python selectors.
    if is_match(line, "\\[[^\\]]*\\bpy(27|34|35|36)\\b") {
        hint(`Old-style Python selector py2x/py3x on line ${line_no} is deprecated; use comparisons on the integer 'py' instead (e.g. # [py>=38]).`);
    } else if is_match(line, "\\[[^\\]]*\\bpy[0-9]{2,3}\\b") {
        lint(`Old-style Python selector on line ${line_no}; use comparisons on the integer 'py' instead (e.g. # [py>=310]).`);
    }
}

if untidy.len() > 0 {
    lint(`Selectors are suggested to take a '<two spaces>#<one space>[<expression>]' form. See lines ${untidy}.`);
}
