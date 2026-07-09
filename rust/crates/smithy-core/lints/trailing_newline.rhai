// conda-smithy R-011 + R-012: files end with exactly one newline.

let text = recipe_text;
let trailing = 0;
while text.ends_with("\n") {
    text = text.sub_string(0, text.len() - 1);
    trailing += 1;
}

if trailing > 1 {
    lint(`There are ${trailing - 1} too many lines. There should be one empty line at the end of the file.`);
} else if trailing < 1 {
    lint("There are too few lines. There should be one empty line at the end of the file.");
}
