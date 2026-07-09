// conda-smithy R-028 + R-029 + R-030: don't repackage wheels.
//
// This rule works on the raw recipe text so it also catches wheel URLs
// hidden behind templating.

let all_wheels = find_all(recipe_text, "http[^\\s\"']+?\\.whl");
let compiled = all_wheels.filter(|w| !w.ends_with("-none-any.whl"));
let pure = all_wheels.filter(|w| w.ends_with("-none-any.whl"));

if compiled.len() > 0 {
    lint(`Detected compiled wheel(s) in source: ${compiled}. This is disallowed; all packages should be built from source.`);
}

if pure.len() > 0 {
    if get(recipe, "build.noarch") == "python" {
        hint(`Detected pure Python wheel(s) in source: ${pure}. This is generally ok for noarch packages, but prefer a source distribution (sdist) when possible.`);
    } else {
        lint(`Detected pure Python wheel(s) in source: ${pure}. This is discouraged; prefer a source distribution (sdist).`);
    }
}
